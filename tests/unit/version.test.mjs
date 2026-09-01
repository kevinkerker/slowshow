import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { androidVersionCode, versionCodeIssue } from '../../scripts/lib/version.mjs'

/**
 * Die Versionsnummer steht an fünf Stellen — und muss an allen dieselbe sein.
 *
 * Jede hat einen anderen Abnehmer: `Cargo.toml` liefert `CARGO_PKG_VERSION` und
 * damit das, was Oberfläche und Diagnosebericht (Wartung F11) anzeigen;
 * `tauri.conf.json` wird zu `versionName` und `versionCode` im APK und ist das,
 * was der Play Store und die App-Einstellungen des Geräts nennen; `package.json`
 * und die beiden Sperrdateien führen sie mit. Laufen sie auseinander, meldet ein
 * Nutzer eine Fassung, die es so nie gab.
 *
 * Gehoben wird ausschließlich mit `npm run bump` (E-44); dieser Test ist die
 * Gegenprobe dazu.
 */

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const lies = (p) => readFileSync(resolve(root, p), 'utf8')

/** Die `version`-Zeile des `[package]`-Abschnitts aus einer Cargo.toml. */
function cargoVersion(toml) {
  const paket = toml.split(/^\[/m).find((block) => block.startsWith('package]'))
  const treffer = paket?.match(/^version\s*=\s*"([^"]+)"/m)
  if (!treffer) throw new Error('Keine version im [package]-Abschnitt gefunden')
  return treffer[1]
}

describe('Versionsnummer', () => {
  const pkg = JSON.parse(lies('package.json')).version
  const tauri = JSON.parse(lies('src-tauri/tauri.conf.json')).version
  const cargo = cargoVersion(lies('src-tauri/Cargo.toml'))
  const cargoLock = /name = "slowshow"\r?\nversion = "([^"]+)"/.exec(lies('src-tauri/Cargo.lock'))?.[1]
  const npmLock = JSON.parse(lies('package-lock.json'))

  it('ist in package.json und tauri.conf.json dieselbe', () => {
    expect(tauri).toBe(pkg)
  })

  it('ist in Cargo.toml dieselbe', () => {
    // Aus dieser Zahl wird `CARGO_PKG_VERSION` — sie steht in der Oberfläche
    // und im Diagnosebericht.
    expect(cargo).toBe(pkg)
  })

  it('ist in beiden Sperrdateien dieselbe', () => {
    // Bleibt eine stehen, meldet `npm ci` eine Sperrdatei, die nicht zu ihrem
    // Paket passt, und `cargo` schreibt sie beim nächsten Bau still um.
    expect(cargoLock).toBe(pkg)
    expect(npmLock.version).toBe(pkg)
    expect(npmLock.packages[''].version).toBe(pkg)
  })

  it('steht ausdruecklich in tauri.conf.json', () => {
    // Das Konfigurationsschema der Tauri-CLI sagt, ohne dieses Feld werde die
    // Version aus Cargo.toml genommen. Am Gerät gemessen stimmt das für Android
    // **nicht**: ohne das Feld schreibt `tauri android build` die Datei
    // `gen/android/app/tauri.properties` gar nicht, und `build.gradle.kts`
    // fällt auf `versionName "1.0"` und `versionCode 1` zurück. Ein so gebautes
    // Bundle wäre im Play Store unbrauchbar — und der Fehler bliebe unsichtbar,
    // weil der Bau gelingt.
    expect(JSON.parse(lies('src-tauri/tauri.conf.json'))).toHaveProperty('version')
  })

  it('ist eine dreistellige Fassung', () => {
    expect(pkg).toMatch(/^\d+\.\d+\.\d+$/)
  })

  it('ergibt einen brauchbaren versionCode', () => {
    expect(versionCodeIssue(pkg)).toBeNull()
    expect(androidVersionCode(pkg)).toBeGreaterThan(0)
  })

  it('wird in der Oberflaeche nicht fest verdrahtet', () => {
    // Der Fehler, mit dem das anfing: eine Zahl in einer .vue-Datei hängt an
    // nichts und veraltet lautlos. Die Fassung kommt über `appVersion()` aus
    // dem Backend.
    const pane = lies('src/components/panes/SystemPane.vue')
    expect(pane).toContain('api.appVersion()')
    expect(pane).not.toMatch(/version:\s*'\d+\.\d+\.\d+'/)
  })
})
