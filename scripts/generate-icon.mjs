// Erzeugt das App-Icon aus der Design-Vorlage (E-13, Artboard "App-Icon (final)").
//
// Motiv "Rahmen & Horizont": weißer Rahmen, Messing-Horizont und -Sonne auf
// Tiefschwarz. Unter 48 px entfällt die Sonne — dafür wird eine eigene
// Kleinvariante gerendert.
//
// Ablauf:
//   node scripts/generate-icon.mjs      -> schreibt src-tauri/icons/icon-source.png
//   npx tauri icon src-tauri/icons/icon-source.png
//
// Der zweite Schritt erzeugt den kompletten Satz (ico, icns, Android-mipmaps).

import { Resvg } from '@resvg/resvg-js'
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const outDir = resolve(root, 'src-tauri/icons')
const docsDir = resolve(root, 'docs')

// Farben aus dem Design-Canvas (E-13).
const BLACK = '#0A0A0A'
const OFFWHITE = '#F2EFE9'
const BRASS = '#C2A878'

/**
 * Das Icon-Motiv im 160er-Koordinatensystem der Design-Vorlage.
 * @param {{ sun: boolean, stroke: number, padding: number }} opts
 */
function motif({ sun, stroke, padding }) {
  const x = 34 + padding
  const size = 92 - padding * 2
  const horizonY = 96
  return `
    <rect x="${x}" y="${x}" width="${size}" height="${size}"
          fill="none" stroke="${OFFWHITE}" stroke-width="${stroke}"/>
    <line x1="${x + 12}" y1="${horizonY}" x2="${x + size - 12}" y2="${horizonY}"
          stroke="${BRASS}" stroke-width="${stroke}"/>
    ${sun ? `<circle cx="96" cy="64" r="9" fill="${BRASS}"/>` : ''}
  `
}

/**
 * @param {{ size: number, sun?: boolean, stroke?: number, padding?: number, rounded?: boolean }} opts
 */
function svg({ size, sun = true, stroke = 4, padding = 0, rounded = false }) {
  const bg = rounded
    ? `<rect width="160" height="160" rx="36" fill="${BLACK}"/>`
    : `<rect width="160" height="160" fill="${BLACK}"/>`
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 160 160">
    ${bg}
    ${motif({ sun, stroke, padding })}
  </svg>`
}

function render(source, size) {
  const resvg = new Resvg(source, { fitTo: { mode: 'width', value: size } })
  return resvg.render().asPng()
}

mkdirSync(outDir, { recursive: true })
mkdirSync(docsDir, { recursive: true })

// Quellbild für `tauri icon` — quadratisch, ohne eigene Rundung, weil Tauri
// und Android die Maske selbst anlegen.
writeFileSync(resolve(outDir, 'icon-source.png'), render(svg({ size: 1024 }), 1024))

// Android Adaptive Foreground: Motiv muss in der 66-dp-Safe-Zone liegen,
// deshalb zusätzliches Padding.
writeFileSync(
  resolve(outDir, 'icon-adaptive-foreground.png'),
  render(svg({ size: 1024, padding: 10 }), 1024),
)

// Kleinvariante ohne Sonne (E-13: „unter 48 px entfällt die Sonne").
writeFileSync(resolve(outDir, 'icon-small.png'), render(svg({ size: 48, sun: false, stroke: 11 }), 48))

// Vorschau für den Store-Eintrag und die Dokumentation (RB-03).
writeFileSync(resolve(docsDir, 'slowshow-icon-512.png'), render(svg({ size: 512, rounded: true }), 512))

console.log('Icons geschrieben nach src-tauri/icons/ und docs/')
console.log('Weiter mit: npx tauri icon src-tauri/icons/icon-source.png')
