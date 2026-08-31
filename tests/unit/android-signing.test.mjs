import { describe, expect, it } from 'vitest'
import { SIGN_MARKER, keystoreIssue, withSigningConfig } from '../../scripts/lib/android-signing.mjs'

/**
 * Auszug aus der von Tauri erzeugten Datei -- gekuerzt, aber mit allen drei
 * Ankern, an denen der Eingriff haengt.
 */
const VORLAGE = `import java.util.Properties

plugins {
    id("com.android.application")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
}

android {
    compileSdk = 36
    defaultConfig {
        applicationId = "dev.kerker.slowshow"
    }
    buildTypes {
        getByName("debug") {
            isMinifyEnabled = false
        }
        getByName("release") {
            isMinifyEnabled = true
        }
    }
}
`

describe('withSigningConfig', () => {
  it('traegt Loader, signingConfig und Zuweisung ein', () => {
    const { gradle, changed } = withSigningConfig(VORLAGE)

    expect(changed).toBe(true)
    expect(gradle).toContain('val keystoreProperties = Properties().apply {')
    expect(gradle).toContain('signingConfigs {')
    expect(gradle).toContain('signingConfig = signingConfigs.getByName("release")')
  })

  it('deklariert signingConfigs vor buildTypes', () => {
    // Die Kotlin-DSL wertet den Block der Reihe nach aus. Stuende die
    // Deklaration danach, braeche der Bau beim Zugriff -- und zwar erst im
    // Release-Bau, den kaum jemand vor dem Hochladen laufen laesst.
    const { gradle } = withSigningConfig(VORLAGE)

    expect(gradle.indexOf('signingConfigs {')).toBeLessThan(gradle.indexOf('buildTypes {'))
  })

  it('laesst den Loader vor android { stehen', () => {
    // `keystoreProperties` wird innerhalb von `android { }` gelesen; eine
    // Deklaration darin waere zu spaet und ausserhalb des Sichtbereichs.
    const { gradle } = withSigningConfig(VORLAGE)

    expect(gradle.indexOf('val keystoreProperties')).toBeLessThan(gradle.indexOf('android {'))
  })

  it('ruehrt den Debug-Zweig nicht an', () => {
    // Debug signiert Gradle selbst. Griffe der Eingriff dort hinein, waere der
    // taegliche Testweg kaputt -- fuer einen Nutzen, den es dort nicht gibt.
    const { gradle } = withSigningConfig(VORLAGE)
    const debug = gradle.slice(
      gradle.indexOf('getByName("debug")'),
      gradle.indexOf('getByName("release")'),
    )

    expect(debug).not.toContain('signingConfig')
  })

  it('bleibt beim zweiten Lauf unveraendert', () => {
    // Das Skript laeuft vor jedem Android-Bau. Ohne Erkennungszeile stapelten
    // sich die Bloecke, bis Gradle mit einem Duplikat abbricht.
    const einmal = withSigningConfig(VORLAGE).gradle
    const zweimal = withSigningConfig(einmal)

    expect(zweimal.changed).toBe(false)
    expect(zweimal.gradle).toBe(einmal)
    expect(einmal.split(SIGN_MARKER)).toHaveLength(2)
  })

  it('bricht ab, wenn die Tauri-Vorlage nicht mehr passt', () => {
    // Lieber ein lauter Fehler als eine Datei, die zwar gebaut wird, aber
    // unsigniert bleibt -- das faellt sonst erst im Play Store auf.
    expect(() => withSigningConfig('plugins {\n}\n')).toThrow(/nicht gefunden/)
  })
})

describe('keystoreIssue', () => {
  const vorhanden = () => true
  const gut = [
    'storeFile=C:/Users/test/slowshow.keystore',
    'keyAlias=slowshow',
    'storePassword=geheim',
    'keyPassword=geheim',
  ].join('\n')

  it('meldet nichts, wenn alles eingetragen ist', () => {
    expect(keystoreIssue(gut, vorhanden)).toBeNull()
  })

  it('meldet die fehlende Datei', () => {
    expect(keystoreIssue(null, vorhanden)).toMatch(/fehlt/)
  })

  it('meldet die fehlende Schluesseldatei mit Pfad', () => {
    // Der Pfad gehoert in die Meldung: der haeufigste Fehler ist ein
    // Rueckstrich statt Vorwaertsschrägstrich, und den sieht man nur so.
    const issue = keystoreIssue(gut, () => false)

    expect(issue).toContain('C:/Users/test/slowshow.keystore')
  })

  it('meldet leere Felder einzeln', () => {
    expect(keystoreIssue(gut.replace('storePassword=geheim', 'storePassword='), vorhanden))
      .toMatch(/storePassword/)
    expect(keystoreIssue(gut.replace('keyAlias=slowshow', 'keyAlias='), vorhanden))
      .toMatch(/keyAlias/)
  })

  it('meldet ein Feld, das gar nicht dasteht', () => {
    const ohne = gut.split('\n').filter((l) => !l.startsWith('keyPassword')).join('\n')

    expect(keystoreIssue(ohne, vorhanden)).toMatch(/keyPassword/)
  })

  it('stoert sich nicht an Kommentaren und Windows-Zeilenenden', () => {
    // Die ausgelieferte Vorlage ist voller Kommentare, und Windows-Editoren
    // schreiben CRLF. Beides darf die Pruefung nicht als "leer" lesen.
    const mitBallast = `# Signaturschluessel (RB-03)\r\n${gut.replace(/\n/g, '\r\n')}\r\n`

    expect(keystoreIssue(mitBallast, vorhanden)).toBeNull()
  })
})
