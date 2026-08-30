<script setup lang="ts">
/**
 * Uhrzeit und Datum unten links (FA-07, Artboard „Diashow").
 *
 * Beide Zeilen sind einzeln abschaltbar; die Gruppe wandert langsam gegen das
 * Einbrennen (NF-07). Die Uhr steht wahlweise als Ziffern oder als Zeiger
 * (E-20) — der Nachtmodus wird getrennt eingestellt.
 */
import { computed, toRef } from 'vue'
import AnalogClock from './AnalogClock.vue'
import { useNow } from '@/composables/useNow'
import { usePixelShift } from '@/composables/usePixelShift'
import { formatClock, formatDateLine } from '@/lib/format'
import { localeTag } from '@/lib/i18n'
import type { Language } from '@/lib/i18n'
import type { ClockStyle } from '@/lib/types'

const props = defineProps<{
  showClock: boolean
  showDate: boolean
  clockStyle: ClockStyle
  pixelShift: boolean
  language: Language
}>()

const now = useNow()
const { transform } = usePixelShift(toRef(props, 'pixelShift'))

const time = computed(() => formatClock(now.value))
const date = computed(() => formatDateLine(now.value, localeTag(props.language)))
</script>

<template>
  <div v-if="showClock || showDate" class="clock" :style="{ transform }">
    <template v-if="showClock">
      <div v-if="clockStyle === 'analog'" class="dial">
        <AnalogClock :date="now" />
      </div>
      <div v-else class="time">{{ time }}</div>
    </template>
    <div v-if="showDate" class="date">{{ date }}</div>
  </div>
</template>

<style scoped>
.clock {
  position: absolute;
  left: var(--ss-overlay-inset);
  bottom: 48px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  /* Langsam genug, dass die Verschiebung nicht als Bewegung auffällt. */
  transition: transform 4s ease-in-out;
  pointer-events: none;
}

.time {
  font-size: 92px;
  font-weight: 400;
  line-height: 1;
  color: var(--ss-text);
  letter-spacing: -0.01em;
  /* Ohne Tabellenziffern springt die Uhr bei jedem Minutenwechsel in der
     Breite — auf einem Bild, das 30 Sekunden stillsteht, sehr auffällig. */
  font-variant-numeric: tabular-nums;
  text-shadow: 0 1px 24px rgba(0, 0, 0, 0.45);
}

/* Die Analoguhr braucht mehr Fläche als die Textzeile, sonst ist der
   Strichindex nicht mehr zu erkennen. Der Schlagschatten entspricht dem
   `text-shadow` der Ziffern — über einem hellen Foto wäre sie sonst weg. */
.dial {
  width: 150px;
  height: 150px;
  margin-bottom: 10px;
  color: var(--ss-text);
  filter: drop-shadow(0 1px 16px rgba(0, 0, 0, 0.5));
}

.date {
  font-size: 15px;
  font-weight: 500;
  color: rgba(242, 239, 233, 0.72);
  letter-spacing: 0.22em;
  text-transform: uppercase;
  text-shadow: 0 1px 16px rgba(0, 0, 0, 0.45);
}

@media (max-width: 900px) {
  .time {
    font-size: 64px;
  }

  .dial {
    width: 112px;
    height: 112px;
  }

  .date {
    font-size: 13px;
  }
}

/* Flache Ansichten (Smartphone quer, ~390 px hoch): die Uhr darf nicht
   ein Drittel des Bildes einnehmen. */
@media (max-height: 520px) {
  .clock {
    bottom: 28px;
  }

  .time {
    font-size: 48px;
  }

  .dial {
    width: 84px;
    height: 84px;
    margin-bottom: 6px;
  }
}
</style>
