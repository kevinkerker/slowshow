<script setup lang="ts">
/**
 * Einstellungen der Diashow: Takt, Reihenfolge, Darstellung, Einblendungen
 * (FA-02, FA-03, FA-05 bis FA-08, FA-10, FA-30, NF-07).
 */
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import SettingRow from '../SettingRow.vue'
import ToggleSwitch from '../ToggleSwitch.vue'
import { useConfigStore } from '@/stores/config'
import { formatInterval } from '@/lib/format'
import type { ClockStyle, FitMode, Orientation, PlayOrder } from '@/lib/types'

const { t } = useI18n()
const store = useConfigStore()

const cfg = computed(() => store.config)

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

</style>
