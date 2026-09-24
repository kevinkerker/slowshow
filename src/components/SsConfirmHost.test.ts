import { afterEach, beforeAll, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import SsConfirmHost from './SsConfirmHost.vue'
import { CANCEL, confirm, confirmAction, settle } from '@/composables/useConfirm'
import { i18n } from '@/lib/i18n'

/**
 * Der Rückfragedialog (E-59, Vorlage `Vorschlag-Rueckfrage`).
 *
 * Zwei Ausgänge stehen in einer Reihe, „Abbrechen" links; drei untereinander,
 * „Abbrechen" zuletzt. Die Reihenfolge ist Absicht: bei der Reihe landet der
 * Daumen rechts auf der Aktion, bei der Liste liest man von oben nach unten
 * und endet beim harmlosen Ausgang.
 */

beforeAll(() => {
  i18n.global.locale.value = 'de'
})

afterEach(() => settle(CANCEL))

const tick = () => new Promise((r) => setTimeout(r, 0))

function host() {
  return mount(SsConfirmHost, { global: { plugins: [i18n] }, attachTo: document.body })
}

describe('SsConfirmHost', () => {
  it('zeigt nichts, solange niemand fragt', () => {
    expect(host().find('.panel').exists()).toBe(false)
  })

  it('zeigt Titel und Text der Frage', async () => {
    const w = host()
    void confirmAction('Quelle „NAS" entfernen?', 'Die Bilder im Cache werden gelöscht.', 'Quelle entfernen')
    await tick()

    expect(w.get('.title').text()).toBe('Quelle „NAS" entfernen?')
    expect(w.get('.body').text()).toContain('Cache werden gelöscht')
    expect(w.get('.panel').attributes('role')).toBe('alertdialog')
    w.unmount()
  })

  it('stellt bei zwei Ausgängen Abbrechen links neben die Aktion', async () => {
    const w = host()
    void confirmAction('Frage', undefined, 'Quelle entfernen')
    await tick()

    const knoepfe = w.findAll('.actions button')
    expect(knoepfe.map((b) => b.text())).toEqual(['Abbrechen', 'Quelle entfernen'])
    expect(knoepfe[0].classes()).toContain('ss-btn--ghost')
    expect(knoepfe[1].classes()).toContain('ss-btn--danger')
    expect(w.get('.actions').classes()).not.toContain('stacked')
    w.unmount()
  })

  it('stapelt drei Ausgänge und setzt Abbrechen ans Ende', async () => {
    const w = host()
    void confirm({
      title: 'oma@example.org entfernen?',
      actions: [
        { id: 'keep', label: 'Entfernen · Fotos bleiben sichtbar', variant: 'secondary' },
        { id: 'back', label: 'Entfernen · 148 Fotos zurück in die Quarantäne', variant: 'danger' },
      ],
    })
    await tick()

    expect(w.get('.actions').classes()).toContain('stacked')
    expect(w.findAll('.actions button').map((b) => b.text())).toEqual([
      'Entfernen · Fotos bleiben sichtbar',
      'Entfernen · 148 Fotos zurück in die Quarantäne',
      'Abbrechen',
    ])
    w.unmount()
  })

  it('liefert die angetippte Aktion und schließt', async () => {
    const w = host()
    const answer = confirmAction('Frage', undefined, 'Los')
    await tick()
    await w.findAll('.actions button')[1].trigger('click')

    await expect(answer).resolves.toBe(true)
    await tick()
    expect(w.find('.panel').exists()).toBe(false)
    w.unmount()
  })

  it('zählt einen Tipp auf den Hintergrund als Abbrechen', async () => {
    const w = host()
    const answer = confirmAction('Frage', undefined, 'Los')
    await tick()
    await w.get('.backdrop').trigger('click')
    await expect(answer).resolves.toBe(false)
    w.unmount()
  })

  it('zählt einen Tipp in die Tafel nicht als Abbrechen', async () => {
    const w = host()
    const answer = confirmAction('Frage', 'Text', 'Los')
    await tick()
    await w.get('.body').trigger('click')
    await tick()
    expect(w.find('.panel').exists(), 'Dialog blieb nicht offen').toBe(true)
    settle('confirm')
    await expect(answer).resolves.toBe(true)
    w.unmount()
  })

  it('bricht mit Escape ab', async () => {
    const w = host()
    const answer = confirmAction('Frage', undefined, 'Los')
    await tick()
    await w.get('.panel').trigger('keydown', { key: 'Escape' })
    await expect(answer).resolves.toBe(false)
    w.unmount()
  })
})
