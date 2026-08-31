<script setup lang="ts">
/**
 * Einstellungen der Diashow: Takt, Reihenfolge, Darstellung, Einblendungen
 * (FA-02, FA-03, FA-05 bis FA-08, FA-10, FA-30, NF-07).
 */
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import SettingRow from '../SettingRow.vue'
import ToggleSwitch from '../ToggleSwitch.vue'
import { useConfigStore } from '@/stores/config'
import * as api from '@/lib/api'
import type { FilterFacets, PlaybackStats, TimeFilter } from '@/lib/types'
import { formatInterval, formatRelativeTime } from '@/lib/format'
import type { ClockStyle, FitMode, Orientation, PlayOrder } from '@/lib/types'

const { t } = useI18n()
const store = useConfigStore()

const cfg = computed(() => store.config)

// ── Durchlauf und Statistik (Wartung F1–F3, E-31) ───────────────────────────
// Nach E-31 kein eigener Navigationsbereich: die Zahlen stehen dort, wo die
// Wiedergabe eingestellt wird — wer sich fragt, warum ein Bild nicht kommt,
// sucht hier und nicht unter „System".
const stats = ref<PlaybackStats | null>(null)
const statsBusy = ref(false)
const notice = ref<string | null>(null)

async function loadStats() {
  try {
    stats.value = await api.playbackStats()
  } catch (e) {
    // Die Statistik ist eine Beigabe; ihr Fehlen darf die Einstellungen
    // darunter nicht unbedienbar machen.
    console.warn('Statistik nicht ladbar', e)
  }
}

/**
 * Wann ein Bild zuletzt lief — die Spalte der Liste „am laengsten nicht
 * gezeigt".
 *
 * Dort stand zuerst der Anzeigezaehler, und am Geraet las sich das als „0×"
 * neben jedem Eintrag: `show_count` ist juenger als der Bestand (E-29, mit
 * `serde(default)` nachgeruestet), vorhandene Bilder tragen also einen
 * Zeitpunkt ohne Zaehler. Vor allem aber ist die Frage dieser Liste „wann
 * zuletzt", nicht „wie oft" — danach ist sie schliesslich sortiert.
 */
function zuletzt(unix: number | null): string {
  return formatRelativeTime(unix, new Date(), t)
}

function melde(text: string) {
  notice.value = text
  setTimeout(() => (notice.value = null), 6000)
}

async function onRestartCycle() {
  statsBusy.value = true
  try {
    await api.restartCycle()
    await loadStats()
    melde(t('show.restartCycleDone'))
  } finally {
    statsBusy.value = false
  }
}

async function onResetHistory() {
  // Die Zahl steht schon in der Statistik — die Rueckfrage nennt sie, damit
  // niemand raten muss, wie viel er gerade verwirft.
  const betroffen = stats.value?.eligible ?? 0
  if (!confirm(t('show.resetHistoryAsk', { n: betroffen }))) return

  statsBusy.value = true
  try {
    const n = await api.resetHistory()
    await loadStats()
    melde(t('show.resetHistoryDone', { n }))
  } finally {
    statsBusy.value = false
  }
}

/**
 * Zulässige Anzeigedauern (FA-02: 5 Sekunden bis 30 Minuten).
 *
 * Feste Stufen statt eines freien Feldes — auf einem Touchgerät ist die
 * Auswahl schneller, und unsinnige Zwischenwerte entstehen gar nicht erst.
 */
const INTERVALS = [5, 10, 15, 30, 60, 120, 300, 600, 1800]

const ORDERS: PlayOrder[] = ['smart', 'random', 'fileName', 'chronological']
const FIT_MODES: FitMode[] = ['contain', 'cover']
const CLOCK_STYLES: ClockStyle[] = ['digital', 'analog']
const ORIENTATIONS: Orientation[] = ['landscape', 'portrait', 'auto']

// ── Auswahl der Bilder (F5) ──────────────────────────────────────────────────

/**
 * Jahre und Absender kommen aus dem Backend, nicht aus der Konfiguration:
 * welche es gibt, ergibt sich aus dem Bestand und aendert sich mit jedem Sync.
 */
const facets = ref<FilterFacets>({ years: [], senders: [], undated: 0 })

onMounted(async () => {
  void loadStats()
  facets.value = await api.filterFacets()
})

const QUICK: TimeFilter['type'][] = ['all', 'last12Months', 'thisYear']

