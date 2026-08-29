<script setup lang="ts">
/**
 * Cache, Sprache, Heimnetz-Steuerung und Konfigurationsdateien
 * (FA-27, FA-31, FA-43, FA-45, FA-55, NF-09, NF-12).
 */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import SettingRow from '../SettingRow.vue'
import ToggleSwitch from '../ToggleSwitch.vue'
import * as api from '@/lib/api'
import { useConfigStore } from '@/stores/config'
import { formatBytes } from '@/lib/format'
import { localeTag, type Language } from '@/lib/i18n'
import { EVENTS, type MqttStatus } from '@/lib/types'

const { t } = useI18n()
const store = useConfigStore()
const notice = ref<string | null>(null)
const mqttPassword = ref('')
const mqttHasPassword = ref(false)
const mqttReconnecting = ref(false)

/**
 * Verbindungszustand, live.
 *
 * Kommt per Ereignis aus dem Backend, nicht per Abfrage: bei falscher
 * Broker-Adresse wiederholt der Client im Fünf-Sekunden-Takt, und der Nutzer
 * soll sehen, dass es hakt — samt Grund.
 */
const mqtt = ref<MqttStatus>({ running: false, connected: false, lastError: null })
let unlistenMqtt: UnlistenFn | null = null

onMounted(async () => {
  mqttHasPassword.value = await api.hasMqttPassword().catch(() => false)
  mqtt.value = await api.mqttStatus().catch(() => mqtt.value)
  unlistenMqtt = await listen<MqttStatus>(EVENTS.mqtt, (e) => {
    mqtt.value = e.payload
  })
})

onBeforeUnmount(() => {
  unlistenMqtt?.()
})

async function reconnectMqtt() {
  mqttReconnecting.value = true
  try {
    mqtt.value = await api.mqttReconnect()
  } catch (e) {
    flash(e instanceof Error ? e.message : String(e))
  } finally {
    mqttReconnecting.value = false
  }
}

/** Passwort getrennt von der übrigen Konfiguration speichern (NF-05). */
async function saveMqttPassword() {
  try {
    await api.setMqttPassword(mqttPassword.value)
    mqttHasPassword.value = mqttPassword.value.length > 0
    mqttPassword.value = ''
    flash(t('system.mqttPasswordSaved'))
  } catch (e) {
    flash(e instanceof Error ? e.message : String(e))
  }
}

const cfg = computed(() => store.config)
const locale = computed(() => localeTag(cfg.value?.language ?? 'auto'))

/** Cachegrößen in Gigabyte — der Standard aus FA-27 ist 2 GB. */
const CACHE_SIZES_GB = [0.5, 1, 2, 4, 8, 16]
const LANGUAGES: Language[] = ['auto', 'de', 'en']

function flash(message: string) {
  notice.value = message
  setTimeout(() => (notice.value = null), 5000)
}

/**
 * Konfiguration exportieren (FA-45).
 *
 * Bewusst über die Zwischenablage statt über einen Datei-Dialog: der
 * Sandkasten des Anzeigefensters unterbindet Downloads, und auf einem Tablet
 * ohne Tastatur ist die Zwischenablage der kürzere Weg.
 */
async function exportConfig() {
  try {
    const json = await api.exportConfig()
    await navigator.clipboard.writeText(json)
    flash(t('system.exported'))
  } catch (e) {
    flash(t('system.importFailed', { error: e instanceof Error ? e.message : String(e) }))
  }
}

async function importConfig() {
  try {
    const json = await navigator.clipboard.readText()
    await api.importConfig(json)
    await store.refreshStats()
    flash(t('system.imported'))
  } catch (e) {
    flash(t('system.importFailed', { error: e instanceof Error ? e.message : String(e) }))
  }
}
</script>

