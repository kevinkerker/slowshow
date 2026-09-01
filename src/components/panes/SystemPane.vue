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
import { currentDevice } from '@/lib/device'
import { backupFileName, isAvailable, openTextFile, saveTextFile } from '@/lib/saf'
import { localeTag, type Language } from '@/lib/i18n'
import { EVENTS, type DatabaseCheck, type MqttStatus, type StorageBreakdown } from '@/lib/types'

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

/**
 * Fassung der laufenden App.
 *
 * Aus dem Backend statt als feste Zeichenkette: hier stand `0.1.0`, waehrend
 * Paket, Cargo-Manifest und `tauri.conf.json` laengst auf 1.0.0 standen. Wer
 * eine Fehlermeldung schickt, nennt darin die Fassung aus dieser Zeile — eine
 * falsche schickt die Fehlersuche in die Irre.
 *
 * Leer, bis die Antwort da ist: eine Platzhalterzahl waere wieder eine
 * Versionsnummer, die von nichts abhaengt.
 */
const version = ref('')

onMounted(async () => {
  void loadBreakdown()
  version.value = await api.appVersion().catch(() => '')
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

// ── Speicher, Datenbank, Diagnose (Wartung F9–F11, E-31) ────────────────────
// Nach E-31 kein eigener Navigationsbereich: was den Speicher und die Ablage
// betrifft, steht bei System.
const breakdown = ref<StorageBreakdown | null>(null)
const dbCheck = ref<DatabaseCheck | null>(null)
const dbBusy = ref(false)
const dbNotice = ref<string | null>(null)
const report = ref<string | null>(null)
const reportNotice = ref<string | null>(null)
const reportError = ref<string | null>(null)

async function loadBreakdown() {
  try {
    breakdown.value = await api.storageBreakdown()
  } catch (e) {
    // Beigabe: ihr Fehlen darf die Einstellungen darunter nicht sperren.
    console.warn('Speicheruebersicht nicht ladbar', e)
  }
}

async function runCheck() {
  dbBusy.value = true
  dbNotice.value = null
  try {
    dbCheck.value = await api.checkDatabase()
  } finally {
    dbBusy.value = false
  }
}

async function runRepair() {
  const c = dbCheck.value
  if (!c) return
  if (
    !confirm(
      t('system.databaseRepairAsk', {
        orphan: c.orphanFiles.length + c.orphanThumbs.length,
        missing: c.missingFiles.length,
      }),
    )
  )
    return

  dbBusy.value = true
  try {
    const frei = await api.repairDatabase()
    dbNotice.value = t('system.databaseRepaired', { bytes: formatBytes(frei, locale.value) })
    // Neu pruefen statt das alte Ergebnis stehen zu lassen: sonst boete die
    // Oberflaeche weiter „Aufraeumen" fuer etwas an, das schon weg ist.
    dbCheck.value = await api.checkDatabase()
    await loadBreakdown()
  } finally {
    dbBusy.value = false
  }
}

/** Erzeugt den Bericht und zeigt ihn, bevor er irgendwo hingeht (F11). */
async function makeReport() {
  reportNotice.value = null
  reportError.value = null
  // Geraet und Fassung aus der WebView-Kennung statt aus einem Plugin: fuer
  // zwei Zeichenketten eine Abhaengigkeit samt Android-Anteil aufzunehmen
  // stuende in keinem Verhaeltnis (siehe `lib/device.ts`).
  const { androidRelease, deviceModel } = currentDevice()
  try {
    report.value = await api.diagnosticReport(androidRelease, deviceModel)
  } catch (e) {
    // Ohne diesen Zweig blieb ein Fehlschlag unsichtbar: die Zusage wurde
    // abgelehnt, niemand fing sie auf, und die Schaltflaeche tat scheinbar
    // nichts. Am Geraet zweimal getippt, ohne eine einzige Spur.
    reportError.value = e instanceof Error ? e.message : String(e)
  }
}

async function copyReport() {
  if (!report.value) return
  try {
    await navigator.clipboard.writeText(report.value)
    reportNotice.value = t('system.diagnosticsCopied')
  } catch (e) {
    reportNotice.value = String(e)
  }
}

/**
 * Sicherung in eine Datei schreiben (FA-45).
 *
 * Frueher lief das ueber die Zwischenablage. Das war nur halb benutzbar: der
 * Export gelang, der Import scheiterte immer an "Read permission denied" —
 * Androids WebView kann `clipboard-read` nicht gewaehren. Eine Sicherung, die
 * man nie zurueckspielen kann, sieht funktionierend aus und ist keine.
 *
 * Ausserdem ueberlebt eine Datei, was die Zwischenablage nicht ueberlebt:
 * Neustart, Werksreset, Geraetewechsel. Genau dafuer macht man eine Sicherung.
 *
 * Auf dem Schreibtisch (Nebenprodukt, Lastenheft 1.3) gibt es keinen
 * SAF-Dialog; dort bleibt die Zwischenablage, weil sie da vollstaendig
 * funktioniert.
 */
async function exportConfig() {
  try {
    const json = await api.exportConfig()

    if (await isAvailable()) {
      const name = await saveTextFile(backupFileName(), json)
      if (name) flash(t('system.exportedFile', { name }))
      return
    }

    await navigator.clipboard.writeText(json)
    flash(t('system.exported'))
  } catch (e) {
    flash(t('system.exportFailed', { error: e instanceof Error ? e.message : String(e) }))
  }
}

/** Sicherung aus einer Datei einlesen (FA-45). Gegenstueck zu `exportConfig`. */
async function importConfig() {
  try {
    let json: string | null = null

    if (await isAvailable()) {
      const picked = await openTextFile()
      if (!picked) return
      json = picked.content
    } else {
      json = await navigator.clipboard.readText()
    }

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
    <!-- Wartung F9–F11 (E-31): Speicher und Datenbank stehen bei System. -->
    <section>
      <h3 class="ss-label">{{ t('system.storage') }}</h3>

      <div v-if="breakdown" class="breakdown">
        <div class="col">
          <h4 class="ss-label">{{ t('system.storageByYear') }}</h4>
          <ul>
            <li v-for="g in breakdown.byYear" :key="g.label">
              <span class="label">{{ g.label === '—' ? t('system.storageUnknown') : g.label }}</span>
              <span class="count">{{ t('system.storagePhotos', { n: g.count }, g.count) }}</span>
              <span class="bytes">{{ formatBytes(g.bytes, locale) }}</span>
            </li>
          </ul>
        </div>
        <div v-if="breakdown.bySender.length" class="col">
          <h4 class="ss-label">{{ t('system.storageBySender') }}</h4>
          <ul>
            <li v-for="g in breakdown.bySender" :key="g.label">
              <span class="label">{{ g.label }}</span>
              <span class="count">{{ t('system.storagePhotos', { n: g.count }, g.count) }}</span>
              <span class="bytes">{{ formatBytes(g.bytes, locale) }}</span>
            </li>
          </ul>
        </div>
      </div>

      <SettingRow :label="t('system.database')" :hint="t('system.databaseHint')">
        <button class="secondary" :disabled="dbBusy" @click="runCheck">
          {{ t('system.databaseCheck') }}
        </button>
      </SettingRow>

      <p v-if="dbCheck" class="db-result">
        <template v-if="dbCheck.missingFiles.length === 0 && dbCheck.orphanFiles.length === 0 && dbCheck.orphanThumbs.length === 0">
          {{ t('system.databaseClean') }}
        </template>
        <template v-else>
          {{
            t('system.databaseFound', {
              missing: dbCheck.missingFiles.length,
              orphan: dbCheck.orphanFiles.length + dbCheck.orphanThumbs.length,
              bytes: formatBytes(dbCheck.reclaimableBytes, locale),
            })
          }}
          <button class="danger inline" :disabled="dbBusy" @click="runRepair">
            {{ t('system.databaseRepair') }}
          </button>
        </template>
      </p>
      <p v-if="dbNotice" class="notice">{{ dbNotice }}</p>

      <SettingRow :label="t('system.diagnostics')" :hint="t('system.diagnosticsHint')">
        <button class="secondary" @click="makeReport">
          {{ t('system.diagnosticsShow') }}
        </button>
      </SettingRow>
      <p v-if="reportError" class="db-result error">{{ reportError }}</p>
    </section>

    <!-- Der Bericht wird gezeigt, bevor er irgendwohin geht: was das Geraet
         verlaesst, soll vorher jemand gesehen haben (F11). -->
    <div v-if="report" class="backdrop" @click.self="report = null">
      <div class="report" role="dialog" aria-modal="true">
        <pre>{{ report }}</pre>
        <p v-if="reportNotice" class="notice">{{ reportNotice }}</p>
        <div class="actions">
          <button class="primary" @click="copyReport">{{ t('system.diagnosticsCopy') }}</button>
          <button class="secondary" @click="report = null">{{ t('system.diagnosticsClose') }}</button>
        </div>
      </div>
    </div>

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
      <p v-if="version" class="dim">{{ t('system.version', { version }) }}</p>
      <p class="dim">{{ t('system.license') }}</p>
    </section>
  </div>
</template>

<style scoped>
/* ── Speicher, Datenbank, Diagnose (Wartung F9–F11) ─────────────────────── */

.breakdown {
  display: flex;
  flex-wrap: wrap;
  gap: 24px;
  margin-bottom: 18px;
}

.breakdown .col {
  flex: 1 1 260px;
  min-width: 0;
}

.breakdown ul {
  list-style: none;
  margin: 6px 0 0;
  padding: 0;
  font-size: 13px;
}

.breakdown li {
  display: flex;
  gap: 10px;
  padding: 3px 0;
}

/* Die Beschriftung darf kuerzen, die Zahlen nie — sonst stuende dort „1,2…" */
.breakdown .label {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.breakdown .count,
.breakdown .bytes {
  flex: 0 0 auto;
  font-variant-numeric: tabular-nums;
  color: var(--ss-text-dim);
}

.db-result {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  margin: 0 0 12px;
  font-size: 13px;
  color: var(--ss-text-dim);
}

.db-result.error {
  color: var(--ss-error);
}

.db-result .inline {
  padding: 4px 12px;
  font-size: 13px;
}

/* Ohne diese Regel stand der Bericht inline unter der Schaltflaeche — am
   Geraet ausserhalb des sichtbaren Bereichs, und ich hielt ihn zweimal fuer
   nicht erzeugt. Die Klasse gibt es in `SourceDialog` schon, Stile sind aber
   je Komponente gekapselt und werden nicht mitgeerbt. */
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 40;
  display: grid;
  place-items: center;
  padding: 16px;
  background: rgba(0, 0, 0, 0.72);
}

.report {
  width: min(760px, 100%);
  max-height: 100%;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 20px;
  border: 1px solid var(--ss-border-soft);
  border-radius: 16px;
  background: var(--ss-surface);
}

/* Der Bericht ist Fliesstext in fester Breite — umgebrochen waere die
   Spaltenausrichtung hin, und die traegt hier die Lesbarkeit. */
.report pre {
  margin: 0;
  overflow: auto;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 12px;
  line-height: 1.45;
  white-space: pre;
}

.report .actions {
  display: flex;
  gap: 10px;
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
