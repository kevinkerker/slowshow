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
import { dimOpacity as computeDim } from '@/lib/dim'
import { applyOrientation, setFrameOrientation } from '@/lib/api'

const store = useConfigStore()
const loaded = ref(false)

/**
 * Software-Abdunkelung als schwarzes Overlay.
 *
 * Die echte Displayhelligkeit setzt der native Teil (MainActivity.kt); dieses
 * Overlay wirkt zusätzlich und funktioniert auch dort, wo das Setzen der
 * Helligkeit vom Hersteller-ROM ignoriert wird (R-04). Seit E-22 trägt es die
 * Nachtschwärzung allein, wenn die App die Beleuchtung nicht regeln darf.
 *
 * Die Fallunterscheidung liegt in `lib/dim.ts` — dort ist sie testbar.
 */
const dimOpacity = computed(() => computeDim(store.display ?? null))

/**
 * Meldet die tatsächliche Ausrichtung ans Backend (E-26).
 *
 * Die Einstellung allein genügt nicht: bei „automatisch" bestimmt der
 * Lagesensor, und nur die WebView sieht das Ergebnis. Das Backend braucht es
 * für die Paarbildung (FA-08) — hochkant werden Querformate gepaart statt
 * Hochformate.
 */
const portraitQuery = window.matchMedia('(orientation: portrait)')

function reportOrientation() {
  void setFrameOrientation(portraitQuery.matches)
}

onMounted(async () => {
  await store.load()
  loaded.value = true
  // Erst die eingestellte Ausrichtung durchsetzen, dann melden, was dabei
  // herauskam — in der anderen Reihenfolge meldeten wir die alte Lage.
  await applyOrientation()
  reportOrientation()
  portraitQuery.addEventListener('change', reportOrientation)
  await keepAwake()
})

onBeforeUnmount(async () => {
  portraitQuery.removeEventListener('change', reportOrientation)
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
