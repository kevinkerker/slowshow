<script setup lang="ts">
/**
 * Uhrzeit und Datum unten links (FA-07, Artboard „Diashow").
 *
 * Beide Zeilen sind einzeln abschaltbar; die Gruppe wandert langsam gegen das
 * Einbrennen (NF-07).
 */
import { computed, toRef } from 'vue'
import { useNow } from '@/composables/useNow'
import { usePixelShift } from '@/composables/usePixelShift'
import { formatClock, formatDateLine } from '@/lib/format'
import { localeTag } from '@/lib/i18n'
import type { Language } from '@/lib/i18n'

const props = defineProps<{
  showClock: boolean
  showDate: boolean
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
    <div v-if="showClock" class="time">{{ time }}</div>
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
}
</style>
