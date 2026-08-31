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
import { imageUrl, thumbUrl } from '@/lib/api'
import { formatBytes, formatTakenAt, stripExtension } from '@/lib/format'
import { localeTag } from '@/lib/i18n'
import { useConfigStore } from '@/stores/config'
import type { CacheEntry, ImageFilter } from '@/lib/types'

const { t } = useI18n()
const store = useConfigStore()

/** Wie viele Einträge je Abruf. Deckt sich mit `PAGE_LIMIT` in `commands.rs`. */
const PAGE_SIZE = 200

const FILTERS: ImageFilter[] = [
  'all',
  'included',
  'excluded',
  'quarantine',
  // Wartung F4: die Gegenprobe zur Statistik — wer „noch nie gezeigt: 214"
  // liest, will sehen, welche das sind.
  'neverShown',
]

const filter = ref<ImageFilter>('all')

const emit = defineEmits<{ openSource: [id: string] }>()

// ── Verweis auf die Freigabeliste (F4, E-32) ─────────────────────────────────
// Verwaltet wird sie im Postfach-Dialog; hier steht nur, wie viele Absender
// freigegeben sind, mit einem Weg dorthin. Grund: freigegeben wird in diesem
// Bereich, nachgesehen aber bei der Quelle — ohne diesen Verweis fände den
// Zusammenhang niemand.
const mailSource = computed(() => store.sources.find((s) => s.kind.type === 'mail') ?? null)
const senderCount = ref(0)

async function loadSenderCount() {
  const source = mailSource.value
  if (!source) {
    senderCount.value = 0
    return
  }
  try {
    senderCount.value = (await api.allowedSenders(source.id)).length
  } catch {
    // Nebensache: der Bild-Browser muss auch ohne diese Zahl brauchbar sein.
    senderCount.value = 0
  }
}
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
 * Gibt ein wartendes Foto frei (F4, E-31).
 *
 * Zwei Wege, weil zwei Fälle gemeint sind: Ein Tipp gibt genau dieses Bild
 * frei. Der Knopf „Absender vertrauen" nimmt die Adresse dauerhaft auf und
 * holt alle wartenden Fotos derselben Person mit — das ist der eigentliche
 * Vorgang, wenn die Tante zum ersten Mal schickt.
 */
/**
 * Wartendes Foto, ueber das gerade entschieden wird (F4, E-35).
 *
 * Bis hierher gab ein Tipp das Bild sofort frei — und „Absender vertrauen"
 * war ueberhaupt nicht erreichbar, obwohl der Rust-Befehl es seit jeher kann.
 * Die Freigabeliste konnte sich deshalb nie fuellen.
 */
const releasing = ref<CacheEntry | null>(null)

async function confirmRelease(trustSender: boolean) {
  const entry = releasing.value
  if (!entry) return
  releasing.value = null
  await release(entry, trustSender)
  // Nach einem „alle von …" stimmt die Zahl der freigegebenen Absender nicht
  // mehr; der Verweis daneben soll sie sofort richtig zeigen.
  if (trustSender) void loadSenderCount()
}

