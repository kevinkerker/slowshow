import { describe, expect, it } from 'vitest'
import { classify, IMAGE_EXTENSIONS, SKIP_EXTENSIONS } from './file-class'

describe('classify', () => {
  it('erkennt die unterstützten Bildformate (FA-04)', () => {
    expect(classify('strand.jpg')).toBe('image')
    expect(classify('strand.JPG')).toBe('image')
    expect(classify('a.jpeg')).toBe('image')
    expect(classify('a.png')).toBe('image')
    expect(classify('a.webp')).toBe('image')
  })

  it('überspringt HEIC und Video (FA-09, E-04, E-07)', () => {
    expect(classify('IMG_0042.HEIC')).toBe('skipped')
    expect(classify('a.heif')).toBe('skipped')
    expect(classify('a.avif')).toBe('skipped')
    expect(classify('clip.mp4')).toBe('skipped')
    expect(classify('clip.MOV')).toBe('skipped')
  })

  it('ignoriert Fremddateien ohne Log-Eintrag', () => {
    expect(classify('Thumbs.db')).toBe('irrelevant')
    expect(classify('notizen.txt')).toBe('irrelevant')
    expect(classify('ordner_ohne_endung')).toBe('irrelevant')
    expect(classify('')).toBe('irrelevant')
  })

  it('behandelt einen führenden Punkt nicht als Endung', () => {
    // `.jpg` als kompletter Dateiname ist eine versteckte Datei, kein Foto.
    expect(classify('.jpg')).toBe('irrelevant')
  })

  it('wertet nur die letzte Endung aus', () => {
    expect(classify('urlaub.mp4.jpg')).toBe('image')
    expect(classify('urlaub.jpg.mp4')).toBe('skipped')
  })
})

describe('Abgleich mit dem Backend', () => {
  // Diese Listen stehen doppelt: hier und in src-tauri/src/decode.rs.
  // Die Zusicherung hält fest, was das Backend kennt — läuft die Liste
  // auseinander, würden lokale und entfernte Quellen unterschiedlich
  // filtern.
  it('führt genau die vier Bildformate aus FA-04', () => {
    expect([...IMAGE_EXTENSIONS]).toEqual(['jpg', 'jpeg', 'png', 'webp'])
  })

  it('führt dieselben Ausschlüsse wie decode.rs', () => {
    expect([...SKIP_EXTENSIONS]).toEqual([
      'heic',
      'heif',
      'avif',
      'mp4',
      'mov',
      'avi',
      'mkv',
      'webm',
      'm4v',
      '3gp',
      'gif',
    ])
  })

  it('überschneidet die beiden Listen nicht', () => {
    const overlap = IMAGE_EXTENSIONS.filter((e) =>
      (SKIP_EXTENSIONS as readonly string[]).includes(e),
    )
    expect(overlap).toEqual([])
  })
})
