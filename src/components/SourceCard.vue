<script setup lang="ts">
/**
 * Eine Quelle in der Liste (Artboard „Einstellungen · Quellen").
 *
 * Symbol, Name, Statuszeile und Schalter — der Schalter steuert direkt, ob die
 * Quelle in die Diashow einfließt (FA-25).
 */
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import ToggleSwitch from './ToggleSwitch.vue'
import { formatRelativeTime } from '@/lib/format'
import type { Source, SyncProgress } from '@/lib/types'

const props = defineProps<{
  source: Source
  photoCount: number
  syncing: boolean
  syncingPath?: string
  /** Zwischenstand, wenn diese Quelle gerade laeuft. */
  progress?: SyncProgress | null
}>()

const emit = defineEmits<{
  toggle: [enabled: boolean]
  edit: []
  sync: []
  remove: []
}>()

const { t } = useI18n()

const enabled = computed({
  get: () => props.source.enabled,
  set: (v: boolean) => emit('toggle', v),
})

/**
 * Statuszeile — im Entwurf trägt jede Quelle eine andere Information:
 * lokal die Anzahl, das NAS den letzten Sync, Nextcloud den Preview-Hinweis.
 * Hier zusammengesetzt aus dem, was für die jeweilige Quelle zutrifft.
 */
const status = computed(() => {
  if (props.syncing) {
    // Entfernte Quellen melden Zaehlerstaende, lokale den aktuellen Pfad.
    if (props.progress && props.progress.total > 0) {
      return `${t('sources.syncProgress', {
        done: props.progress.done,
        total: props.progress.total,
      })} · ${shortPath(props.progress.current)}`
    }
    if (props.syncingPath) return t('sources.scanning', { path: shortPath(props.syncingPath) })
    return props.progress ? t('sources.syncListing') : t('sources.syncing')
  }

  const parts: string[] = []
  const kind = props.source.kind

  if (kind.type === 'local') {
    parts.push(t('sources.photos', { n: props.photoCount }))
    parts.push(t('sources.localFolder'))
  } else {
    parts.push(kind.type === 'nextcloud' ? t('sources.nextcloud') : t('sources.nas'))
    parts.push(
      t('sources.lastSync', {
        when: formatRelativeTime(props.source.lastSync, new Date(), t),
      }),
    )
    parts.push(t('sources.photosCached', { n: props.photoCount }))
    if (kind.type === 'nextcloud' && kind.usePreviewApi) {
      parts.push(t('sources.previewActive'))
    }
  }

  if (!props.source.enabled) parts.push(t('sources.excludedFromShow'))
  return parts.join(' · ')
})

/** Lange Pfade auf das Wesentliche kuerzen — die Zeile ist einzeilig. */
function shortPath(path: string): string {
  const name = path.split('/').pop() ?? path
  return name.length > 40 ? `${name.slice(0, 37)}…` : name
}

/** Anteil des Fortschritts fuer den Balken, 0..1. */
const fraction = computed(() => {
  const p = props.progress
  if (!props.syncing || !p || p.total === 0) return 0
  return Math.min(1, p.done / p.total)
})

const iconPath = computed(() => {
  switch (props.source.kind.type) {
    // Ordner
    case 'local':
      return 'M3 7 L3 18 A2 2 0 0 0 5 20 L19 20 A2 2 0 0 0 21 18 L21 9 A2 2 0 0 0 19 7 L12 7 L10 4.5 L5 4.5 A2 2 0 0 0 3 6.5 Z'
    // Wolke
    case 'nextcloud':
      return 'M7 18 A4 4 0 0 1 7 10 A5.5 5.5 0 0 1 17.5 11.5 A3.5 3.5 0 0 1 17 18 Z'
    default:
      return ''
  }
})
</script>

