<script setup lang="ts">
/**
 * Bild-Browser (E-25).
 *
 * Bis hierher zeigte die Ausschlussliste (FA-30) nur Dateinamen — bei tausend
 * Bildern unbrauchbar. Das Raster zeigt, was im Cache liegt, und lässt Bilder
 * ausschließen und zurückholen.
 *
 * ## Warum seitenweise geladen wird
 *
 * Der Cache ist auf 5 000 Bilder ausgelegt (Abnahmekriterium 5.2). Alle
 * Einträge auf einmal durch die IPC-Brücke zu schicken wäre gut anderthalb
 * Megabyte JSON — genau die Last, gegen die R-03 gerichtet ist. Nachgeladen
 * wird, wenn der Fußpunkt in Sicht kommt.
 *
 * ## Warum die Bilder trotzdem nicht den Speicher sprengen
 *
 * `loading="lazy"` und `content-visibility: auto` halten die Zahl gleichzeitig
 * dekodierter Bilder klein, unabhängig davon, wie weit gescrollt wurde.
 *
 * Wie weit „lazy" reicht, ist allerdings Chromiums Entscheidung und nicht
 * unsere: am Gerät gemessen lädt es beim Öffnen die Vorschau der gesamten
 * Seite von 200 Einträgen, nicht nur die sichtbaren sechs. Das ist tragbar —
 * 200 Vorschaubilder wiegen 4,2 MB — aber es ist nicht das, was der Name
 * verspricht. Die Obergrenze ergibt sich deshalb aus `PAGE_SIZE`, nicht aus
 * der Sichtbarkeit.
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import * as api from '@/lib/api'
import { thumbUrl } from '@/lib/api'
import { formatBytes, formatTakenAt, stripExtension } from '@/lib/format'
import { localeTag } from '@/lib/i18n'
import { useConfigStore } from '@/stores/config'
import type { CacheEntry, ImageFilter } from '@/lib/types'

const { t } = useI18n()
const store = useConfigStore()

/** Wie viele Einträge je Abruf. Deckt sich mit `PAGE_LIMIT` in `commands.rs`. */
const PAGE_SIZE = 200

const FILTERS: ImageFilter[] = ['all', 'included', 'excluded']

const filter = ref<ImageFilter>('all')
const entries = ref<CacheEntry[]>([])
const total = ref(0)
const loading = ref(false)
const sentinel = ref<HTMLElement | null>(null)

const locale = computed(() => localeTag(store.config?.language ?? 'auto'))
const done = computed(() => entries.value.length >= total.value)

async function loadPage(reset = false) {
  if (loading.value) return
  loading.value = true
  try {
    const offset = reset ? 0 : entries.value.length
    const page = await api.imagePage(offset, PAGE_SIZE, filter.value)
    entries.value = reset ? page.entries : [...entries.value, ...page.entries]
    total.value = page.total
  } finally {
    loading.value = false
  }
}

/**
 * Schaltet ein Bild aus der Diashow oder zurück (FA-30).
 *
 * Der Eintrag wird an Ort und Stelle geändert statt die Seite neu zu laden:
 * ein Neuladen spränge im Filter „Ausgeblendet" beim Zurückholen an den
 * Anfang, und man verlöre die Stelle, an der man gerade war.
 */
async function toggle(entry: CacheEntry) {
  if (entry.excluded) {
    await api.includeImage(entry.id)
  } else {
    await api.excludeImage(entry.id)
  }
  entry.excluded = !entry.excluded

  // Im gefilterten Bild gehört der Eintrag jetzt nicht mehr dazu.
  if (filter.value !== 'all') {
    entries.value = entries.value.filter((e) => e.id !== entry.id)
    total.value = Math.max(0, total.value - 1)
  }
}

function caption(entry: CacheEntry): string {
  const taken = formatTakenAt(entry.takenAt, locale.value)
  return taken || stripExtension(entry.fileName)
}

let observer: IntersectionObserver | null = null

onMounted(async () => {
  await loadPage(true)

  // Nachladen, sobald der Fußpunkt in Sicht kommt. Ein Scroll-Listener täte es
  // auch, liefe aber bei jedem Pixel — auf einem Tablet unnötig teuer (NF-06).
  observer = new IntersectionObserver((records) => {
    if (records.some((r) => r.isIntersecting) && !done.value) void loadPage()
  })
  if (sentinel.value) observer.observe(sentinel.value)
})

onBeforeUnmount(() => observer?.disconnect())

watch(filter, () => void loadPage(true))
</script>

<template>
  <div class="pane ss-scroll">
    <div class="head">
      <div class="ss-segmented">
        <button
          v-for="f in FILTERS"
          :key="f"
          class="ss-segment"
          :class="{ active: filter === f }"
          @click="filter = f"
        >
          {{ t(`images.filter.${f}`) }}
        </button>
      </div>
      <span class="count">{{ t('images.count', { n: total }) }}</span>
    </div>

    <p v-if="!loading && total === 0" class="muted">{{ t('images.empty') }}</p>

    <div class="grid">
      <button
        v-for="entry in entries"
        :key="entry.id"
        class="cell"
        :class="{ excluded: entry.excluded }"
        :title="entry.fileName"
        @click="toggle(entry)"
      >
        <img :src="thumbUrl(entry.id)" :alt="entry.fileName" loading="lazy" decoding="async" />
        <span class="label">{{ caption(entry) }}</span>
        <span v-if="entry.excluded" class="badge">{{ t('images.hidden') }}</span>
      </button>
    </div>

    <div ref="sentinel" class="sentinel">
      <span v-if="loading">{{ t('images.loading') }}</span>
      <span v-else-if="done && total > 0" class="muted">
        {{ t('images.allLoaded', { bytes: formatBytes(store.stats?.thumbBytes ?? 0) }) }}
      </span>
    </div>
  </div>
</template>

<style scoped>
.pane {
  height: 100%;
  padding-right: 8px;
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 18px;
}

.count {
  font-size: 13px;
  color: var(--ss-text-dim);
  font-variant-numeric: tabular-nums;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(128px, 1fr));
  gap: 10px;
}

.cell {
  position: relative;
  display: block;
  padding: 0;
  aspect-ratio: 1;
  border-radius: var(--ss-radius-nav);
  overflow: hidden;
  background: var(--ss-surface);
  /* Überspringt Layout und Malen für alles, was gerade nicht sichtbar ist —
     der Grund, warum auch 5 000 Zellen flüssig bleiben (R-03, NF-03). */
  content-visibility: auto;
  contain-intrinsic-size: 128px;
}

.cell img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
  transition: opacity var(--ss-transition);
}

.cell.excluded img {
  opacity: 0.28;
}

.label {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  padding: 14px 8px 6px;
  font-size: 11px;
  color: var(--ss-text-body);
  text-align: left;
  background: linear-gradient(180deg, rgba(5, 5, 6, 0) 0%, rgba(5, 5, 6, 0.8) 100%);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.badge {
  position: absolute;
  top: 6px;
  right: 6px;
  padding: 2px 8px;
  border-radius: var(--ss-radius-pill);
  background: rgba(10, 10, 10, 0.82);
  color: var(--ss-accent);
  font-size: 10px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
}

.sentinel {
  display: flex;
  justify-content: center;
  padding: 24px 0;
  font-size: 13px;
  color: var(--ss-text-dim);
}

.muted {
  padding: 10px 0;
  font-size: 14px;
  color: var(--ss-text-dim);
}
</style>
