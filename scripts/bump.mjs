// Hebt die Versionsnummer und setzt einen Git-Tag (E-44).
//
//     npm run bump -- patch|minor|major
//     npm run bump -- 2.0.0
//     npm run bump -- patch --no-git      (nur die Dateien schreiben)
//
// ## Warum es dieses Skript gibt
//
// Die Nummer steht an fuenf Stellen (siehe `lib/version.mjs`), und keine davon
// laesst sich einsparen: `Cargo.toml` speist die Anzeige in der App,
// `tauri.conf.json` den `versionName` des APK, die uebrigen drei die
// npm-Sperrdateien. Von Hand sind das fuenf Handgriffe — und der fuenfte ist
// der, den man vergisst. Dieses Skript ist der einzige Schreiber.
//
// Gehoben wird **vor einem Release**, nicht bei jedem Commit: den Quellstand
// nennt der Commit-Hash, die Version nennt, was auf dem Geraet liegt. Erzwungen
// wird sie durch den Play Store, der jeden Upload mit einem nicht gewachsenen
// `versionCode` abweist.

import { execFileSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  androidVersionCode,
  nextVersion,
  STUFEN,
  versionCodeIssue,
  withCargoLockVersion,
  withCargoVersion,
  withPackageLockVersion,
  withPackageVersion,
  withTauriConfVersion,
} from './lib/version.mjs'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const args = process.argv.slice(2)
const kind = args.find((a) => !a.startsWith('-'))
const mitGit = !args.includes('--no-git')

if (!kind) {
  console.error(`Aufruf: npm run bump -- ${STUFEN.join('|')} | <x.y.z> [--no-git]`)
  process.exit(1)
}

/** Die fuenf Stellen, an denen die Nummer steht. */
const DATEIEN = [
  { pfad: 'src-tauri/Cargo.toml', umformen: withCargoVersion },
  { pfad: 'src-tauri/Cargo.lock', umformen: withCargoLockVersion },
  { pfad: 'src-tauri/tauri.conf.json', umformen: withTauriConfVersion },
  { pfad: 'package.json', umformen: withPackageVersion },
  { pfad: 'package-lock.json', umformen: withPackageLockVersion },
]

const lies = (p) => readFileSync(resolve(root, p), 'utf8')

// Cargo.toml gibt die heutige Nummer vor. Nicht weil sie dort wichtiger
// waere, sondern weil eine der fuenf Stellen den Anfang machen muss — und
// der Test in tests/unit/version.test.mjs haelt fest, dass alle gleich sind.
const cargo = lies('src-tauri/Cargo.toml')
const paket = cargo.split(/^\[/m).find((block) => block.startsWith('package]'))
const heute = /^version\s*=\s*"([^"]+)"/m.exec(paket ?? '')?.[1]
if (!heute) {
  console.error('Keine version im [package]-Abschnitt von Cargo.toml gefunden.')
  process.exit(1)
}

let neu
try {
  neu = nextVersion(heute, kind)
} catch (e) {
  console.error(e.message)
  process.exit(1)
}

// Vor dem Schreiben pruefen, nicht danach: eine ueberlaufende Stelle liesse
// sich nur mit einem weiteren Bump wieder einfangen.
const problem = versionCodeIssue(neu)
if (problem) {
  console.error(`${neu} geht nicht: ${problem}`)
  process.exit(1)
}

// Erst alle Umformungen rechnen, dann schreiben. Schlaegt eine fehl, weil sich
// der Aufbau einer Datei geaendert hat, bleibt keine halb gehobene Fassung
// zurueck.
let geschrieben
try {
  geschrieben = DATEIEN.map(({ pfad, umformen }) => ({
    pfad,
    inhalt: umformen(lies(pfad), heute, neu),
  }))
} catch (e) {
  console.error(e.message)
  process.exit(1)
}

for (const { pfad, inhalt } of geschrieben) {
  writeFileSync(resolve(root, pfad), inhalt, 'utf8')
}

console.log(`${heute} -> ${neu}  (versionCode ${androidVersionCode(neu)})`)
for (const { pfad } of DATEIEN) console.log(`  ${pfad}`)

if (!mitGit) {
  console.log('\nGit uebersprungen (--no-git).')
  process.exit(0)
}

const git = (...a) => execFileSync('git', a, { cwd: root, encoding: 'utf8' }).trim()
const tag = `v${neu}`

try {
  if (git('tag', '--list', tag)) {
    console.error(`\nTag ${tag} gibt es schon — die Dateien stehen, der Tag fehlt.`)
    process.exit(1)
  }

  // `--only`: nur diese Pfade, egal was sonst vorgemerkt ist. Sonst landete
  // angefangene Arbeit im Versions-Commit.
  git('commit', '--only', ...DATEIEN.map((d) => d.pfad), '-m', `Version ${neu}`)
  git('tag', '-a', tag, '-m', `Version ${neu}`)
  console.log(`\nCommit und Tag ${tag} gesetzt.`)
  console.log(`Naechster Schritt: git push && git push origin ${tag}`)
} catch (e) {
  console.error(`\nGit-Schritt fehlgeschlagen: ${e.message}`)
  console.error('Die Dateien sind gehoben — Commit und Tag von Hand nachziehen.')
  process.exit(1)
}
