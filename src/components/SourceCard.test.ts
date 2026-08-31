import { beforeAll, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import SourceCard from './SourceCard.vue'
import { i18n } from '@/lib/i18n'
import type { Source, SourceKind } from '@/lib/types'

/**
 * Wie sich eine Quelle auf ihrer Karte vorstellt.
 *
 * Anlass: das Postfach gab sich am Gerät als „NAS · WebDAV" aus und hatte gar
 * kein Symbol. Beides derselbe Fehler wie im Formular — ein `else`
 * beziehungsweise ein `default`, das einen Fall mitnahm, der nicht
 * hineingehörte. Dass die Quellenart stimmt, ist keine Kleinigkeit: sie ist
 * die einzige Stelle, an der in der Liste steht, womit man es zu tun hat.
 */

beforeAll(() => {
  i18n.global.locale.value = 'de'
})

const KINDS: Record<string, SourceKind> = {
  mail: {
    type: 'mail',
    host: 'imap.example.org',
    port: 993,
    username: 'wer@example.org',
    passwordRef: 'q1',
    folder: 'INBOX',
    allowedSenders: [],
    includeSeen: false,
    quarantineAll: false,
    maxAttachmentBytes: 26214400,
    maxMailsPerHour: 30,
    quality: 'standard',
  },
  local: { type: 'local', safUri: 'content://x', displayPath: '/Bilder' },
  webDav: {
    type: 'webDav',
    url: 'https://nas.local/dav',
    username: 'kevin',
    passwordRef: 'q1',
    allowInsecureTls: false,
  },
  nextcloud: {
    type: 'nextcloud',
    url: 'https://cloud.local',
    username: 'kevin',
    passwordRef: 'q1',
    allowInsecureTls: false,
    album: 'Urlaub',
    usePreviewApi: true,
  },
}

function card(kind: keyof typeof KINDS, photoCount = 12, lastSync: number | null = null) {
  const source: Source = {
    id: 'q1',
    name: 'Testquelle',
    kind: KINDS[kind],
    enabled: true,
    subfolders: [],
    minWidth: 0,
    minHeight: 0,
    syncIntervalMinutes: 360,
    lastSync,
  }
  return mount(SourceCard, {
    props: { source, photoCount, syncing: false, progress: null },
    global: { plugins: [i18n] },
  })
}

describe('SourceCard', () => {
  it('nennt das Postfach ein Postfach', () => {
    const text = card('mail').text()
    expect(text).toContain('Postfach')
    expect(text).not.toContain('NAS')
    expect(text).not.toContain('WebDAV')
  })

  it('verwechselt die Quellenarten nicht untereinander', () => {
    expect(card('webDav').text()).toContain('NAS')
    expect(card('nextcloud').text()).toContain('Nextcloud')
    expect(card('local').text()).toContain('Lokaler Ordner')
    // Gegenprobe in die andere Richtung: kein Ordner gibt sich als Postfach aus.
    expect(card('local').text()).not.toContain('Postfach')
    expect(card('webDav').text()).not.toContain('Postfach')
  })

  it('gibt jeder Quellenart ein Symbol', () => {
    // Ein leeres `d` zeichnet nichts — am Geraet blieb ein dunkles Quadrat.
    for (const kind of ['mail', 'local', 'webDav', 'nextcloud'] as const) {
      const svg = card(kind).get('.icon svg')
      const paths = svg.findAll('path, rect')
      expect(paths.length, `kein Symbol bei ${kind}`).toBeGreaterThan(0)
      for (const p of paths) {
        // `rect` hat kein `d`; nur die Pfade pruefen.
        if (p.element.tagName.toLowerCase() !== 'path') continue
        expect(p.attributes('d'), `leerer Pfad bei ${kind}`).toBeTruthy()
      }
    }
  })

  it('setzt Ein- und Mehrzahl bei den Fotozaehlern', () => {
    // Am Geraet stand „1 Fotos im Cache", nachdem das erste Mail-Foto
    // angekommen war.
    expect(card('mail', 1).text()).toContain('1 Foto im Cache')
    expect(card('mail', 1).text()).not.toContain('1 Fotos')
    expect(card('mail', 5).text()).toContain('5 Fotos im Cache')
    expect(card('local', 1).text()).toContain('1 Foto')
    expect(card('local', 1).text()).not.toContain('1 Fotos')
  })

  it('zeigt beim Postfach den Stand des Abrufs', () => {
    // Ohne diese Angaben ist nicht zu erkennen, ob der Abruf ueberhaupt
    // laeuft — genau die Frage, die am Geraet aufkam.
    const text = card('mail', 7).text()
    expect(text).toContain('7')
    expect(text).toContain('synchronisiert')
  })

  it('sagt einmal, dass noch nie synchronisiert wurde', () => {
    // Am Tablet stand „zuletzt synchronisiert noch nie synchronisiert".
    // `formatRelativeTime(null)` liefert einen ganzen Satz, die uebrigen
    // Zweige nur eine Zeitangabe — beides wanderte in dieselbe Schablone.
    // Am Telefon blieb es verborgen, weil die Zeile dort abgeschnitten war;
    // ein Test darf sich darauf nicht verlassen.
    for (const kind of ['mail', 'webDav', 'nextcloud'] as const) {
      const text = card(kind).text()
      expect(text, kind).toContain('noch nie synchronisiert')
      expect(text.match(/synchronisiert/g)?.length, `doppelt bei ${kind}`).toBe(1)
    }
  })

  it('nennt den Zeitpunkt, sobald einmal synchronisiert wurde', () => {
    const vorhin = Math.floor(Date.now() / 1000) - 12 * 60
    const text = card('mail', 3, vorhin).text()
    expect(text).toContain('zuletzt synchronisiert')
    expect(text).toContain('12')
    expect(text).not.toContain('noch nie')
  })
})
