import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { mount } from '@vue/test-utils'
import SystemPane from './SystemPane.vue'
import { i18n } from '@/lib/i18n'
import * as api from '@/lib/api'
import * as saf from '@/lib/saf'
import { useConfigStore } from '@/stores/config'
import type { AppConfig } from '@/lib/types'

/**
 * Sicherung exportieren und einspielen (FA-45).
 *
 * Geprueft wird die Weiche, nicht der Systemdialog: auf Android geht beides
 * ueber eine Datei, auf dem Schreibtisch ueber die Zwischenablage. Der Grund
 * fuer die Weiche steckt in einem Fehler, der lange unbemerkt blieb — der
 * Export ueber `navigator.clipboard` gelang, der Import scheiterte immer an
 * "Read permission denied", weil Androids WebView `clipboard-read` nicht
 * gewaehren kann. Eine Sicherung, die man nie zurueckspielen kann, meldet
 * trotzdem Erfolg. Deshalb liegt hier der Test.
 */

beforeAll(() => {
  i18n.global.locale.value = 'de'
})

const CONFIG = {
  language: 'de',
  cache: { maxBytes: 2_000_000_000, quality: 'standard' },
  remote: { enabled: false, port: 8090 },
  mqtt: { enabled: false, host: '', port: 1883, username: '', baseTopic: 'slowshow' },
  sources: [],
} as unknown as AppConfig

function setup() {
  setActivePinia(createPinia())
  const store = useConfigStore()
  store.config = CONFIG
  return mount(SystemPane, { global: { plugins: [i18n] } })
}

beforeEach(() => {
  vi.restoreAllMocks()
  vi.spyOn(api, 'storageBreakdown').mockResolvedValue({ byYear: [], bySender: [] })
  vi.spyOn(api, 'hasMqttPassword').mockResolvedValue(false)
  vi.spyOn(api, 'mqttStatus').mockResolvedValue({ connected: false, lastError: null } as never)
  vi.spyOn(api, 'appVersion').mockResolvedValue('1.0.0')
})

/**
 * Die angezeigte Fassung.
 *
 * Hier stand eine feste Zeichenkette `0.1.0`, waehrend `package.json`,
 * `Cargo.toml` und `tauri.conf.json` laengst auf 1.0.0 standen. Wer eine
 * Fehlermeldung schickt, liest die Zahl aus dieser Zeile ab — eine falsche
 * schickt die Fehlersuche in die Irre, und auffallen kann es niemandem, weil
 * nichts sie mit dem Bau verbindet.
 */
describe('Fassung', () => {
  it('zeigt die Fassung des laufenden Baus', async () => {
    vi.spyOn(api, 'appVersion').mockResolvedValue('2.3.4')

    const w = setup()
    await vi.waitFor(() => expect(w.text()).toContain('Version 2.3.4'))
  })

  it('schreibt keine Fassung hin, solange keine bekannt ist', async () => {
    // Lieber gar nichts als eine Platzhalterzahl: eine angezeigte Version, die
    // von nichts abhaengt, ist genau der Fehler von vorher.
    vi.spyOn(api, 'appVersion').mockRejectedValue(new Error('Brücke weg'))

    const w = setup()
    await vi.waitFor(() => expect(w.text()).not.toContain('Version'))
  })
})

/** Findet einen Knopf ueber seine Beschriftung. */
function button(wrapper: ReturnType<typeof setup>, label: string) {
  const found = wrapper.findAll('button').find((b) => b.text() === label)
  if (!found) throw new Error(`Knopf "${label}" nicht gefunden`)
  return found
}

