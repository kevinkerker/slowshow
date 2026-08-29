<script setup lang="ts">
/**
 * Quellenverwaltung (Artboard „Einstellungen · Quellen").
 *
 * Liste, Hinzufügen-Feld und die Cache-Fußzeile — genau der Aufbau aus dem
 * Entwurf.
 */
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import SourceCard from '../SourceCard.vue'
import SourceDialog from '../SourceDialog.vue'
import { useConfigStore } from '@/stores/config'
import { formatBytes } from '@/lib/format'
import { localeTag } from '@/lib/i18n'
import type { Source, SyncReport } from '@/lib/types'

const { t } = useI18n()
const store = useConfigStore()

const dialogOpen = ref(false)
const editing = ref<Source | null>(null)
const notice = ref<string | null>(null)
const saveError = ref('')

const stats = computed(() => store.stats)
const locale = computed(() => localeTag(store.config?.language ?? 'auto'))

const cacheFill = computed(() => {
  if (!stats.value || stats.value.maxBytes === 0) return 0
  return Math.min(100, (stats.value.bytes / stats.value.maxBytes) * 100)
})

function openAdd() {
  editing.value = null
  saveError.value = ''
  dialogOpen.value = true
}

function openEdit(source: Source) {
  editing.value = source
  saveError.value = ''
  dialogOpen.value = true
}

async function onSave(source: Source, password: string | undefined) {
  saveError.value = ''
  try {
    const exists = store.sources.some((s) => s.id === source.id)
    if (exists) await store.updateSource(source, password)
    else await store.addSource(source, password)
    dialogOpen.value = false
  } catch (e) {
    // Ohne diesen Zweig blieb der Dialog bei einem Fehler einfach offen
    // stehen — ohne Meldung, ohne Hinweis, was schiefging.
    saveError.value = e instanceof Error ? e.message : String(e)
  }
}

async function toggle(source: Source, enabled: boolean) {
  await store.updateSource({ ...source, enabled })
}

async function remove(source: Source) {
  if (!confirm(t('sources.removeConfirm', { name: source.name }))) return
  await store.removeSource(source.id)
}

/** Loeschen aus dem Dialog heraus. */
async function onRemove(id: string) {
  const source = store.sources.find((s) => s.id === id)
  if (!source) return
  if (!confirm(t('sources.removeConfirm', { name: source.name }))) return
  try {
    await store.removeSource(id)
    dialogOpen.value = false
  } catch (e) {
    saveError.value = e instanceof Error ? e.message : String(e)
  }
}

async function sync(source: Source) {
  const report = await store.syncSource(source.id)
  notice.value = describe(report)
  setTimeout(() => (notice.value = null), 6000)
}

/** Ergebnis eines Sync-Laufs in einem Satz. */
function describe(report: SyncReport | null): string {
  if (!report) return store.error ? t('sources.syncFailed', { error: store.error }) : ''
  if (report.error) return t('sources.syncFailed', { error: report.error })
  if (report.truncated) return t('sources.syncTruncated')
  const parts: string[] = []
  if (report.added + report.updated + report.removed === 0) {
    parts.push(t('sources.syncNothing'))
  } else {
    parts.push(
      t('sources.syncResult', {
        added: report.added,
        updated: report.updated,
        removed: report.removed,
      }),
    )
  }
  // Uebersprungene und fehlgeschlagene Dateien mit ausweisen. Ohne das meldete
  // ein Lauf, bei dem jede einzelne Datei scheiterte, froehlich
  // "Keine Aenderungen" — genau so blieb der IPC-Fehler lange unbemerkt.
  if (report.skipped > 0) parts.push(t('sources.syncSkipped', { n: report.skipped }))
  if (report.failed > 0) parts.push(t('sources.syncFailedCount', { n: report.failed }))
  return parts.join(' · ')
}
</script>

<template>
  <div class="pane">
    <div class="list ss-scroll">
      <SourceCard
        v-for="source in store.sources"
        :key="source.id"
        :source="source"
        :photo-count="store.counts[source.id] ?? 0"
        :syncing="store.activeSyncSourceId === source.id"
        :progress="store.progress?.sourceId === source.id ? store.progress : null"
        @toggle="(v) => toggle(source, v)"
        @edit="openEdit(source)"
        @sync="sync(source)"
        @remove="remove(source)"
      />

      <p v-if="store.sources.length === 0" class="empty">{{ t('sources.empty') }}</p>

      <button class="add" @click="openAdd">
        <svg width="18" height="18" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <path d="M10 4 L10 16 M4 10 L16 10" />
        </svg>
        <span>{{ t('sources.add') }}</span>
      </button>

      <p v-if="notice" class="notice">{{ notice }}</p>
    </div>

    <!-- Fußzeile mit Cache-Auslastung, wie im Entwurf. -->
    <footer v-if="stats" class="cache">
      <span class="cache-text">
        {{ t('system.cacheUsage', {
          used: formatBytes(stats.bytes, locale),
          total: formatBytes(stats.maxBytes, locale),
        }) }}
      </span>
      <span class="bar">
        <span class="fill" :style="{ width: `${cacheFill}%` }" />
      </span>
      <span class="cache-text">
        {{ t('system.prefetch') }}: {{ store.config?.cache.prefetchCount ?? 0 }}
      </span>
    </footer>

    <SourceDialog
      v-if="dialogOpen"
      :source="editing"
      :save-error="saveError"
      @save="onSave"
      @remove="onRemove"
      @cancel="dialogOpen = false"
    />
  </div>
</template>

<style scoped>
.pane {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.list {
  display: flex;
  flex-direction: column;
  gap: 16px;
  flex-grow: 1;
  min-height: 0;
  padding-bottom: 8px;
}

.empty {
  padding: 28px 0;
  text-align: center;
  font-size: 15px;
  color: var(--ss-text-dim);
}

.add {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 18px;
  border: 1px dashed var(--ss-border-dashed);
  border-radius: var(--ss-radius-card);
  color: var(--ss-text-muted);
  font-size: 15px;
  font-weight: 500;
  transition: color var(--ss-transition), border-color var(--ss-transition);
}

.add:active {
  color: var(--ss-accent);
  border-color: var(--ss-accent);
}

.notice {
  padding: 12px 4px 0;
  font-size: 13px;
  color: var(--ss-text-dim);
}

.cache {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-top: auto;
  padding-top: 18px;
  border-top: 1px solid var(--ss-border-soft);
  flex-shrink: 0;
}

.cache-text {
  font-size: 13px;
  color: var(--ss-text-dim);
  white-space: nowrap;
}

.bar {
  flex-grow: 1;
  height: 3px;
  border-radius: var(--ss-radius-pill);
  background: var(--ss-border);
  overflow: hidden;
}

.fill {
  display: block;
  height: 100%;
  background: var(--ss-accent);
  transition: width var(--ss-transition);
}
</style>