async function release(entry: CacheEntry, trustSender: boolean) {
  const freed = await api.releaseQuarantine(entry.id, trustSender)
  if (freed > 0) {
    entries.value = entries.value.filter(
      (e) => e.id !== entry.id && !(trustSender && e.mail?.sender === entry.mail?.sender),
    )
    total.value = Math.max(0, total.value - freed)
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
  // In der Quarantaene heisst Tippen "freigeben", nicht "ausblenden" -- ein
  // wartendes Bild auszublenden waere die Handlung, die niemand sucht.
  if (entry.mail?.quarantined) {
    // Nicht sofort freigeben: erst zeigen, von wem das Bild kommt (E-35).
    releasing.value = entry
    return
  }
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

/**
 * Absender einer wartenden Kachel — nur in der Quarantaene.
 *
 * Dort ist „von wem" die Frage, die zur Entscheidung fuehrt; das
 * Aufnahmedatum allein half nicht weiter (E-35).
 */
function senderOf(entry: CacheEntry): string | null {
  if (!entry.mail?.quarantined) return null
  return entry.mail.sender || null
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

watch(filter, () => {
  void loadPage(true)
  // Erst beim Umschalten auf die Quarantaene laden: sonst fragte jeder
  // Aufruf des Bild-Browsers eine Zahl ab, die dort niemand sieht.
  if (filter.value === 'quarantine') void loadSenderCount()
})
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

    <p v-if="filter === 'quarantine' && total > 0" class="hint">
      {{ t('images.quarantineHint') }}
    </p>

    <button
      v-if="filter === 'quarantine' && mailSource"
      class="senders-link"
      @click="emit('openSource', mailSource.id)"
    >
      {{ t('images.allowedSenders', { n: senderCount }, senderCount) }}
      <span aria-hidden="true">›</span>
    </button>

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
        <span class="label">
          <span v-if="senderOf(entry)" class="sender">{{ senderOf(entry) }}</span>
          <span class="taken">{{ caption(entry) }}</span>
        </span>
        <span v-if="entry.excluded" class="badge">{{ t('images.hidden') }}</span>
        <span v-else-if="entry.mail?.quarantined" class="badge waiting">
          {{ t('images.waiting') }}
        </span>
      </button>
    </div>

    <!-- Freigabe-Dialog (F4, E-35). Zeigt zuerst, von wem das Bild kommt,
         und laesst dann zwischen diesem einen Bild und allen dieser Person
         waehlen. Vorher gab ein Tipp das Bild sofort frei, ohne dass der
         Absender ueberhaupt zu sehen war. -->
    <div v-if="releasing" class="backdrop" @click.self="releasing = null">
      <div class="release" role="dialog" aria-modal="true">
        <img
          class="preview"
          :src="imageUrl(releasing.id)"
          :alt="releasing.fileName"
          decoding="async"
        />
        <dl class="meta">
          <dt>{{ t('images.releaseFrom') }}</dt>
          <dd>{{ releasing.mail?.sender }}</dd>
          <dt>{{ t('images.releaseSubject') }}</dt>
          <dd>{{ releasing.mail?.subject || t('images.releaseNoSubject') }}</dd>
        </dl>
        <div class="actions">
          <button class="primary" @click="confirmRelease(false)">
            {{ t('images.releaseOne') }}
          </button>
          <button class="secondary" @click="confirmRelease(true)">
            {{ t('images.releaseSender', { sender: releasing.mail?.sender }) }}
          </button>
          <p class="trust-hint">{{ t('images.releaseSenderHint') }}</p>
          <button class="ghost" @click="releasing = null">{{ t('common.cancel') }}</button>
        </div>
      </div>
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

/* Zweizeilig in der Quarantaene: Absender oben, Datum darunter. In den
   uebrigen Filtern bleibt nur die zweite Zeile uebrig, die Kachel sieht dort
   also unveraendert aus. */
.label .sender {
  display: block;
  font-weight: 500;
  color: var(--ss-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.label .taken {
  display: block;
  opacity: 0.75;
}

/* ── Freigabe-Dialog (F4, E-35) ─────────────────────────────────────────── */

.backdrop {
  position: fixed;
  inset: 0;
  z-index: 40;
  display: grid;
  place-items: center;
  padding: 16px;
  background: rgba(0, 0, 0, 0.72);
}

.release {
  display: flex;
  flex-direction: column;
  gap: 16px;
  width: min(520px, 100%);
  max-height: 100%;
  overflow-y: auto;
  padding: 20px;
  border: 1px solid var(--ss-border-soft);
  border-radius: 16px;
  background: var(--ss-surface);
}

/* Begrenzt, damit ein Hochformat den Dialog nicht ueber den Schirm schiebt --
   die Schaltflaechen darunter muessen ohne Scrollen erreichbar bleiben. */
.release .preview {
  width: 100%;
  max-height: 38vh;
  object-fit: contain;
  border-radius: 10px;
  background: #000;
}

.release .meta {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 4px 12px;
  margin: 0;
  font-size: 14px;
}

.release .meta dt {
  color: var(--ss-text-dim);
}

.release .meta dd {
  margin: 0;
  overflow-wrap: anywhere;
}

.release .actions {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.release .actions button {
  padding: 12px 16px;
  border-radius: 999px;
  border: 1px solid transparent;
  font: inherit;
  font-size: 15px;
  cursor: pointer;
}

.release .actions .primary {
  background: var(--ss-accent);
  color: #14100a;
}

.release .actions .secondary {
  background: transparent;
  border-color: var(--ss-border-soft);
  color: var(--ss-text);
  /* Lange Adressen duerfen umbrechen -- die Beschriftung traegt den Absender,
     und abgeschnitten waere sie wertlos. */
  white-space: normal;
}

.release .actions .ghost {
  background: transparent;
  color: var(--ss-text-dim);
}

.release .trust-hint {
  margin: -4px 0 4px;
  font-size: 12px;
  line-height: 1.45;
  color: var(--ss-text-dim);
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

.senders-link {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin: 0 0 14px;
  padding: 0;
  border: none;
  background: none;
  font: inherit;
  font-size: 13px;
  color: var(--ss-accent);
  cursor: pointer;
}

.hint {
  margin: 0 0 14px;
  font-size: 13px;
  line-height: 1.5;
  color: var(--ss-text-dim);
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

/* Wartende Bilder tragen den Akzent, ausgeblendete bleiben stumm: das eine
   verlangt eine Handlung, das andere ist erledigt. */
.badge.waiting {
  color: var(--ss-accent);
  border: 1px solid var(--ss-border-strong);
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
