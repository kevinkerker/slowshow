import { describe, expect, it } from 'vitest'
import { WEBVIEW_MARKER, withRenderProcessGuard } from '../../scripts/lib/android-webview.mjs'

/**
 * Auszug aus der von Tauri erzeugten Datei — gekuerzt, aber mit dem Anker, an
 * dem der Eingriff haengt.
 */
const VORLAGE = `/* THIS FILE IS AUTO-GENERATED. DO NOT MODIFY!! */

package dev.kerker.slowshow

import android.webkit.*
import android.content.Context

class RustWebViewClient(webView: RustWebView, context: Context): WebViewClient() {
    var currentUrl: String = "about:blank"

    override fun onPageFinished(view: WebView, url: String) {
        Rust.onPageLoaded(url)
    }
}
`

describe('withRenderProcessGuard', () => {
  it('setzt onRenderProcessGone in die Klasse', () => {
    const { text, changed } = withRenderProcessGuard(VORLAGE)

    expect(changed).toBe(true)
    expect(text).toContain('override fun onRenderProcessGone')
    // Innerhalb der Klasse, nicht davor: sonst uebersetzt Kotlin es nicht.
    expect(text.indexOf('class RustWebViewClient')).toBeLessThan(
      text.indexOf('override fun onRenderProcessGone'),
    )
  })

  it('haelt den App-Prozess am Leben und startet neu', () => {
    // Beides gehoert zusammen: `false` liesse Android die App abraeumen, und
    // `true` allein hinterliesse eine tote WebView — also einen schwarzen
    // Rahmen, den niemand mehr weckt.
    const { text } = withRenderProcessGuard(VORLAGE)

    expect(text).toContain('return true')
    expect(text).toContain('startActivity(restart)')
    expect(text).toContain('Runtime.getRuntime().exit(0)')
  })

  it('laeuft ein zweites Mal ohne zu verdoppeln', () => {
    // Das Skript laeuft vor jedem Android-Bau, `gen/` ist dabei nicht frisch.
    const einmal = withRenderProcessGuard(VORLAGE)
    const zweimal = withRenderProcessGuard(einmal.text)

    expect(zweimal.changed).toBe(false)
    expect(zweimal.text).toBe(einmal.text)
    expect(zweimal.text.split(WEBVIEW_MARKER)).toHaveLength(2)
  })

  it('bricht ab, wenn Tauri die erzeugte Datei umbaut', () => {
    // Ein still uebersprungener Eingriff faellt erst nach Tagen Dauerbetrieb
    // auf — und dann als schwarzer Schirm. Also lieber der Bau.
    expect(() => withRenderProcessGuard('class GanzAnders : Irgendwas() {\n}\n')).toThrow(
      /Klassenkopf nicht gefunden/,
    )
  })
})
