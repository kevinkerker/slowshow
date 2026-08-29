<script setup lang="ts">
/**
 * Wurzelkomponente.
 *
 * Verantwortlich für drei Dinge, die die ganze App betreffen:
 *  1. Konfiguration laden, bevor irgendeine Ansicht rendert
 *  2. Bildschirm wachhalten (FA-50)
 *  3. Weiches Abdunkeln über ein Overlay (FA-52, FA-53)
 */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useConfigStore } from '@/stores/config'
import { keepAwake, releaseAwake } from '@/lib/wake'

const store = useConfigStore()
const loaded = ref(false)

/**
 * Software-Abdunkelung als schwarzes Overlay.
 *
 * Die echte Displayhelligkeit setzt der native Teil (MainActivity.kt); dieses
 * Overlay wirkt zusätzlich und funktioniert auch dort, wo das Setzen der
 * Helligkeit vom Hersteller-ROM ignoriert wird (R-04).
 */
const dimOpacity = computed(() => {
  // Im Nachtmodus nicht abdunkeln: dort steht die gedimmte Uhr auf Schwarz
  // (FA-54), und die ist bereits die abgedunkelte Darstellung. Ein zusätzliches
  // 99-Prozent-Overlay darüber machte sie unsichtbar — der Nachtmodus wäre
  // dann nicht von einem schwarzen Bildschirm zu unterscheiden.
  if (store.display?.showNightClock) return 0

  const level = store.display?.brightness ?? 100
  return 1 - Math.max(1, Math.min(100, level)) / 100
})

onMounted(async () => {
  await store.load()
  loaded.value = true
  await keepAwake()
})

onBeforeUnmount(async () => {
  store.dispose()
  await releaseAwake()
})
</script>

<template>
  <div class="app">
    <RouterView v-if="loaded" />

    <!-- Liegt über allem, fängt aber keine Berührungen ab: der Rahmen muss
         auch im abgedunkelten Zustand aufweckbar bleiben (FA-41, FA-55). -->
    <div
      v-if="dimOpacity > 0"
      class="dim"
      :style="{ opacity: dimOpacity }"
      aria-hidden="true"
    />
  </div>
</template>

<style scoped>
.app {
  position: relative;
  width: 100%;
  height: 100%;
  background: var(--ss-bg);
  overflow: hidden;
}

.dim {
  position: fixed;
  inset: 0;
  background: #000;
  pointer-events: none;
  z-index: 100;
  transition: opacity 1.2s ease;
}
</style>
