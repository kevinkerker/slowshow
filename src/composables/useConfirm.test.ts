import { afterEach, describe, expect, it } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'
import { CANCEL, confirm, confirmAction, confirmRequest, installConfirmGuard, settle } from './useConfirm'

/**
 * Rückfragen im eigenen Dialog (E-59).
 *
 * Geprüft wird der Vertrag, auf den sich die Bereiche verlassen: genau eine
 * offene Frage, jede Antwort kommt genau einmal an, und die Zurück-Taste von
 * Android bricht ab, statt die Einstellungen zu verlassen.
 */

afterEach(() => settle(CANCEL))

describe('confirm', () => {
  it('liefert die gewählte Aktion', async () => {
    const answer = confirm({ title: 'Wirklich?', actions: [{ id: 'go', label: 'Los' }] })
    expect(confirmRequest.value?.title).toBe('Wirklich?')
    settle('go')
    await expect(answer).resolves.toBe('go')
    expect(confirmRequest.value).toBeNull()
  })

  it('liefert „cancel", wenn abgebrochen wird', async () => {
    const answer = confirm({ title: 'Wirklich?', actions: [{ id: 'go', label: 'Los' }] })
    settle(CANCEL)
    await expect(answer).resolves.toBe(CANCEL)
  })

  it('bricht eine offene Frage ab, wenn eine neue kommt', async () => {
    // Es gibt nur einen Dialog. Die erste Frage darf nicht ewig hängen
    // bleiben — ihr Aufrufer wartet sonst auf eine Antwort, die nie kommt.
    const first = confirm({ title: 'Eins', actions: [{ id: 'a', label: 'A' }] })
    const second = confirm({ title: 'Zwei', actions: [{ id: 'b', label: 'B' }] })
    await expect(first).resolves.toBe(CANCEL)
    expect(confirmRequest.value?.title).toBe('Zwei')
    settle('b')
    await expect(second).resolves.toBe('b')
  })

  it('ist ohne offene Frage wirkungslos', () => {
    expect(() => settle('go')).not.toThrow()
    expect(confirmRequest.value).toBeNull()
  })
})

describe('confirmAction', () => {
  it('stellt eine Aktion in der Gefahrenfarbe neben Abbrechen', async () => {
    const answer = confirmAction('Quelle entfernen?', 'Text', 'Quelle entfernen')
    expect(confirmRequest.value?.actions).toEqual([
      { id: 'confirm', label: 'Quelle entfernen', variant: 'danger' },
    ])
    settle('confirm')
    await expect(answer).resolves.toBe(true)
  })

  it('liefert false beim Abbrechen', async () => {
    const answer = confirmAction('Frage', undefined, 'Los', 'primary')
    expect(confirmRequest.value?.actions[0].variant).toBe('primary')
    settle(CANCEL)
    await expect(answer).resolves.toBe(false)
  })
})

describe('Zurück-Taste', () => {
  async function routerAt(path: string) {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/', component: { template: '<div />' } },
        { path: '/settings', component: { template: '<div />' } },
      ],
    })
    await router.push('/')
    await router.push(path)
    return router
  }

  it('bricht eine offene Rückfrage ab und bleibt in den Einstellungen', async () => {
    const router = await routerAt('/settings')
    const remove = installConfirmGuard(router)

    const answer = confirm({ title: 'Entfernen?', actions: [{ id: 'go', label: 'Los' }] })
    // So kommt „Zurück" an: die WebView geht in der Verlaufsliste zurück.
    await router.push('/')

    await expect(answer).resolves.toBe(CANCEL)
    expect(router.currentRoute.value.path).toBe('/settings')
    remove()
  })

  it('lässt die Navigation durch, wenn nichts offen ist', async () => {
    const router = await routerAt('/settings')
    const remove = installConfirmGuard(router)
    await router.push('/')
    expect(router.currentRoute.value.path).toBe('/')
    remove()
  })
})
