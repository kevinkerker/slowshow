import { beforeAll, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'
import SettingsView from './SettingsView.vue'
import { i18n } from '@/lib/i18n'

/**
 * Bereich beim Öffnen der Einstellungen.
 *
 * Anlass: der Hinweis „Fotos warten auf Freigabe" in der Diashow öffnete die
 * Einstellungen immer bei den Quellen. Die wartenden Fotos stehen aber im
 * Bild-Browser — wer dem Hinweis folgte, fand dort nichts. Jetzt lässt sich
 * der Bereich über die Adresse vorwählen.
 */

beforeAll(() => {
  i18n.global.locale.value = 'de'
})

// Die Bereiche selbst laden Daten über die Brücke; hier zählt nur, welcher
// gezeigt wird und womit.
const STUBS = {
  SourcesPane: { template: '<div class="stub-sources" />' },
  ImagesPane: { props: ['initialFilter'], template: '<div class="stub-images">{{ initialFilter }}</div>' },
  ShowPane: { template: '<div class="stub-show" />' },
  SchedulePane: { template: '<div class="stub-schedule" />' },
  SystemPane: { template: '<div class="stub-system" />' },
}

async function open(path: string) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/', component: { template: '<div />' } },
      { path: '/settings', component: SettingsView },
    ],
  })
  await router.push(path)
  return mount(SettingsView, { global: { plugins: [router, i18n], stubs: STUBS } })
}

describe('SettingsView', () => {
  it('öffnet ohne Angabe bei den Quellen', async () => {
    const w = await open('/settings')
    expect(w.find('.stub-sources').exists()).toBe(true)
    expect(w.get('.head .ss-label').text()).toBe('Quellen')
  })

  it('öffnet den Bild-Browser in der Quarantäne, wenn danach gefragt wird', async () => {
    const w = await open('/settings?pane=images&filter=quarantine')
    expect(w.get('.stub-images').text()).toBe('quarantine')
    expect(w.get('.nav-item.active').text()).toBe('Bilder')
  })

  it('bleibt bei einem unbekannten Bereich bei den Quellen', async () => {
    const w = await open('/settings?pane=gibtsnicht')
    expect(w.find('.stub-sources').exists()).toBe(true)
  })
})
