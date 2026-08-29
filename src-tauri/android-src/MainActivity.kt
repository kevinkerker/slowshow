package dev.kerker.slowshow

import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.View
import android.view.WindowManager
import androidx.annotation.Keep
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

/**
 * Nativer Teil von Slowshow.
 *
 * ACHTUNG: Diese Datei ist die versionierte Vorlage. Sie wird von
 * `scripts/patch-android.mjs` nach
 * `src-tauri/gen/android/app/src/main/java/dev/kerker/slowshow/MainActivity.kt`
 * kopiert. Änderungen bitte **hier** vornehmen — `gen/` ist generiert,
 * gitignored und wird von `tauri android init` überschrieben.
 *
 * Drei Anforderungen, die Tauri nicht selbst abdeckt (Lastenheft Abschnitt 8):
 *
 *  - FA-01  Vollbild ohne Status- und Navigationsleiste (Immersive Sticky)
 *  - FA-50  Bildschirm bleibt dauerhaft an (FLAG_KEEP_SCREEN_ON)
 *  - FA-53  Displayhelligkeit aus der App heraus setzen
 *
 * Bewusst **nicht** übernommen aus vergleichbaren Projekten: `FLAG_SECURE`.
 * Für eine Tresor-App ist das richtig, für einen Bilderrahmen wäre es
 * funktional falsch — es unterbindet Screenshots und blendet den Inhalt in der
 * App-Übersicht aus.
 */
class MainActivity : TauriActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        // FA-50: Der Bildschirm darf im Dauerbetrieb nie von selbst ausgehen.
        // Als Fenster-Flag statt als Wakelock — kein Recht nötig, und Android
        // gibt es beim Beenden der Activity automatisch wieder frei.
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)

        // Der Bilderrahmen zeichnet bis in die Aussparungen des Displays.
        //
        // Die Attribute müssen zurückgeschrieben werden: `window.attributes` zu
        // verändern allein löst kein `dispatchWindowAttributesChanged` aus, die
        // Änderung käme also je nach Zeitpunkt gar nicht an.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            window.attributes = window.attributes.apply {
                layoutInDisplayCutoutMode =
                    WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            }
        }

        WindowCompat.setDecorFitsSystemWindows(window, false)
        super.onCreate(savedInstanceState)

        // Nach super.onCreate: erst dort ist die Rust-Bibliothek geladen.
        //
        // Der Rust-Teil kann die Activity nicht selbst finden — Tauri
        // initialisiert keinen ndk-Kontext. Also meldet sie sich hier einmal an;
        // ab dann kann `brightness::apply` setScreenBrightness aufrufen.
        try {
            nativeRegisterActivity()
        } catch (e: Throwable) {
            // Kein Grund, den Start abzubrechen: ohne Brücke bleibt die
            // Abdunkelung im Frontend wirksam (FA-53 teilweise).
            Log.w(TAG, "Helligkeitsbrücke nicht verfügbar: $e")
        }

        enterImmersiveMode()
    }

    /** Registriert diese Activity im Rust-Backend (siehe `src/brightness.rs`). */
    private external fun nativeRegisterActivity()

    /**
     * FA-01: Vollbild ohne Systemleisten.
     *
     * `BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE` ist der „sticky" Modus: wischt
     * jemand die Leisten hervor, verschwinden sie von selbst wieder. Ohne das
     * bliebe die Navigationsleiste nach einer versehentlichen Berührung
     * dauerhaft über dem Foto stehen.
     */
    private fun enterImmersiveMode() {
        val controller = WindowInsetsControllerCompat(window, window.decorView)
        controller.hide(WindowInsetsCompat.Type.systemBars())
        controller.systemBarsBehavior =
            WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
    }

    /**
     * Nach jedem Zurückkehren in den Vordergrund erneut in den Vollbildmodus.
     *
     * Android stellt die Systemleisten unter anderem nach einem Dialog oder
     * dem Sperrbildschirm wieder her — ohne diesen Aufruf liefe der Rahmen
     * über Tage hinweg irgendwann doch mit sichtbarer Leiste.
     */
    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) enterImmersiveMode()
    }

    override fun onResume() {
        super.onResume()
        enterImmersiveMode()
    }

    /**
     * FA-53: Displayhelligkeit für dieses Fenster setzen.
     *
     * Wirkt nur solange die App im Vordergrund ist und ändert die
     * Systemeinstellung nicht — genau das gewünschte Verhalten für einen
     * Zeitplan, der abends abdunkelt (FA-52).
     *
     * @param level Helligkeit in Prozent (1..100).
     */
    // @Keep ist keine Kosmetik: die Methode wird ausschließlich über JNI
    // gerufen, R8 sieht im Release-Build also keinen Aufrufer und würde sie
    // sonst entfernen oder umbenennen — der Aufruf schlüge erst auf dem
    // fertigen Gerät fehl, nicht im Debug-Build.
    @Keep
    fun setScreenBrightness(level: Int) {
        val clamped = level.coerceIn(1, 100)
        runOnUiThread {
            val params = window.attributes
            params.screenBrightness = clamped / 100f
            window.attributes = params
        }
    }

    /** Hilfsaufruf für Tests am Gerät: aktueller Zustand der Systemleisten. */
    @Suppress("DEPRECATION")
    fun areSystemBarsVisible(): Boolean =
        (window.decorView.systemUiVisibility and View.SYSTEM_UI_FLAG_FULLSCREEN) == 0

    private companion object {
        const val TAG = "Slowshow"
    }
}
