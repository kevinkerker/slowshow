/**
 * Gestenerkennung für die Diashow (FA-41, FA-43).
 *
 * Bedienung:
 *
 * ```text
 *   ┌───────────┬───────────┬───────────┐
 *   │  zurück   │  Pause    │  weiter   │   Tippen
 *   └───────────┴───────────┴───────────┘
 *        ← wischen: weiter   wischen: zurück →
 *        lang drücken: Einstellungen
 * ```
 *
 * Tippzonen zusätzlich zum Wischen: auf einem an der Wand hängenden Rahmen ist
 * ein kurzer Tipp bequemer als eine Wischbewegung, und wer den Rahmen nur
 * anhalten will, trifft die große Mitte.
 *
 * Als reine Funktion über Pointer-Ereignisse geschrieben, damit sich die
 * Schwellwerte ohne Touchgerät testen lassen. Pointer-Events statt
 * Touch-Events, weil dieselbe Logik dann auch mit der Maus im Desktop-Build
 * funktioniert.
 */

/** Ab dieser Strecke gilt es als Wischen, nicht als Tippen. */
export const SWIPE_THRESHOLD_PX = 60
/** So weit darf sich der Finger bewegen, ohne den langen Druck abzubrechen. */
export const MOVE_TOLERANCE_PX = 12
/** Ab hier zählt es als langer Druck. */
export const LONG_PRESS_MS = 650

/**
 * Anteil der seitlichen Tippzonen an der Bildschirmbreite.
 *
 * Drittel sind die verbreitete Aufteilung (E-Book-Leser machen es genauso) und
 * damit die am wenigsten überraschende. Die Mitte bleibt großzügig — Pause ist
 * die harmlose Aktion, ein Fehlgriff dorthin ist folgenlos.
 */
export const SIDE_ZONE_FRACTION = 1 / 3

export type TapZone = 'left' | 'center' | 'right'

/** In welcher Zone liegt ein Tipp? */
export function tapZone(x: number, width: number): TapZone {
  // Unbekannte Breite: alles gilt als Mitte, damit ein Tipp nicht
  // versehentlich weiterschaltet.
  if (!Number.isFinite(width) || width <= 0) return 'center'

  const side = width * SIDE_ZONE_FRACTION
  if (x < side) return 'left'
  if (x > width - side) return 'right'
  return 'center'
}

export interface GestureHandlers {
  onSwipeLeft: () => void
  onSwipeRight: () => void
  /** Tipp links — ein Bild zurück. */
  onTapLeft: () => void
  /** Tipp in die Mitte — Pause bzw. weiter. */
  onTapCenter: () => void
  /** Tipp rechts — ein Bild weiter. */
  onTapRight: () => void
  onLongPress: () => void
}

export interface GestureState {
  down: (x: number, y: number) => void
  move: (x: number, y: number) => void
  /** `width` ist die Breite der Bedienfläche — daraus ergibt sich die Tippzone. */
  up: (x: number, y: number, width: number) => void
  cancel: () => void
}

export function createGestureRecognizer(handlers: GestureHandlers): GestureState {
  let startX = 0
  let startY = 0
  let active = false
  let longPressFired = false
  let timer: ReturnType<typeof setTimeout> | null = null

  function clearTimer() {
    if (timer) clearTimeout(timer)
    timer = null
  }

  return {
    down(x, y) {
      startX = x
      startY = y
      active = true
      longPressFired = false
      clearTimer()
      timer = setTimeout(() => {
        if (!active) return
        longPressFired = true
        handlers.onLongPress()
      }, LONG_PRESS_MS)
    },

    move(x, y) {
      if (!active) return
      const moved =
        Math.abs(x - startX) > MOVE_TOLERANCE_PX || Math.abs(y - startY) > MOVE_TOLERANCE_PX
      if (moved) clearTimer()
    },

    up(x, y, width) {
      if (!active) return
      active = false
      clearTimer()

      // Nach einem langen Druck ist die Geste erledigt — sonst würde das
      // Loslassen zusätzlich als Tippen zählen und die Diashow pausieren.
      if (longPressFired) return

      const dx = x - startX
      const dy = y - startY

      // Nur waagerechte Bewegungen zählen als Wischen; senkrechtes Ziehen ist
      // meist ein versehentliches Streifen (FA-43).
      if (Math.abs(dx) >= SWIPE_THRESHOLD_PX && Math.abs(dx) > Math.abs(dy)) {
        if (dx < 0) handlers.onSwipeLeft()
        else handlers.onSwipeRight()
        return
      }

      if (Math.abs(dx) < MOVE_TOLERANCE_PX && Math.abs(dy) < MOVE_TOLERANCE_PX) {
        switch (tapZone(x, width)) {
          case 'left':
            handlers.onTapLeft()
            break
          case 'right':
            handlers.onTapRight()
            break
          default:
            handlers.onTapCenter()
        }
      }
    },

    cancel() {
      active = false
      longPressFired = false
      clearTimer()
    },
  }
}
