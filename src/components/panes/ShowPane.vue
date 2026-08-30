<script setup lang="ts">
/**
 * Einstellungen der Diashow: Takt, Reihenfolge, Darstellung, Einblendungen
 * (FA-02, FA-03, FA-05 bis FA-08, FA-10, FA-30, NF-07).
 */
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import SettingRow from '../SettingRow.vue'
import ToggleSwitch from '../ToggleSwitch.vue'
import * as api from '@/lib/api'
import { useConfigStore } from '@/stores/config'
import { formatInterval } from '@/lib/format'
import type { CacheEntry, ClockStyle, FitMode, PlayOrder } from '@/lib/types'

const { t } = useI18n()
const store = useConfigStore()
const excluded = ref<CacheEntry[]>([])

const cfg = computed(() => store.config)

/**
 * Zulässige Anzeigedauern (FA-02: 5 Sekunden bis 30 Minuten).
 *
 * Feste Stufen statt eines freien Feldes — auf einem Touchgerät ist die
 * Auswahl schneller, und unsinnige Zwischenwerte entstehen gar nicht erst.
 */
const INTERVALS = [5, 10, 15, 30, 60, 120, 300, 600, 1800]

const ORDERS: PlayOrder[] = ['random', 'fileName', 'takenAt', 'modified']
const FIT_MODES: FitMode[] = ['contain', 'cover']
const CLOCK_STYLES: ClockStyle[] = ['digital', 'analog']

function orderLabel(order: PlayOrder): string {
  return t(
    {
      random: 'show.orderRandom',
      fileName: 'show.orderFileName',
      takenAt: 'show.orderTakenAt',
      modified: 'show.orderModified',
    }[order],
  )
}

async function loadExcluded() {
  excluded.value = await api.excludedImages()
}

async function restore(id: string) {
  await api.includeImage(id)
  await loadExcluded()
}

onMounted(loadExcluded)
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

      <SettingRow :label="t('show.pairMode')" :hint="t('show.pairModeHint')">
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

      <SettingRow :label="t('show.pixelShift')" :hint="t('show.pixelShiftHint')">
        <ToggleSwitch
          :model-value="cfg.overlays.pixelShift"
          :label="t('show.pixelShift')"
          @update:model-value="(v) => store.patch((d) => (d.overlays.pixelShift = v))"
        />
      </SettingRow>
    </section>

    <!-- Ausschlussliste (FA-30) -->
    <section>
      <h3 class="ss-label">{{ t('show.excluded') }}</h3>
      <p v-if="excluded.length === 0" class="muted">{{ t('show.excludedNone') }}</p>
      <ul v-else class="excluded">
        <li v-for="entry in excluded" :key="entry.id">
          <span class="file">{{ entry.fileName }}</span>
          <button class="link" @click="restore(entry.id)">{{ t('show.restore') }}</button>
        </li>
      </ul>
    </section>
  </div>
</template>

<style scoped>
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

.excluded {
  list-style: none;
  margin: 0;
  padding: 0;
}

.excluded li {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 10px 0;
  border-bottom: 1px solid var(--ss-border-soft);
}

.file {
  flex-grow: 1;
  font-size: 14px;
  color: var(--ss-text-body);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.link {
  padding: 0 12px;
  font-size: 14px;
  color: var(--ss-accent);
}
</style>
