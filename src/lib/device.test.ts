import { describe, expect, it } from 'vitest'
import { parseUserAgent, UNKNOWN } from './device'

/**
 * Gerät und Systemfassung aus der WebView-Kennung (Wartung F11).
 *
 * Die Kennung ist kein Vertrag — deshalb prüfen die Tests echte Beispiele aus
 * den beiden Geräten dieses Projekts und die Fälle, in denen nichts
 * herauszulesen ist. Wichtig ist vor allem, dass ein Fehlschlag zu `?` führt
 * und nicht zu einem halben Wort, das im Diagnosebericht wie ein Gerätename
 * aussähe.
 */

const XIAOMI =
  'Mozilla/5.0 (Linux; Android 15; 23043RP34G Build/AQ3A.240912.001; wv) ' +
  'AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/131.0.6778.135 Mobile Safari/537.36'

const PIXEL =
  'Mozilla/5.0 (Linux; Android 16; Pixel 9a Build/BP2A.250705.008; wv) ' +
  'AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/138.0.7204.63 Mobile Safari/537.36'

describe('parseUserAgent', () => {
  it('liest Fassung und Modell des Referenzgeraets', () => {
    // Xiaomi Pad 6, das Referenzgeraet aus E-10.
    expect(parseUserAgent(XIAOMI)).toEqual({
      androidRelease: '15',
      deviceModel: '23043RP34G',
    })
  })

  it('haelt Geraetenamen mit Leerzeichen zusammen', () => {
    // „Pixel 9a" — ein Abbruch beim ersten Leerzeichen ergaebe „Pixel".
    expect(parseUserAgent(PIXEL)).toEqual({
      androidRelease: '16',
      deviceModel: 'Pixel 9a',
    })
  })

  it('kommt ohne Build-Angabe aus', () => {
    const ua = 'Mozilla/5.0 (Linux; Android 14; SM-X200) AppleWebKit/537.36'
    expect(parseUserAgent(ua)).toEqual({
      androidRelease: '14',
      deviceModel: 'SM-X200',
    })
  })

  it('meldet Fragezeichen statt zu raten', () => {
    // Ein Bruchstueck saehe im Bericht wie ein Geraetename aus und schickte
    // jeden, der ihn liest, in die Irre.
    for (const ua of [
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
      '',
      'Android',
    ]) {
      expect(parseUserAgent(ua)).toEqual({
        androidRelease: UNKNOWN,
        deviceModel: UNKNOWN,
      })
    }
  })

  it('nimmt „wv" nicht fuer einen Geraetenamen', () => {
    // Manche Kennungen fuehren nur „wv" fuer WebView, ohne Modell.
    const ua = 'Mozilla/5.0 (Linux; Android 13; wv) AppleWebKit/537.36'
    expect(parseUserAgent(ua).deviceModel).toBe(UNKNOWN)
    expect(parseUserAgent(ua).androidRelease).toBe('13')
  })

  it('nimmt auch Fassungen mit Punkt', () => {
    const ua = 'Mozilla/5.0 (Linux; Android 12.1; Foo Build/X)'
    expect(parseUserAgent(ua).androidRelease).toBe('12.1')
  })
})
