import { describe, expect, it } from 'vitest'
import {
  formatBytes,
  formatClock,
  formatDateLine,
  formatDimensions,
  formatInterval,
  formatRelativeTime,
  formatTakenAt,
  stripExtension,
} from './format'

/** Minimaler Ersatz für vue-i18n `t` — gibt Schlüssel und Parameter zurück. */
const t = (key: string, params?: Record<string, unknown>) =>
  params ? `${key}:${Object.values(params).join(',')}` : key

describe('formatClock', () => {
  it('füllt Stunden und Minuten auf zwei Stellen', () => {
    expect(formatClock(new Date(2025, 5, 15, 9, 5))).toBe('09:05')
    expect(formatClock(new Date(2025, 5, 15, 21, 47))).toBe('21:47')
    expect(formatClock(new Date(2025, 5, 15, 0, 0))).toBe('00:00')
  })
})

describe('formatDateLine', () => {
  it('setzt Wochentag und Datum wie im Entwurf zusammen', () => {
    // 29. August 2026 ist ein Samstag — dasselbe Datum wie im Design-Canvas.
    const line = formatDateLine(new Date(2026, 7, 29), 'de-DE')
    expect(line).toContain('Samstag')
    expect(line).toContain('29. August')
    expect(line).toContain('·')
  })

  it('folgt der eingestellten Sprache', () => {
    expect(formatDateLine(new Date(2026, 7, 29), 'en-GB')).toContain('Saturday')
  })
})

describe('formatTakenAt', () => {
  it('zeigt Monat und Jahr', () => {
    const ts = Math.floor(new Date(2025, 5, 15).getTime() / 1000)
    expect(formatTakenAt(ts, 'de-DE')).toBe('Juni 2025')
  })

  it('liefert bei fehlendem Datum eine leere Zeichenkette', () => {
    // Die Bildunterschrift blendet die Zeile dann aus (FA-07).
    expect(formatTakenAt(null, 'de-DE')).toBe('')
  })

  it('verträgt einen unsinnigen Zeitstempel', () => {
    expect(formatTakenAt(Number.NaN, 'de-DE')).toBe('')
  })
})

describe('formatRelativeTime', () => {
  const now = new Date(2026, 7, 29, 12, 0, 0)
  const nowSec = Math.floor(now.getTime() / 1000)

  it('meldet eine nie synchronisierte Quelle', () => {
    expect(formatRelativeTime(null, now, t)).toBe('sources.neverSynced')
  })

  it('staffelt von Sekunden bis Tagen', () => {
    expect(formatRelativeTime(nowSec - 30, now, t)).toBe('time.justNow')
    expect(formatRelativeTime(nowSec - 12 * 60, now, t)).toBe('time.minutesAgo:12')
    expect(formatRelativeTime(nowSec - 3 * 3600, now, t)).toBe('time.hoursAgo:3')
    expect(formatRelativeTime(nowSec - 50 * 3600, now, t)).toBe('time.daysAgo:2')
  })

  it('behandelt die Grenzen sauber', () => {
    expect(formatRelativeTime(nowSec - 59, now, t)).toBe('time.justNow')
    expect(formatRelativeTime(nowSec - 60, now, t)).toBe('time.minutesAgo:1')
    expect(formatRelativeTime(nowSec - 3600, now, t)).toBe('time.hoursAgo:1')
  })
})

describe('formatBytes', () => {
  it('rechnet in die passende Einheit um', () => {
    expect(formatBytes(512, 'de-DE')).toBe('512 B')
    expect(formatBytes(1536, 'de-DE')).toBe('1,5 KB')
    expect(formatBytes(2 * 1024 * 1024 * 1024, 'de-DE')).toBe('2,0 GB')
  })

  it('zeigt den Cache-Stand aus dem Entwurf', () => {
    // Fußzeile im Artboard: „Cache 1,4 / 2,0 GB"
    const used = Math.round(1.4 * 1024 * 1024 * 1024)
    expect(formatBytes(used, 'de-DE')).toBe('1,4 GB')
  })

  it('lässt bei großen Werten die Nachkommastelle weg', () => {
    expect(formatBytes(512 * 1024 * 1024, 'de-DE')).toBe('512 MB')
  })

  it('verträgt null Bytes', () => {
    expect(formatBytes(0, 'de-DE')).toBe('0 B')
  })
})

describe('formatInterval', () => {
  it('bleibt unter einer Minute bei Sekunden', () => {
    // Untergrenze aus FA-02.
    expect(formatInterval(5, t)).toBe('time.seconds:5')
    expect(formatInterval(30, t)).toBe('time.seconds:30')
  })

  it('rechnet ab einer Minute in Minuten um', () => {
    expect(formatInterval(60, t)).toBe('time.minutes:1')
    expect(formatInterval(300, t)).toBe('time.minutes:5')
    // Obergrenze aus FA-02: 30 Minuten.
    expect(formatInterval(1800, t)).toBe('time.minutes:30')
  })
})

describe('stripExtension', () => {
  it('entfernt die Endung', () => {
    expect(stripExtension('gardasee.jpg')).toBe('gardasee')
    expect(stripExtension('urlaub.2025.jpeg')).toBe('urlaub.2025')
  })

  it('lässt Namen ohne Endung unangetastet', () => {
    expect(stripExtension('ohneendung')).toBe('ohneendung')
    // Führender Punkt ist keine Endung.
    expect(stripExtension('.versteckt')).toBe('.versteckt')
  })
})

describe('formatDimensions', () => {
  it('nutzt das typografische Malzeichen', () => {
    expect(formatDimensions(1920, 1080)).toBe('1920 × 1080')
  })
})
