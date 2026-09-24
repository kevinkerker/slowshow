<script setup lang="ts">
/**
 * Knopf nach dem Knopfsystem (E-58, Vorlage `Vorschlag-Knoepfe`).
 *
 * Fünf Varianten nach den Android-Mustern Filled, Outlined und Text:
 *  - `primary`   Messingfläche — die einzige helle Fläche einer Ansicht
 *  - `secondary` Rand
 *  - `danger`    Rand, Aufschrift in der Fehlerfarbe
 *  - `ghost`     nur Text, für „Abbrechen"
 *  - `link`      Messingtext ohne Innenabstand, für Verweise
 *
 * Vorher war dieselbe Variante je Bereich anders gebaut und teils gar nicht
 * gestaltet: „Aufräumen" war nicht rot, „Durchlauf neu starten" bloßer Text.
 *
 * `busy` zeigt einen Kreisel vor der Aufschrift und nimmt weitere Tipps nicht
 * an, bleibt aber sichtbar bedienbar — gesperrt (`disabled`) ist ein anderer
 * Zustand und wird blass.
 */
withDefaults(
  defineProps<{
    variant?: 'primary' | 'secondary' | 'danger' | 'ghost' | 'link'
    busy?: boolean
    disabled?: boolean
    type?: 'button' | 'submit'
  }>(),
  { variant: 'secondary', busy: false, disabled: false, type: 'button' },
)
</script>

<template>
  <button
    :type="type"
    class="ss-btn"
    :class="[`ss-btn--${variant}`, { busy }]"
    :disabled="disabled"
    :aria-busy="busy || undefined"
  >
    <svg v-if="busy" class="spinner" width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="8" cy="8" r="6" stroke="currentColor" stroke-opacity="0.25" stroke-width="1.5" />
      <path d="M8 2 A6 6 0 0 1 14 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
    </svg>
    <slot />
  </button>
</template>

<style scoped>
.ss-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  height: var(--ss-button-height);
  min-height: var(--ss-button-height);
  padding: 0 22px;
  border: 1px solid transparent;
  border-radius: var(--ss-radius-pill);
  font-size: var(--ss-fs-m);
  font-weight: 500;
  white-space: nowrap;
  flex-shrink: 0;
  transition: background var(--ss-transition), color var(--ss-transition),
    border-color var(--ss-transition);
}

.ss-btn:disabled {
  opacity: var(--ss-opacity-disabled);
  cursor: default;
}

.ss-btn.busy {
  pointer-events: none;
}

.ss-btn--primary {
  background: var(--ss-accent-fill);
  color: var(--ss-on-accent);
}

.ss-btn--primary:active:not(:disabled) {
  background: var(--ss-accent-hover);
}

.ss-btn--secondary {
  border-color: var(--ss-border-strong);
  color: var(--ss-text-body);
}

.ss-btn--secondary:active:not(:disabled) {
  background: var(--ss-surface-accent);
  color: var(--ss-text-accent);
}

.ss-btn--danger {
  border-color: var(--ss-border-strong);
  color: var(--ss-error);
}

.ss-btn--danger:active:not(:disabled) {
  border-color: var(--ss-error);
  background: var(--ss-surface-accent);
}

.ss-btn--ghost {
  color: var(--ss-text-muted);
}

.ss-btn--ghost:active:not(:disabled) {
  background: var(--ss-surface-accent);
  color: var(--ss-text-accent);
}

.ss-btn--link {
  padding: 0;
  color: var(--ss-accent);
}

.ss-btn--link:active:not(:disabled) {
  color: var(--ss-text-accent);
}

.spinner {
  flex-shrink: 0;
  animation: ss-btn-spin 0.9s linear infinite;
}

@keyframes ss-btn-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
