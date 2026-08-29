<script setup lang="ts">
/**
 * Bildunterschrift unten rechts (FA-07, Artboard „Diashow").
 *
 * Dateiname in Cormorant Garamond kursiv, darunter Aufnahmedatum und Quelle in
 * Versalien — genau die Aufteilung aus dem Entwurf.
 */
import { computed, toRef } from 'vue'
import { usePixelShift } from '@/composables/usePixelShift'
import { formatTakenAt, stripExtension } from '@/lib/format'
import { localeTag, type Language } from '@/lib/i18n'
import type { CacheEntry } from '@/lib/types'

const props = defineProps<{
  entry: CacheEntry | null
  sourceName: string
  showFileName: boolean
  showTakenAt: boolean
  pixelShift: boolean
  language: Language
}>()

const { transform } = usePixelShift(toRef(props, 'pixelShift'))

const title = computed(() =>
  props.showFileName && props.entry ? stripExtension(props.entry.fileName) : '',
)

/** Zweite Zeile: „Juni 2025 · Nextcloud" — beide Teile optional. */
const meta = computed(() => {
  const parts: string[] = []
  if (props.showTakenAt && props.entry?.takenAt != null) {
    const when = formatTakenAt(props.entry.takenAt, localeTag(props.language))
    if (when) parts.push(when)
  }
  if (props.showTakenAt && props.sourceName) parts.push(props.sourceName)
  return parts.join(' · ')
})

const visible = computed(() => title.value !== '' || meta.value !== '')
</script>

<template>
  <div v-if="visible" class="caption" :style="{ transform }">
    <div v-if="title" class="title">{{ title }}</div>
    <div v-if="meta" class="meta">{{ meta }}</div>
  </div>
</template>

<style scoped>
.caption {
  position: absolute;
  right: var(--ss-overlay-inset);
  bottom: 52px;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 4px;
  max-width: 45%;
  text-align: right;
  transition: transform 4s ease-in-out;
  pointer-events: none;
}

.title {
  font-family: var(--ss-font-display);
  font-style: italic;
  font-size: 24px;
  line-height: 1.2;
  color: rgba(242, 239, 233, 0.85);
  text-shadow: 0 1px 16px rgba(0, 0, 0, 0.5);
  /* Lange Dateinamen dürfen nicht über den halben Schirm laufen. */
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
}

.meta {
  font-size: 12px;
  font-weight: 500;
  color: rgba(242, 239, 233, 0.55);
  letter-spacing: 0.18em;
  text-transform: uppercase;
  text-shadow: 0 1px 12px rgba(0, 0, 0, 0.5);
}

@media (max-width: 900px) {
  .title {
    font-size: 19px;
  }
}
</style>
