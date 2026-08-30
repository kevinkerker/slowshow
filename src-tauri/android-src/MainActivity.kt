package dev.kerker.slowshow

import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ActivityInfo
import android.os.BatteryManager
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
 *  - R-08   Akkuzustand im Dauerbetrieb auslesen (E-23)
 *  - NF-01  Vordergrunddienst gegen den Abschuss bei Speicherdruck (E-24)
 *  - E-26   Ausrichtung des Rahmens aus der App heraus setzen
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

        // Nach dem Immersive-Modus, damit ein Fehler im Dienst den sichtbaren
        // Teil des Starts nicht aufhaelt (NF-01, E-24).
        SlowshowService.start(this)
    }

    /** Registriert diese Activity im Rust-Backend (siehe `src/android_bridge.rs`). */
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
     * @param level Helligkeit in Prozent (1..100), oder 0 für „Gerät regelt
     *   selbst" (E-22). Bei 0 gibt das Fenster den Override mit
     *   `BRIGHTNESS_OVERRIDE_NONE` zurück, und die Helligkeitsautomatik des
     *   Systems greift wieder. Ohne diesen Weg bliebe die zuletzt gesetzte
     *   Helligkeit stehen, solange die App im Vordergrund ist — das Abschalten
     *   der Steuerung hätte dann sichtbar keine Wirkung.
     */
    // @Keep ist keine Kosmetik: die Methode wird ausschließlich über JNI
    // gerufen, R8 sieht im Release-Build also keinen Aufrufer und würde sie
    // sonst entfernen oder umbenennen — der Aufruf schlüge erst auf dem
    // fertigen Gerät fehl, nicht im Debug-Build.
    @Keep
    fun setScreenBrightness(level: Int) {
        val value = if (level <= 0) {
            WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE
        } else {
            level.coerceIn(1, 100) / 100f
        }
        runOnUiThread {
            val params = window.attributes
            params.screenBrightness = value
            window.attributes = params
        }
    }

    /**
     * Ausrichtung des Rahmens setzen (E-26).
     *
     * Über `requestedOrientation` statt über das Manifest: ein fest montierter
     * Rahmen wird einmal eingestellt und soll danach nie wieder drehen, aber
     * *welche* Ausrichtung das ist, weiß erst der Nutzer beim Aufhängen.
     *
     * `SENSOR_LANDSCAPE` und `SENSOR_PORTRAIT` statt der festen Varianten:
     * damit ist es gleichgültig, ob der Rahmen um 180 Grad gedreht hängt —
     * das Kabel darf auf der Seite herauskommen, auf der die Steckdose ist.
     *
     * @param mode 0 = quer, 1 = hoch, 2 = dem Lagesensor folgen.
     */
    @Keep
    fun setOrientation(mode: Int) {
        runOnUiThread {
            requestedOrientation = when (mode) {
                1 -> ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT
                2 -> ActivityInfo.SCREEN_ORIENTATION_FULL_SENSOR
                else -> ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
            }
        }
    }

    /**
     * Akkuzustand als "Prozent;Zehntelgrad;Laedt" (E-23).
     *
     * Eine Zeichenkette statt drei Werte, weil ein einzelner JNI-Aufruf mit
     * String-Rückgabe erheblich weniger Zeremonie braucht als drei Aufrufe oder
     * ein `int[]`. Zerlegt wird sie in `battery::parse` — dort hat sie Tests,
     * hier hätte sie keine.
     *
     * Der Ladestand kommt aus dem `BatteryManager`, Temperatur und Netzbetrieb
     * aus dem klebenden `ACTION_BATTERY_CHANGED`. Fällt eines davon aus, wird
     * -1 gemeldet; `battery::parse` verwirft die Angabe dann, statt einen
     * Rahmen mit 255 Prozent Ladestand nach Home Assistant zu senden.
     */
    @Keep
    fun batteryState(): String = try {
        val manager = getSystemService(BATTERY_SERVICE) as BatteryManager
        val percent = manager.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY)

        // Klebender Broadcast, deshalb `null` als Empfänger: das liest den
        // zuletzt gesendeten Zustand, ohne einen Empfänger anzumelden — und
        // umgeht damit die Flag-Pflicht aus Android 14 für echte Empfänger.
        val status = registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        val deciCelsius = status?.getIntExtra(BatteryManager.EXTRA_TEMPERATURE, -1) ?: -1
        val plugged = status?.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) ?: 0

        "$percent;$deciCelsius;${if (plugged != 0) 1 else 0}"
    } catch (e: Throwable) {
        // Ein Bilderrahmen darf an der Akkuanzeige nicht scheitern (NF-01).
        Log.w(TAG, "Akkuzustand nicht lesbar: $e")
        "-1;-1;0"
    }

    /** Hilfsaufruf für Tests am Gerät: aktueller Zustand der Systemleisten. */
    @Suppress("DEPRECATION")
    fun areSystemBarsVisible(): Boolean =
        (window.decorView.systemUiVisibility and View.SYSTEM_UI_FLAG_FULLSCREEN) == 0

    private companion object {
        const val TAG = "Slowshow"
    }
}