function timeType(): TimeFilter['type'] {
  return cfg.value?.filter.time.type ?? 'all'
}

function selectedYears(): number[] {
  const t = cfg.value?.filter.time
  return t?.type === 'years' ? t.years : []
}

function setQuick(type: TimeFilter['type']) {
  store.patch((d) => {
    d.filter.time = { type } as TimeFilter
  })
}

/**
 * Ein Jahr zu- oder abwaehlen.
 *
 * Die leere Liste bedeutet "alle" -- wer das letzte Jahr abwaehlt, bekommt
 * also wieder alles statt einer leeren Diashow, aus der kein Weg zurueckfuehrt.
 */
function toggleYear(year: number) {
  store.patch((d) => {
    const current = d.filter.time.type === 'years' ? [...d.filter.time.years] : []
    const at = current.indexOf(year)
    if (at >= 0) current.splice(at, 1)
    else current.push(year)
    d.filter.time = { type: 'years', years: current.sort((a, b) => b - a) }
  })
}

function toggleSender(sender: string) {
  store.patch((d) => {
    const at = d.filter.senders.indexOf(sender)
    if (at >= 0) d.filter.senders.splice(at, 1)
    else d.filter.senders.push(sender)
  })
}

function orderLabel(order: PlayOrder): string {
  return t(
    {
      smart: 'show.orderSmart',
      random: 'show.orderRandom',
      fileName: 'show.orderFileName',
      chronological: 'show.orderChronological',
    }[order],
  )
}

</script>

