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
