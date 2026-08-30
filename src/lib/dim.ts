/**
 * Abdunkelung als Overlay (FA-52, FA-53).
 *
 * Das schwarze Overlay wirkt überall — auch dort, wo ein Hersteller-ROM das
 * Setzen der Fensterhelligkeit ignoriert (R-04). Seit E-22 trägt es zusätzlich
 * die Nachtschwärzung allein, wenn die App die Beleuchtung gar nicht regeln
 * darf.
 *
 * Als reine Funktion und nicht als `computed` in `App.vue`: die Reihenfolge
 * der vier Fälle ist die eigentliche Logik, und jeder einzelne davon war schon
 * einmal ein Fehler.
 */
import { DEVICE_CONTROLLED_BRIGHTNESS, type DisplayState } from './types'

/**
 * Deckkraft des Abdunkelungs-Overlays, 0 (nichts) bis 1 (schwarz).
 *
 * @param display Anzeigezustand aus dem Backend; `null` vor dem ersten Laden.
 */
export function dimOpacity(display: DisplayState | null): number {
  // Ohne Zustand nicht abdunkeln: beim Start ist noch nichts bekannt, und ein
  // schwarzer Schirm wäre von einem Fehler nicht zu unterscheiden.
  if (!display) return 0

  // Im Nachtmodus nicht abdunkeln: dort steht die gedimmte Uhr auf Schwarz
  // (FA-54), und die ist bereits die abgedunkelte Darstellung. Ein
  // zusätzliches Overlay darüber machte sie unsichtbar — der Nachtmodus wäre
  // dann nicht von einem schwarzen Bildschirm zu unterscheiden.
  if (display.showNightClock) return 0

  // Außerhalb der Aktivzeit ohne Nachtuhr: schwarz, unabhängig von der
  // Helligkeit. Die hing hier früher mit drin — mit gerätegesteuerter
  // Helligkeit (E-22) senkt die App die Beleuchtung nachts aber nicht mehr,
  // und der Rahmen zeigte dann die ganze Nacht das letzte Foto.
  if (!display.slideshowActive) return 1

  // Regelt das Gerät die Helligkeit, hat das Overlay nichts zu tun. Ohne
  // diesen Fall klemmte die Null unten auf 1 und legte 99 Prozent Schwarz über
  // das Bild — aus „App regelt nicht" würde ein schwarzer Schirm.
  if (display.brightness === DEVICE_CONTROLLED_BRIGHTNESS) return 0

  return 1 - Math.max(1, Math.min(100, display.brightness)) / 100
}
