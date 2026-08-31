/**
 * Testumgebung für das Frontend.
 *
 * Die Tauri-Brücke gibt es im Test nicht. Statt jeden Test einzeln zu mocken,
 * wird `@tauri-apps/api` hier zentral ersetzt — so bleiben die Tests auf die
 * eigene Logik konzentriert, und ein versehentlicher echter `invoke`-Aufruf
 * fällt sofort als Fehler auf.
 */
import { vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async () => {
    throw new Error('invoke ist im Test nicht verfügbar — bitte gezielt mocken')
  }),
  convertFileSrc: (path: string, protocol = 'asset') =>
    `http://${protocol}.localhost/${path}`,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
  emit: vi.fn(async () => {}),
}))

/**
 * `IntersectionObserver` kennt jsdom nicht.
 *
 * Der Bild-Browser laedt damit nach, sobald der Fusspunkt in Sicht kommt
 * (NF-06). Ohne diesen Ersatz bricht sein `mounted`-Haken ab — die
 * Zusicherungen liefen zwar trotzdem, aber jeder Lauf meldete unbehandelte
 * Fehler, und darin gingen echte unter.
 *
 * Bewusst ohne Verhalten: Nachladen beim Scrollen laesst sich in jsdom
 * ohnehin nicht ausloesen, und ein halbherziger Ersatz taeuschte eine
 * Abdeckung vor, die es nicht gibt.
 */
class ObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
  takeRecords() {
    return []
  }
  readonly root = null
  readonly rootMargin = ''
  readonly thresholds: number[] = []
}

vi.stubGlobal('IntersectionObserver', ObserverStub)
