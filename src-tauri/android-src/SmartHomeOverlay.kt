package dev.kerker.slowshow

import android.annotation.SuppressLint
import android.content.res.ColorStateList
import android.graphics.Canvas
import android.graphics.ColorFilter
import android.graphics.Paint
import android.graphics.Path
import android.graphics.PixelFormat
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.StateListDrawable
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.util.TypedValue
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.RenderProcessGoneDetail
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.webkit.WebSettingsCompat
import androidx.webkit.WebViewFeature

/**
 * Smart-Home-Ansicht über der Diashow (FA-46, E-72 bis E-79).
 *
 * ACHTUNG: versionierte Vorlage wie `MainActivity.kt`. `scripts/patch-android.mjs`
 * kopiert sie nach `gen/` — dort nie direkt bearbeiten.
 *
 * Eine **native** `WebView` über der Tauri-WebView, kein iFrame (E-72): die
 * CSP der App kennt kein `frame-src`, Home Assistant und andere Systeme setzen
 * `X-Frame-Options`, und Android-WebViews sperren Cookies von Drittseiten —
 * in einem iFrame bräche bei manchen Systemen die Anmeldung.
 *
 * Was hier entschieden wird, ist nur das, was allein die native Seite sehen
 * kann: Berührungen (für den Timeout ab der letzten Berührung, E-74), die
 * Zurück-Taste (E-78) und der Lebenslauf der WebView (E-75). Ob die Ansicht
 * überhaupt öffnet und mit welchen Werten, entscheidet `smart_home.rs`.
 *
 * Die WebView entsteht erst beim ersten Öffnen und wird danach nur versteckt,
 * nicht zerstört (E-75): beim nächsten Tipp ist sie sofort da, und Anmeldung
 * und Verbindung zum Smart Home bleiben bestehen. Eine geleerte Adresse
 * zerstört sie.
 *
 * @param onClosed wird gerufen, sobald die Ansicht zugeht — gleich aus
 *   welchem Grund. Die Activity meldet es an Rust, Rust an die Oberfläche,
 *   und die Diashow läuft weiter.
 */