<template>
  <div v-if="cfg" class="pane ss-scroll">
    <section>
      <h3 class="ss-label">{{ t('system.cache') }}</h3>

      <div v-if="store.stats" class="usage">
        <span>
          {{ t('system.cacheUsage', {
            used: formatBytes(store.stats.bytes, locale),
            total: formatBytes(store.stats.maxBytes, locale),
          }) }}
        </span>
        <span class="dim">{{ t('system.cacheImages', { n: store.stats.images }) }}</span>
      </div>

      <SettingRow :label="t('system.cacheSize')" :hint="t('system.cacheHint')">
        <select
          :value="cfg.cache.maxBytes"
          class="narrow"
          @change="store.patch((d) => (d.cache.maxBytes = Number(($event.target as HTMLSelectElement).value)))"
        >
          <option v-for="gb in CACHE_SIZES_GB" :key="gb" :value="Math.round(gb * 1024 * 1024 * 1024)">
            {{ gb < 1 ? `${gb * 1024} MB` : `${gb} GB` }}
          </option>
        </select>
      </SettingRow>

      <SettingRow :label="t('system.prefetch')" :hint="t('system.prefetchHint')">
        <div class="slider">
          <input
            type="range"
            min="1"
            max="12"
            step="1"
            :value="cfg.cache.prefetchCount"
            @change="store.patch((d) => (d.cache.prefetchCount = Number(($event.target as HTMLInputElement).value)))"
          />
          <span class="value">{{ cfg.cache.prefetchCount }}</span>
        </div>
      </SettingRow>

      <SettingRow :label="t('system.quality')" :hint="t('system.qualityHint')">
        <div class="slider">
          <input
            type="range"
            min="40"
            max="100"
            step="5"
            :value="cfg.cache.jpegQuality"
            @change="store.patch((d) => (d.cache.jpegQuality = Number(($event.target as HTMLInputElement).value)))"
          />
          <span class="value">{{ cfg.cache.jpegQuality }}</span>
        </div>
      </SettingRow>
    </section>

    <section>
      <h3 class="ss-label">{{ t('system.title') }}</h3>

      <SettingRow :label="t('system.language')">
        <select
          :value="cfg.language"
          class="narrow"
          @change="store.patch((d) => (d.language = ($event.target as HTMLSelectElement).value as Language))"
        >
          <option v-for="lang in LANGUAGES" :key="lang" :value="lang">
            {{ lang === 'auto' ? t('system.languageAuto') : lang === 'de' ? 'Deutsch' : 'English' }}
          </option>
        </select>
      </SettingRow>

      <SettingRow :label="t('system.protectSettings')" :hint="t('system.protectSettingsHint')">
        <ToggleSwitch
          :model-value="cfg.protectSettings"
          :label="t('system.protectSettings')"
          @update:model-value="(v) => store.patch((d) => (d.protectSettings = v))"
        />
      </SettingRow>
    </section>

    <!-- Heimnetz-Steuerung (FA-55). Der Server wird beim Speichern sofort neu
         gebunden, ein Neustart der App ist nicht nötig. -->
    <section>
      <h3 class="ss-label">{{ t('system.remote') }}</h3>

      <SettingRow :label="t('system.remote')" :hint="t('system.remoteHint')">
        <ToggleSwitch
          :model-value="cfg.remote.enabled"
          :label="t('system.remote')"
          @update:model-value="(v) => store.patch((d) => (d.remote.enabled = v))"
        />
      </SettingRow>

      <template v-if="cfg.remote.enabled">
        <SettingRow :label="t('system.remotePort')" :hint="t('system.remoteAddress', { port: cfg.remote.port })">
          <input
            type="number"
            class="narrow"
            min="1024"
            max="65535"
            :value="cfg.remote.port"
            @change="store.patch((d) => (d.remote.port = Number(($event.target as HTMLInputElement).value)))"
          />
        </SettingRow>

        <SettingRow :label="t('system.remoteToken')" :hint="t('system.remoteTokenHint')" stacked>
          <input
            type="text"
            autocapitalize="off"
            autocomplete="off"
            spellcheck="false"
            :value="cfg.remote.token"
            @change="store.patch((d) => (d.remote.token = ($event.target as HTMLInputElement).value))"
          />
        </SettingRow>
      </template>
    </section>

    <!-- MQTT (FA-55). Gegenstück zur REST-Steuerung: der Rahmen verbindet
         sich zum Broker, statt dass Home Assistant ihn suchen muss. -->
    <section>
      <h3 class="ss-label">{{ t('system.mqtt') }}</h3>

      <SettingRow :label="t('system.mqtt')" :hint="t('system.mqttHint')">
        <ToggleSwitch
          :model-value="cfg.mqtt.enabled"
          :label="t('system.mqtt')"
          @update:model-value="(v) => store.patch((d) => (d.mqtt.enabled = v))"
        />
      </SettingRow>

      <template v-if="cfg.mqtt.enabled">
        <SettingRow :label="t('system.mqttHost')" stacked>
          <input
            type="text"
            autocapitalize="off"
            autocomplete="off"
            spellcheck="false"
            :placeholder="t('system.mqttHostPlaceholder')"
            :value="cfg.mqtt.host"
            @change="store.patch((d) => (d.mqtt.host = ($event.target as HTMLInputElement).value))"
          />
        </SettingRow>

        <SettingRow :label="t('system.mqttPort')">
          <input
            type="number"
            class="narrow"
            min="1"
            max="65535"
            :value="cfg.mqtt.port"
            @change="store.patch((d) => (d.mqtt.port = Number(($event.target as HTMLInputElement).value)))"
          />
        </SettingRow>

        <SettingRow :label="t('system.mqttUser')" stacked>
          <input
            type="text"
            autocapitalize="off"
            autocomplete="off"
            spellcheck="false"
            :value="cfg.mqtt.username"
            @change="store.patch((d) => (d.mqtt.username = ($event.target as HTMLInputElement).value))"
          />
        </SettingRow>

        <SettingRow
          :label="t('system.mqttPassword')"
          :hint="mqttHasPassword ? t('system.mqttPasswordKeep') : undefined"
          stacked
        >
          <div class="password-row">
            <input v-model="mqttPassword" type="password" autocomplete="off" />
            <button class="secondary" @click="saveMqttPassword">{{ t('common.save') }}</button>
          </div>
        </SettingRow>

        <SettingRow :label="t('system.mqttBaseTopic')" :hint="t('system.mqttBaseTopicHint')" stacked>
          <input
            type="text"
            autocapitalize="off"
            autocomplete="off"
            spellcheck="false"
            :value="cfg.mqtt.baseTopic"
            @change="store.patch((d) => (d.mqtt.baseTopic = ($event.target as HTMLInputElement).value))"
          />
        </SettingRow>

        <SettingRow :label="t('system.mqttDiscovery')" :hint="t('system.mqttDiscoveryHint')">
          <ToggleSwitch
            :model-value="cfg.mqtt.discovery"
            :label="t('system.mqttDiscovery')"
            @update:model-value="(v) => store.patch((d) => (d.mqtt.discovery = v))"
          />
        </SettingRow>

        <SettingRow v-if="cfg.mqtt.discovery" :label="t('system.mqttDiscoveryPrefix')" stacked>
          <input
            type="text"
            autocapitalize="off"
            autocomplete="off"
            spellcheck="false"
            :value="cfg.mqtt.discoveryPrefix"
            @change="store.patch((d) => (d.mqtt.discoveryPrefix = ($event.target as HTMLInputElement).value))"
          />
        </SettingRow>

        <!-- Verbindungszustand und Neuverbinden. Der Knopf erspart nach
             einem korrigierten Tippfehler den Umweg über den Schalter. -->
        <div class="mqtt-state">
          <div class="state-line" :class="{ ok: mqtt.connected }">
            <span class="dot" />
            <span>
              {{ mqtt.connected ? t('system.mqttConnected') : t('system.mqttDisconnected') }}
            </span>
          </div>
          <button class="secondary" :disabled="mqttReconnecting" @click="reconnectMqtt">
            {{ mqttReconnecting ? t('system.mqttReconnecting') : t('system.mqttReconnect') }}
          </button>
        </div>

        <p v-if="!mqtt.connected && mqtt.lastError" class="mqtt-error">
          {{ mqtt.lastError }}
        </p>
      </template>
    </section>

    <section>
      <h3 class="ss-label">{{ t('system.config') }}</h3>
      <div class="buttons">
        <button class="secondary" @click="exportConfig">{{ t('system.export') }}</button>
        <button class="secondary" @click="importConfig">{{ t('system.import') }}</button>
      </div>
      <p v-if="notice" class="notice">{{ notice }}</p>
    </section>

    <section class="about">
      <div class="ss-wordmark">{{ t('app.name') }}</div>
      <p class="dim">{{ t('system.version', { version: '0.1.0' }) }}</p>
      <p class="dim">{{ t('system.license') }}</p>
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

