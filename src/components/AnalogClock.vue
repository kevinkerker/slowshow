<script setup lang="ts">
/**
 * Analoguhr mit Strichindex (E-20).
 *
 * Wird an zwei Stellen verwendet: über dem Foto (FA-07) und im Nachtmodus
 * (FA-54). Deshalb bringt sie weder Größe noch Farbe mit — beides setzt die
 * aufrufende Komponente. Die Farbe kommt über `currentColor`, damit dieselbe
 * Uhr nachts in `--ss-night-clock` und tagsüber in `--ss-text` steht.
 *
 * Die Zeit wird als Prop hereingereicht statt selbst über `useNow` geholt:
 * beide Aufrufer haben den Zeitgeber ohnehin schon, und zwei Uhren im selben
 * Bild dürften sonst um eine Minute auseinanderlaufen.
 *
 * Kein Sekundenzeiger — siehe `clockAngles` in `lib/format.ts` (NF-06).
 */
import { computed } from 'vue'
import { clockAngles, formatClock } from '@/lib/format'

const props = defineProps<{ date: Date }>()

/**
 * Zwölf Marken; die auf zwölf, drei, sechs und neun sind länger.
 * Ohne diese vier fehlt der Uhr ohne Ziffern jeder Anhaltspunkt.
 */
const MARKS = Array.from({ length: 12 }, (_, i) => ({
  angle: i * 30,
  major: i % 3 === 0,
}))

const angles = computed(() => clockAngles(props.date))
/** Für Screenreader bleibt die Uhrzeit lesbar — Zeiger sind es nicht. */
const label = computed(() => formatClock(props.date))
</script>

<template>
  <svg class="dial" viewBox="0 0 100 100" role="img" :aria-label="label">
    <!-- Ring und Marken als Haarlinie: `non-scaling-stroke` hält sie bei
         jeder Uhrengröße bei 1 px, passend zu den 1-px-Rahmen der App. -->
    <circle class="ring" cx="50" cy="50" r="47" vector-effect="non-scaling-stroke" />
    <line
      v-for="mark in MARKS"
      :key="mark.angle"
      class="mark"
      :class="{ major: mark.major }"
      x1="50"
      y1="3"
      x2="50"
      :y2="mark.major ? 11.5 : 8"
      :transform="`rotate(${mark.angle} 50 50)`"
      vector-effect="non-scaling-stroke"
    />
    <!-- Zeiger skalieren mit, sonst wirkten sie auf der großen Nachtuhr
         wie Fäden. -->
    <line
      class="hand hour"
      x1="50"
      y1="50"
      x2="50"
      y2="26"
      :transform="`rotate(${angles.hour} 50 50)`"
    />
    <line
      class="hand minute"
      x1="50"
      y1="50"
      x2="50"
      y2="16"
      :transform="`rotate(${angles.minute} 50 50)`"
    />
  </svg>
</template>

<style scoped>
.dial {
  display: block;
  width: 100%;
  height: 100%;
}

.ring,
.mark {
  fill: none;
  stroke: currentColor;
  stroke-width: 1;
  /* Über die Variablen kann der Nachtmodus das Zifferblatt anheben: dort ist
     die Grundfarbe schon fast Schwarz, ein zusätzlicher Abschlag machte den
     Ring unsichtbar. */
  stroke-opacity: var(--ss-clock-face, 0.24);
}

.mark.major {
  stroke-opacity: var(--ss-clock-major, 0.55);
}

.hand {
  stroke: currentColor;
  stroke-linecap: round;
}

.hour {
  stroke-width: 2.8;
}

.minute {
  stroke-width: 1.7;
}
</style>
