import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  createGestureRecognizer,
  LONG_PRESS_MS,
  MOVE_TOLERANCE_PX,
  SWIPE_THRESHOLD_PX,
  tapZone,
} from './useGestures'

/** Breite der Bedienfläche in den Tests — Querformat eines Tablets. */
const WIDTH = 1280

function setup() {
  const calls = {
    onSwipeLeft: vi.fn(),
    onSwipeRight: vi.fn(),
    onTapLeft: vi.fn(),
    onTapCenter: vi.fn(),
    onTapRight: vi.fn(),
    onLongPress: vi.fn(),
  }
  return { calls, g: createGestureRecognizer(calls) }
}

/** Kurzer Tipp an Position `x`. */
function tap(g: ReturnType<typeof setup>['g'], x: number, y = 400) {
  g.down(x, y)
  g.up(x, y, WIDTH)
}

beforeEach(() => {
  vi.useFakeTimers()
})

describe('tapZone', () => {
  it('teilt die Fläche in Drittel', () => {
    expect(tapZone(0, 1200)).toBe('left')
    expect(tapZone(399, 1200)).toBe('left')
    expect(tapZone(600, 1200)).toBe('center')
    expect(tapZone(801, 1200)).toBe('right')
    expect(tapZone(1200, 1200)).toBe('right')
  })

  it('rechnet die Grenzen der Mitte zu', () => {
    // Genau auf der Kante soll nichts weitergeschaltet werden.
    expect(tapZone(400, 1200)).toBe('center')
    expect(tapZone(800, 1200)).toBe('center')
  })

  it('behandelt eine unbekannte Breite als Mitte', () => {
    // Sonst könnte ein Tipp bei fehlender Breite ungewollt weiterschalten.
    expect(tapZone(100, 0)).toBe('center')
    expect(tapZone(100, Number.NaN)).toBe('center')
    expect(tapZone(100, -5)).toBe('center')
  })
})

describe('Tippen (FA-41)', () => {
  it('rechts schaltet weiter', () => {
    const { calls, g } = setup()
    tap(g, WIDTH - 50)
    expect(calls.onTapRight).toHaveBeenCalledOnce()
    expect(calls.onTapCenter).not.toHaveBeenCalled()
  })

  it('links schaltet zurück', () => {
    const { calls, g } = setup()
    tap(g, 50)
    expect(calls.onTapLeft).toHaveBeenCalledOnce()
  })

  it('in der Mitte pausiert', () => {
    const { calls, g } = setup()
    tap(g, WIDTH / 2)
    expect(calls.onTapCenter).toHaveBeenCalledOnce()
    expect(calls.onTapLeft).not.toHaveBeenCalled()
    expect(calls.onTapRight).not.toHaveBeenCalled()
  })

  it('toleriert ein leichtes Verwackeln', () => {
    const { calls, g } = setup()
    g.down(WIDTH / 2, 400)
    g.up(WIDTH / 2 + MOVE_TOLERANCE_PX - 1, 402, WIDTH)
    expect(calls.onTapCenter).toHaveBeenCalledOnce()
  })

  it('zählt eine mittlere Strecke weder als Tippen noch als Wischen', () => {
    const { calls, g } = setup()
    g.down(WIDTH / 2, 400)
    g.up(WIDTH / 2 + 30, 400, WIDTH)
    expect(calls.onTapCenter).not.toHaveBeenCalled()
    expect(calls.onSwipeRight).not.toHaveBeenCalled()
  })

  it('wertet die Zone am Ort des Loslassens', () => {
    // Wer am Rand aufsetzt und minimal zur Mitte rutscht, meint die Zone,
    // in der der Finger abhebt.
    const { calls, g } = setup()
    g.down(WIDTH - 50, 400)
    g.up(WIDTH - 45, 400, WIDTH)
    expect(calls.onTapRight).toHaveBeenCalledOnce()
  })
})