<template>
  <div v-if="cfg" class="pane ss-scroll">
    <section>
      <SettingRow :label="t('show.interval')">
        <select
          :value="cfg.intervalSeconds"
          class="narrow"
          @change="store.patch((d) => (d.intervalSeconds = Number(($event.target as HTMLSelectElement).value)))"
        >
          <option v-for="s in INTERVALS" :key="s" :value="s">{{ formatInterval(s, t) }}</option>
        </select>
      </SettingRow>

      <SettingRow :label="t('show.order')">
        <select
          :value="cfg.order"
          class="wide"
          @change="store.patch((d) => (d.order = ($event.target as HTMLSelectElement).value as PlayOrder))"
        >
          <option v-for="o in ORDERS" :key="o" :value="o">{{ orderLabel(o) }}</option>
        </select>
      </SettingRow>

      <!-- Ganz oben, weil davon abhängt, wie alles darunter aussieht (E-26). -->
      <SettingRow :label="t('show.orientation')" :hint="t('show.orientationHint')">
        <div class="ss-segmented">
          <button
            v-for="o in ORIENTATIONS"
            :key="o"
            class="ss-segment"
            :class="{ active: cfg.orientation === o }"
            @click="store.patch((d) => (d.orientation = o))"
          >
            {{ t(`orientation.${o}`) }}
          </button>
        </div>
      </SettingRow>

      <!-- Auswahl der Bilder (F5). Steht vor der Reihenfolge: erst wird
           ausgewaehlt, dann gemischt. -->
      <SettingRow :label="t('filter.time')">
        <div class="ss-segmented">
          <button
            v-for="q in QUICK"
            :key="q"
            class="ss-segment"
            :class="{ active: timeType() === q }"
            @click="setQuick(q)"
          >
            {{ t(`filter.${q}`) }}
          </button>
        </div>
      </SettingRow>

      <SettingRow
        v-if="facets.years.length > 0"
        :label="t('filter.years')"
        :hint="t('filter.yearsHint')"
        stacked
      >
        <div class="chips">
          <button
            v-for="[year, count] in facets.years"
            :key="year"
            class="chip"
            :class="{ active: selectedYears().includes(year) }"
            @click="toggleYear(year)"
          >
            {{ year }} <span class="count">{{ count }}</span>
          </button>
        </div>
      </SettingRow>

      <SettingRow v-if="facets.senders.length > 0" :label="t('filter.senders')" stacked>
        <div class="chips">
          <button
            v-for="[sender, count] in facets.senders"
            :key="sender"
            class="chip"
            :class="{ active: cfg.filter.senders.includes(sender) }"
            @click="toggleSender(sender)"
          >
            {{ sender }} <span class="count">{{ count }}</span>
          </button>
        </div>
      </SettingRow>

      <SettingRow
        v-if="facets.undated > 0"
        :label="t('filter.undated', { n: facets.undated })"
        :hint="t('filter.undatedHint')"
      >
        <ToggleSwitch
          :model-value="cfg.filter.includeUndated"
          :label="t('filter.undated', { n: facets.undated })"
          @update:model-value="(v) => store.patch((d) => (d.filter.includeUndated = v))"
        />
      </SettingRow>

      <!-- Feineinstellungen der Ziehung (E-29). Nur sichtbar, wo sie wirken:
           ein Schalter, der nichts tut, ist schlimmer als keiner. -->
      <template v-if="cfg.order === 'smart'">
        <SettingRow :label="t('show.newBoost')" :hint="t('show.newBoostHint')">
          <ToggleSwitch
            :model-value="cfg.playback.newBoost"
            :label="t('show.newBoost')"
            @update:model-value="(v) => store.patch((d) => (d.playback.newBoost = v))"
          />
        </SettingRow>

        <SettingRow :label="t('show.leastRecentlyShown')">
          <ToggleSwitch
            :model-value="cfg.playback.leastRecentlyShown"
            :label="t('show.leastRecentlyShown')"
            @update:model-value="(v) => store.patch((d) => (d.playback.leastRecentlyShown = v))"
          />
        </SettingRow>

        <SettingRow :label="t('show.clusterFilter')" :hint="t('show.clusterFilterHint')">
          <ToggleSwitch
            :model-value="cfg.playback.clusterFilter"
            :label="t('show.clusterFilter')"
            @update:model-value="(v) => store.patch((d) => (d.playback.clusterFilter = v))"
          />
        </SettingRow>
      </template>

      <SettingRow v-if="cfg.order === 'chronological'" :label="t('show.direction')">
        <div class="ss-segmented">
          <button
            v-for="newest in [false, true]"
            :key="String(newest)"
            class="ss-segment"
            :class="{ active: cfg.playback.newestFirst === newest }"
            @click="store.patch((d) => (d.playback.newestFirst = newest))"
          >
            {{ newest ? t('show.directionNewest') : t('show.directionOldest') }}
          </button>
        </div>
      </SettingRow>

      <SettingRow
        :label="t('show.fitMode')"
        :hint="cfg.fitMode === 'contain' ? t('show.fitContainHint') : t('show.fitCoverHint')"
      >
        <div class="ss-segmented">
          <button
            v-for="mode in FIT_MODES"
            :key="mode"
            class="ss-segment"
            :class="{ active: cfg.fitMode === mode }"
            @click="store.patch((d) => (d.fitMode = mode))"
          >
            {{ mode === 'contain' ? t('show.fitContain') : t('show.fitCover') }}
          </button>
        </div>
      </SettingRow>
    </section>

    <!-- Wartung F1–F3 (E-31): Statistik und Durchlauf stehen bei der
         Wiedergabe, nicht in einem eigenen Bereich. -->
    <section v-if="stats">
      <h3 class="ss-label">{{ t('show.maintenance') }}</h3>

      <dl class="stats">
        <div>
          <dt>{{ t('show.statTotal') }}</dt>
          <dd>{{ stats.total }}</dd>
        </div>
        <div>
          <dt>{{ t('show.statEligible') }}</dt>
          <dd>{{ stats.eligible }}</dd>
        </div>
        <div>
          <dt>{{ t('show.statNeverShown') }}</dt>
          <dd>{{ stats.neverShown }}</dd>
        </div>
      </dl>

      <p class="progress-line">
        {{ t('show.statBag', { n: stats.bagRemaining, total: stats.eligible }) }}
        <span class="cycles">{{ t('show.statCycles', { n: stats.cycles }) }}</span>
      </p>

      <div v-if="stats.mostShown.length" class="tops">
        <div class="top">
          <h4 class="ss-label">{{ t('show.statMostShown') }}</h4>
          <ol>
            <li v-for="e in stats.mostShown" :key="e.id">
              <span class="name">{{ e.fileName }}</span>
              <span class="value">{{ t('show.statTimes', { n: e.showCount }) }}</span>
            </li>
          </ol>
        </div>
        <div class="top">
          <h4 class="ss-label">{{ t('show.statLongestUnseen') }}</h4>
          <ol>
            <li v-for="e in stats.longestUnseen" :key="e.id">
              <span class="name">{{ e.fileName }}</span>
              <span class="value">{{ zuletzt(e.lastShown) }}</span>
            </li>
          </ol>
        </div>
      </div>
      <p v-else class="muted">{{ t('show.statNone') }}</p>

      <SettingRow :label="t('show.restartCycle')" :hint="t('show.restartCycleHint')">
        <button class="secondary" :disabled="statsBusy" @click="onRestartCycle">
          {{ t('show.restartCycle') }}
        </button>
      </SettingRow>

      <SettingRow :label="t('show.resetHistory')" :hint="t('show.resetHistoryHint')">
        <button class="danger" :disabled="statsBusy" @click="onResetHistory">
          {{ t('show.resetHistory') }}
        </button>
      </SettingRow>

      <p v-if="notice" class="notice">{{ notice }}</p>
    </section>

    <section>
      <h3 class="ss-label">{{ t('show.title') }}</h3>

      <SettingRow :label="t('show.transition')">
        <ToggleSwitch
          :model-value="cfg.transition.enabled"
          :label="t('show.transition')"
          @update:model-value="(v) => store.patch((d) => (d.transition.enabled = v))"
        />
      </SettingRow>

      <SettingRow v-if="cfg.transition.enabled" :label="t('show.transitionDuration')">
        <div class="slider">
          <input
            type="range"
            min="200"
            max="4000"
            step="100"
            :value="cfg.transition.durationMs"
            @change="store.patch((d) => (d.transition.durationMs = Number(($event.target as HTMLInputElement).value)))"
          />
          <span class="value">{{ (cfg.transition.durationMs / 1000).toFixed(1) }} s</span>
        </div>
      </SettingRow>

      <SettingRow :label="t('show.kenBurns')" :hint="t('show.kenBurnsHint')">
        <ToggleSwitch
          :model-value="cfg.kenBurns"
          :label="t('show.kenBurns')"
          @update:model-value="(v) => store.patch((d) => (d.kenBurns = v))"
        />
      </SettingRow>

      <SettingRow
        :label="t('show.pairMode')"
        :hint="cfg.orientation === 'portrait' ? t('show.pairModeHintPortrait') : t('show.pairModeHint')"
      >
        <ToggleSwitch
          :model-value="cfg.pairMode"
          :label="t('show.pairMode')"
          @update:model-value="(v) => store.patch((d) => (d.pairMode = v))"
        />
      </SettingRow>
    </section>

    <section>
      <h3 class="ss-label">{{ t('show.overlays') }}</h3>

      <SettingRow :label="t('show.showClock')">
        <ToggleSwitch
          :model-value="cfg.overlays.showClock"
          :label="t('show.showClock')"
          @update:model-value="(v) => store.patch((d) => (d.overlays.showClock = v))"
        />
      </SettingRow>

      <SettingRow v-if="cfg.overlays.showClock" :label="t('show.clockStyle')">
        <div class="ss-segmented">
          <button
            v-for="style in CLOCK_STYLES"
            :key="style"
            class="ss-segment"
            :class="{ active: cfg.overlays.clockStyle === style }"
            @click="store.patch((d) => (d.overlays.clockStyle = style))"
          >
            {{ t(`clock.${style}`) }}
          </button>
        </div>
      </SettingRow>

      <SettingRow :label="t('show.showDate')">
        <ToggleSwitch
          :model-value="cfg.overlays.showDate"
          :label="t('show.showDate')"
          @update:model-value="(v) => store.patch((d) => (d.overlays.showDate = v))"
        />
      </SettingRow>

      <SettingRow :label="t('show.showFileName')">
        <ToggleSwitch
          :model-value="cfg.overlays.showFileName"
          :label="t('show.showFileName')"
          @update:model-value="(v) => store.patch((d) => (d.overlays.showFileName = v))"
        />
      </SettingRow>

      <SettingRow :label="t('show.showTakenAt')">
        <ToggleSwitch
          :model-value="cfg.overlays.showTakenAt"
          :label="t('show.showTakenAt')"
          @update:model-value="(v) => store.patch((d) => (d.overlays.showTakenAt = v))"
        />
      </SettingRow>

      <SettingRow :label="t('show.showSettingsButton')" :hint="t('show.showSettingsButtonHint')">
        <ToggleSwitch
          :model-value="cfg.overlays.showSettingsButton"
          :label="t('show.showSettingsButton')"
          @update:model-value="(v) => store.patch((d) => (d.overlays.showSettingsButton = v))"
        />
      </SettingRow>

      <SettingRow :label="t('show.showExcludeButton')" :hint="t('show.showExcludeButtonHint')">
        <ToggleSwitch
          :model-value="cfg.overlays.showExcludeButton"
          :label="t('show.showExcludeButton')"
          @update:model-value="(v) => store.patch((d) => (d.overlays.showExcludeButton = v))"
        />
      </SettingRow>

      <SettingRow
        :label="t('show.quarantineHint')"
        :hint="t('show.quarantineHintHint')"
      >
        <ToggleSwitch
          :model-value="cfg.overlays.showQuarantineHint"
          :label="t('show.quarantineHint')"
          @update:model-value="(v) => store.patch((d) => (d.overlays.showQuarantineHint = v))"
        />
      </SettingRow>

      <SettingRow :label="t('show.pixelShift')" :hint="t('show.pixelShiftHint')">
        <ToggleSwitch
          :model-value="cfg.overlays.pixelShift"
          :label="t('show.pixelShift')"
          @update:model-value="(v) => store.patch((d) => (d.overlays.pixelShift = v))"
        />
      </SettingRow>
    </section>

    <!-- Die Ausschlussliste (FA-30) sitzt seit E-25 im Bild-Browser: dort
         steht sie mit Vorschaubildern statt als Verzeichnis von Dateinamen. -->
  </div>
