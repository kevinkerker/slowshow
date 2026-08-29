<script setup lang="ts">
/**
 * Zeitplan, Helligkeit und Nachtmodus (FA-52, FA-53, FA-54).
 *
 * Der aktuelle Zustand steht ganz oben — beim Einrichten eines Zeitplans will
 * man sofort sehen, ob die Diashow gerade laufen sollte oder nicht.
 */
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import SettingRow from '../SettingRow.vue'
import ToggleSwitch from '../ToggleSwitch.vue'
import { useConfigStore } from '@/stores/config'

const { t } = useI18n()
const store = useConfigStore()

const cfg = computed(() => store.config)
const active = computed(() => store.display?.slideshowActive ?? true)

/**
 * Läuft die Aktivzeit über Mitternacht? Dann bekommt der Nutzer einen Hinweis,
 * damit die Eingabe „22:00 bis 07:00" nicht wie ein Fehler aussieht.
 */
const overnight = computed(() => {
  if (!cfg.value?.schedule.enabled) return false
  const [from, to] = [cfg.value.schedule.activeFrom, cfg.value.schedule.activeTo]
  return to !== '' && from !== '' && to < from
})
</script>

<template>
  <div v-if="cfg" class="pane ss-scroll">
    <div class="state" :class="{ resting: !active }">
      <span class="dot" />
      {{ active ? t('schedule.currentlyActive') : t('schedule.currentlyResting') }}
    </div>

    <section>
      <SettingRow :label="t('schedule.enabled')" :hint="t('schedule.enabledHint')">
        <ToggleSwitch
          :model-value="cfg.schedule.enabled"
          :label="t('schedule.enabled')"
          @update:model-value="(v) => store.patch((d) => (d.schedule.enabled = v))"
        />
      </SettingRow>

      <template v-if="cfg.schedule.enabled">
        <SettingRow :label="t('schedule.activeFrom')">
          <input
            type="time"
            class="time"
            :value="cfg.schedule.activeFrom"
            @change="store.patch((d) => (d.schedule.activeFrom = ($event.target as HTMLInputElement).value))"
          />
        </SettingRow>

        <SettingRow
          :label="t('schedule.activeTo')"
          :hint="overnight ? t('schedule.overnightHint') : undefined"
        >
          <input
            type="time"
            class="time"
            :value="cfg.schedule.activeTo"
            @change="store.patch((d) => (d.schedule.activeTo = ($event.target as HTMLInputElement).value))"
          />
        </SettingRow>

        <SettingRow :label="t('schedule.nightClock')" :hint="t('schedule.nightClockHint')">
          <ToggleSwitch
            :model-value="cfg.schedule.nightClock"
            :label="t('schedule.nightClock')"
            @update:model-value="(v) => store.patch((d) => (d.schedule.nightClock = v))"
          />
        </SettingRow>
      </template>
    </section>

    <section>
      <h3 class="ss-label">{{ t('schedule.brightness') }}</h3>

      <SettingRow :label="t('schedule.brightness')">
        <div class="slider">
          <input
            type="range"
            min="5"
            max="100"
            step="5"
            :value="cfg.brightness.level"
            @change="store.patch((d) => (d.brightness.level = Number(($event.target as HTMLInputElement).value)))"
          />
          <span class="value">{{ cfg.brightness.level }} %</span>
        </div>
      </SettingRow>

      <SettingRow :label="t('schedule.autoDim')">
        <ToggleSwitch
          :model-value="cfg.brightness.autoDim"
          :label="t('schedule.autoDim')"
          @update:model-value="(v) => store.patch((d) => (d.brightness.autoDim = v))"
        />
      </SettingRow>

      <template v-if="cfg.brightness.autoDim">
        <SettingRow :label="t('schedule.dimFrom')">
          <input
            type="time"
            class="time"
            :value="cfg.brightness.dimFrom"
            @change="store.patch((d) => (d.brightness.dimFrom = ($event.target as HTMLInputElement).value))"
          />
        </SettingRow>

        <SettingRow :label="t('schedule.dimLevel')">
          <div class="slider">
            <input
              type="range"
              min="5"
              max="100"
              step="5"
              :value="cfg.brightness.dimLevel"
              @change="store.patch((d) => (d.brightness.dimLevel = Number(($event.target as HTMLInputElement).value)))"
            />
            <span class="value">{{ cfg.brightness.dimLevel }} %</span>
          </div>
        </SettingRow>
      </template>
    </section>
  </div>
</template>

<style scoped>
.pane {
  height: 100%;
  padding-right: 8px;
}

.state {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 24px;
  padding: 14px 18px;
  border: 1px solid var(--ss-border);
  border-radius: var(--ss-radius-card);
  background: var(--ss-surface);
  font-size: 14px;
  color: var(--ss-text-accent);
}

.state.resting {
  color: var(--ss-text-dim);
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: var(--ss-radius-pill);
  background: var(--ss-accent);
}

.state.resting .dot {
  background: var(--ss-toggle-knob-off);
}

section {
  margin-bottom: 28px;
}

section > .ss-label {
  display: block;
  margin-bottom: 6px;
}

.time {
  width: 140px;
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
</style>
