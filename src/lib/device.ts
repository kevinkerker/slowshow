/**
 * Gerät und Systemfassung aus der Kennung der WebView (Wartung F11).
 *
 * ## Warum nicht `@tauri-apps/plugin-os`
 *
 * Für zwei Zeichenketten eine Abhängigkeit samt Android-Anteil aufzunehmen
 * steht in keinem Verhältnis — dieselbe Überlegung wie bei E-02 und E-04.
 * Die Kennung der WebView trägt beides ohnehin:
 *
 * ```text
 * Mozilla/5.0 (Linux; Android 15; 23043RP34G Build/AQ3A…) AppleWebKit/537.36 …
 * ```
 *
 * ## Was das kostet
 *
 * Die Kennung ist kein Vertrag. Ändert Chromium ihren Aufbau, stehen im
 * Diagnosebericht Fragezeichen statt Gerätename — ärgerlich, aber folgenlos:
 * der Bericht bleibt im Übrigen vollständig, und nichts hängt davon ab.
 */

/** Was sich aus der Kennung herauslesen ließ. */
export interface DeviceInfo {
  /** Android-Fassung, etwa `15`. `?`, wenn nicht erkennbar. */
  androidRelease: string
  /** Modellkennung, etwa `23043RP34G`. `?`, wenn nicht erkennbar. */
  deviceModel: string
}

/** Steht in beiden Feldern, wenn die Kennung nichts hergibt. */
export const UNKNOWN = '?'

/**
 * Liest Android-Fassung und Modell aus einer User-Agent-Kennung.
 *
 * Das Modell endet vor ` Build/` oder der schließenden Klammer — Geräte­namen
 * enthalten Leerzeichen („Pixel 9a"), ein Abbruch beim ersten Leerzeichen
 * schnitte sie ab.
 */
export function parseUserAgent(ua: string): DeviceInfo {
  const treffer = /Android\s+([\d.]+);\s*([^;)]+?)(?:\s+Build\/|\)|;)/.exec(ua)
  if (!treffer) return { androidRelease: UNKNOWN, deviceModel: UNKNOWN }

  const modell = treffer[2].trim()
  return {
    androidRelease: treffer[1],
    // „wv" steht für WebView und ist kein Gerätename.
    deviceModel: modell && modell !== 'wv' ? modell : UNKNOWN,
  }
}

/** Kennung des laufenden Browsers auswerten. */
export function currentDevice(): DeviceInfo {
  if (typeof navigator === 'undefined') {
    return { androidRelease: UNKNOWN, deviceModel: UNKNOWN }
  }
  return parseUserAgent(navigator.userAgent)
}
