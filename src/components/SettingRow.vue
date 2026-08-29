<script setup lang="ts">
/**
 * Eine Zeile in den Einstellungen: Beschriftung links, Bedienelement rechts,
 * darunter bei Bedarf ein erklärender Satz.
 *
 * Die Erklärung ist kein Beiwerk — NF-08 verlangt, dass die Ersteinrichtung
 * ohne Anleitung gelingt. Was eine Option bewirkt, muss also an der Option
 * selbst stehen.
 */
defineProps<{
  label: string
  hint?: string
  /** Bedienelement unter statt neben die Beschriftung setzen. */
  stacked?: boolean
}>()
</script>

<template>
  <div class="row" :class="{ stacked }">
    <div class="text">
      <div class="label">{{ label }}</div>
      <div v-if="hint" class="hint">{{ hint }}</div>
    </div>
    <div class="control">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.row {
  display: flex;
  align-items: center;
  gap: 24px;
  padding: 14px 0;
  border-bottom: 1px solid var(--ss-border-soft);
}

.row:last-child {
  border-bottom: none;
}

.text {
  flex-grow: 1;
  min-width: 0;
}

.label {
  font-size: 15px;
  color: var(--ss-text-strong);
}

.hint {
  margin-top: 3px;
  font-size: 13px;
  line-height: 1.4;
  color: var(--ss-text-dim);
}

.control {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 12px;
}

/* Gestapelt für breite Bedienelemente wie Schieberegler und Textfelder. */
.row.stacked {
  flex-direction: column;
  align-items: stretch;
  gap: 10px;
}

.row.stacked .control {
  width: 100%;
}
</style>
