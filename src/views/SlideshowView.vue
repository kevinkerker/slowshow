<script setup lang="ts">
/**
 * Die Diashow — Hauptansicht der App (Artboard „Diashow").
 *
 * Vollbild, Endlosschleife, Bedienung ausschließlich über Gesten. Außerhalb der
 * Aktivzeit übernimmt der Nachtmodus (FA-52, FA-54).
 */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import SlideStage from '@/components/SlideStage.vue'
import ClockOverlay from '@/components/ClockOverlay.vue'
import CaptionOverlay from '@/components/CaptionOverlay.vue'
import NightClock from '@/components/NightClock.vue'
import { useConfigStore } from '@/stores/config'
import { useSlideshowStore } from '@/stores/slideshow'
import { createGestureRecognizer } from '@/composables/useGestures'
import { usePixelShift } from '@/composables/usePixelShift'

const router = useRouter()
const { t } = useI18n()
const config = useConfigStore()
const show = useSlideshowStore()

const toast = ref<string | null>(null)
let toastTimer: ReturnType<typeof setTimeout> | null = null

const cfg = computed(() => config.config)
const active = computed(() => config.display?.slideshowActive ?? true)
const nightClock = computed(() => config.display?.showNightClock ?? false)

/** Metadaten des ersten sichtbaren Bildes — Grundlage der Bildunterschrift. */
const currentEntry = computed(() => {
  const id = show.currentIds[0]
  return id ? (show.info[id] ?? null) : null
})

const currentSourceName = computed(() => {
  const sourceId = currentEntry.value?.sourceId
  return config.sources.find((s) => s.id === sourceId)?.name ?? ''
})

const isEmpty = computed(() => config.ready && !show.hasImages)

/**
 * Die Pause wird dauerhaft angezeigt, nicht nur beim Umschalten (E-21).
 *
 * Ein Rahmen, der stehenbleibt, sieht sonst aus wie einer, der hängt. Der
 * kurze Hinweis war genau dann verschwunden, wenn jemand später davorstand und
 * sich fragte, warum sich nichts mehr tut.
 *
 * Nicht im Nachtmodus: dort soll der Schirm dunkel bleiben (FA-54), und dass
 * nichts weiterläuft, ist die erwartete Lage.
 */
const paused = computed(() => show.hasImages && !show.playing && active.value)

/* Ein dauerhaftes Abzeichen ist genau das statische Overlay, gegen das NF-07
 * gedacht ist — eine Pause kann Tage dauern. Es wandert deshalb wie Uhr und
 * Bildunterschrift. */
const { transform: pausedShift } = usePixelShift(
  computed(() => cfg.value?.overlays.pixelShift ?? true),
)

// ── Gesten (FA-41, FA-43) ────────────────────────────────────────────────────

/**
 * Ohne Bilder tut keine Geste etwas.
 *
 * Sonst schaltet ein Tipp auf den leeren Schirm die Diashow unbemerkt ab —
 * sichtbar wird das erst, wenn nach dem ersten Sync nichts weiterläuft und
 * niemand weiß, warum.
 */
function whenReady(action: () => void) {
  return () => {
    if (show.hasImages) action()
  }
}

const gestures = createGestureRecognizer({
  onSwipeLeft: whenReady(() => void show.next()),
  onSwipeRight: whenReady(() => void show.prev()),

  // Tippzonen: rechts weiter, links zurück, Mitte Pause. Auf einem Rahmen an
  // der Wand ist ein Tipp bequemer als eine Wischbewegung.
  onTapRight: whenReady(() => void show.next()),
  onTapLeft: whenReady(() => void show.prev()),
  onTapCenter: whenReady(async () => {
    await show.togglePlaying()
    // Kein kurzer Hinweis: den Zustand zeigt das Abzeichen, solange er gilt.
    flash('')
  }),

  // Der lange Druck bleibt immer erreichbar — er ist der Weg in die
  // Einstellungen und wird gerade im leeren Zustand gebraucht (FA-43).
  onLongPress: () => router.push('/settings'),
})

/**
 * Das Zahnrad öffnet die Einstellungen — immer.
 *
 * Vorher hing es an `protectSettings` und tat bei der Voreinstellung gar
 * nichts: ein Knopf, der aussieht wie ein Knopf und keiner ist.
 *
 * Der Schutz aus FA-43 bleibt trotzdem gewahrt. Er richtet sich gegen
 * *versehentliche* Bedienung — die droht beim flächigen Tippen, nicht bei
 * einem 44 Pixel großen Ziel in der Ecke. Wer auch das nicht will, blendet das
 * Zahnrad in den Einstellungen aus; dann bleibt nur der lange Druck.
 */