describe('Sicherung auf Android', () => {
  beforeEach(() => {
    vi.spyOn(saf, 'isAvailable').mockResolvedValue(true)
  })

  it('schreibt den Export in eine Datei statt in die Zwischenablage', async () => {
    const save = vi.spyOn(saf, 'saveTextFile').mockResolvedValue('sicherung.json')
    vi.spyOn(api, 'exportConfig').mockResolvedValue('{"schemaVersion":1}')
    const clipboard = vi.fn()
    vi.stubGlobal('navigator', { clipboard: { writeText: clipboard } })

    const w = setup()
    await button(w, 'Exportieren').trigger('click')
    await vi.waitFor(() => expect(save).toHaveBeenCalled())

    expect(save.mock.calls[0][1]).toBe('{"schemaVersion":1}')
    expect(clipboard).not.toHaveBeenCalled()
  })

  it('nennt den geschriebenen Dateinamen', async () => {
    // Ohne Namen weiss niemand, wo die Sicherung gelandet ist -- der
    // Systemdialog laesst den Ordner frei waehlen.
    vi.spyOn(saf, 'saveTextFile').mockResolvedValue('slowshow-sicherung-2026-08-31.json')
    vi.spyOn(api, 'exportConfig').mockResolvedValue('{}')

    const w = setup()
    await button(w, 'Exportieren').trigger('click')
    await vi.waitFor(() => expect(w.text()).toContain('slowshow-sicherung-2026-08-31.json'))
  })

  it('liest den Import aus einer Datei', async () => {
    const importConfig = vi.spyOn(api, 'importConfig').mockResolvedValue(CONFIG)
    vi.spyOn(saf, 'openTextFile').mockResolvedValue({ content: '{"a":1}', name: 'x.json' })

    const w = setup()
    await button(w, 'Importieren').trigger('click')
    await vi.waitFor(() => expect(importConfig).toHaveBeenCalledWith('{"a":1}'))
  })

  it('meldet nichts, wenn der Dialog abgebrochen wird', async () => {
    // Abbrechen ist kein Fehler. Eine Meldung waere hier schlimmer als keine:
    // sie liest sich wie ein Scheitern, obwohl der Nutzer es so wollte.
    const importConfig = vi.spyOn(api, 'importConfig')
    vi.spyOn(saf, 'openTextFile').mockResolvedValue(null)

    const w = setup()
    await button(w, 'Importieren').trigger('click')
    await new Promise((r) => setTimeout(r, 0))

    expect(importConfig).not.toHaveBeenCalled()
    expect(w.find('.notice').exists()).toBe(false)
  })

  it('meldet einen fehlgeschlagenen Export als Export, nicht als Import', async () => {
    // Stand vorher falsch da: schlug der Export fehl, erschien "Import
    // fehlgeschlagen". Wer das liest, sucht an der falschen Stelle.
    vi.spyOn(api, 'exportConfig').mockRejectedValue(new Error('kein Platz'))

    const w = setup()
    await button(w, 'Exportieren').trigger('click')
    await vi.waitFor(() => expect(w.text()).toContain('Export fehlgeschlagen'))
    expect(w.text()).toContain('kein Platz')
  })
})

describe('Sicherung auf dem Schreibtisch', () => {
  beforeEach(() => {
    vi.spyOn(saf, 'isAvailable').mockResolvedValue(false)
  })

  it('faellt auf die Zwischenablage zurueck', async () => {
    // Der Schreibtisch-Build ist Nebenprodukt (Lastenheft 1.3), aber dort
    // funktioniert die Zwischenablage vollstaendig -- lesen wie schreiben.
    const writeText = vi.fn(async () => {})
    vi.stubGlobal('navigator', { clipboard: { writeText } })
    vi.spyOn(api, 'exportConfig').mockResolvedValue('{"b":2}')

    const w = setup()
    await button(w, 'Exportieren').trigger('click')
    await vi.waitFor(() => expect(writeText).toHaveBeenCalledWith('{"b":2}'))
  })

  it('liest den Import aus der Zwischenablage', async () => {
    vi.stubGlobal('navigator', { clipboard: { readText: async () => '{"c":3}' } })
    const importConfig = vi.spyOn(api, 'importConfig').mockResolvedValue(CONFIG)

    const w = setup()
    await button(w, 'Importieren').trigger('click')
    await vi.waitFor(() => expect(importConfig).toHaveBeenCalledWith('{"c":3}'))
  })
})
