<script setup lang="ts">
/**
 * Schalter im Stil des Entwurfs (Artboard „Einstellungen · Quellen").
 *
 * Maße direkt aus der Vorlage: 52 × 30 mit 24er Knopf. Die Trefferfläche ist
 * über ein unsichtbares Polster auf 44 px vergrößert (FA-40) — der sichtbare
 * Schalter bleibt dabei genau so groß wie gezeichnet.
 */
const model = defineModel<boolean>({ required: true })

defineProps<{
  /** Beschriftung für Screenreader, wenn daneben kein Text steht (NF-11). */
  label?: string
  disabled?: boolean
}>()
</script>

<template>
  <button
    type="button"
    class="toggle"
    role="switch"
    :aria-checked="model"
    :aria-label="label"
    :disabled="disabled"
    @click="model = !model"
  >
    <span class="track" :class="{ on: model }">
      <span class="knob" />
    </span>
  </button>
</template>

<style scoped>
.toggle {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--ss-touch-target);
  height: var(--ss-touch-target);
  min-height: var(--ss-touch-target);
  padding: 0;
  flex-shrink: 0;
}

.toggle:disabled {
  opacity: 0.45;
  cursor: default;
}

.track {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  width: 52px;
  height: 30px;
  padding: 0 3px;
  border-radius: var(--ss-radius-pill);
  background: var(--ss-toggle-off);
  transition: background var(--ss-transition);
}

.track.on {
  justify-content: flex-end;
  background: var(--ss-accent);
}

.knob {
  width: 24px;
  height: 24px;
  border-radius: var(--ss-radius-pill);
  background: var(--ss-toggle-knob-off);
  transition: background var(--ss-transition);
}

.track.on .knob {
  background: var(--ss-bg);
}
</style>
