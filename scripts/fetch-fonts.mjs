// Holt die im Design festgelegten Schriften (E-13) einmalig ins Repository.
//
// Warum ein eigenes Skript statt eines Stylesheet-Links?
// NF-04 verbietet Drittanbieter-Aufrufe zur Laufzeit, FA-26 verlangt
// unterbrechungsfreien Betrieb bei Netzausfall. Die Schriften müssen also
// mit ins APK. Dieses Skript läuft einmal beim Einrichten des Projekts,
// nie im Betrieb.
//
//     node scripts/fetch-fonts.mjs
//
// Lizenz: Instrument Sans und Cormorant Garamond stehen unter der SIL Open
// Font License 1.1 — Apache-2.0-verträglich (RB-05). Die Lizenztexte landen
// mit im Zielverzeichnis.

import { mkdirSync, writeFileSync, existsSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const outDir = resolve(root, 'public/fonts')

// Google Fonts liefert je nach User-Agent unterschiedliche Formate.
// Mit einem modernen UA bekommen wir woff2.
const UA =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120 Safari/537.36'

const FONTS = [
  {
    file: 'instrument-sans.woff2',
    css: 'https://fonts.googleapis.com/css2?family=Instrument+Sans:wght@400;500;600&display=block',
  },
  {
    file: 'cormorant-garamond.woff2',
    css: 'https://fonts.googleapis.com/css2?family=Cormorant+Garamond:wght@300;400&display=block',
  },
  {
    file: 'cormorant-garamond-italic.woff2',
    css: 'https://fonts.googleapis.com/css2?family=Cormorant+Garamond:ital,wght@1,400&display=block',
  },
]

/** Zieht die erste woff2-URL aus einem Google-Fonts-Stylesheet. */
function firstWoff2(css) {
  const match = css.match(/url\((https:\/\/fonts\.gstatic\.com\/[^)]+\.woff2)\)/)
  return match ? match[1] : null
}

async function fetchFont({ file, css }) {
  const target = resolve(outDir, file)
  if (existsSync(target)) {
    console.log(`  ${file} — bereits vorhanden, übersprungen`)
    return
  }

  const cssResponse = await fetch(css, { headers: { 'User-Agent': UA } })
  if (!cssResponse.ok) {
    throw new Error(`Stylesheet für ${file}: HTTP ${cssResponse.status}`)
  }

  const url = firstWoff2(await cssResponse.text())
  if (!url) throw new Error(`Keine woff2-URL im Stylesheet für ${file} gefunden`)

  const fontResponse = await fetch(url, { headers: { 'User-Agent': UA } })
  if (!fontResponse.ok) throw new Error(`${file}: HTTP ${fontResponse.status}`)

  writeFileSync(target, Buffer.from(await fontResponse.arrayBuffer()))
  console.log(`  ${file} — geladen`)
}

mkdirSync(outDir, { recursive: true })

writeFileSync(
  resolve(outDir, 'LIZENZ.md'),
  [
    '# Schriften',
    '',
    'Instrument Sans und Cormorant Garamond stehen unter der',
    'SIL Open Font License 1.1: https://openfontlicense.org',
    '',
    'Die OFL ist mit der Apache-2.0-Lizenz dieses Projekts vereinbar (RB-05).',
    'Die Dateien werden mit `node scripts/fetch-fonts.mjs` geholt und sind',
    'bewusst Teil des Repositories: die App darf zur Laufzeit keine',
    'Drittanbieter kontaktieren (NF-04) und muss offline laufen (FA-26).',
    '',
  ].join('\n'),
)

console.log('Lade Schriften nach public/fonts/ ...')
let failed = 0
for (const font of FONTS) {
  try {
    await fetchFont(font)
  } catch (e) {
    failed++
    console.error(`  ${font.file} — fehlgeschlagen: ${e.message}`)
  }
}

if (failed > 0) {
  console.error(
    `\n${failed} Schrift(en) konnten nicht geladen werden.\n` +
      'Die App läuft trotzdem — sie greift dann auf Systemschriften zurück.',
  )
  process.exitCode = 1
} else {
  console.log('\nFertig.')
}
