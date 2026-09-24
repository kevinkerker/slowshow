<script setup lang="ts">
/**
 * Anzeige einer Rückmeldung (E-62). Zeitsteuerung und Regeln liegen in
 * `composables/useFeedback.ts`; hier nur Aussehen und Vorlesen.
 *
 * Der äußere Bereich steht immer im DOM: Screenreader kündigen nur Änderungen
 * in einem Live-Bereich an, der schon vorher da war. Fehler kommen als
 * `role="alert"` und damit sofort, Erfolge höflich hinterher.
 */
import type { FeedbackState } from '@/composables/useFeedback'

defineProps<{
  feedback: Readonly<FeedbackState> | null
}>()
</script>

<template>
  <span class="ss-feedback" aria-live="polite">
    <Transition name="ss-feedback" mode="out-in">
      <span
        v-if="feedback"
        :key="feedback.id"
        class="text"
        :class="feedback.kind"
        :role="feedback.kind === 'error' ? 'alert' : 'status'"
      >{{ feedback.text }}</span>
    </Transition>
  </span>
</template>

<style scoped>
.ss-feedback {
  display: inline-flex;
  min-width: 0;
}

.text {
  font-size: var(--ss-fs-m);
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.text.ok {
  color: var(--ss-ok);
}

.text.error {
  color: var(--ss-error);
}

.ss-feedback-leave-active {
  transition: opacity var(--ss-transition);
}

.ss-feedback-leave-to {
  opacity: 0;
}
</style>