.usage {
  display: flex;
  justify-content: space-between;
  padding: 12px 0;
  font-size: 14px;
  color: var(--ss-text-body);
}

.dim {
  color: var(--ss-text-dim);
  font-size: 13px;
}

.narrow {
  width: 150px;
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
  min-width: 32px;
  font-variant-numeric: tabular-nums;
}

.buttons {
  display: flex;
  gap: 10px;
  padding-top: 10px;
}

.secondary {
  padding: 0 22px;
  border: 1px solid var(--ss-border-strong);
  border-radius: var(--ss-radius-pill);
  color: var(--ss-text-body);
  font-size: 15px;
  font-weight: 500;
}

.secondary:active {
  background: var(--ss-surface-accent);
  color: var(--ss-accent);
}

.password-row {
  display: flex;
  gap: 10px;
}

.mqtt-state {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding-top: 6px;
}

.mqtt-error {
  padding-top: 6px;
  font-size: 13px;
  line-height: 1.4;
  color: var(--ss-error);
  /* Broker-Fehlermeldungen sind manchmal lang und ohne Leerzeichen. */
  overflow-wrap: anywhere;
}

.state-line {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 0 4px;
  font-size: 13px;
  color: var(--ss-text-dim);
}

.state-line .dot {
  width: 8px;
  height: 8px;
  border-radius: var(--ss-radius-pill);
  background: var(--ss-toggle-knob-off);
}

.state-line.ok {
  color: var(--ss-text-accent);
}

.state-line.ok .dot {
  background: var(--ss-accent);
}

.notice {
  padding-top: 12px;
  font-size: 13px;
  color: var(--ss-text-dim);
}

.about {
  padding-top: 8px;
  opacity: 0.8;
}

.about .ss-wordmark {
  font-size: 22px;
  margin-bottom: 6px;
}
</style>
