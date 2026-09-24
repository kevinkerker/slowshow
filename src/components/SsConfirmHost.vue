<script setup lang="ts">
/**
 * Rückfragedialog (E-59, Vorlage `Vorschlag-Rueckfrage`).
 *
 * Einmal in `App.vue` eingehängt; geöffnet wird er über
 * `confirm()` aus `composables/useConfirm.ts`.
 *
 * Titel als Frage, Text darunter. Zwei Ausgänge stehen rechtsbündig in einer
 * Reihe, „Abbrechen" links neben der bestätigenden Aktion. Bei drei Ausgängen
 * stehen die Knöpfe untereinander über die volle Breite, „Abbrechen" zuletzt —
 * nebeneinander würden die langen Aufschriften nicht passen, und die Wahl
 * soll sich lesen lassen wie eine Liste.
 */
import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import SsButton from './SsButton.vue'
import { CANCEL, confirmRequest, settle } from '@/composables/useConfirm'

const { t } = useI18n()

const request = confirmRequest
const stacked = computed(() => (request.value?.actions.length ?? 0) > 1)
const panel = ref<HTMLElement | null>(null)

// Fokus in den Dialog, damit Tastatur und Screenreader dort landen (NF-11).
watch(request, async (value) => {
  if (!value) return
  await nextTick()
  panel.value?.focus()
})
</script>

<template>
  <div v-if="request" class="backdrop" @click.self="settle(CANCEL)">
    <div
      ref="panel"
      class="panel"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="ss-confirm-title"
      :aria-describedby="request.body ? 'ss-confirm-body' : undefined"
      tabindex="-1"
      @keydown.esc="settle(CANCEL)"
    >
      <h2 id="ss-confirm-title" class="title">{{ request.title }}</h2>
      <p v-if="request.body" id="ss-confirm-body" class="body">{{ request.body }}</p>

      <div class="actions" :class="{ stacked }">
        <SsButton v-if="!stacked" variant="ghost" @click="settle(CANCEL)">
          {{ t('common.cancel') }}
        </SsButton>
        <SsButton
          v-for="action in request.actions"
          :key="action.id"
          :variant="action.variant ?? 'danger'"
          @click="settle(action.id)"
        >
          {{ action.label }}
        </SsButton>
        <SsButton v-if="stacked" variant="ghost" @click="settle(CANCEL)">
          {{ t('common.cancel') }}
        </SsButton>
      </div>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--ss-space-3);
  background: var(--ss-dialog-scrim);
}

.panel {
  display: flex;
  flex-direction: column;
  gap: var(--ss-space-2);
  width: min(520px, 100%);
  max-height: 100%;
  overflow-y: auto;
  padding: var(--ss-space-3);
  background: var(--ss-surface);
  border: 1px solid var(--ss-border);
  border-radius: var(--ss-radius-lg);
  box-shadow: var(--ss-shadow-dialog);
  outline: none;
}

.title {
  font-family: var(--ss-font-display);
  font-size: var(--ss-fs-xl);
  font-weight: 400;
  line-height: 1.25;
  color: var(--ss-text);
  overflow-wrap: anywhere;
}

.body {
  font-size: var(--ss-fs-m);
  line-height: 1.55;
  color: var(--ss-text-dim);
  white-space: pre-line;
}

.actions {
  display: flex;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: var(--ss-space-1);
  margin-top: var(--ss-space-1);
}

.actions.stacked {
  flex-direction: column;
  align-items: stretch;
}

/* Lange Aufschriften wie „Entfernen · 148 Fotos zurück in die Quarantäne"
   dürfen in der Liste umbrechen, statt aus dem Dialog zu ragen. */
.actions.stacked :deep(.ss-btn) {
  white-space: normal;
  height: auto;
  padding-top: 12px;
  padding-bottom: 12px;
  text-align: center;
}
</style>
