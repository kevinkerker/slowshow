// Faengt den Abschuss des WebView-Renderers ab (R-03, NF-02).
//
// ## Warum das noetig ist
//
// Die WebView laeuft in einem eigenen, ausgelagerten Renderer-Prozess. Android
// beendet den bei Speicherdruck **unabhaengig** davon, ob die App eine
// sichtbare Activity hat — die Ueberlegung aus E-38 gilt fuer den App-Prozess,
// nicht fuer den Renderer.
//
// Stirbt er, greift das Standardverhalten von `WebViewClient`: Android reisst
// die ganze App mit. Auf einem Rahmen an der Wand ist das ein schwarzer
// Schirm, bis jemand die App von Hand neu startet — genau das Bild, das nach
// langer Laufzeit gemeldet wurde.
//
// ## Warum es hier steht und nicht in android-src/
//
// Der einzige Ort, an dem sich `onRenderProcessGone` unterbringen laesst, ist
// der `RustWebViewClient` — und den erzeugt Tauri. Ihn zu beerben geht nicht
// (Kotlin-Klassen sind ohne `open` final), und ihn zu ersetzen auch nicht:
// `Ipc` haelt eine Referenz auf **die** Instanz und liest daraus `currentUrl`.
// Ein zweiter Client bekaeme die Adresse nie zu sehen.
//
// Bleibt der Weg, den dieses Skript ohnehin schon fuer Manifest und Gradle
// geht: den erzeugten Text ergaenzen. Eigenes Modul, weil `patch-android.mjs`
// beim Import sofort Dateien kopiert und `process.exit` ruft — hier steht nur
// Textumformung, damit es pruefbar bleibt.

/** Erkennungszeile im erzeugten Client. Macht den Lauf wiederholbar. */
export const WEBVIEW_MARKER = 'slowshow-render-process-gone'

/** Anker: die Kopfzeile der von Tauri erzeugten Klasse. */
const ANCHOR = /class RustWebViewClient\([^)]*\)\s*:\s*WebViewClient\(\)\s*\{/

const OVERRIDE = `
    // ${WEBVIEW_MARKER}: eingespielt von scripts/patch-android.mjs
    //
    // Ohne diese Ueberschreibung beendet Android die ganze App, sobald der
    // Renderer der WebView wegen Speicherdrucks abgeraeumt wird (R-03). Der
    // Rahmen bliebe schwarz, bis jemand davorsteht.
    //
    // \`true\` heisst "wir haben uns gekuemmert" und haelt den App-Prozess am
    // Leben. Wiederbeleben laesst sich die tote WebView damit aber nicht, und
    // neu bauen kann sie nur wry beim Start. Deshalb der ehrliche Weg: die App
    // neu starten. Der Prozess lebt in diesem Moment noch und gilt als im
    // Vordergrund — nur deshalb darf er ueberhaupt eine Activity starten.
    //
    // Was das kostet: der Cache-Index wird alle zwei Minuten geschrieben, es
    // koennen also bis zu zwei Minuten Anzeigeverlauf fehlen. Der Absturz, der
    // ohne diesen Block stattfaende, kostet dasselbe — und den Rahmen dazu.
    override fun onRenderProcessGone(
        view: android.webkit.WebView,
        detail: android.webkit.RenderProcessGoneDetail,
    ): Boolean {
        android.util.Log.e(
            "Slowshow",
            "WebView-Renderer beendet (abgestuerzt=\${detail.didCrash()}) - App wird neu gestartet",
        )
        val context = view.context.applicationContext
        val restart = context.packageManager.getLaunchIntentForPackage(context.packageName)
        if (restart != null) {
            restart.addFlags(
                android.content.Intent.FLAG_ACTIVITY_NEW_TASK or
                    android.content.Intent.FLAG_ACTIVITY_CLEAR_TASK,
            )
            context.startActivity(restart)
        }
        Runtime.getRuntime().exit(0)
        return true
    }
`

/**
 * Ergaenzt den erzeugten `RustWebViewClient` um `onRenderProcessGone`.
 *
 * @param {string} source Inhalt der erzeugten RustWebViewClient.kt
 * @returns {{ text: string, changed: boolean }}
 * @throws {Error} wenn der Anker fehlt — dann hat Tauri die Datei umgebaut und
 *   der Eingriff muss nachgezogen werden, statt still auszufallen.
 */
export function withRenderProcessGuard(source) {
  if (source.includes(WEBVIEW_MARKER)) {
    return { text: source, changed: false }
  }

  const anchor = source.match(ANCHOR)
  if (!anchor) {
    throw new Error(
      'RustWebViewClient.kt: Klassenkopf nicht gefunden — Tauri hat die erzeugte ' +
        'Datei umgebaut. scripts/lib/android-webview.mjs muss nachgezogen werden.',
    )
  }

  const at = anchor.index + anchor[0].length
  return { text: source.slice(0, at) + OVERRIDE + source.slice(at), changed: true }
}