</template>

<style scoped>
/* ── Statistik (Wartung F1) ──────────────────────────────────────────────── */

.stats {
  display: flex;
  flex-wrap: wrap;
  gap: 10px 28px;
  margin: 0 0 12px;
}

.stats div {
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.stats dt {
  order: 2;
  font-size: 13px;
  color: var(--ss-text-dim);
}

/* Die Zahl zuerst und groesser: sie ist die Antwort, die Beschriftung nur
   die Frage dazu. */
.stats dd {
  order: 1;
  margin: 0;
  font-size: 22px;
  font-variant-numeric: tabular-nums;
  color: var(--ss-accent);
}

.progress-line {
  margin: 0 0 16px;
  font-size: 13px;
  color: var(--ss-text-dim);
}

.progress-line .cycles {
  margin-left: 14px;
}

.tops {
  display: flex;
  flex-wrap: wrap;
  gap: 24px;
  margin-bottom: 18px;
}

.top {
  flex: 1 1 260px;
  min-width: 0;
}

.top ol {
  margin: 6px 0 0;
  padding-left: 20px;
  font-size: 13px;
}

.top li {
  display: flex;
  gap: 10px;
  padding: 2px 0;
}

/* Der Dateiname darf kuerzen, die Zahl daneben nie — sonst steht dort „12…" */
.top .name {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.top .value {
  flex: 0 0 auto;
  font-variant-numeric: tabular-nums;
  color: var(--ss-text-dim);
}

.notice {
  margin: 4px 0 0;
  font-size: 13px;
  color: var(--ss-accent);
}

.pane {
  height: 100%;
  padding-right: 8px;
}

section {
  margin-bottom: 28px;
}

section > .ss-label {
  display: block;
  margin-bottom: 6px;
}

.narrow {
  width: 150px;
}

.wide {
  width: 200px;
}

/* Jahre und Absender als Chips: eine Mehrfachauswahl mit zwanzig Jahren
   waere als Liste von Schaltern unbedienbar. */
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.chip {
  padding: 6px 14px;
  border: 1px solid var(--ss-border-strong);
  border-radius: var(--ss-radius-pill);
  font-size: 13px;
  color: var(--ss-text-muted);
  transition: background var(--ss-transition), color var(--ss-transition);
}

.chip.active {
  background: var(--ss-surface-accent);
  color: var(--ss-accent);
}

.chip .count {
  margin-left: 6px;
  font-size: 11px;
  color: var(--ss-text-faint);
  font-variant-numeric: tabular-nums;
}

.slider {
  display: flex;
  align-items: center;
  gap: 14px;
}

.slider input {
  width: 180px;
  accent-color: var(--ss-accent);
}

.value {
  font-size: 14px;
  color: var(--ss-text-dim);
  min-width: 46px;
  font-variant-numeric: tabular-nums;
}

.muted {
  padding: 10px 0;
  font-size: 14px;
  color: var(--ss-text-dim);
}

</style>
