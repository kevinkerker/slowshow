import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import SourceDialog from './SourceDialog.vue'
import { CANCEL, confirmRequest, settle } from '@/composables/useConfirm'
import { i18n } from '@/lib/i18n'
import * as api from '@/lib/api'
import type { FetchLogEntry, Source, SourceKind } from '@/lib/types'

/**
 * Welche Felder eine Quellenart zeigt.
 *
 * Anlass: das Formular hängte den WebDAV-Block an ein `v-else`, das sich nur
 * auf den lokalen Ordner bezog. Beim Postfach war diese Bedingung falsch —
 * also rendete der Sonst-Zweig mit, und über den Postfach-Feldern lagen
 * plötzlich „Adresse", ein zweites „Benutzername" und ein zweites „Passwort",
 * alle drei an dieselben Variablen gebunden. Der Typprüfer sieht so etwas
 * nicht, und am Gerät fiel es erst beim Durchscrollen auf.
 */

// Die Oberflaechensprache richtet sich sonst nach dem Rechner, auf dem der
// Test laeuft — und dann pruefte er auf einem englischen System nichts mehr.
beforeAll(() => {
  i18n.global.locale.value = 'de'
})

beforeEach(() => {
  // Vorgabe fuer alle Tests: der Dialog laedt beim Bearbeiten eines Postfachs
  // die Freigabeliste. Ohne diese Grundeinstellung liefe jeder andere Test in
  // den abgefangenen Fehler der Test-Bruecke — sichtbar als Rauschen im
  // Protokoll, das echte Fehler zudeckt.
  vi.spyOn(api, 'allowedSenders').mockResolvedValue([])
  vi.spyOn(api, 'lastFetch').mockResolvedValue(null)
  vi.spyOn(api, 'fetchLog').mockResolvedValue([])
  vi.spyOn(api, 'onResyncProgress').mockResolvedValue(() => {})
})

// Eine Rueckfrage, die ein Test offen laesst, bliebe sonst fuer den naechsten
// stehen — die Rueckfragen teilen sich einen Zustand (E-59).
afterEach(() => settle(CANCEL))

const flush = () => new Promise((r) => setTimeout(r, 0))

/**
 * Beantwortet die offene Rueckfrage (E-59) und gibt sie zurueck.
 *
 * Der Dialog selbst haengt in `App.vue`; hier zaehlt, was gefragt wird und
 * was die Antwort bewirkt.
 */
async function answer(id: string) {
  await flush()
  const request = confirmRequest.value
  expect(request, 'keine Rueckfrage offen').toBeTruthy()
  settle(id)
  await flush()
  return request!
}

/** Schaltflaeche nach ihrer Aufschrift — Klassen sind seit E-58 Sache von SsButton. */
function button(w: { findAll: (s: string) => Array<{ text: () => string }> }, text: string) {
  const found = w.findAll('button').find((b) => b.text().includes(text))
  expect(found, `Schaltflaeche „${text}" nicht gefunden`).toBeTruthy()
  return found as unknown as { trigger: (e: string) => Promise<void> }
}

const BASE: Omit<Source, 'kind'> = {
  id: 'q1',
  name: 'Testquelle',
  enabled: true,
  subfolders: [],
  minWidth: 0,
  minHeight: 0,
  syncIntervalMinutes: 360,
  lastSync: null,
}

const KINDS = {
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
    maxAttachmentBytes: 25 * 1024 * 1024,
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
} satisfies Record<string, SourceKind>

function form(kind: keyof typeof KINDS) {
  const w = mount(SourceDialog, {
    props: { source: { ...BASE, kind: KINDS[kind] } },
    global: { plugins: [i18n] },
  })
  return w
}

/** Alle sichtbaren Beschriftungen des Formulars. */
function labels(w: ReturnType<typeof form>): string[] {
  return w.findAll('.row-label, label, .label').map((el) => el.text())
}

