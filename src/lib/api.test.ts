import { describe, expect, it } from 'vitest'
import { imageUrl, THUMB_PREFIX, thumbUrl } from './api'

/**
 * Die Bild-URLs sind eine Systemgrenze, die kein Compiler prüft: das Präfix
 * steht hier und noch einmal als `THUMB_PREFIX` in `src-tauri/src/lib.rs`.
 * Läuft es auseinander, liefert das Asset-Protokoll für jedes Vorschaubild
 * eine 404 — sichtbar erst als leeres Raster auf dem Gerät.
 */
describe('Bild-URLs', () => {
  it('nutzt fuer Vorschaubilder das vereinbarte Praefix', () => {
    expect(THUMB_PREFIX).toBe('t_')
  })

  it('haengt das Praefix vor die Id, nicht dahinter', () => {
    expect(thumbUrl('abc123')).toContain('t_abc123')
  })

  it('haelt Vollbild und Vorschau auseinander', () => {
    const id = 'ea9c9c9a37489830'
    expect(imageUrl(id)).not.toBe(thumbUrl(id))
    expect(imageUrl(id)).toContain(id)
  })

  it('laesst die Vollbild-URL unveraendert', () => {
    // Die Diashow laeuft ueber diese URL. Ein Praefix hier waere ein
    // Totalausfall der Anzeige, kein kosmetischer Fehler.
    const id = 'ea9c9c9a37489830'
    expect(imageUrl(id).endsWith(id)).toBe(true)
  })

  it('erzeugt eine Id, die als ein Pfadsegment durchgeht', () => {
    // Der Grund fuer das Praefix: `convertFileSrc` kodiert die Id als *ein*
    // Segment. Ein Schraegstrich darin wuerde zu %2F und liesse sich im
    // Backend nicht mehr am Pfad auftrennen.
    expect(THUMB_PREFIX).not.toContain('/')
    expect(encodeURIComponent(THUMB_PREFIX)).toBe(THUMB_PREFIX)
  })
})
