<script setup lang="ts">
/**
 * Runder Symbolknopf (E-58, Vorlage `Vorschlag-Knoepfe`, Zeile „Symbolknopf").
 *
 * 48 × 48 mit Rand, Symbol 20 px. Die Beschriftung ist Pflicht: ein Knopf ohne
 * Text braucht sie für Screenreader (NF-11) und als Tooltip.
 *
 * `busy` dreht das Symbol in Messing — so meldet der Sync-Knopf der
 * Quellenkarte, dass ein Abgleich läuft, ohne die Karte umzubauen.
 */
withDefaults(
  defineProps<{
    label: string
    busy?: boolean
    disabled?: boolean
  }>(),
  { busy: false, disabled: false },
)
</script>

<template>
  <button
    type="button"
    class="ss-icon-btn"
    :class="{ busy }"
    :disabled="disabled"
    :aria-label="label"
    :title="label"
    :aria-busy="busy || undefined"
  >
    <span class="glyph" :class="{ spinning: busy }">
      <slot />
    </span>
  </button>
</template>

<style scoped>
.ss-icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--ss-touch-target);
  height: var(--ss-touch-target);
  min-height: var(--ss-touch-target);
  padding: 0;
  flex-shrink: 0;
  border: 1px solid var(--ss-border-strong);
  border-radius: var(--ss-radius-pill);
  color: var(--ss-icon-soft);
  transition: background var(--ss-transition), color var(--ss-transition);
}

.ss-icon-btn:active:not(:disabled) {
  background: var(--ss-surface-accent);
  color: var(--ss-accent);
}

.ss-icon-btn:disabled {
  opacity: var(--ss-opacity-disabled);
  cursor: default;
}

.ss-icon-btn.busy {
  color: var(--ss-accent);
  pointer-events: none;
}

.glyph {
  display: flex;
}

.spinning {
  animation: ss-icon-spin 1.1s linear infinite;
  transform-origin: center;
}

@keyframes ss-icon-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