describe('SourceDialog — Felder je Quellenart', () => {
  it('zeigt beim Postfach keine WebDAV-Adresse', () => {
    const text = form('mail').text()
    expect(text).toContain('IMAP-Server')
    expect(text).not.toContain('remote.php')
  })

  it('bindet Benutzername und Passwort beim Postfach nur einmal', () => {
    // Zwei Eingabefelder auf derselben Variablen sind kein Datenverlust, aber
    // niemand weiss dann, welches gilt.
    const w = form('mail')
    expect(w.findAll('input[type="password"]')).toHaveLength(1)
    expect(w.findAll('input[autocomplete="username"]')).toHaveLength(1)
  })

  it('zeigt beim Postfach keine Unterordner-Auswahl', () => {
    // Der Abruf setzt `subfolders` fest auf leer (mail/sync.rs) — ein Feld,
    // das nichts bewirkt, ist schlimmer als keines.
    expect(form('mail').text()).not.toContain('Nur diese Unterordner')
    expect(form('webDav').text()).toContain('Nur diese Unterordner')
  })

  it('bietet den Abrufabstand auch beim Postfach an', () => {
    // Beim Verschieben aus dem WebDAV-Zweig heraus leicht zu verlieren: ohne
    // ihn liefe das Postfach nur beim Start der App.
    expect(form('mail').text()).toContain('Synchronisieren alle')
    expect(form('webDav').text()).toContain('Synchronisieren alle')
  })

  it('laesst den lokalen Ordner ohne Zugangsdaten', () => {
    const text = form('local').text()
    expect(text).not.toContain('IMAP-Server')
    expect(text).not.toContain('Passwort')
  })

  it('beschriftet jedes Feld — kein roher Schluessel', () => {
    // `sourceForm.user` stand so tatsaechlich am Geraet.
    for (const kind of ['mail', 'local', 'webDav'] as const) {
      const all = labels(form(kind))
      // Ohne diese Zeile waere der Test gruen, sobald sich der Klassenname
      // von SettingRow aendert und `labels()` nichts mehr findet.
      expect(all.length, `keine Beschriftungen gefunden bei ${kind}`).toBeGreaterThan(2)
      const roh = all.filter((l) => /^[a-z]+[A-Za-z]*\.[a-zA-Z.]+$/.test(l))
      expect(roh, `rohe Schluessel bei ${kind}`).toEqual([])
    }
  })

  it('meldet das Ergebnis des Verbindungstests in der Fusszeile', async () => {
    // Die Schaltflaeche sitzt in der festen Fusszeile, die Meldung stand
    // vorher am Ende des scrollbaren Rumpfs — wer beim Passwortfeld tippte,
    // sah nichts geschehen (E-31).
    vi.spyOn(api, 'testSource').mockResolvedValue(0)
    const w = form('mail')
    await button(w, 'Verbindung testen').trigger('click')
    await flush()
    await w.vm.$nextTick()

    const foot = w.get('.foot')
    expect(foot.find('.result .ok').exists(), 'Meldung gehoert in die Fusszeile').toBe(true)
    expect(w.find('.body .result').exists(), 'und nicht mehr in den Rumpf').toBe(false)
  })

  it('nennt beim Postfach die Zahl der ungelesenen Nachrichten', async () => {
    // Die Zahl belegt, dass auch der Ordner stimmt, nicht nur die Anmeldung.
    // Sie ging vorher nur ins Protokoll (commands.rs).
    vi.spyOn(api, 'testSource').mockResolvedValue(3)
    const w = form('mail')
    await button(w, 'Verbindung testen').trigger('click')
    await flush()
    await w.vm.$nextTick()

    expect(w.get('.foot .result').text()).toContain('3')
  })

  it('bleibt bei anderen Quellen bei der schlichten Meldung', async () => {
    // WebDAV liefert keine Zahl (`null`) — dann darf dort auch keine stehen.
    vi.spyOn(api, 'testSource').mockResolvedValue(null)
    const w = form('webDav')
    await button(w, 'Verbindung testen').trigger('click')
    await flush()
    await w.vm.$nextTick()

    const text = w.get('.foot .result').text()
    expect(text).toBe('Verbindung erfolgreich')
  })

  it('laesst einen fehlgeschlagenen Test stehen und faerbt ihn als Fehler', async () => {
    // E-62: Erfolg verblasst, ein Fehler bleibt, bis ein Versuch gelingt.
    vi.spyOn(api, 'testSource').mockRejectedValue(new Error('Zeitueberschreitung'))
    const w = form('webDav')
    await button(w, 'Verbindung testen').trigger('click')
    await flush()
    await w.vm.$nextTick()

    const meldung = w.get('.foot .result .error')
    expect(meldung.text()).toContain('Zeitueberschreitung')
    expect(meldung.attributes('role')).toBe('alert')
  })

  // ── Freigegebene Absender (F4, E-32) ──────────────────────────────────────

  /// Mountet den Dialog und wartet, bis die Absenderliste geladen ist.
  async function withSenders(list: api.AllowedSender[]) {
    vi.spyOn(api, 'allowedSenders').mockResolvedValue(list)
    const w = form('mail')
    await new Promise((r) => setTimeout(r, 0))
    await w.vm.$nextTick()
    return w
  }

  it('listet die freigegebenen Absender mit ihrer Fotozahl', async () => {
    const w = await withSenders([
      { address: 'oma@example.org', photoCount: 12 },
      { address: 'opa@example.org', photoCount: 1 },
    ])
    const zeilen = w.findAll('.senders li')
    expect(zeilen).toHaveLength(2)
    expect(zeilen[0].text()).toContain('oma@example.org')
    expect(zeilen[0].text()).toContain('12 Fotos')
    // Einzahl bei genau einem Foto -- sonst stuende dort "1 Fotos".
    expect(zeilen[1].text()).toContain('1 Foto')
    expect(zeilen[1].text()).not.toContain('1 Fotos')
  })

  it('sagt es, wenn noch niemand freigegeben ist', async () => {
    const w = await withSenders([])
    expect(w.find('.senders').exists()).toBe(false)
    expect(w.get('.senders-empty').text()).toContain('Noch niemand')
  })

  it('fragt vor dem Entfernen nach den vorhandenen Fotos', async () => {
    // Der Kern von E-32, seit E-59 mit drei benannten Ausgaengen: jede
    // Aufschrift sagt, was mit den Fotos geschieht.
    const remove = vi.spyOn(api, 'removeAllowedSender').mockResolvedValue(12)

    const w = await withSenders([{ address: 'oma@example.org', photoCount: 12 }])
    await w.get('.sender-remove').trigger('click')
    const frage = await answer('requarantine')

    expect(frage.title).toContain('oma@example.org')
    expect(frage.body).toContain('12 Fotos')
    expect(frage.actions.map((a) => a.label)).toEqual([
      'Entfernen · Fotos bleiben sichtbar',
      'Entfernen · 12 Fotos zurück in die Quarantäne',
    ])
    expect(remove).toHaveBeenCalledWith('q1', 'oma@example.org', true)
    expect(w.findAll('.senders li')).toHaveLength(0)
  })

  it('laesst die Fotos sichtbar, wenn man das so waehlt', async () => {
    const remove = vi.spyOn(api, 'removeAllowedSender').mockResolvedValue(0)

    const w = await withSenders([{ address: 'oma@example.org', photoCount: 12 }])
    await w.get('.sender-remove').trigger('click')
    await answer('keep')

    // Der Absender geht von der Liste, nur die Bilder bleiben.
    expect(remove).toHaveBeenCalledWith('q1', 'oma@example.org', false)
  })

  it('bricht wirklich ab, wenn man abbricht', async () => {
    // Vorher hiess „Abbrechen" im nativen Dialog „sichtbar lassen", und der
    // Absender verschwand trotzdem — ein Abbrechen gab es nicht (E-59).
    const remove = vi.spyOn(api, 'removeAllowedSender').mockResolvedValue(0)

    const w = await withSenders([{ address: 'oma@example.org', photoCount: 12 }])
    await w.get('.sender-remove').trigger('click')
    await answer(CANCEL)

    expect(remove).not.toHaveBeenCalled()
    expect(w.findAll('.senders li')).toHaveLength(1)
  })

  it('bricht bei einem Absender ohne Fotos wirklich ab', async () => {
    // Ohne Fotos gibt es nichts zu entscheiden — dann gibt es nur Entfernen
    // oder Abbrechen, und Abbrechen darf nichts entfernen.
    const remove = vi.spyOn(api, 'removeAllowedSender').mockResolvedValue(0)

    const w = await withSenders([{ address: 'neu@example.org', photoCount: 0 }])
    await w.get('.sender-remove').trigger('click')
    const frage = await answer(CANCEL)

    expect(frage.actions).toHaveLength(1)
    expect(remove).not.toHaveBeenCalled()
    expect(w.findAll('.senders li')).toHaveLength(1)
  })

  it('zeigt die Liste nicht beim Anlegen einer neuen Quelle', async () => {
    // Eine neue Quelle hat noch keine Id — ein Ladeversuch liefe ins Leere.
    const load = vi.spyOn(api, 'allowedSenders').mockResolvedValue([])
    const w = mount(SourceDialog, {
      props: { source: null },
      global: { plugins: [i18n] },
    })
    await w.vm.$nextTick()
    expect(load).not.toHaveBeenCalled()
    expect(w.find('.senders').exists()).toBe(false)
    expect(w.find('.senders-empty').exists()).toBe(false)
  })

  it('laesst die Tastatur keine Zugangsdaten grossschreiben', () => {
    // Am Xiaomi Pad aufgefallen: Gboard machte aus der Adresse
    // „Vorname.Nachname@example.org". Ein Postfach steht am Satzanfang, also
    // grossgeschrieben — bei einem Server mit unterscheidender
    // Gross-/Kleinschreibung waere die Anmeldung gescheitert, und der Grund
    // stuende nirgends. Die WebDAV-Felder schuetzten sich seit jeher davor,
    // die Postfach-Felder nicht.
    const w = form('mail')
    const felder = [
      'input[inputmode="email"]', // Benutzername
      'input[inputmode="url"]', // IMAP-Server
    ]
    for (const sel of felder) {
      const el = w.get(sel)
      expect(el.attributes('autocapitalize'), sel).toBe('off')
      expect(el.attributes('spellcheck'), sel).toBe('false')
    }

    // Auch der Ordnername: „INBOX" ist ein Bezeichner, keine Prosa.
    const alle = w.findAll('input[type="text"]')
    const ordner = alle.find((i) => i.attributes('placeholder') === 'INBOX')
    expect(ordner, 'Ordnerfeld nicht gefunden').toBeTruthy()
    expect(ordner!.attributes('autocapitalize')).toBe('off')
  })

  it('schuetzt auch die WebDAV-Zugangsdaten weiterhin', () => {
    // Gegenprobe, damit die vorhandene Absicherung nicht bei einem Umbau
    // verlorengeht.
    const w = form('webDav')
    expect(w.get('input[type="url"]').attributes('autocapitalize')).toBe('off')
    // `find`, nicht `get`: `get` wirft bei einem Fehltreffer und hat deshalb
    // gar kein `exists`.
    expect(w.find('input[autocomplete="off"][spellcheck="false"]').exists()).toBe(true)
  })

  it('bietet den Schalter fuer gelesene Nachrichten nur beim Postfach', () => {
    expect(form('mail').text()).toContain('Auch gelesene Nachrichten')
    expect(form('webDav').text()).not.toContain('Auch gelesene')
    expect(form('local').text()).not.toContain('Auch gelesene')
  })

  it('warnt beim Schalter vor der Wirkung auf das Postfach', () => {
    // Eingeschaltet sieht der Rahmen den ganzen Ordner durch und markiert
    // alles als gelesen — auch Post, die noch niemand gelesen hat. Wer das
    // auf seine INBOX loslaesst, soll es vorher wissen (E-34).
    const text = form('mail').text()
    expect(text).toContain('gelesen')
    expect(text).toMatch(/eigenen Ordner|INBOX/)
  })

  it('uebernimmt den Schalter aus der gespeicherten Quelle', async () => {
    const w = mount(SourceDialog, {
      props: {
        source: {
          ...BASE,
          kind: { ...KINDS.mail, type: 'mail' as const, includeSeen: true },
        },
      },
      global: { plugins: [i18n] },
    })
    await w.vm.$nextTick()
    // Der Schalter darf beim Bearbeiten nicht stillschweigend zurueckfallen —
    // sonst schaltet ein Umbenennen der Quelle den Abruf wieder um.
    const schalter = w.findAllComponents({ name: 'ToggleSwitch' })
    const werte = schalter.map((c) => c.props('modelValue'))
    expect(werte).toContain(true)
  })

  // ── Abrufstand und Protokoll (Wartung F5–F7) ──────────────────────────────

  function lauf(over: Partial<FetchLogEntry> = {}): FetchLogEntry {
    return {
      at: Math.floor(Date.now() / 1000) - 8 * 60,
      sourceId: 'q1',
      trigger: 'interval',
      seenInFolder: 3,
      alreadyKnown: 1,
      checked: 2,
      added: 2,
      quarantined: 0,
      skipped: 0,
      failed: 0,
      error: null,
      ...over,
    }
  }

  async function mitAbruf(letzter: FetchLogEntry | null, protokoll: FetchLogEntry[] = []) {
    vi.spyOn(api, 'lastFetch').mockResolvedValue(letzter)
    vi.spyOn(api, 'fetchLog').mockResolvedValue(protokoll)
    const w = form('mail')
    await new Promise((r) => setTimeout(r, 0))
    await w.vm.$nextTick()
    return w
  }

  it('sagt, wann zuletzt abgerufen wurde und was kam', async () => {
    const w = await mitAbruf(lauf())
    const zeile = w.get('.fetch-status').text()
    expect(zeile).toContain('8')
    expect(zeile).toContain('2')
  })

  it('unterscheidet nichts Neues von nie abgerufen', async () => {
    // Der Kern von F5: „seit Tagen kommt nichts" liess sich vorher nicht von
    // „es wurde nichts geschickt" unterscheiden.
    const leer = await mitAbruf(null)
    expect(leer.get('.fetch-status').text()).toContain('Noch nie')

    const ohneNeue = await mitAbruf(lauf({ added: 0 }))
    expect(ohneNeue.get('.fetch-status').text()).toContain('nichts Neues')
  })

  it('hebt einen fehlgeschlagenen Abruf hervor und nennt den Grund', async () => {
    const w = await mitAbruf(lauf({ error: 'Anmeldung abgelehnt' }))
    const zeile = w.get('.fetch-status')
    expect(zeile.classes()).toContain('bad')
    expect(zeile.text()).toContain('Anmeldung abgelehnt')
  })

  it('haelt das Protokoll eingeklappt, bis jemand danach fragt', async () => {
    // 50 Zeilen machten das Formular sonst unbrauchbar lang.
    const w = await mitAbruf(lauf(), [lauf(), lauf({ error: 'kaputt' })])
    expect(w.find('.fetch-log').exists()).toBe(false)

    await button(w, 'Abruf-Protokoll').trigger('click')
    expect(w.findAll('.fetch-log li')).toHaveLength(2)
    expect(w.findAll('.fetch-log li')[1].classes()).toContain('bad')
  })

  it('zeigt nur die Laeufe dieser Quelle', async () => {
    // Zwei Postfaecher sind moeglich; das Protokoll der einen gehoert nicht
    // in den Dialog der anderen.
    const w = await mitAbruf(lauf(), [lauf(), lauf({ sourceId: 'fremd' })])
    await button(w, 'Abruf-Protokoll').trigger('click')
    expect(w.findAll('.fetch-log li')).toHaveLength(1)
  })

  it('ruft auf Wunsch sofort ab', async () => {
    const sync = vi.spyOn(api, 'syncNow').mockResolvedValue([])
    const w = await mitAbruf(lauf())
    const knopf = w.findAll('.fetch-actions button').find((b) => b.text().includes('Jetzt abrufen'))
    await knopf!.trigger('click')
    await new Promise((r) => setTimeout(r, 0))
    expect(sync).toHaveBeenCalledWith('q1')
  })

  it('fragt vor dem Neuabgleich und nennt die Folgen', async () => {
    // F8 laeuft bei einem vollen Postfach minutenlang — das gehoert vorher
    // gesagt, samt der Zusicherung, dass die Diashow weiterlaeuft.
    const resync = vi.spyOn(api, 'resyncMailbox').mockResolvedValue(3)

    const w = await mitAbruf(lauf())
    const knopf = w.findAll('.resync button').find((b) => b.text().includes('neu abgleichen'))
    await knopf!.trigger('click')
    const frage = await answer('confirm')

    expect(frage.body).toContain('Minuten')
    expect(frage.body).toContain('abbrechen')
    // Nicht destruktiv, nur langwierig: kein Knopf in der Gefahrenfarbe.
    expect(frage.actions[0].variant).toBe('primary')
    expect(resync).toHaveBeenCalledWith('q1')
  })

  it('gleicht nichts ab, wenn die Rueckfrage verneint wird', async () => {
    const resync = vi.spyOn(api, 'resyncMailbox').mockResolvedValue(0)

    const w = await mitAbruf(lauf())
    const knopf = w.findAll('.resync button').find((b) => b.text().includes('neu abgleichen'))
    await knopf!.trigger('click')
    await answer(CANCEL)

    expect(resync).not.toHaveBeenCalled()
  })

  it('meldet sich vom Fortschritt wieder ab', async () => {
    // Ohne Abmeldung sammelten sich Zuhoerer bei jedem Neuabgleich an, und
    // der Fortschritt eines alten Laufs schriebe in die neue Anzeige.
    const ab = vi.fn()
    vi.spyOn(api, 'onResyncProgress').mockResolvedValue(ab)
    vi.spyOn(api, 'resyncMailbox').mockResolvedValue(1)

    const w = await mitAbruf(lauf())
    const knopf = w.findAll('.resync button').find((b) => b.text().includes('neu abgleichen'))
    await knopf!.trigger('click')
    await answer('confirm')

    expect(ab).toHaveBeenCalled()
  })

  // ── Hinweise zur Absicherung (E-39) ───────────────────────────────────────

  it('empfiehlt beim Anlegen ein App-Passwort und ein eigenes Postfach', async () => {
    // Der Keystore schuetzt den Schluessel; diese beiden Saetze begrenzen,
    // was ein gestohlenes Passwort ueberhaupt oeffnet. Sie stehen im
    // Formular, nicht in einer Anleitung, die niemand aufschlaegt.
    const w = mount(SourceDialog, {
      props: { source: null },
      global: { plugins: [i18n] },
    })
    // Auf „Postfach" umschalten: die Hinweise haengen an dessen Feldern.
    // Es sind echte Radiofelder; `setValue` schaltet sie um.
    const arten = w.findAll('.kinds input[type="radio"]')
    expect(arten.length, 'Auswahl der Quellenart nicht gefunden').toBe(4)
    await arten[3].setValue()
    await w.vm.$nextTick()

    const text = w.text()
    expect(text).toContain('App-Passwort')
    expect(text).toContain('eigenes Postfach')
  })

  it('zeigt beim Bearbeiten den Hinweis zum Beibehalten statt der Empfehlung', async () => {
    // Dort ist „leer lassen behaelt das gespeicherte" die dringendere
    // Auskunft — die Empfehlung kommt beim Anlegen zur richtigen Zeit.
    const w = form('mail')
    await w.vm.$nextTick()
    expect(w.text()).toContain('Leer lassen')
    expect(w.text()).not.toContain('App-Passwort')
  })
})
