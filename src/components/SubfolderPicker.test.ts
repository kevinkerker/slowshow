import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import SubfolderPicker from './SubfolderPicker.vue'
import { i18n } from '@/lib/i18n'
import * as api from '@/lib/api'
import { allSelected, type FolderSelection } from '@/lib/subfolders'
import type { Source, SubfolderEntry } from '@/lib/types'

/**
 * Auswahl der Unterordner (E-70). Geprüft wird, was die Liste anzeigt und
 * was sie zurückgibt; die Regel selbst prüft `subfolders.test.ts`.
 */

beforeAll(() => {
  i18n.global.locale.value = 'de'
})

const SOURCE: Source = {
  id: 'q1',
  name: 'WhatsApp Images',
  kind: { type: 'local', safUri: 'content://wa', displayPath: 'WhatsApp Images' },
  enabled: true,
  subfolders: [],
  excludedSubfolders: [],
  includeRoot: true,
  includeNewSubfolders: true,
  minWidth: 0,
  minHeight: 0,
  syncIntervalMinutes: 360,
  lastSync: null,
}

const WHATSAPP: SubfolderEntry[] = [
  { name: 'Private', path: 'Private', nomedia: true, hasChildren: false },
  { name: 'Sent', path: 'Sent', nomedia: true, hasChildren: false },
  { name: 'Urlaub', path: 'Urlaub', nomedia: false, hasChildren: true },
]

beforeEach(() => {
  vi.restoreAllMocks()
})

function picker(opts: { selection?: FolderSelection; hideNomedia?: boolean; source?: Source | null; autoLoad?: boolean } = {}) {
  let current = opts.selection ?? allSelected()
  const w = mount(SubfolderPicker, {
    props: {
      source: opts.source === undefined ? SOURCE : opts.source,
      password: '',
      rootName: 'WhatsApp Images',
      hideNomedia: opts.hideNomedia ?? false,
      autoLoad: opts.autoLoad ?? true,
      modelValue: current,
      'onUpdate:modelValue': (v: FolderSelection) => {
        current = v
        void w.setProps({ modelValue: v })
      },
    },
    global: { plugins: [i18n] },
  })
  return { w, selection: () => current }
}

const toggle = (w: ReturnType<typeof mount>, label: string) => w.get(`[role="switch"][aria-label="${label}"]`)

describe('SubfolderPicker', () => {
  it('liest die erste Ebene und zeigt .nomedia an', async () => {
    const list = vi.spyOn(api, 'listSubfolders').mockResolvedValue(WHATSAPP)
    const { w } = picker()
    await flushPromises()

    expect(list).toHaveBeenCalledWith(SOURCE, '', '')
    expect(w.text()).toContain('Bilder direkt in „WhatsApp Images“')
    expect(w.text()).toContain('Urlaub')
    expect(w.findAll('.badge').map((b) => b.text())).toEqual(['versteckt (.nomedia)', 'versteckt (.nomedia)'])
  })

  it('schaltet .nomedia-Ordner eines frisch gewählten Ordners vorab ab (E-70)', async () => {
    vi.spyOn(api, 'listSubfolders').mockResolvedValue(WHATSAPP)
    const { w, selection } = picker({ hideNomedia: true })
    await flushPromises()

    expect(selection().exclude.sort()).toEqual(['Private', 'Sent'])
    expect(toggle(w, 'Sent').attributes('aria-checked')).toBe('false')
    expect(toggle(w, 'Urlaub').attributes('aria-checked')).toBe('true')
  })

  it('lässt bestehende Quellen beim Öffnen unverändert', async () => {
    vi.spyOn(api, 'listSubfolders').mockResolvedValue(WHATSAPP)
    const { selection } = picker({ hideNomedia: false })
    await flushPromises()
    expect(selection()).toEqual(allSelected())
  })

  it('gibt einen umgelegten Schalter als Auswahl zurück', async () => {
    vi.spyOn(api, 'listSubfolders').mockResolvedValue(WHATSAPP)
    const { w, selection } = picker()
    await flushPromises()

    await toggle(w, 'Sent').trigger('click')
    expect(selection().exclude).toEqual(['Sent'])

    await toggle(w, 'Bilder direkt in „WhatsApp Images“').trigger('click')
    expect(selection().includeRoot).toBe(false)
  })

  it('lädt eine tiefere Ebene erst beim Aufklappen', async () => {
    const list = vi.spyOn(api, 'listSubfolders').mockImplementation(async (_s, _p, path) =>
      path === 'Urlaub' ? [{ name: '2025', path: 'Urlaub/2025', nomedia: false, hasChildren: false }] : WHATSAPP,
    )
    const { w } = picker()
    await flushPromises()
    expect(list).toHaveBeenCalledTimes(1)
    expect(w.text()).not.toContain('2025')

    await w.get('[aria-label="„Urlaub“ aufklappen"]').trigger('click')
    await flushPromises()
    expect(list).toHaveBeenLastCalledWith(SOURCE, '', 'Urlaub')
    expect(w.text()).toContain('2025')

    await w.get('[aria-label="„Urlaub“ zuklappen"]').trigger('click')
    expect(w.text()).not.toContain('2025')
  })

  it('meldet einen Lesefehler und bietet den nächsten Versuch an', async () => {
    const list = vi.spyOn(api, 'listSubfolders').mockRejectedValueOnce(new Error('Server antwortete mit 401'))
    const { w } = picker()
    await flushPromises()
    expect(w.text()).toContain('Unterordner nicht lesbar: Server antwortete mit 401')

    list.mockResolvedValue(WHATSAPP)
    const retry = w.findAll('button').find((b) => b.text() === 'Erneut versuchen')!
    await retry.trigger('click')
    await flushPromises()
    expect(w.text()).toContain('Urlaub')
  })

  it('zeigt Entscheidungen für verschwundene Ordner und lässt sie vergessen', async () => {
    // Etwa ein vertippter Eintrag aus dem alten Textfeld.
    vi.spyOn(api, 'listSubfolders').mockResolvedValue(WHATSAPP)
    const { w, selection } = picker({ selection: { include: ['Urlaub', 'Urlab'], exclude: [], includeRoot: false, includeNew: false } })
    await flushPromises()

    expect(w.get('.stale').text()).toContain('Urlab')
    expect(w.get('.stale').text()).toContain('nicht gefunden')
    await w.get('[aria-label="„Urlab“ vergessen"]').trigger('click')
    expect(selection().include).toEqual(['Urlaub'])
  })

  it('wartet beim NAS auf den Knopf, statt beim Tippen zu lesen', async () => {
    const list = vi.spyOn(api, 'listSubfolders').mockResolvedValue([])
    const { w } = picker({ autoLoad: false })
    await flushPromises()
    expect(list).not.toHaveBeenCalled()

    await w.findAll('button').find((b) => b.text() === 'Unterordner anzeigen')!.trigger('click')
    await flushPromises()
    expect(list).toHaveBeenCalledTimes(1)
    expect(w.text()).toContain('Keine Unterordner.')
  })

  it('sagt, was fehlt, solange es keinen Ordner gibt', () => {
    const { w } = picker({ source: null })
    expect(w.text()).toContain('Zuerst einen Ordner wählen.')
  })
})
