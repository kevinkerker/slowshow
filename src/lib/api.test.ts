import { afterEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { imageUrl, reportDisplaySize, THUMB_PREFIX, thumbUrl } from './api'

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

/**
 * Die gemeldete Displaygröße deckelt im Backend die Zielgröße beim Aufbereiten
 * (NF-12). Wird sie zu klein gemeldet, werden Fotos unnötig weichgezeichnet;
 * zu groß gemeldet bleibt der Speicherfehler bestehen, an dem Android den
 * WebView-Renderer abgeschossen hat (R-03). `screen` liefert CSS-Pixel — die
 * Multiplikation mit `devicePixelRatio` ist der ganze Punkt der Funktion.
 */
describe('reportDisplaySize', () => {
  afterEach(() => vi.restoreAllMocks())

  function mitSchirm(w: number, h: number, ratio: number) {
    vi.spyOn(window.screen, 'width', 'get').mockReturnValue(w)
    vi.spyOn(window.screen, 'height', 'get').mockReturnValue(h)
    vi.spyOn(window, 'devicePixelRatio', 'get').mockReturnValue(ratio)
    return vi.mocked(invoke).mockResolvedValue(undefined)
  }

  it('rechnet CSS-Pixel in echte Pixel um', async () => {
    const aufruf = mitSchirm(960, 600, 2)
    await reportDisplaySize()
    expect(aufruf).toHaveBeenCalledWith('set_display_size', {
      width: 1920,
      height: 1200,
    })
  })

  it('kommt ohne devicePixelRatio aus', async () => {
    // In jsdom und auf manchen ROMs ist der Wert 0 oder undefiniert. Ohne den
    // Ersatz durch 1 meldete die App eine Displaygröße von null Pixeln — und
    // das Backend bereitete jedes Foto auf nichts herunter.
    const aufruf = mitSchirm(1920, 1200, 0)
    await reportDisplaySize()
    expect(aufruf).toHaveBeenCalledWith('set_display_size', {
      width: 1920,
      height: 1200,
    })
  })
})
