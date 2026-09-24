import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { effectScope } from 'vue'
import { FEEDBACK_DURATION_MS, useFeedback } from './useFeedback'

/**
 * Die Regel aus E-62: Erfolg verblasst nach 6 s, ein Fehler bleibt stehen,
 * bis ein Versuch gelingt. Vorher hatte jeder Bereich eine eigene Dauer — und
 * im System verschwanden auch Fehler nach fünf Sekunden, bevor jemand sie
 * gelesen hatte.
 */

beforeEach(() => {
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('useFeedback', () => {
  it('lässt eine Erfolgsmeldung nach der Anzeigedauer verschwinden', () => {
    const { feedback, ok } = useFeedback()
    ok('Gespeichert')
    expect(feedback.value).toMatchObject({ kind: 'ok', text: 'Gespeichert' })

    vi.advanceTimersByTime(FEEDBACK_DURATION_MS - 1)
    expect(feedback.value).not.toBeNull()
    vi.advanceTimersByTime(1)
    expect(feedback.value).toBeNull()
  })

  it('lässt einen Fehler stehen', () => {
    const { feedback, error } = useFeedback()
    error('Verbindung fehlgeschlagen')
    vi.advanceTimersByTime(FEEDBACK_DURATION_MS * 10)
    expect(feedback.value).toMatchObject({ kind: 'error', text: 'Verbindung fehlgeschlagen' })
  })

  it('ersetzt einen Fehler durch den nächsten Erfolg', () => {
    const { feedback, error, ok } = useFeedback()
    error('kaputt')
    ok('geht wieder')
    expect(feedback.value?.kind).toBe('ok')
    vi.advanceTimersByTime(FEEDBACK_DURATION_MS)
    expect(feedback.value).toBeNull()
  })

  it('hält einen Fehler, der kurz nach einem Erfolg kommt, über dessen Ablauf hinaus', () => {
    // Der Zeitgeber des Erfolgs darf den späteren Fehler nicht mitnehmen.
    const { feedback, ok, error } = useFeedback()
    ok('erst gut')
    vi.advanceTimersByTime(1000)
    error('dann schlecht')
    vi.advanceTimersByTime(FEEDBACK_DURATION_MS)
    expect(feedback.value?.text).toBe('dann schlecht')
  })

  it('zählt dieselbe Meldung zweimal als zwei Meldungen', () => {
    // Sonst bliebe die Anzeige beim zweiten „Passwort gespeichert" stehen,
    // ohne neu zu erscheinen — und verschwände nach der Frist der ersten.
    const { feedback, ok } = useFeedback()
    ok('Passwort gespeichert')
    const first = feedback.value?.id
    vi.advanceTimersByTime(4000)
    ok('Passwort gespeichert')
    expect(feedback.value?.id).not.toBe(first)
    vi.advanceTimersByTime(4000)
    expect(feedback.value, 'die Frist beginnt neu').not.toBeNull()
  })

  it('räumt mit leerem Text auf', () => {
    const { feedback, error, ok } = useFeedback()
    error('kaputt')
    ok('')
    expect(feedback.value).toBeNull()
  })

  it('räumt den Zeitgeber mit dem Effektbereich weg', () => {
    const scope = effectScope()
    const f = scope.run(() => useFeedback())!
    f.ok('läuft')
    scope.stop()
    // Nach dem Abbau läuft kein Zeitgeber mehr, der den Zustand noch ändert.
    expect(vi.getTimerCount()).toBe(0)
  })
})