function openSettings() {
  router.push('/settings')
}

function flash(message: string) {
  if (toastTimer) clearTimeout(toastTimer)
  toast.value = message || null
  if (message) toastTimer = setTimeout(() => (toast.value = null), 2200)
}

async function excludeCurrent() {
  if (await show.excludeCurrent()) flash(t('slideshow.excluded'))
}

onMounted(async () => {
  await show.start(
    () => cfg.value?.intervalSeconds ?? 30,
    () => active.value,
  )
})

onBeforeUnmount(() => {
  show.dispose()
  if (toastTimer) clearTimeout(toastTimer)
})
</script>

<template>
  <div
    class="slideshow"
    @pointerdown="gestures.down($event.clientX, $event.clientY)"
    @pointermove="gestures.move($event.clientX, $event.clientY)"
    @pointerup="gestures.up($event.clientX, $event.clientY, ($event.currentTarget as HTMLElement).clientWidth)"
    @pointercancel="gestures.cancel()"
    @contextmenu.prevent
  >
    <SlideStage
      v-if="cfg"
      :slide="show.slide"
      :fit-mode="cfg.fitMode"
      :transition-enabled="cfg.transition.enabled"
      :transition-ms="cfg.transition.durationMs"
      :ken-burns="cfg.kenBurns"
    />

    <!-- Verlauf für die Lesbarkeit der Einblendungen. Nur dort, wo etwas
         steht — ein Bild ohne Einblendungen bleibt unangetastet. -->
    <div
      v-if="cfg && (cfg.overlays.showClock || cfg.overlays.showDate || cfg.overlays.showFileName || cfg.overlays.showTakenAt)"
      class="scrim"
      aria-hidden="true"
    />

    <template v-if="cfg && show.hasImages">
      <ClockOverlay
        :show-clock="cfg.overlays.showClock"
        :show-date="cfg.overlays.showDate"
        :clock-style="cfg.overlays.clockStyle"
        :pixel-shift="cfg.overlays.pixelShift"
        :language="cfg.language"
      />
      <CaptionOverlay
        :entry="currentEntry"
        :source-name="currentSourceName"
        :show-file-name="cfg.overlays.showFileName"
        :show-taken-at="cfg.overlays.showTakenAt"
        :pixel-shift="cfg.overlays.pixelShift"
        :language="cfg.language"
      />
    </template>

    <!-- Leerer Zustand: Erstinbetriebnahme soll ohne Anleitung gelingen (NF-08). -->
    <div v-if="isEmpty" class="empty">
      <h1 class="ss-wordmark">{{ t('app.name') }}</h1>
      <p class="empty-title">{{ t('slideshow.empty') }}</p>
      <p class="empty-hint">{{ t('slideshow.emptyHint') }}</p>
      <button class="empty-action" @click="router.push('/settings')">
        {{ t('slideshow.addSource') }}
      </button>
    </div>

    <!-- Nachtmodus liegt über allem (FA-54). -->
    <NightClock
      v-if="!active && nightClock && cfg"
      :resume-at="cfg.schedule.activeFrom"
      :clock-style="cfg.schedule.nightClockStyle"
      :pixel-shift="cfg.overlays.pixelShift"
    />

    <!-- Pausenzustand und kurze Rückmeldungen stehen übereinander, damit
         sich beides nicht überdeckt, wenn während der Pause ein Bild
         ausgeblendet wird. -->
    <div class="top-stack">
      <Transition name="fade">
        <div v-if="paused" class="paused" :style="{ transform: pausedShift }" role="status">
          <svg width="13" height="15" viewBox="0 0 12 14" aria-hidden="true">
            <rect x="0" y="0" width="4" height="14" rx="1.4" fill="currentColor" />
            <rect x="8" y="0" width="4" height="14" rx="1.4" fill="currentColor" />
          </svg>
          {{ t('slideshow.paused') }}
        </div>
      </Transition>
      <Transition name="fade">
        <div v-if="toast" class="toast">{{ toast }}</div>
      </Transition>
    </div>

    <!-- Schaltflächen oben rechts. Beide einzeln abschaltbar wie Uhr und
         Datum (FA-07), beide im Entwurf nicht vorgesehen — wer den Rahmen
         puristisch will, blendet sie aus.

         `@pointerdown.stop` ist nötig, nicht nur `@click.stop`: die
         Gestenerkennung des Containers hört auf Pointer-Ereignisse, und die
         laufen vor dem Klick. Ohne das schaltete ein Tipp auf die Schaltfläche
         zusätzlich ein Bild weiter, und langes Drücken öffnete die
         Einstellungen. -->
    <button
      v-if="!isEmpty && cfg?.overlays.showSettingsButton"
      class="corner-button settings-hint"
      :title="t('slideshow.openSettings')"
      :aria-label="t('slideshow.openSettings')"
      @pointerdown.stop
      @pointerup.stop
      @click.stop="openSettings"
    >
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="3.2" />
        <path d="M19.2 14.6a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.55v.17a2 2 0 0 1-4 0v-.09a1.7 1.7 0 0 0-1.11-1.55 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.55-1.03H2.6a2 2 0 0 1 0-4h.09A1.7 1.7 0 0 0 4.24 8.4a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34h.08A1.7 1.7 0 0 0 9.71 2.5v-.17a2 2 0 0 1 4 0v.09a1.7 1.7 0 0 0 1.03 1.55 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87v.08a1.7 1.7 0 0 0 1.55 1.03h.17a2 2 0 0 1 0 4h-.09a1.7 1.7 0 0 0-1.55 1.03z" />
      </svg>
    </button>

    <button
      v-if="show.hasImages && active && cfg?.overlays.showExcludeButton"
      class="corner-button exclude-hint"
      :class="{ alone: !cfg?.overlays.showSettingsButton }"
      :aria-label="t('slideshow.excludeImage')"
      :title="t('slideshow.excludeImage')"
      @pointerdown.stop
      @pointerup.stop
      @click.stop="excludeCurrent"
    >
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M17.94 17.94A10.1 10.1 0 0 1 12 20c-7 0-10-8-10-8a18.5 18.5 0 0 1 5.06-5.94M9.9 4.24A9.1 9.1 0 0 1 12 4c7 0 10 8 10 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" />
        <path d="M2 2 L22 22" />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.slideshow {
  position: relative;
  width: 100%;
  height: 100%;
  background: var(--ss-bg);
  overflow: hidden;
  cursor: none;
}

