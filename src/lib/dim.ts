/**
 * Schwärzung als Overlay (FA-52, FA-54).
 *
 * Seit E-51 nur noch für die Nacht: Außerhalb der Aktivzeit legt das Overlay
 * den Schirm auf Schwarz — auch dann, wenn die App die Beleuchtung gar nicht
 * regeln darf (E-22). Die Helligkeit tags ist allein Sache der
 * Fensterhelligkeit im nativen Teil (FA-53); wer sie aus Home Assistant
 * stellt, stellt die Beleuchtung des Tablets, nicht ein Bild, das ein Overlay
 * abdunkelt. Vorher dimmte beides zugleich, und ein Regler auf 50 ließ ein
 * Viertel des Lichts durch.
 *
 * Als reine Funktion und nicht als `computed` in `App.vue`: die Reihenfolge
 * der Fälle ist die eigentliche Logik, und jeder einzelne davon war schon
 * einmal ein Fehler.
 */
import type { DisplayState } from './types'

/**
 * Deckkraft des Overlays, 0 (nichts) oder 1 (schwarz).
 *
 * @param display Anzeigezustand aus dem Backend; `null` vor dem ersten Laden.
 */
export function dimOpacity(display: DisplayState | null): number {
  // Ohne Zustand nicht schwärzen: beim Start ist noch nichts bekannt, und ein
  // schwarzer Schirm wäre von einem Fehler nicht zu unterscheiden.
  if (!display) return 0

  // Im Nachtmodus nicht schwärzen: dort steht die gedimmte Uhr auf Schwarz
  // (FA-54), und die ist bereits die abgedunkelte Darstellung. Ein
  // zusätzliches Overlay darüber machte sie unsichtbar — der Nachtmodus wäre
  // dann nicht von einem schwarzen Bildschirm zu unterscheiden.
  if (display.showNightClock) return 0

  // Außerhalb der Aktivzeit ohne Nachtuhr: schwarz, unabhängig von der
  // Helligkeit. Mit gerätegesteuerter Helligkeit (E-22) senkt die App die
  // Beleuchtung nachts nicht — ohne diesen Fall zeigte der Rahmen die ganze
  // Nacht das letzte Foto.
  if (!display.slideshowActive) return 1

  // Tags dimmt nichts (E-51): die Helligkeit stellt der native Teil.
  return 0
}
