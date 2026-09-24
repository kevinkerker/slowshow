import { describe, expect, it } from 'vitest'
import { woff2ForSubset } from '../../scripts/lib/fonts.mjs'

/**
 * Auszug aus einem Stylesheet von fonts.googleapis.com, gekürzt, aber in der
 * Reihenfolge, in der Google die Zeichenvorräte ausliefert. Genau diese
 * Reihenfolge hat den Fehler gemacht: die erste Datei ist nie `latin`.
 */
const CSS = `
/* cyrillic-ext */
@font-face {
  font-family: 'Cormorant Garamond';
  font-style: normal;
  font-weight: 300;
  src: url(https://fonts.gstatic.com/s/cormorant/v1/cyrillic-ext.woff2) format('woff2');
  unicode-range: U+0460-052F, U+1C80-1C8A;
}
/* latin-ext */
@font-face {
  font-family: 'Cormorant Garamond';
  font-style: normal;
  font-weight: 300;
  src: url(https://fonts.gstatic.com/s/cormorant/v1/latin-ext.woff2) format('woff2');
  unicode-range: U+0100-02BA, U+02BD-02C5;
}
/* latin */
@font-face {
  font-family: 'Cormorant Garamond';
  font-style: normal;
  font-weight: 300;
  src: url(https://fonts.gstatic.com/s/cormorant/v1/latin.woff2) format('woff2');
  unicode-range: U+0000-00FF, U+0131, U+2000-206F;
}
/* latin */
@font-face {
  font-family: 'Cormorant Garamond';
  font-style: normal;
  font-weight: 400;
  src: url(https://fonts.gstatic.com/s/cormorant/v1/latin-400.woff2) format('woff2');
  unicode-range: U+0000-00FF, U+0131, U+2000-206F;
}
`

describe('woff2ForSubset', () => {
  it('nimmt den Zeichenvorrat latin, nicht die erste Datei', () => {
    // Vorher: die erste URL — hier cyrillic-ext. Ohne a–z, 0–9 und Umlaute
    // fiel am Gerät jedes Zeichen auf die Systemschrift zurück.
    expect(woff2ForSubset(CSS)).toBe('https://fonts.gstatic.com/s/cormorant/v1/latin.woff2')
  })

  it('verwechselt latin nicht mit latin-ext', () => {
    // Beide Kommentare beginnen gleich; ein Vergleich nur des Anfangs hätte
    // die zweite Datei geliefert.
    expect(woff2ForSubset(CSS)).not.toContain('latin-ext')
  })

  it('findet auf Wunsch einen anderen Zeichenvorrat', () => {
    expect(woff2ForSubset(CSS, 'latin-ext')).toBe(
      'https://fonts.gstatic.com/s/cormorant/v1/latin-ext.woff2',
    )
  })

  it('meldet null, wenn es den Zeichenvorrat nicht gibt', () => {
    // Lieber ein Abbruch mit Meldung als stillschweigend die falsche Datei.
    expect(woff2ForSubset(CSS, 'greek')).toBeNull()
    expect(woff2ForSubset('')).toBeNull()
  })
})
