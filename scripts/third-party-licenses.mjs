// Erzeugt die Drittlizenz-Übersicht aus dem tatsächlichen Abhängigkeitsbaum.
//
// ## Warum generiert und nicht von Hand gepflegt
//
// RB-05 und Abschnitt 5.1 des Lastenhefts verlangen eine Drittlizenz-Übersicht
// im Repository. Eine handgeschriebene Liste ist beim nächsten `cargo update`
// falsch, ohne dass es jemand merkt — und falsch ist hier schlimmer als gar
// nicht vorhanden, weil sie eine Prüfung behauptet, die nicht stattfand.
//
// Deshalb liest dieses Skript die Lizenzangaben dort, wo sie stehen: aus
// `cargo metadata` und aus den `package.json` der installierten npm-Pakete.
//
//     npm run licenses
//
// ## Warum nach Android gefiltert wird
//
// Der volle Cargo-Baum enthält über 540 Kisten, darunter GTK- und
// WebKit-Bindungen, die nur ein Linux-Desktop-Build zieht. Ausgeliefert wird
// eine Android-APK (RB-02), also zählt `--filter-platform
// aarch64-linux-android`. Eine Übersicht, die Abhängigkeiten aufführt, die im
// Produkt gar nicht vorkommen, ist nicht gründlicher, sondern ungenauer.
//
// ## Was das Skript nicht leistet
//
// Es liest die SPDX-Angabe der Pakete, es prüft sie nicht. Bei Mehrfachlizenzen
// ("MIT OR Apache-2.0") trifft es keine Wahl — die steht dem Verwender zu und
// gehört in eine Lizenzentscheidung, nicht in ein Werkzeug.

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const outFile = resolve(root, 'docs/third-party-licenses.md')

/** Lizenzfamilien, die mehr verlangen als einen Copyright-Hinweis. */
const COPYLEFT = ['GPL', 'MPL', 'CDDL', 'EPL', 'CC-BY-SA', 'OSL']

/**
 * Trifft nur zu, wenn *jede* Wahlmöglichkeit copyleft ist.
 *
 * "MIT OR LGPL-2.1-or-later" zählt also nicht — dort darf MIT gewählt werden.
 * Ohne diese Unterscheidung stünden Kisten wie `r-efi` in der Auflagenliste,
 * obwohl sie keine Auflage erzeugen.
 */
export function isCopyleft(spdx) {
  const upper = spdx.toUpperCase()
  if (!COPYLEFT.some((k) => upper.includes(k))) return false
  return upper.split(/\s+OR\s+/).every((alt) => COPYLEFT.some((k) => alt.includes(k)))
}

function cargoPackages() {
  const raw = execFileSync(
    'cargo',
    [
      'metadata',
      '--format-version',
      '1',
      '--manifest-path',
      resolve(root, 'src-tauri/Cargo.toml'),
      '--filter-platform',
      'aarch64-linux-android',
    ],
    { encoding: 'utf8', maxBuffer: 256 * 1024 * 1024, stdio: ['ignore', 'pipe', 'ignore'] },
  )
  const meta = JSON.parse(raw)

  // `--filter-platform` beschneidet den Resolve-Graphen, nicht die Paketliste.
  // Wer nur `meta.packages` liest, bekommt wieder alle Plattformen.
  const inTree = new Set(meta.resolve.nodes.map((n) => n.id))

  return meta.packages
    .filter((p) => inTree.has(p.id) && p.name !== 'slowshow')
    .map((p) => ({
      name: p.name,
      version: p.version,
      license: p.license || (p.license_file ? 'Datei: ' + p.license_file : 'keine Angabe'),
    }))
    .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version))
}

function npmPackages() {
  const raw = execFileSync('npm', ['ls', '--omit=dev', '--all', '--json'], {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    stdio: ['ignore', 'pipe', 'ignore'],
    shell: process.platform === 'win32',
  })

  const seen = new Map()
  const walk = (node) => {
    for (const [name, info] of Object.entries(node.dependencies || {})) {
      const version = info.version
      // Nicht installierte optionale Peers (etwa @vue/composition-api unter
      // vue-demi) haben keine Version. Sie liegen nicht im Baum und gehören
      // nicht in eine Übersicht dessen, was ausgeliefert wird.
      if (version && !seen.has(name + '@' + version)) {
        const pkgJson = resolve(root, 'node_modules', ...name.split('/'), 'package.json')
        let license = 'keine Angabe'
        if (existsSync(pkgJson)) {
          const d = JSON.parse(readFileSync(pkgJson, 'utf8'))
          license = d.license || 'keine Angabe'
          if (Array.isArray(d.licenses)) license = d.licenses.map((l) => l.type).join(' OR ')
        }
        seen.set(name + '@' + version, { name, version, license })
      }
      walk(info)
    }
  }
  walk(JSON.parse(raw))

  return [...seen.values()].sort((a, b) => a.name.localeCompare(b.name))
}

function counts(packages) {
  const map = new Map()
  for (const p of packages) map.set(p.license, (map.get(p.license) || 0) + 1)
  return map
}