<template>
  <div class="card" :class="{ dimmed: !source.enabled }">
    <div class="icon">
      <!-- NAS: zwei gestapelte Einschübe, wie im Entwurf. -->
      <svg
        v-if="source.kind.type === 'webDav'"
        width="22"
        height="22"
        viewBox="0 0 24 24"
        fill="none"
        stroke="var(--ss-accent)"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <rect x="4" y="4" width="16" height="7" rx="1.5" />
        <rect x="4" y="13" width="16" height="7" rx="1.5" />
        <path d="M7.5 7.5 L7.5 7.51 M7.5 16.5 L7.5 16.51" />
      </svg>
      <svg
        v-else
        width="22"
        height="22"
        viewBox="0 0 24 24"
        fill="none"
        stroke="var(--ss-accent)"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path :d="iconPath" />
      </svg>
    </div>

    <div class="body">
      <div class="name">{{ source.name }}</div>
      <div class="status">{{ status }}</div>
      <!-- Schmaler Balken statt Zahlenkolonne: zeigt auf einen Blick, ob es
           vorangeht (der eigentliche Zweck waehrend eines langen Laufs). -->
      <div v-if="syncing" class="progress">
        <div class="progress-fill" :class="{ indeterminate: fraction === 0 }" :style="fraction > 0 ? { width: `${fraction * 100}%` } : undefined" />
      </div>
    </div>

    <div class="actions">
      <button
        class="action"
        :disabled="syncing"
        :aria-label="t('sources.syncNow')"
        :title="t('sources.syncNow')"
        @click="emit('sync')"
      >
        <svg
          width="18"
          height="18"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          :class="{ spinning: syncing }"
        >
          <path d="M20 11 A8 8 0 1 0 18.5 16" />
          <path d="M20 5 L20 11 L14.5 11" />
        </svg>
      </button>

      <button
        class="action"
        :aria-label="t('sources.edit')"
        :title="t('sources.edit')"
        @click="emit('edit')"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M4 20 L4 16 L16 4 L20 8 L8 20 Z" />
        </svg>
      </button>

      <ToggleSwitch v-model="enabled" :label="source.name" />
    </div>
  </div>
</template>

<style scoped>
.card {
  display: flex;
  align-items: center;
  gap: 20px;
  padding: 22px 24px;
  border: 1px solid var(--ss-border);
  border-radius: var(--ss-radius-card);
  background: var(--ss-surface);
  transition: opacity var(--ss-transition);
}

/* Abgeschaltete Quellen bleiben lesbar, treten aber zurück. */
.card.dimmed {
  opacity: 0.62;
}

.icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 48px;
  height: 48px;
  flex-shrink: 0;
  border-radius: 10px;
  background: var(--ss-surface-accent);
}

.body {
  display: flex;
  flex-direction: column;
  gap: 3px;
  flex-grow: 1;
  min-width: 0;
}

.name {
  font-size: 16px;
  font-weight: 500;
  color: var(--ss-text-strong);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.progress {
  height: 2px;
  margin-top: 8px;
  border-radius: var(--ss-radius-pill);
  background: var(--ss-border);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--ss-accent);
  transition: width var(--ss-transition);
}

/* Solange die Gesamtzahl unbekannt ist (Quelle wird noch gelistet), laeuft
   ein Streifen — sonst stuende der Balken minutenlang auf null. */
.progress-fill.indeterminate {
  width: 35%;
  animation: sweep 1.4s ease-in-out infinite;
}

@keyframes sweep {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(340%); }
}

.status {
  font-size: 13px;
  color: var(--ss-text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

.action {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--ss-touch-target);
  height: var(--ss-touch-target);
  border-radius: var(--ss-radius-pill);
  color: var(--ss-text-muted);
  transition: color var(--ss-transition), background var(--ss-transition);
}

.action:active:not(:disabled) {
  color: var(--ss-accent);
  background: var(--ss-surface-accent);
}

.action:disabled {
  opacity: 0.5;
  cursor: default;
}

.spinning {
  animation: spin 1.1s linear infinite;
  transform-origin: center;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
