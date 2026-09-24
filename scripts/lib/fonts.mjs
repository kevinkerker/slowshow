// Auswahl der Schriftdatei aus einem Google-Fonts-Stylesheet.
//
// Google liefert je Schnitt mehrere @font-face-Bloecke, einen je Zeichenvorrat
// (cyrillic-ext, cyrillic, vietnamese, latin-ext, latin), jeder mit eigenem
// `unicode-range`. Vorher nahm `fetch-fonts.mjs` schlicht die erste woff2-URL —
// und das war bei Instrument Sans `latin-ext`, bei Cormorant Garamond
// `cyrillic-ext`. Keine der drei gebuendelten Dateien enthielt a–z, 0–9 oder
// Umlaute. Weil die @font-face-Regeln in `fonts.css` kein `unicode-range`
// tragen, fiel der Browser Zeichen fuer Zeichen auf die Systemschrift zurueck:
// Uhr, Datum und Bildunterschrift standen am Geraet nie in den Schriften des
// Entwurfs, und niemand sah es, weil die Ersatzschrift ordentlich aussieht.
//
// Genommen wird deshalb ausdruecklich der Block `latin`. Er deckt U+0000–00FF
// ab — also auch ä, ö, ü, ß, · und × — sowie U+2000–206F mit den deutschen
// Anfuehrungszeichen, Gedankenstrich und Auslassungspunkten. Fuer eine
// deutsch-englische Oberflaeche reicht das; `latin-ext` braeuchte erst eine
// Sprache mit ł oder ő.

/** Zeichenvorrat, den die Oberflaeche braucht. */
export const SUBSET = 'latin'

/**
 * URL der woff2-Datei fuer einen Zeichenvorrat.
 *
 * Google kennzeichnet jeden Block mit einem Kommentar wie `/* latin *\/`
 * direkt davor. `latin-ext` beginnt mit denselben Buchstaben; verglichen wird
 * deshalb der ganze Kommentar, nicht sein Anfang.
 *
 * @param {string} css Stylesheet von fonts.googleapis.com
 * @param {string} [subset]
 * @returns {string | null}
 */
export function woff2ForSubset(css, subset = SUBSET) {
  const block = new RegExp(
    String.raw`/\*\s*${subset}\s*\*/\s*@font-face\s*\{([^}]*)\}`,
    'g',
  )
  for (const [, body] of css.matchAll(block)) {
    const url = body.match(/url\((https:\/\/fonts\.gstatic\.com\/[^)]+\.woff2)\)/)
    if (url) return url[1]
  }
  return null
}