.scrim {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 220px;
  background: linear-gradient(180deg, rgba(5, 5, 6, 0) 0%, rgba(5, 5, 6, 0.55) 100%);
  pointer-events: none;
}

.empty {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  text-align: center;
  padding: 32px;
}

.empty .ss-wordmark {
  font-size: 42px;
  margin-bottom: 12px;
}

.empty-title {
  font-size: 17px;
  color: var(--ss-text-strong);
}

.empty-hint {
  font-size: 14px;
  color: var(--ss-text-dim);
  max-width: 32ch;
}

.empty-action {
  margin-top: 18px;
  padding: 0 26px;
  border: 1px solid var(--ss-accent);
  border-radius: var(--ss-radius-pill);
  color: var(--ss-accent);
  font-size: 15px;
  font-weight: 500;
  transition: background var(--ss-transition), color var(--ss-transition);
}

.empty-action:active {
  background: var(--ss-accent);
  color: var(--ss-bg);
}

.top-stack {
  position: absolute;
  top: 32px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  pointer-events: none;
  z-index: 20;
}

/* Messing statt Off-White: das Abzeichen meldet einen Zustand, keine Meldung.
   Dieselbe Rolle hat die Farbe in der Zeitplan-Anzeige der Einstellungen. */
.paused {
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 9px 20px;
  background: rgba(10, 10, 10, 0.82);
  border: 1px solid var(--ss-border-strong);
  border-radius: var(--ss-radius-pill);
  color: var(--ss-accent);
  font-size: 12px;
  font-weight: 500;
  letter-spacing: 0.2em;
  text-transform: uppercase;
  /* Wie bei den übrigen Einblendungen: langsam genug, um nicht als Bewegung
     wahrgenommen zu werden (NF-07). */
  transition: transform 4s ease-in-out;
}

.toast {
  padding: 10px 22px;
  background: rgba(10, 10, 10, 0.82);
  border: 1px solid var(--ss-border-strong);
  border-radius: var(--ss-radius-pill);
  color: var(--ss-text-body);
  font-size: 14px;
  letter-spacing: 0.04em;
}

.corner-button {
  position: absolute;
  top: 24px;
  width: var(--ss-touch-target);
  height: var(--ss-touch-target);
  min-height: var(--ss-touch-target);
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid transparent;
  border-radius: var(--ss-radius-pill);
  color: rgba(242, 239, 233, 0.28);
  transition: color var(--ss-transition), border-color var(--ss-transition);
  z-index: 15;
}

.settings-hint {
  right: 24px;
}

.exclude-hint {
  right: calc(24px + var(--ss-touch-target) + 8px);
}

/* Ohne Zahnrad rückt das Auge an dessen Platz. */
.exclude-hint.alone {
  right: 24px;
}

.corner-button:active {
  color: var(--ss-accent);
  border-color: var(--ss-border-strong);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