class SmartHomeOverlay(
    private val activity: ComponentActivity,
    private val onClosed: () -> Unit,
) {
    private val handler = Handler(Looper.getMainLooper())

    private var container: FrameLayout? = null
    private var webView: WebView? = null
    private var button: LinearLayout? = null
    private var buttonText: TextView? = null

    /** Die zuletzt **geladene** Einstellungsadresse, nicht die aktuelle Seite. */
    private var loadedUrl = ""
    private var timeoutMs = 120_000L
    private var showButton = true
    private var corner = CORNER_TOP_RIGHT

    private val timeout = Runnable {
        Log.i(TAG, "Smart-Home-Ansicht: Zeit ohne Berührung abgelaufen")
        hide()
    }

    /**
     * Zurück-Taste bzw. Zurück-Geste bei offener Ansicht (E-78, vorläufig):
     * erst in der Seite zurück, wenn sie einen Verlauf hat, sonst zur Diashow.
     *
     * Wird bei jedem Öffnen neu angemeldet: der `OnBackPressedDispatcher` fragt
     * den zuletzt angemeldeten Rückruf zuerst, und Tauri meldet eigene an.
     */
    private val backCallback = object : OnBackPressedCallback(false) {
        override fun handleOnBackPressed() {
            val view = webView
            if (view != null && view.canGoBack()) {
                view.goBack()
                restartTimeout()
            } else {
                hide()
            }
        }
    }

    val isVisible: Boolean
        get() = container?.visibility == View.VISIBLE

    /** Öffnet die Ansicht; legt die WebView beim ersten Mal an (E-75). */
    fun show(url: String, timeoutSeconds: Int, showButton: Boolean, corner: Int, label: String) {
        if (url.isEmpty()) return
        applySettings(timeoutSeconds, showButton, corner)

        val root = container ?: create()
        val view = webView ?: return
        buttonText?.text = label
        button?.contentDescription = label

        if (url != loadedUrl) {
            loadedUrl = url
            view.loadUrl(url)
        }

        root.visibility = View.VISIBLE
        root.bringToFront()
        view.onResume()
        view.requestFocus()

        backCallback.remove()
        activity.onBackPressedDispatcher.addCallback(activity, backCallback)
        backCallback.isEnabled = true

        restartTimeout()
    }

    /**
     * Neue Einstellungen (E-75): eine geänderte Adresse wird geladen, eine
     * geleerte zerstört die WebView. Gibt es noch keine, gilt alles ab dem
     * ersten Öffnen.
     */
    fun configure(url: String, timeoutSeconds: Int, showButton: Boolean, corner: Int) {
        if (url.isEmpty()) {
            destroy()
            return
        }
        applySettings(timeoutSeconds, showButton, corner)
        val view = webView ?: return
        if (url != loadedUrl) {
            loadedUrl = url
            view.loadUrl(url)
        }
        if (isVisible) restartTimeout()
    }

    /**
     * Zurück zur Diashow. Die WebView bleibt bestehen (E-75), wird aber
     * angehalten: `WebView.onPause` meldet der Seite „verborgen“, Kamerabilder
     * und Animationen ruhen. Bewusst **nicht** `pauseTimers` — das hielte alle
     * WebViews der App an, auch die Diashow.
     */
    fun hide() {
        val root = container ?: return
        if (root.visibility != View.VISIBLE) return
        handler.removeCallbacks(timeout)
        root.visibility = View.GONE
        webView?.onPause()
        backCallback.isEnabled = false
        // Die Anmeldung soll einen Neustart der App überstehen.
        CookieManager.getInstance().flush()
        onClosed()
    }

    /**
     * Jede Berührung startet den Timeout neu (E-74).
     *
     * Aus `MainActivity.dispatchTouchEvent`, also vor der WebView und vor dem
     * Knopf — dort kommt jede Berührung vorbei. Nur Aufsetzen und Loslassen,
     * nicht jede Bewegung: das genügt für „ab der letzten Berührung“ und
     * erspart bei einer Wischgeste sechzig Neustarts je Sekunde.
     */
    fun onTouch(ev: MotionEvent) {
        if (!isVisible) return
        when (ev.actionMasked) {
            MotionEvent.ACTION_DOWN,
            MotionEvent.ACTION_POINTER_DOWN,
            MotionEvent.ACTION_UP -> restartTimeout()
        }
    }

    fun onActivityPause() {
        if (isVisible) webView?.onPause()
        if (webView != null) CookieManager.getInstance().flush()
    }

    fun onActivityResume() {
        if (isVisible) webView?.onResume()
    }

    /**
     * Baut die WebView ab — bei geleerter Adresse, abgestürztem Renderer oder
     * beim Beenden der Activity.
     *
     * @param notify `false` beim Beenden der Activity: dann läuft die App
     *   herunter, und eine Meldung an Rust träfe einen Zustand, den Tauri schon
     *   abräumt (siehe `SHUTTING_DOWN` in `lib.rs`).
     */
    fun destroy(notify: Boolean = true) {
        val wasVisible = isVisible
        handler.removeCallbacks(timeout)
        backCallback.isEnabled = false
        webView?.let { view ->
            // Erst aus der Ansicht nehmen, dann zerstören — so verlangt es die
            // Dokumentation von `WebView.destroy`.
            (view.parent as? ViewGroup)?.removeView(view)
            view.stopLoading()
            view.destroy()
        }
        webView = null
        container?.let { (it.parent as? ViewGroup)?.removeView(it) }
        container = null
        button = null
        buttonText = null
        loadedUrl = ""
        try {
            CookieManager.getInstance().flush()
        } catch (e: Throwable) {
            Log.w(TAG, "Cookies nicht gesichert: $e")
        }
        if (wasVisible && notify) onClosed()
    }

    // ── Aufbau ───────────────────────────────────────────────────────────────

    private fun applySettings(timeoutSeconds: Int, showButton: Boolean, corner: Int) {
        // Rust klemmt schon (E-76); hier nur gegen einen Fehler auf der
        // Brücke, der sonst einen Timeout von null Sekunden ergäbe.
        timeoutMs = timeoutSeconds.coerceAtLeast(MIN_TIMEOUT_SECONDS) * 1000L
        this.showButton = showButton
        this.corner = corner
        updateButton()
    }

    private fun restartTimeout() {
        handler.removeCallbacks(timeout)
        handler.postDelayed(timeout, timeoutMs)
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun create(): FrameLayout {
        val view = WebView(activity).apply {
            // Tiefschwarz wie der Rahmen (`--ss-bg`), damit beim Laden nichts
            // weiß aufblitzt.
            setBackgroundColor(COLOR_BG)
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            // Kamerastreams auf dem Dashboard sollen ohne Tipp laufen.
            settings.mediaPlaybackRequiresUserGesture = false
            // Wie in `MainActivity.onWebViewCreate` (E-64): die Seite färbt
            // sich selbst, Android soll sie nicht zusätzlich abdunkeln.
            if (WebViewFeature.isFeatureSupported(WebViewFeature.ALGORITHMIC_DARKENING)) {
                WebSettingsCompat.setAlgorithmicDarkeningAllowed(settings, false)
            }
            webViewClient = OverlayClient()
            webChromeClient = WebChromeClient()
        }

        // Anmeldung bleibt gespeichert; Drittseiten-Cookies, weil manche
        // Systeme sich über eine zweite Adresse anmelden.
        CookieManager.getInstance().apply {
            setAcceptCookie(true)
            setAcceptThirdPartyCookies(view, true)
        }

        val back = createButton()
        val root = FrameLayout(activity).apply {
            setBackgroundColor(COLOR_BG)
            visibility = View.GONE
            // Nichts fällt zur Diashow durch, auch nicht, solange die Seite
            // lädt: eine Geste dort würde sonst ein Bild weiterschalten.
            isClickable = true
            addView(
                view,
                FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT,
                ),
            )
            addView(back, buttonParams())
        }

        // Über den Inhalt der Activity, also über die Tauri-WebView.
        activity.addContentView(
            root,
            ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT),
        )

        container = root
        webView = view
        button = back
        updateButton()
        Log.i(TAG, "Smart-Home-Ansicht angelegt")
        return root
    }

    /** Navigation bleibt in der WebView; fremde Schemata werden verworfen. */
    private inner class OverlayClient : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
            val scheme = request.url.scheme?.lowercase()
            // `false` heißt: die WebView lädt selbst. `intent:`, `tel:` und
            // Ähnliches würden dort nur eine Fehlerseite zeigen.
            return scheme != "http" && scheme != "https"
        }

        /**
         * Beide WebViews teilen sich den Renderer-Prozess (R-03). Stirbt er,
         * startet der Absturzschutz im `RustWebViewClient` die App neu (E-42).
         * Hier nur `true`: ohne diese Überschreibung risse Android die App
         * sofort mit, bevor der Neustart greift.
         */
        override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
            Log.e(TAG, "Renderer der Smart-Home-Ansicht beendet (abgestürzt=${detail.didCrash()})")
            destroy()
            return true
        }
    }

    /**
     * Der Knopf „Diashow“ (E-77, vorläufig).
     *
     * Gestaltet wie die Pillen der Diashow (`.toast`, `.paused` in
     * `SlideshowView.vue`): `--ss-overlay-scrim` als Fläche, 1 dp
     * `--ss-border-strong`, `--ss-text-body` in `--ss-fs-m`, gedrückt in
     * Messing. 48 dp hoch (E-63), 24 dp vom Rand wie das Zahnrad.
     */
    private fun createButton(): LinearLayout {
        val pressed = intArrayOf(android.R.attr.state_pressed)
        val textColors = ColorStateList(arrayOf(pressed, intArrayOf()), intArrayOf(COLOR_ACCENT, COLOR_TEXT))

        val icon = ImageView(activity).apply {
            setImageDrawable(ChevronDrawable(textColors, dp(1.5f)))
            isDuplicateParentStateEnabled = true
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        }
        val text = TextView(activity).apply {
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
            setTextColor(textColors)
            letterSpacing = 0.04f
            includeFontPadding = false
            isDuplicateParentStateEnabled = true
        }
        buttonText = text

        return LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            minimumHeight = dp(48f).toInt()
            setPadding(dp(16f).toInt(), 0, dp(20f).toInt(), 0)
            background = StateListDrawable().apply {
                addState(pressed, pill(COLOR_PRESSED))
                addState(intArrayOf(), pill(COLOR_SCRIM))
            }
            isClickable = true
            isFocusable = true
            setOnClickListener { hide() }
            addView(
                icon,
                LinearLayout.LayoutParams(dp(18f).toInt(), dp(18f).toInt()).apply {
                    marginEnd = dp(10f).toInt()
                },
            )
            addView(text)
        }
    }

    private fun pill(fill: Int) = GradientDrawable().apply {
        shape = GradientDrawable.RECTANGLE
        cornerRadius = dp(24f)
        setColor(fill)
        setStroke(dp(1f).toInt().coerceAtLeast(1), COLOR_BORDER)
    }

    /** Lage des Knopfs je Ecke; die Zahlen stehen auch in `smart_home.rs`. */
    private fun buttonParams(): FrameLayout.LayoutParams {
        val gravity = when (corner) {
            CORNER_TOP_LEFT -> Gravity.TOP or Gravity.LEFT
            CORNER_BOTTOM_LEFT -> Gravity.BOTTOM or Gravity.LEFT
            CORNER_BOTTOM_RIGHT -> Gravity.BOTTOM or Gravity.RIGHT
            else -> Gravity.TOP or Gravity.RIGHT
        }
        val margin = dp(24f).toInt()
        return FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
            gravity,
        ).apply { setMargins(margin, margin, margin, margin) }
    }

    private fun updateButton() {
        val b = button ?: return
        b.visibility = if (showButton) View.VISIBLE else View.GONE
        b.layoutParams = buttonParams()
    }

    private fun dp(value: Float): Float =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, value, activity.resources.displayMetrics)

    /** Pfeil nach links, Strich 1,5 dp wie die Symbole der Diashow. */
    private class ChevronDrawable(
        private val colors: ColorStateList,
        strokeWidth: Float,
    ) : Drawable() {
        private val path = Path()
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            this.strokeWidth = strokeWidth
            strokeCap = Paint.Cap.ROUND
            strokeJoin = Paint.Join.ROUND
            color = colors.defaultColor
        }

        override fun draw(canvas: Canvas) {
            val b = bounds
            path.reset()
            path.moveTo(b.left + b.width() * 0.62f, b.top + b.height() * 0.2f)
            path.lineTo(b.left + b.width() * 0.32f, b.top + b.height() * 0.5f)
            path.lineTo(b.left + b.width() * 0.62f, b.top + b.height() * 0.8f)
            canvas.drawPath(path, paint)
        }

        override fun isStateful(): Boolean = true

        override fun onStateChange(state: IntArray): Boolean {
            val next = colors.getColorForState(state, colors.defaultColor)
            if (next == paint.color) return false
            paint.color = next
            invalidateSelf()
            return true
        }

        override fun setAlpha(alpha: Int) {
            paint.alpha = alpha
            invalidateSelf()
        }

        override fun setColorFilter(colorFilter: ColorFilter?) {
            paint.colorFilter = colorFilter
            invalidateSelf()
        }

        @Deprecated("Deprecated in Java")
        override fun getOpacity(): Int = PixelFormat.TRANSLUCENT
    }

    companion object {
        private const val TAG = "Slowshow"

        /** Ecken wie `smart_home::corner_jni_value`. */
        const val CORNER_TOP_LEFT = 0
        const val CORNER_TOP_RIGHT = 1
        const val CORNER_BOTTOM_LEFT = 2
        const val CORNER_BOTTOM_RIGHT = 3

        /** Untergrenze aus `smart_home::TIMEOUT_MIN_SECS`. */
        private const val MIN_TIMEOUT_SECONDS = 30

        // Farben aus `src/styles/tokens.css` (dunkler Satz, E-13, E-64).
        /** `--ss-bg` */
        private const val COLOR_BG = 0xFF0A0A0A.toInt()
        /** `--ss-overlay-scrim`: rgba(10, 10, 10, 0.82) */
        private const val COLOR_SCRIM = 0xD10A0A0A.toInt()
        /** `--ss-surface-accent`, gedrückt */
        private const val COLOR_PRESSED = 0xFF17150F.toInt()
        /** `--ss-border-strong` */
        private const val COLOR_BORDER = 0xFF26241F.toInt()
        /** `--ss-text-body` */
        private const val COLOR_TEXT = 0xFFE8E5E0.toInt()
        /** `--ss-accent` */
        private const val COLOR_ACCENT = 0xFFC2A878.toInt()
    }
}
