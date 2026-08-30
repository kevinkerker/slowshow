package dev.kerker.slowshow

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log

/**
 * Vordergrunddienst für den Dauerbetrieb (NF-01, NF-02, R-04).
 *
 * ACHTUNG: versionierte Vorlage. Wird von `scripts/patch-android.mjs` nach
 * `src-tauri/gen/android/...` kopiert — `gen/` ist generiert und gitignored.
 *
 * ## Was der Dienst leistet
 *
 * Android stuft eine App ohne sichtbare Activity als entbehrlich ein und
 * beendet sie bei Speicherdruck. Für einen Rahmen, der wochenlang laufen soll,
 * ist das der wahrscheinlichste Grund, warum er morgens schwarz ist. Ein
 * Vordergrunddienst hebt die Prozesspriorität dauerhaft an; `START_STICKY`
 * sorgt dafür, dass Android ihn nach einem Abschuss neu anlegt.
 *
 * ## Was er ausdrücklich nicht leistet
 *
 * Er ist **kein vollständiger Watchdog**. Zwei Grenzen sind unvermeidbar:
 *
 *  - Bei `panic = "abort"` reisst ein Rust-Panic den ganzen Prozess mit, den
 *    Dienst eingeschlossen. Es bleibt niemand übrig, der neu starten könnte.
 *  - Seit Android 10 dürfen Dienste aus dem Hintergrund keine Activity mehr
 *    starten. Der Versuch in [onStartCommand] gelingt, solange die App noch im
 *    Vordergrund gilt, und scheitert still, wenn nicht.
 *
 * NF-02 ist damit besser erfüllt als vorher, aber nicht abgehakt. Ehrlich
 * bleibt: der Dienst verhindert den häufigen Fall (Abschuss wegen Speicher),
 * nicht den seltenen (Absturz).
 *
 * ## Warum `specialUse`
 *
 * Ab Android 14 braucht jeder Vordergrunddienst einen Typ. Zutreffend wäre
 * keiner der vorgegebenen — `mediaPlayback` wäre schlicht gelogen, und eine
 * Falschangabe fliegt bei der Play-Prüfung auf. `specialUse` ist der dafür
 * vorgesehene Weg und verlangt eine Begründung im Manifest sowie im
 * Store-Eintrag (RB-03).
 */
class SlowshowService : Service() {

    override fun onCreate() {
        super.onCreate()
        createChannel()
        startForegroundCompat()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // Wurde der Dienst von Android neu angelegt (`intent == null`), war die
        // App vorher weg. Der Versuch, die Diashow zurückzuholen, kostet nichts
        // und gelingt in dem Fall, für den er gedacht ist.
        if (intent == null) {
            Log.i(TAG, "Dienst wurde neu angelegt - versuche die Diashow zu wecken")
            try {
                startActivity(
                    Intent(this, MainActivity::class.java)
                        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                )
            } catch (e: Throwable) {
                // Seit Android 10 aus dem Hintergrund meist untersagt. Kein
                // Grund, den Dienst zu beenden -- er haelt weiterhin den Prozess.
                Log.i(TAG, "Diashow nicht aus dem Hintergrund startbar: $e")
            }
        }
        return START_STICKY
    }

    /** Kein gebundener Dienst; die App spricht ihn nur über Intents an. */
    override fun onBind(intent: Intent?): IBinder? = null

    private fun createChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return

        // IMPORTANCE_MIN: keine Töne, kein Aufblenden, kein Eintrag in der
        // Statusleiste. Auf einem Bilderrahmen soll nichts blinken.
        val channel = NotificationChannel(
            CHANNEL_ID,
            "Dauerbetrieb",
            NotificationManager.IMPORTANCE_MIN,
        ).apply {
            description = "Haelt die Diashow im Dauerbetrieb am Laufen."
            setShowBadge(false)
        }
        (getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager)
            .createNotificationChannel(channel)
    }

    private fun startForegroundCompat() {
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE,
        )

        val notification: Notification = Notification.Builder(this, CHANNEL_ID)
            .setContentTitle("Slowshow")
            .setContentText("Diashow laeuft")
            .setSmallIcon(android.R.drawable.ic_menu_gallery)
            .setContentIntent(open)
            .setOngoing(true)
            .build()

        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                startForeground(
                    NOTIFICATION_ID,
                    notification,
                    ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE,
                )
            } else {
                startForeground(NOTIFICATION_ID, notification)
            }
        } catch (e: Throwable) {
            // Scheitert der Vordergrundbetrieb, laeuft die App trotzdem weiter --
            // nur ohne den Schutz vor dem Abschuss. Ein Bilderrahmen darf daran
            // nicht scheitern (NF-01).
            Log.w(TAG, "Vordergrundbetrieb nicht moeglich: $e")
        }
    }

    companion object {
        private const val TAG = "Slowshow"
        private const val CHANNEL_ID = "slowshow_dauerbetrieb"
        private const val NOTIFICATION_ID = 1

        /** Startet den Dienst; mehrfacher Aufruf ist unschaedlich. */
        fun start(context: Context) {
            val intent = Intent(context, SlowshowService::class.java)
            try {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    context.startForegroundService(intent)
                } else {
                    context.startService(intent)
                }
            } catch (e: Throwable) {
                Log.w(TAG, "Dienst nicht startbar: $e")
            }
        }
    }
}