function table(packages) {
  return ['| Paket | Version | Lizenz |', '|---|---|---|']
    .concat(packages.map((p) => '| ' + p.name + ' | ' + p.version + ' | ' + p.license + ' |'))
    .join('\n')
}

/**
 * Alles ab hier laeuft nur beim direkten Aufruf.
 *
 * Ohne diese Schranke wuerde schon ein `import` dieser Datei `cargo metadata`
 * und `npm ls` starten -- der Test von `isCopyleft` haette dann eine
 * Laufzeit von Minuten und braeuchte eine Rust-Toolchain.
 */
function main() {
  const cargo = cargoPackages()
  const npm = npmPackages()
  const cargoCounts = counts(cargo)
  const npmCounts = counts(npm)
  const copyleft = [...cargo, ...npm].filter((p) => isCopyleft(p.license))

  const summary = [...new Set([...cargoCounts.keys(), ...npmCounts.keys()])]
    .sort()
    .map(
      (l) =>
        '| ' + l + ' | ' + (cargoCounts.get(l) ?? '—') + ' | ' + (npmCounts.get(l) ?? '—') + ' |',
    )
    .join('\n')

  const copyleftSection =
    copyleft.length === 0
      ? 'Keine. Alle Abhängigkeiten stehen unter permissiven Lizenzen oder lassen eine\npermissive Alternative zur Wahl.'
      : [
          'Diese Pakete stehen ausschließlich unter einer Copyleft-Lizenz und verlangen',
          'mehr als einen Copyright-Hinweis:',
          '',
          table(copyleft),
          '',
          'Alle davon stehen unter der MPL-2.0. Deren Copyleft wirkt **je Datei**, nicht',
          'auf das Gesamtwerk: Solange die Pakete unverändert eingebunden werden — was',
          'hier der Fall ist, sie kommen unverändert von crates.io — genügt es,',
          'Lizenztext und Fundstelle des Quelltexts zu nennen. Die Apache-2.0-Lizenz von',
          'Slowshow bleibt davon unberührt.',
        ].join('\n')

  const doc = [
    '# Drittlizenzen',
    '',
    '<!-- Erzeugt von scripts/third-party-licenses.mjs — nicht von Hand bearbeiten.',
    '     Neu erzeugen mit: npm run licenses -->',
    '',
    'Slowshow selbst steht unter der Apache-Lizenz 2.0 (siehe [LICENSE](../LICENSE)).',
    'Diese Übersicht erfüllt RB-05 und den Lieferpunkt aus Abschnitt 5.1 des',
    'Lastenhefts.',
    '',
    'Aufgeführt ist der Abhängigkeitsbaum der ausgelieferten Android-APK: die',
    'Cargo-Kisten gefiltert auf `aarch64-linux-android`, dazu der npm-Laufzeitbaum.',
    'Werkzeuge, die nur beim Bauen laufen — Vite, Vitest, `vue-tsc` —, werden nicht',
    'mit ausgeliefert und stehen deshalb nicht hier.',
    '',
    'Der npm-Teil ist bewusst großzügig: Vite entfernt beim Bündeln einen Teil',
    'dieser Pakete wieder. Für eine Lizenzübersicht ist zu viel aber besser als zu',
    'wenig.',
    '',
    '**Stand:** ' +
      new Date().toISOString().slice(0, 10) +
      ' — ' +
      cargo.length +
      ' Rust-Kisten, ' +
      npm.length +
      ' npm-Pakete.',
    '',
    '## Zusammenfassung',
    '',
    '| Lizenz | Rust | npm |',
    '|---|---|---|',
    summary,
    '',
    '## Copyleft-Auflagen',
    '',
    copyleftSection,
    '',
    '## Schriften',
    '',
    'Beide Schriften werden lokal gebündelt und nie zur Laufzeit nachgeladen',
    '(NF-04, FA-26). Sie liegen als woff2 unter `public/fonts/`.',
    '',
    '| Schrift | Lizenz | Herkunft |',
    '|---|---|---|',
    '| Instrument Sans | SIL Open Font License 1.1 | Google Fonts |',
    '| Cormorant Garamond | SIL Open Font License 1.1 | Google Fonts |',
    '',
    'Die OFL ist mit der Apache-2.0 verträglich. Sie verlangt, dass die',
    'Schriftdateien unter derselben Lizenz weitergegeben und nicht verändert unter',
    'ihrem Originalnamen vertrieben werden — beides ist eingehalten, die Dateien',
    'sind unverändert.',
    '',
    '## Rust (`aarch64-linux-android`)',
    '',
    table(cargo),
    '',
    '## npm (Laufzeit)',
    '',
    table(npm),
    '',
  ].join('\n')

  writeFileSync(outFile, doc, 'utf8')
  console.log(
    'docs/third-party-licenses.md geschrieben: ' +
      cargo.length +
      ' Rust-Kisten, ' +
      npm.length +
      ' npm-Pakete, ' +
      copyleft.length +
      ' mit Copyleft-Auflage.',
  )
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main()
}
