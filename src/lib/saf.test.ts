import { describe, expect, it } from 'vitest'
import { backupFileName, uriFromString, uriToString } from './saf'

/**
 * Die Unterordner-Filter (FA-29) werden hier nicht mehr geprüft: seit der
 * SAF-Durchlauf im Rust-Backend liegt, gibt es die Logik nur noch einmal, und
 * `sources::mod` testet sie dort. Übrig bleibt die Serialisierung der URI — die
 * geht weiterhin durch das Frontend, weil nur dort der Ordnerdialog läuft.
 */
describe('SAF-URI Serialisierung', () => {
  it('überlebt den Weg durch die Konfiguration', () => {
    // Die URI wird als Zeichenkette in config.json abgelegt (FA-42) und im
    // Backend von `sources::local` wieder ausgepackt — beide Seiten müssen
    // dieselbe Form erwarten.
    const uri = {
      uri: 'content://com.android.externalstorage.documents/tree/primary%3ADCIM',
      documentTopTreeUri: null,
    }
    const round = uriFromString(uriToString(uri as never))
    expect(round).toEqual(uri)
  })

  it('behält das Feld documentTopTreeUri', () => {
    // Ohne dieses Feld kann das Plugin abgeleitete Einträge nicht auflösen —
    // der Ordner ließe sich dann zwar auswählen, aber nicht durchsuchen.
    const uri = {
      uri: 'content://org.nextcloud.documents/tree/abc/document/abc',
      documentTopTreeUri: 'content://org.nextcloud.documents/tree/abc',
    }
    const round = uriFromString(uriToString(uri as never))
    expect(round).toEqual(uri)
  })

  it('meldet unlesbare Werte statt zu werfen', () => {
    expect(uriFromString('')).toBeNull()
    expect(uriFromString('kein json')).toBeNull()
  })
})

describe('backupFileName', () => {
  it('nennt die Datei nach dem Tag, in ISO-Reihenfolge', () => {
    // ISO und nicht die deutsche Schreibweise: nur so sortiert eine Liste
    // mehrerer Sicherungen von selbst chronologisch. `31.08.2026` taete das
    // nicht, und genau daneben liegt die Sicherung, die man sucht.
    expect(backupFileName(new Date(2026, 7, 31))).toBe('slowshow-sicherung-2026-08-31.json')
  })

  it('fuellt Monat und Tag auf zwei Stellen auf', () => {
    // Ohne Auffuellen wuerde `2026-1-5` neben `2026-10-05` einsortiert.
    expect(backupFileName(new Date(2026, 0, 5))).toBe('slowshow-sicherung-2026-01-05.json')
  })

  it('endet auf .json', () => {
    // Der Speicherdialog uebernimmt den Vorschlag; ohne Endung findet der
    // Oeffnen-Dialog die Datei spaeter nicht ueber den MIME-Filter.
    expect(backupFileName()).toMatch(/\.json$/)
  })
})
