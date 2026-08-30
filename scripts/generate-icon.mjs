// Erzeugt das App-Icon aus der Design-Vorlage (E-13, fortgeschrieben durch E-27).
//
//     npm run icons
//
// Das Skript ruft `tauri icon` selbst auf und legt anschließend die adaptiven
// Android-Vordergründe neu an. Die Reihenfolge ist der Grund, warum das ein
// Skript ist und keine Kette aus zwei npm-Aufrufen — siehe Abschnitt 2 und 3.
//
// ## Motiv (E-27)
//
// Ursprünglich „Rahmen & Horizont": ein gezeichneter Rahmen in Off-White, darin
// Messing-Horizont und -Sonne auf Tiefschwarz. Auf dem Pixel war der Rahmen
// beschnitten, weil das Telefon runde Icons verwendet — und ein Quadrat, dessen
// Ecken in einen Kreis passen sollen, wird zwangsläufig klein: die Ecken lagen
// bei Radius 65, erlaubt sind 48,9.
//
// Seit E-27 zeichnet das Icon den Rahmen nicht mehr selbst. **Die Kontur des
// Icons ist der Rahmen** — auf Android die Maske des Launchers, überall sonst
// die Kreisscheibe, die dieses Skript rendert. Horizont und Sonne bleiben
// unverändert und liegen mit 37,6 bzw. 31,6 bequem in der Sicherheitszone; eine
// Verkleinerung wie in der Zwischenfassung ist damit hinfällig.
//
// Bewusste Folge: Off-White kommt im Icon nicht mehr vor. Von den drei Farben
// aus E-13 tragen es nur noch Oberfläche und Wortmarke.

import { Resvg } from '@resvg/resvg-js'
import { execFileSync } from 'node:child_process'
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const outDir = resolve(root, 'src-tauri/icons')
const androidDir = resolve(outDir, 'android')
const docsDir = resolve(root, 'docs')

// Farben aus dem Design-Canvas (E-13).
const BLACK = '#0A0A0A'
const BRASS = '#C2A878'

/** Kantenlängen, die `tauri icon` für die adaptiven Vordergründe anlegt. */
const FOREGROUND_SIZES = {
  'mipmap-mdpi': 108,
  'mipmap-hdpi': 162,
  'mipmap-xhdpi': 216,
  'mipmap-xxhdpi': 324,
  'mipmap-xxxhdpi': 432,
}

/**
 * Horizont und Sonne im 160er-Koordinatensystem der Vorlage.
 *
 * Die Maße stammen unverändert aus E-13 — nur der Rahmen ist fort. Beide
 * Elemente liegen innerhalb der 66-dp-Sicherheitszone (Radius 48,9): die
 * Horizontenden bei 37,6, der äußere Sonnenrand bei 31,6.
 *
 * @param {{ sun: boolean, stroke: number }} opts
 */
function motif({ sun, stroke }) {
  const horizonY = 96
  return `
    <line x1="46" y1="${horizonY}" x2="114" y2="${horizonY}"
          stroke="${BRASS}" stroke-width="${stroke}" stroke-linecap="round"/>
    ${sun ? `<circle cx="96" cy="64" r="9" fill="${BRASS}"/>` : ''}
  `
}

/**
 * @param {{ size: number, sun?: boolean, stroke?: number, shape?: 'disc' | 'bleed' }} opts
 *
 * `disc` zeichnet die Kreisscheibe selbst — für alles, wo keine Maske greift
 * (Desktop, Store, Dokumentation). `bleed` füllt die ganze Fläche und überlässt
 * die Kontur der Maske; das ist der Android-Vordergrund.
 */
function svg({ size, sun = true, stroke = 4, shape = 'disc' }) {
  const bg =
    shape === 'disc'
      ? `<circle cx="80" cy="80" r="80" fill="${BLACK}"/>`
      : `<rect width="160" height="160" fill="${BLACK}"/>`

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 160 160">
    ${bg}
    ${motif({ sun, stroke })}
  </svg>`
}

function render(source, size) {
  const resvg = new Resvg(source, { fitTo: { mode: 'width', value: size } })
  return resvg.render().asPng()
}

mkdirSync(outDir, { recursive: true })
mkdirSync(docsDir, { recursive: true })

// ── 1. Quellbild und Nebenformate ────────────────────────────────────────────

// Kreisscheibe auf durchsichtigem Grund: daraus leitet `tauri icon` die
// Desktop-Icons und die klassischen Android-Mipmaps ab. Wo ein Launcher selbst
// maskiert, liegt die Scheibe passgenau darunter; wo nicht, ist sie die Kontur.
writeFileSync(resolve(outDir, 'icon-source.png'), render(svg({ size: 1024 }), 1024))

// Kleinvariante ohne Sonne (E-13: „unter 48 px entfällt die Sonne").
writeFileSync(resolve(outDir, 'icon-small.png'), render(svg({ size: 48, sun: false, stroke: 11 }), 48))

// Vorschau für den Store-Eintrag und die Dokumentation (RB-03).
writeFileSync(resolve(docsDir, 'slowshow-icon-512.png'), render(svg({ size: 512 }), 512))

console.log('Quellbilder geschrieben nach src-tauri/icons/ und docs/')

// ── 2. Vollen Satz erzeugen lassen ───────────────────────────────────────────

// `shell: true` ist unter Windows Pflicht: seit Node 20 verweigert `spawnSync`
// das direkte Starten einer `.cmd`, und `npx` ist genau das. Damit wird das
// Kommando aber von der Shell zerlegt — der Pfad enthaelt ein Leerzeichen
// ("Repo Privat") und braucht deshalb Anfuehrungszeichen.
const source = resolve(outDir, 'icon-source.png')
execFileSync('npx', ['tauri', 'icon', JSON.stringify(source)], {
  cwd: root,
  stdio: 'inherit',
  shell: true,
})

// ── 3. Adaptive Vordergründe ersetzen ────────────────────────────────────────
//
// `tauri icon` leitet auch `ic_launcher_foreground.png` aus der Quelle ab und
// verkleinert sie dabei — aus der Kreisscheibe würde ein Kreis mit Luft
// ringsum, der unter der Maske als Kreis im Kreis erschiene. Der Vordergrund
// muss stattdessen randlos schwarz sein, damit die Maske die Kontur bildet.

const foreground = svg({ size: 1024, shape: 'bleed' })
for (const [density, size] of Object.entries(FOREGROUND_SIZES)) {
  const target = resolve(androidDir, density, 'ic_launcher_foreground.png')
  mkdirSync(dirname(target), { recursive: true })
  writeFileSync(target, render(foreground, size))
}

console.log('Adaptive Vordergruende ersetzt (randlos, Kontur kommt von der Maske — E-27)')
console.log('Weiter mit: node scripts/patch-android.mjs')
