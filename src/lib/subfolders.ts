/**
 * Auswahl der Unterordner einer Quelle (FA-29, E-70).
 *
 * Dieselbe Regel wie `src-tauri/src/sources/filter.rs`, das beim Abgleich
 * entscheidet; hier setzt sie die Schalter im Quellendialog. Die Tests beider
 * Seiten prüfen dieselben Fälle — weichen sie voneinander ab, zeigte der
 * Dialog etwas anderes an, als der Abgleich holt.
 *
 * Die Regel: Für einen Ordner gilt die nächstgelegene Entscheidung auf seinem
 * Weg nach oben (`include` = an, `exclude` = aus). Ohne Entscheidung gilt
 * `includeNew`. Die Bilder direkt im gewählten Ordner schaltet `includeRoot`;
 * die Wurzel ist dabei kein Ahne, ihr Abschalten nimmt den Unterordnern nichts.
 */
import type { Source } from './types'

export interface FolderSelection {
  include: string[]
  exclude: string[]
  includeRoot: boolean
  includeNew: boolean
}

/** Pfad ohne Leerraum und Schrägstriche am Rand, wie in `filter.rs`. */
function clean(path: string): string {
  return path.trim().replace(/^\/+|\/+$/g, '')
}

/** Liegt `path` in `dir` oder ist es `dir` selbst? Mit Trennzeichen verglichen. */
export function within(path: string, dir: string): boolean {
  return path === dir || path.startsWith(`${dir}/`)
}

/**
 * Die Auswahl einer gespeicherten Quelle.
 *
 * Quellen von vor E-70 kennen nur `subfolders` als „nur diese": dann fiel die
 * Wurzel weg und neue Ordner blieben draußen; ohne Filter zählte alles. Die
 * Rückfälle hier und in `Source::include_root` müssen übereinstimmen.
 */
export function selectionOf(
  source: Pick<Source, 'subfolders' | 'excludedSubfolders' | 'includeRoot' | 'includeNewSubfolders'>,
): FolderSelection {
  const legacy = source.subfolders.length === 0
  const cleaned = (list: string[] | undefined) => (list ?? []).map(clean).filter(Boolean)
  return {
    include: cleaned(source.subfolders),
    exclude: cleaned(source.excludedSubfolders),
    includeRoot: source.includeRoot ?? legacy,
    includeNew: source.includeNewSubfolders ?? legacy,
  }
}

/** Alles an — die Vorgabe einer neuen Quelle. */
export function allSelected(): FolderSelection {
  return { include: [], exclude: [], includeRoot: true, includeNew: true }
}

function nearest(list: string[], dir: string): number {
  return list.filter((d) => within(dir, d)).reduce((max, d) => Math.max(max, d.length), -1)
}

/** Zählen die Bilder direkt in diesem Ordner? `''` ist der gewählte Ordner. */
export function dirOn(sel: FolderSelection, dir: string): boolean {
  if (dir === '') return sel.includeRoot
  const i = nearest(sel.include, dir)
  const e = nearest(sel.exclude, dir)
  if (i < 0 && e < 0) return sel.includeNew
  // Gleich lang heißt: derselbe Pfad in beiden Listen — dann gewinnt „an".
  return i >= e
}

/** Hat dieser Ordner eine eigene Entscheidung, statt sie zu erben? */
export function hasOwnDecision(sel: FolderSelection, dir: string): boolean {
  return sel.include.includes(dir) || sel.exclude.includes(dir)
}

/**
 * Schaltet einen Ordner — und damit alles darunter.
 *
 * Entscheidungen darunter fallen weg: wer „Familie" abschaltet, meint die
 * ganze Familie, nicht „alles außer dem, was ich früher einzeln angeschaltet
 * habe". Stimmt der gewünschte Zustand mit dem geerbten überein, bleibt keine
 * eigene Entscheidung stehen — die Listen bleiben so kurz, wie es geht, und
 * ein Ordner folgt weiter seinem Elternordner.
 */
export function setDir(sel: FolderSelection, dir: string, on: boolean): FolderSelection {
  if (dir === '') return { ...sel, includeRoot: on }
  const next: FolderSelection = {
    ...sel,
    include: sel.include.filter((d) => !within(d, dir)),
    exclude: sel.exclude.filter((d) => !within(d, dir)),
  }
  if (dirOn(next, dir) !== on) {
    if (on) next.include = [...next.include, dir]
    else next.exclude = [...next.exclude, dir]
  }
  return next
}

/**
 * Kurzangabe für die Karte der Quelle (E-70).
 *
 * `null`, wenn nichts gefiltert ist — dann gibt es nichts zu sagen. Mit
 * Automatik zählt, was aus ist (die Ausnahme), ohne Automatik, was an ist.
 * Die Bilder direkt im Ordner zählen jeweils mit, als ein Ordner.
 */
export function summaryOf(
  sel: FolderSelection,
): { kind: 'excluded' | 'selected'; n: number } | null {
  if (sel.includeNew) {
    const n = sel.exclude.length + (sel.includeRoot ? 0 : 1)
    return n > 0 ? { kind: 'excluded', n } : null
  }
  return { kind: 'selected', n: sel.include.length + (sel.includeRoot ? 1 : 0) }
}