describe('Wischen (FA-41)', () => {
  it('nach links schaltet weiter', () => {
    const { calls, g } = setup()
    g.down(700, 300)
    g.up(700 - SWIPE_THRESHOLD_PX - 10, 300, WIDTH)
    expect(calls.onSwipeLeft).toHaveBeenCalledOnce()
    expect(calls.onTapLeft).not.toHaveBeenCalled()
  })

  it('nach rechts schaltet zurück', () => {
    const { calls, g } = setup()
    g.down(300, 300)
    g.up(300 + SWIPE_THRESHOLD_PX + 10, 300, WIDTH)
    expect(calls.onSwipeRight).toHaveBeenCalledOnce()
  })

  it('geht dem Tippen vor', () => {
    // Ein Wisch endet fast immer in einer anderen Zone als er begann —
    // er darf nicht zusätzlich als Tipp gelten.
    const { calls, g } = setup()
    g.down(WIDTH - 100, 300)
    g.up(100, 300, WIDTH)
    expect(calls.onSwipeLeft).toHaveBeenCalledOnce()
    expect(calls.onTapLeft).not.toHaveBeenCalled()
    expect(calls.onTapCenter).not.toHaveBeenCalled()
  })

  it('ignoriert eine zu kurze Bewegung', () => {
    const { calls, g } = setup()
    g.down(700, 300)
    g.up(700 - SWIPE_THRESHOLD_PX + 5, 300, WIDTH)
    expect(calls.onSwipeLeft).not.toHaveBeenCalled()
  })

  it('wertet überwiegend senkrechtes Ziehen nicht als Wischen', () => {
    const { calls, g } = setup()
    g.down(700, 100)
    g.up(700 - SWIPE_THRESHOLD_PX - 10, 100 + 200, WIDTH)
    expect(calls.onSwipeLeft).not.toHaveBeenCalled()
    expect(calls.onTapCenter).not.toHaveBeenCalled()
  })
})

describe('Langer Druck (FA-43)', () => {
  it('öffnet nach der Haltezeit die Einstellungen', () => {
    const { calls, g } = setup()
    g.down(WIDTH / 2, 400)
    vi.advanceTimersByTime(LONG_PRESS_MS + 10)
    expect(calls.onLongPress).toHaveBeenCalledOnce()
  })

  it('funktioniert in jeder Zone', () => {
    for (const x of [50, WIDTH / 2, WIDTH - 50]) {
      const { calls, g } = setup()
      g.down(x, 400)
      vi.advanceTimersByTime(LONG_PRESS_MS + 10)
      expect(calls.onLongPress).toHaveBeenCalledOnce()
    }
  })

  it('löst danach kein Tippen aus', () => {
    // Sonst würde das Öffnen der Einstellungen zusätzlich weiterschalten.
    const { calls, g } = setup()
    g.down(WIDTH - 50, 400)
    vi.advanceTimersByTime(LONG_PRESS_MS + 10)
    g.up(WIDTH - 50, 400, WIDTH)
    expect(calls.onTapRight).not.toHaveBeenCalled()
    expect(calls.onTapCenter).not.toHaveBeenCalled()
  })

  it('bricht ab, sobald der Finger wandert', () => {
    const { calls, g } = setup()
    g.down(400, 400)
    g.move(400 + MOVE_TOLERANCE_PX + 5, 400)
    vi.advanceTimersByTime(LONG_PRESS_MS + 50)
    expect(calls.onLongPress).not.toHaveBeenCalled()
  })

  it('bricht bei einem abgebrochenen Zeigerkontakt ab', () => {
    const { calls, g } = setup()
    g.down(400, 400)
    g.cancel()
    vi.advanceTimersByTime(LONG_PRESS_MS + 50)
    expect(calls.onLongPress).not.toHaveBeenCalled()

    g.up(400, 400, WIDTH)
    expect(calls.onTapCenter).not.toHaveBeenCalled()
  })

  it('löst nicht aus, wenn vorher losgelassen wird', () => {
    const { calls, g } = setup()
    g.down(WIDTH / 2, 400)
    vi.advanceTimersByTime(LONG_PRESS_MS - 50)
    g.up(WIDTH / 2, 400, WIDTH)
    vi.advanceTimersByTime(200)
    expect(calls.onLongPress).not.toHaveBeenCalled()
    expect(calls.onTapCenter).toHaveBeenCalledOnce()
  })
})

describe('Robustheit', () => {
  it('ignoriert Bewegung und Loslassen ohne vorheriges Drücken', () => {
    const { calls, g } = setup()
    g.move(100, 100)
    g.up(100, 100, WIDTH)
    expect(calls.onTapLeft).not.toHaveBeenCalled()
    expect(calls.onSwipeLeft).not.toHaveBeenCalled()
  })

  it('verarbeitet mehrere Gesten nacheinander', () => {
    const { calls, g } = setup()
    g.down(700, 300)
    g.up(300, 300, WIDTH)
    tap(g, WIDTH / 2)
    tap(g, WIDTH - 40)
    expect(calls.onSwipeLeft).toHaveBeenCalledOnce()
    expect(calls.onTapCenter).toHaveBeenCalledOnce()
    expect(calls.onTapRight).toHaveBeenCalledOnce()
  })
})
