<script setup lang="ts">
/**
 * Nachtmodus (FA-54, Artboard „Nachtmodus").
 *
 * Außerhalb der Aktivzeit steht wahlweise eine sehr dunkle Uhr auf Schwarz
 * statt eines völlig schwarzen Bildschirms. Die Farben liegen bewusst knapp
 * über Schwarz: ablesbar im dunklen Raum, aber ohne den Raum zu beleuchten.
 *
 * Die Uhr wandert wie die Tages-Einblendungen (NF-07) — bei einer Anzeige, die
 * jede Nacht neun Stunden lang dieselbe Fläche zeigt, ist das der Fall, in dem
 * Einbrennen tatsächlich droht.
 */
import { computed, toRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { useNow } from '@/composables/useNow'
import { usePixelShift } from '@/composables/usePixelShift'
import { formatClock } from '@/lib/format'

const props = defineProps<{
  /** Uhrzeit, ab der die Diashow wieder läuft — "HH:MM". */
  resumeAt: string
  pixelShift: boolean
}>()

const { t } = useI18n()
const now = useNow()
const { transform } = usePixelShift(toRef(props, 'pixelShift'))

const time = computed(() => formatClock(now.value))
</script>

<template>
  <div class="night">
    <div class="inner" :style="{ transform }">
      <div class="time">{{ time }}</div>
      <div class="label">{{ t('night.restUntil', { time: props.resumeAt }) }}</div>
    </div>
  </div>
</template>

<style scoped>
.night {
  position: absolute;
  inset: 0;
  background: var(--ss-bg-night);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 10;
}

.inner {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 14px;
  transition: transform 4s ease-in-out;
}

.time {
  font-size: 120px;
  font-weight: 400;
  line-height: 1;
  color: var(--ss-night-clock);
  letter-spacing: 0.02em;
  font-variant-numeric: tabular-nums;
}

.label {
  font-size: 13px;
  font-weight: 500;
  color: var(--ss-night-label);
  letter-spacing: 0.28em;
  text-transform: uppercase;
}

@media (max-width: 900px) {
  .time {
    font-size: 80px;
  }
}

@media (max-height: 520px) {
  .time {
    font-size: 56px;
  }
}
</style>
