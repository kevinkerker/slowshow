import { describe, expect, it } from 'vitest'
import {
  androidVersionCode,
  nextVersion,
  parseVersion,
  VERSION_CODE_MAX,
  versionCodeIssue,
  withCargoLockVersion,
  withCargoVersion,
  withPackageLockVersion,
  withPackageVersion,
  withTauriConfVersion,
} from '../../scripts/lib/version.mjs'

/**
 * Die Rechenarbeit hinter `npm run bump` (E-44).
 *
 * Sie ist deshalb pruefbar ausgelagert, weil ihr Fehlerfall teuer ist: eine
 * einmal in den Play Store geladene Nummer ist verbraucht, und ein zu kleiner
 * `versionCode` laesst sich nicht mehr einholen.
 */

describe('parseVersion', () => {
  it('zerlegt eine dreistellige Version', () => {
    expect(parseVersion('1.2.3')).toEqual({ major: 1, minor: 2, patch: 3 })
  })

  it('weist alles andere zurueck', () => {
    // Eine krumme Version faellt sonst erst im Play Store auf.
    for (const krumm of ['1.2', '1.2.3.4', 'v1.2.3', '1.2.x', '']) {
      expect(() => parseVersion(krumm), krumm).toThrow()
    }
  })
})

describe('nextVersion', () => {
  it('setzt die kleineren Stellen zurueck', () => {
    expect(nextVersion('1.4.9', 'patch')).toBe('1.4.10')
    expect(nextVersion('1.4.9', 'minor')).toBe('1.5.0')
    expect(nextVersion('1.4.9', 'major')).toBe('2.0.0')
  })

  it('nimmt eine ausgeschriebene Version an', () => {
    expect(nextVersion('1.0.0', '2.0.0')).toBe('2.0.0')
  })

  it('laesst die Nummer nicht sinken', () => {
    // Der Play Store nimmt keinen kleineren versionCode an — eine gesenkte
    // Nummer waere eine Fassung, die sich nie hochladen laesst.
    expect(() => nextVersion('1.2.3', '1.2.3')).toThrow(/nicht ueber/)
    expect(() => nextVersion('1.2.3', '1.2.2')).toThrow(/nicht ueber/)
  })
})

describe('androidVersionCode', () => {
  it('rechnet nach der Formel der Tauri-CLI', () => {
    // major * 1000000 + minor * 1000 + patch, laut config.schema.json.
    expect(androidVersionCode('1.0.0')).toBe(1_000_000)
    expect(androidVersionCode('1.2.3')).toBe(1_002_003)
    expect(androidVersionCode('0.1.0')).toBe(1_000)
  })

  it('waechst mit jeder Stufe', () => {
    const folge = ['1.0.0', '1.0.1', '1.1.0', '2.0.0'].map(androidVersionCode)
    expect(folge).toEqual([...folge].sort((a, b) => a - b))
    expect(new Set(folge).size).toBe(folge.length)
  })
})

describe('versionCodeIssue', () => {
  it('laesst gewoehnliche Versionen durch', () => {
    expect(versionCodeIssue('1.0.0')).toBeNull()
    expect(versionCodeIssue('1.999.999')).toBeNull()
  })

  it('faengt den Ueberlauf der Patchstelle', () => {
    // 1.0.1000 ergaebe denselben versionCode wie 1.1.0 — der Upload wuerde
    // abgewiesen, und die Ursache stuende in keiner Fehlermeldung.
    expect(androidVersionCode('1.0.1000')).toBe(androidVersionCode('1.1.0'))
    expect(versionCodeIssue('1.0.1000')).toMatch(/patch/)
  })

  it('faengt den Ueberlauf der Minorstelle', () => {
    expect(versionCodeIssue('1.1000.0')).toMatch(/minor/)
  })

  it('faengt die Obergrenze des Play Store', () => {
    expect(versionCodeIssue('2101.0.0')).toMatch(/Play Store/)
    expect(androidVersionCode('2100.0.0')).toBeLessThanOrEqual(VERSION_CODE_MAX)
  })
})

describe('Ersetzen in den Dateien', () => {
  const CARGO = `[package]
name = "slowshow"
edition = "2021"
version = "1.0.0"

[dependencies]
tauri = { version = "1.0.0" }
`

  it('trifft in Cargo.toml das Paket und nicht die Abhaengigkeit', () => {
    const neu = withCargoVersion(CARGO, '1.0.0', '1.0.1')
    expect(neu).toContain('name = "slowshow"\nedition = "2021"\nversion = "1.0.1"')
    expect(neu, 'die Abhaengigkeit bleibt').toContain('tauri = { version = "1.0.0" }')
  })

  const LOCK = `{
  "name": "slowshow",
  "version": "1.0.0",
  "packages": {
    "": {
      "name": "slowshow",
      "version": "1.0.0"
    },
    "node_modules/vue": {
      "version": "1.0.0"
    }
  }
}`

  it('trifft in package-lock.json beide eigenen Stellen', () => {
    // npm fuehrt die eigene Version doppelt. Bleibt eine stehen, meldet
    // `npm ci` eine Sperrdatei, die nicht zu ihrem Paket passt.
    const neu = withPackageLockVersion(LOCK, '1.0.0', '1.0.1')
    expect(neu.match(/"version": "1\.0\.1"/g)).toHaveLength(2)
    expect(neu, 'eine Abhaengigkeit gleicher Nummer bleibt').toContain(
      '"node_modules/vue": {\n      "version": "1.0.0"',
    )
  })

  it('trifft in package.json die erste Stelle', () => {
    const neu = withPackageVersion('{\n  "name": "slowshow",\n  "version": "1.0.0"\n}', '1.0.0', '2.0.0')
    expect(neu).toContain('"version": "2.0.0"')
  })

  it('trifft in tauri.conf.json die aeussere Ebene', () => {
    // Aus diesem Feld werden versionName und versionCode. Fehlt es, schreibt
    // `tauri android build` keine tauri.properties, und das APK bekommt aus
    // build.gradle.kts die Vorgaben 1.0 und 1.
    const conf = `{
  "productName": "Slowshow",
  "version": "1.0.0",
  "bundle": {
    "android": {
      "version": "1.0.0"
    }
  }
}`
    const neu = withTauriConfVersion(conf, '1.0.0', '1.0.1')
    expect(neu).toContain('  "version": "1.0.1"')
    expect(neu, 'tiefer Eingerücktes bleibt').toContain('      "version": "1.0.0"')
  })

  it('trifft in Cargo.lock den eigenen Eintrag', () => {
    const lock = '[[package]]\nname = "serde"\nversion = "1.0.0"\n\n[[package]]\nname = "slowshow"\nversion = "1.0.0"\n'
    const neu = withCargoLockVersion(lock, '1.0.0', '1.0.1')
    expect(neu).toContain('name = "slowshow"\nversion = "1.0.1"')
    expect(neu).toContain('name = "serde"\nversion = "1.0.0"')
  })

  it('bricht ab, wenn der Anker nicht mehr passt', () => {
    // Lieber gar nicht heben als halb: ein stillschweigend uebersprungenes
    // Ersetzen hinterliesse eine Datei mit der alten Nummer.
    expect(() => withCargoVersion(CARGO, '9.9.9', '9.9.10')).toThrow(/Cargo.toml/)
    expect(() => withPackageLockVersion('{}', '1.0.0', '1.0.1')).toThrow(/package-lock/)
  })
})
