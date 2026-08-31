<script setup lang="ts">
/**
 * Anlegen und Bearbeiten einer Quelle (FA-20, FA-21, FA-23, FA-29).
 *
 * Vier Quellenarten in einem Formular (E-30). Der Verbindungstest vor dem
 * Speichern ist bewusst prominent: eine Quelle, die erst beim nächtlichen Sync
 * scheitert, fällt auf einem unbeaufsichtigten Gerät niemandem auf. Beim
 * Postfach zählt das doppelt — dort merkt man einen Tippfehler sonst erst,
 * wenn die Fotos der Großeltern ausbleiben.
 */
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import ToggleSwitch from './ToggleSwitch.vue'
import SettingRow from './SettingRow.vue'
import * as api from '@/lib/api'
import { formatRelativeTime } from '@/lib/format'
import type { Album, FetchLogEntry, ResyncProgress, Source, SourceKind } from '@/lib/types'

const props = defineProps<{
  /** `null` legt eine neue Quelle an. */
  source: Source | null
  /** Fehler beim Speichern, vom aufrufenden Bereich gemeldet. */
  saveError?: string
}>()

const emit = defineEmits<{
  save: [source: Source, password: string | undefined]
  remove: [id: string]
  cancel: []
}>()

const { t } = useI18n()

type Kind = SourceKind['type']

const kind = ref<Kind>('local')
const name = ref('')
const url = ref('')
const username = ref('')
const password = ref('')
const album = ref('')
const usePreviewApi = ref(true)
const allowInsecureTls = ref(false)
const safUri = ref('')
const safPath = ref('')
const subfolders = ref('')
const minWidth = ref(0)
const minHeight = ref(0)
const syncIntervalMinutes = ref(360)

// ── Postfach (E-30) ──────────────────────────────────────────────────────────
const mailHost = ref('')
const mailPort = ref(993)
const mailFolder = ref('INBOX')
const quarantineAll = ref(false)
const maxAttachmentMb = ref(25)
const maxMailsPerHour = ref(30)
const includeSeen = ref(false)

// ── Abruf: Stand und Protokoll (Wartung F5–F7) ───────────────────────────────
// Beim Postfach beantwortet die Statuszeile die Frage, die vor dem Rahmen
// wirklich aufkommt: „kommt da noch was an?". Vorher liess sich „der Abruf
// laeuft nicht" nicht von „es wurde nichts geschickt" unterscheiden.
const lastFetch = ref<FetchLogEntry | null>(null)
const fetchEntries = ref<FetchLogEntry[]>([])
const fetching = ref(false)
const showLog = ref(false)

async function loadFetchState() {
  if (!props.source || props.source.kind.type !== 'mail') return
  try {
    const id = props.source.id
    lastFetch.value = await api.lastFetch(id)
    fetchEntries.value = (await api.fetchLog()).filter((e) => e.sourceId === id)
  } catch (e) {
    console.warn('Abrufstand nicht ladbar', e)
  }
}

/** Statuszeile in einem Satz (F5). */
const fetchStatus = computed(() => {
  const e = lastFetch.value
  if (!e) return t('sourceForm.fetchNever')
  const when = formatRelativeTime(e.at, new Date(), t)
  if (e.error) return t('sourceForm.fetchFailed', { when, error: e.error })
  if (e.added > 0) return t('sourceForm.fetchOk', { when, n: e.added })
  return t('sourceForm.fetchNothing', { when })
})

/** Manueller Abruf (F7). */
async function fetchNow() {
  if (!props.source) return
  fetching.value = true
  try {
    await api.syncNow(props.source.id)
    await loadFetchState()
  } catch (e) {
    formError.value = message(e)
  } finally {
    fetching.value = false
  }
}

// ── Neuabgleich (Wartung F8) ─────────────────────────────────────────────────
const resyncing = ref(false)
const resyncProgress = ref<ResyncProgress | null>(null)
let stopResyncListener: (() => void) | null = null

async function startResync() {
  if (!props.source) return
  if (!confirm(t('sourceForm.resyncAsk'))) return

  resyncing.value = true
  resyncProgress.value = null
  // Der Fortschritt kommt als Ereignis: der Aufruf selbst laeuft Minuten und
  // koennte sonst nur „fertig" oder „kaputt" melden.
  stopResyncListener = await api.onResyncProgress((p) => (resyncProgress.value = p))
  try {
    const n = await api.resyncMailbox(props.source.id)
    testResult.value = { ok: true, message: t('sourceForm.resyncDone', { n }) }
    await loadFetchState()
  } catch (e) {
    formError.value = message(e)
  } finally {
    resyncing.value = false
    resyncProgress.value = null
    stopResyncListener?.()
    stopResyncListener = null
  }
}

async function stopResync() {
  await api.cancelResync()
}

function triggerLabel(e: FetchLogEntry): string {
  if (e.trigger === 'manual') return t('sourceForm.triggerManual')
  if (e.trigger === 'resync') return t('sourceForm.triggerResync')
  return t('sourceForm.triggerInterval')
}

function logTime(e: FetchLogEntry): string {
  return formatRelativeTime(e.at, new Date(), t)
}

// ── Freigegebene Absender (F4, E-32) ─────────────────────────────────────────
// Nur beim Bearbeiten geladen: eine neue Quelle hat noch keine Liste, und der
// Aufruf braucht eine Quellen-Id, die es dann noch nicht gibt.
const senders = ref<api.AllowedSender[]>([])
const senderBusy = ref('')

async function loadSenders() {
  senders.value = []
  if (!props.source || props.source.kind.type !== 'mail') return
  try {
    senders.value = await api.allowedSenders(props.source.id)
  } catch (e) {
    // Kein Formularfehler: die Liste ist eine Beigabe, ihr Fehlen darf das
    // Bearbeiten der Quelle nicht blockieren.
    console.warn('Freigegebene Absender nicht ladbar', e)
  }
}

async function removeSender(entry: api.AllowedSender) {
  if (!props.source) return

  // Die Rueckfrage entscheidet ueber die vorhandenen Fotos (E-32). Bei einem
  // Absender ohne Fotos gibt es nichts zu entscheiden -- dann nur bestaetigen.
  let requarantine = false
  if (entry.photoCount === 0) {
    if (!confirm(t('sourceForm.removeSenderAskEmpty', { sender: entry.address }))) return
  } else {
    requarantine = confirm(
      t('sourceForm.removeSenderAsk', { sender: entry.address, n: entry.photoCount }),
    )
  }

  senderBusy.value = entry.address
  try {
    const moved = await api.removeAllowedSender(props.source.id, entry.address, requarantine)
    senders.value = senders.value.filter((s) => s.address !== entry.address)
    formError.value = ''
    testResult.value = {
      ok: true,
      message:
        moved > 0
          ? t('sourceForm.senderRemovedWith', { n: moved })
          : t('sourceForm.senderRemoved'),
    }
  } catch (e) {
    formError.value = message(e)
  } finally {
    senderBusy.value = ''
  }
}

const albums = ref<Album[]>([])
const loadingAlbums = ref(false)
const testing = ref(false)
const testResult = ref<{ ok: boolean; message: string } | null>(null)
const formError = ref('')
const safAvailable = ref(true)

const isEdit = computed(() => props.source !== null)
const isRemote = computed(() => kind.value !== 'local' && kind.value !== 'mail')

/** Auswahl der Quellenart. Die Beschriftungen folgen dem Schlüsselmuster. */
const KINDS: Kind[] = ['local', 'webDav', 'nextcloud', 'mail']

/** Formular aus der übergebenen Quelle füllen — oder auf Standardwerte setzen. */
watch(
  () => props.source,
  (source) => {
    testResult.value = null
    formError.value = ''
    albums.value = []
    password.value = ''

    if (!source) {
      kind.value = 'local'
      name.value = ''
      url.value = ''
      username.value = ''
      album.value = ''
      usePreviewApi.value = true
      allowInsecureTls.value = false
      safUri.value = ''
      safPath.value = ''
      subfolders.value = ''
      minWidth.value = 0
      minHeight.value = 0
      syncIntervalMinutes.value = 360
      return
    }

    kind.value = source.kind.type
    name.value = source.name
    subfolders.value = source.subfolders.join(', ')
    minWidth.value = source.minWidth
    minHeight.value = source.minHeight
    syncIntervalMinutes.value = source.syncIntervalMinutes

    if (source.kind.type === 'local') {
      safUri.value = source.kind.safUri
      safPath.value = source.kind.displayPath
    } else if (source.kind.type === 'mail') {
      mailHost.value = source.kind.host
      mailPort.value = source.kind.port
      mailFolder.value = source.kind.folder
      username.value = source.kind.username
      quarantineAll.value = source.kind.quarantineAll
      maxAttachmentMb.value = Math.round(source.kind.maxAttachmentBytes / 1024 / 1024)
      maxMailsPerHour.value = source.kind.maxMailsPerHour
      includeSeen.value = source.kind.includeSeen
      void loadSenders()
      void loadFetchState()
    } else {
      url.value = source.kind.url
      username.value = source.kind.username
      allowInsecureTls.value = source.kind.allowInsecureTls
      if (source.kind.type === 'nextcloud') {
        album.value = source.kind.album
        usePreviewApi.value = source.kind.usePreviewApi
      }
    }
  },
  { immediate: true },
)

async function chooseFolder() {
  formError.value = ''
  const { isAvailable, pickFolder } = await import('@/lib/saf')
  if (!(await isAvailable())) {
    safAvailable.value = false
    formError.value = t('sourceForm.errorSafUnavailable')
    return
  }
  const picked = await pickFolder()
  if (!picked) return
  safUri.value = picked.uri
  safPath.value = picked.name
  if (!name.value) name.value = picked.name
}

async function loadAlbums() {
  if (!url.value || !username.value) {
    formError.value = t('sourceForm.errorUrlRequired')
    return
  }
  loadingAlbums.value = true
  formError.value = ''
  try {
    albums.value = await api.listNextcloudAlbums(
      url.value,
      username.value,
      password.value,
      allowInsecureTls.value,
    )
    if (albums.value.length === 0) formError.value = t('sourceForm.noAlbums')
  } catch (e) {
    formError.value = t('sourceForm.testFailed', { error: message(e) })
  } finally {
    loadingAlbums.value = false
  }
}

async function testConnection() {
  const draft = build()
  if (!draft) return
  testing.value = true
  testResult.value = null
  try {
    const unseen = await api.testSource(draft, password.value)
    // Beim Postfach die Zahl mitnehmen: sie belegt, dass auch der Ordner
    // stimmt, nicht nur die Anmeldung. Andere Quellenarten liefern `null`.
    testResult.value = {
      ok: true,
      message:
        unseen === null
          ? t('sourceForm.testOk')
          : t('sourceForm.testOkMailbox', { n: unseen }, unseen),
    }
  } catch (e) {
    testResult.value = {
      ok: false,
      message: t('sourceForm.testFailed', { error: message(e) }),
    }
  } finally {
    testing.value = false
  }
}

/** Baut die Quelle aus dem Formular. `null`, wenn Pflichtfelder fehlen. */
function build(): Source | null {
  formError.value = ''

  if (!name.value.trim()) {
    formError.value = t('sourceForm.errorNameRequired')
    return null
  }

  // Genau einmal erzeugen und für Id *und* Passwortverweis verwenden.
  // Zwei getrennte Aufrufe lieferten zwei verschiedene Werte: das Passwort
  // landete unter dem einen, die Quelle verwies auf den anderen — beim
  // nächsten Bearbeiten hätte sich der Verweis auf die Id gezogen und die
  // Anmeldung wäre ab da mit leerem Passwort gelaufen.
  const id = props.source?.id ?? newId()

  let sourceKind: SourceKind
  if (kind.value === 'local') {
    if (!safUri.value) {
      formError.value = t('sourceForm.errorFolderRequired')
      return null
    }
    sourceKind = { type: 'local', safUri: safUri.value, displayPath: safPath.value }
  } else if (kind.value === 'mail') {
    if (!mailHost.value.trim()) {
      formError.value = t('sourceForm.errorHostRequired')
      return null
    }
    if (!username.value.trim()) {
      formError.value = t('sourceForm.errorUserRequired')
      return null
    }
    sourceKind = {
      type: 'mail',
      host: mailHost.value.trim(),
      port: mailPort.value,
      username: username.value.trim(),
      passwordRef: id,
      folder: mailFolder.value.trim() || 'INBOX',
      // Die Freigabeliste wächst durch Freigeben am Rahmen (F4), nicht über
      // dieses Formular — beim Anlegen ist sie leer, also landet die erste
      // Mail jedes Absenders in Quarantäne.
      allowedSenders:
        props.source?.kind.type === 'mail' ? props.source.kind.allowedSenders : [],
      quarantineAll: quarantineAll.value,
      maxAttachmentBytes: Math.max(1, maxAttachmentMb.value) * 1024 * 1024,
      maxMailsPerHour: maxMailsPerHour.value,
      includeSeen: includeSeen.value,
      quality: props.source?.kind.type === 'mail' ? props.source.kind.quality : 'standard',
    }
  } else {
    if (!url.value.trim()) {
      formError.value = t('sourceForm.errorUrlRequired')
      return null
    }
    // Der Verweis auf das Passwort bleibt über den Lebenszyklus stabil, damit
    // ein Umbenennen der Quelle die Zugangsdaten nicht verwaisen lässt.
    const passwordRef = id

    if (kind.value === 'nextcloud') {
      if (!album.value) {
        formError.value = t('sourceForm.errorAlbumRequired')
        return null
      }
      sourceKind = {
        type: 'nextcloud',
        url: url.value.trim(),
        username: username.value.trim(),
        passwordRef,
        album: album.value,
        usePreviewApi: usePreviewApi.value,
        allowInsecureTls: allowInsecureTls.value,
      }
    } else {
      sourceKind = {
        type: 'webDav',
        url: url.value.trim(),
        username: username.value.trim(),
        passwordRef,
        allowInsecureTls: allowInsecureTls.value,
      }
    }
  }

  return {
    id,
    name: name.value.trim(),
    kind: sourceKind,
    enabled: props.source?.enabled ?? true,
    subfolders: subfolders.value
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean),
    minWidth: Math.max(0, Number(minWidth.value) || 0),
    minHeight: Math.max(0, Number(minHeight.value) || 0),
    syncIntervalMinutes: Math.max(5, Number(syncIntervalMinutes.value) || 360),
    lastSync: props.source?.lastSync ?? null,
  }
}

function save() {
  const draft = build()
  if (!draft) return
  // Leeres Feld beim Bearbeiten heißt „Passwort behalten".
  emit('save', draft, password.value || undefined)
}

function newId(): string {
  return `src-${Date.now().toString(36)}-${Math.floor(Math.random() * 1e6).toString(36)}`
}

function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}
</script>

<template>
  <div class="backdrop" @click.self="emit('cancel')">
    <div class="dialog" role="dialog" aria-modal="true">
      <header class="head">
        <h2 class="ss-wordmark">
          {{ isEdit ? t('sourceForm.titleEdit') : t('sourceForm.titleAdd') }}
        </h2>
        <button class="close" :aria-label="t('common.cancel')" @click="emit('cancel')">
          <svg width="18" height="18" viewBox="0 0 20 20" fill="none" stroke="var(--ss-icon-soft)" stroke-width="1.5" stroke-linecap="round">
            <path d="M4 4 L16 16 M16 4 L4 16" />
          </svg>
        </button>
      </header>

      <div class="body ss-scroll">
        <!-- Quellenart. Beim Bearbeiten gesperrt: ein Wechsel würde die
             zwischengespeicherten Bilder ungültig machen. -->
        <fieldset v-if="!isEdit" class="kinds">
          <legend class="ss-label">{{ t('sourceForm.kind') }}</legend>
          <label v-for="option in KINDS" :key="option" class="kind">
            <input v-model="kind" type="radio" :value="option" />
            <span class="kind-body">
              <span class="kind-title">{{ t(`sourceForm.kind_${option}`) }}</span>
              <span class="kind-hint">{{ t(`sourceForm.kind_${option}_hint`) }}</span>
            </span>
          </label>
        </fieldset>

        <SettingRow :label="t('sourceForm.name')" stacked>
          <input v-model="name" type="text" :placeholder="t('sourceForm.namePlaceholder')" />
        </SettingRow>

        <!-- Postfach (E-30). Ein Postfach je Rahmen: die Quellenliste laesst
             kein zweites zu, weil das Papier genau eines vorsieht. -->
        <template v-if="kind === 'mail'">
          <SettingRow :label="t('sourceForm.mailHost')" stacked>
            <input
              v-model="mailHost"
              type="text"
              inputmode="url"
              autocapitalize="off"
              autocomplete="off"
              spellcheck="false"
              placeholder="imap.example.org"
            />
          </SettingRow>

          <SettingRow :label="t('sourceForm.mailPort')">
            <input v-model.number="mailPort" type="number" min="1" max="65535" class="narrow" />
          </SettingRow>

          <SettingRow :label="t('sourceForm.username')" stacked>
            <input
              v-model="username"
              type="text"
              inputmode="email"
              autocapitalize="off"
              autocomplete="username"
              spellcheck="false"
            />
          </SettingRow>

          <SettingRow
            :label="t('sourceForm.password')"
            :hint="isEdit ? t('sourceForm.passwordKeep') : undefined"
            stacked
          >
            <input v-model="password" type="password" autocomplete="current-password" />
          </SettingRow>

          <SettingRow :label="t('sourceForm.mailFolder')" stacked>
            <input
              v-model="mailFolder"
              type="text"
              autocapitalize="off"
              autocomplete="off"
              spellcheck="false"
              placeholder="INBOX"
            />
          </SettingRow>

          <!-- Wartung F5–F7. Nur beim Bearbeiten: eine neue Quelle hat noch
               keinen Abruf hinter sich. -->
          <SettingRow v-if="isEdit" :label="t('sourceForm.fetchStatus')" stacked>
            <div class="fetch-block">
            <p class="fetch-status" :class="{ bad: lastFetch?.error }">
              {{ fetchStatus }}
            </p>
            <div class="fetch-actions">
              <button class="secondary" :disabled="fetching" @click="fetchNow">
                {{ fetching ? t('sourceForm.fetchRunning') : t('sourceForm.fetchNow') }}
              </button>
              <button class="link" @click="showLog = !showLog">
                {{ t('sourceForm.fetchLog') }}
                <span aria-hidden="true">{{ showLog ? '▾' : '▸' }}</span>
              </button>
            </div>

            <!-- Wartung F8. Steht beim Abrufstand, weil es dieselbe Frage
                 beantwortet — nur gruendlicher. -->
            <div class="resync">
              <button
                v-if="!resyncing"
                class="link"
                :title="t('sourceForm.resyncHint')"
                @click="startResync"
              >
                {{ t('sourceForm.resync') }}
              </button>
              <template v-else>
                <span class="progress">
                  {{
                    resyncProgress
                      ? t('sourceForm.resyncRunning', {
                          done: resyncProgress.done,
                          total: resyncProgress.total,
                          added: resyncProgress.added,
                        })
                      : t('sourceForm.fetchRunning')
                  }}
                </span>
                <button class="link" @click="stopResync">
                  {{ t('sourceForm.resyncCancel') }}
                </button>
              </template>
            </div>

            <!-- Eingeklappt: 50 Zeilen sind im Normalfall Beiwerk und wuerden
                 das Formular unbrauchbar lang machen. -->
            <div v-if="showLog" class="fetch-log">
              <p class="hint">{{ t('sourceForm.fetchLogHint') }}</p>
              <p v-if="fetchEntries.length === 0" class="hint">
                {{ t('sourceForm.fetchLogEmpty') }}
              </p>
              <ol v-else>
                <li v-for="(e, i) in fetchEntries" :key="i" :class="{ bad: e.error }">
                  <span class="when">{{ logTime(e) }}</span>
                  <span class="trigger">{{ triggerLabel(e) }}</span>
                  <span class="outcome">
                    {{
                      e.error
                        ? e.error
                        : t('sourceForm.fetchLogLine', {
                            checked: e.checked,
                            added: e.added,
                            known: e.alreadyKnown,
                          })
                    }}
                  </span>
                </li>
              </ol>
            </div>
            </div>
          </SettingRow>

          <!-- E-34. Steht bewusst direkt unter dem Ordner: der Hinweis
               empfiehlt einen eigenen Ordner statt der INBOX, und das Feld
               dafuer soll daneben liegen. -->
          <SettingRow
            :label="t('sourceForm.includeSeen')"
            :hint="t('sourceForm.includeSeenHint')"
          >
            <ToggleSwitch v-model="includeSeen" :label="t('sourceForm.includeSeen')" />
          </SettingRow>

          <SettingRow :label="t('sourceForm.quarantineAll')" :hint="t('sourceForm.quarantineAllHint')">
            <ToggleSwitch
              v-model="quarantineAll"
              :label="t('sourceForm.quarantineAll')"
            />
          </SettingRow>

          <!-- Freigegebene Absender (F4, E-32). Nur beim Bearbeiten: eine
               neue Quelle hat noch keine Liste. Ohne diesen Abschnitt war die
               Freigabe eine Einbahnstrasse — ein einmal bestaetigter Absender
               liess sich nie mehr zuruecknehmen. -->
          <SettingRow
            v-if="isEdit"
            :label="t('sourceForm.allowedSenders')"
            :hint="t('sourceForm.allowedSendersHint')"
            stacked
          >
            <p v-if="senders.length === 0" class="senders-empty">
              {{ t('sourceForm.allowedSendersEmpty') }}
            </p>
            <ul v-else class="senders">
              <li v-for="entry in senders" :key="entry.address">
                <span class="sender-address">{{ entry.address }}</span>
                <span class="sender-count">
                  {{ t('sourceForm.senderPhotos', { n: entry.photoCount }, entry.photoCount) }}
                </span>
                <button
                  class="sender-remove"
                  :disabled="senderBusy === entry.address"
                  :aria-label="t('sourceForm.removeSender')"
                  :title="t('sourceForm.removeSender')"
                  @click="removeSender(entry)"
                >
                  <svg width="16" height="16" viewBox="0 0 20 20" fill="none"
                       stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
                    <path d="M5 5 L15 15 M15 5 L5 15" />
                  </svg>
                </button>
              </li>
            </ul>
          </SettingRow>

          <SettingRow :label="t('sourceForm.maxAttachment')">
            <input v-model.number="maxAttachmentMb" type="number" min="1" max="200" class="narrow" />
          </SettingRow>

          <SettingRow :label="t('sourceForm.maxMailsPerHour')" :hint="t('sourceForm.maxMailsPerHourHint')">
            <input v-model.number="maxMailsPerHour" type="number" min="0" max="500" class="narrow" />
          </SettingRow>
        </template>

        <!-- Lokaler Ordner (FA-20) -->
        <template v-else-if="kind === 'local'">
          <SettingRow
            :label="t('sourceForm.chooseFolder')"
            :hint="safPath ? t('sourceForm.folderChosen', { path: safPath }) : undefined"
          >
            <button class="secondary" :disabled="!safAvailable" @click="chooseFolder">
              {{ t('sourceForm.chooseFolder') }}
            </button>
          </SettingRow>
        </template>

        <!-- WebDAV und Nextcloud (FA-21, FA-23) -->
        <template v-else>
          <SettingRow :label="t('sourceForm.url')" stacked>
            <input
              v-model="url"
              type="url"
              inputmode="url"
              autocapitalize="off"
              autocomplete="off"
              spellcheck="false"
              :placeholder="kind === 'nextcloud' ? t('sourceForm.urlPlaceholderNextcloud') : t('sourceForm.urlPlaceholder')"
            />
          </SettingRow>

          <SettingRow :label="t('sourceForm.username')" stacked>
            <input v-model="username" type="text" autocapitalize="off" autocomplete="off" spellcheck="false" />
          </SettingRow>

          <SettingRow
            :label="t('sourceForm.password')"
            :hint="isEdit ? t('sourceForm.passwordKeep') : t('sourceForm.passwordHint')"
            stacked
          >
            <input v-model="password" type="password" autocomplete="off" />
          </SettingRow>

          <template v-if="kind === 'nextcloud'">
            <SettingRow :label="t('sourceForm.album')" stacked>
              <div class="album-row">
                <select v-model="album">
                  <option v-if="album && !albums.some((a) => a.name === album)" :value="album">
                    {{ album }}
                  </option>
                  <option v-for="a in albums" :key="a.name" :value="a.name">{{ a.name }}</option>
                </select>
                <button class="secondary" :disabled="loadingAlbums" @click="loadAlbums">
                  {{ loadingAlbums ? t('sourceForm.testing') : t('sourceForm.loadAlbums') }}
                </button>
              </div>
            </SettingRow>

            <SettingRow
              :label="t('sourceForm.usePreviewApi')"
              :hint="t('sourceForm.usePreviewApiHint')"
            >
              <ToggleSwitch v-model="usePreviewApi" :label="t('sourceForm.usePreviewApi')" />
            </SettingRow>
          </template>

          <SettingRow
            :label="t('sourceForm.allowInsecureTls')"
            :hint="t('sourceForm.allowInsecureTlsHint')"
          >
            <ToggleSwitch v-model="allowInsecureTls" :label="t('sourceForm.allowInsecureTls')" />
          </SettingRow>
        </template>

        <!-- Abrufabstand: fuer jede Quelle ausser dem lokalen Ordner, der
             beim Oeffnen ohnehin neu eingelesen wird. -->
        <SettingRow v-if="kind !== 'local'" :label="t('sourceForm.syncInterval')">
          <select v-model.number="syncIntervalMinutes" class="narrow">
            <option :value="15">15 min</option>
            <option :value="60">1 h</option>
            <option :value="360">6 h</option>
            <option :value="1440">24 h</option>
          </select>
        </SettingRow>

        <!-- Filter (FA-29) -->
        <SettingRow
          v-if="kind === 'local' || kind === 'webDav'"
          :label="t('sourceForm.subfolders')"
          :hint="t('sourceForm.subfoldersHint')"
          stacked
        >
          <input v-model="subfolders" type="text" :placeholder="t('sourceForm.subfoldersPlaceholder')" />
        </SettingRow>

        <SettingRow :label="t('sourceForm.minResolution')" :hint="t('sourceForm.minResolutionHint')">
          <div class="dimensions">
            <input v-model.number="minWidth" type="number" min="0" step="16" class="narrow" />
            <span class="times">×</span>
            <input v-model.number="minHeight" type="number" min="0" step="16" class="narrow" />
          </div>
        </SettingRow>

        <p v-if="formError" class="error">{{ formError }}</p>
        <p v-if="props.saveError" class="error">{{ props.saveError }}</p>
      </div>

      <footer class="foot">
        <!-- Loeschen sitzt hier und nicht auf der Quellenkarte: der Entwurf
             haelt die Liste bewusst ruhig, und eine Ebene tiefer ist ein
             destruktiver Schritt auf einem Touchgeraet besser aufgehoben
             (FA-43). Die Rueckfrage stellt der aufrufende Bereich. -->
        <button
          v-if="isEdit && props.source"
          class="danger"
          @click="emit('remove', props.source.id)"
        >
          {{ t('sourceForm.removeSource') }}
        </button>
        <button
          v-if="isRemote || kind === 'mail'"
          class="secondary"
          :disabled="testing"
          @click="testConnection"
        >
          {{ testing ? t('sourceForm.testing') : t('sourceForm.test') }}
        </button>

        <!-- Das Ergebnis steht neben seinem Ausloeser, nicht am Ende des
             Formulars. Dort stand es vorher: die Schaltflaeche sitzt in der
             festen Fusszeile, die Meldung im scrollbaren Rumpf — wer beim
             Passwortfeld auf „Verbindung testen" tippte, sah nichts
             geschehen. Am Geraet nachgestellt (E-33). -->
        <p v-if="testResult" class="result" :class="{ ok: testResult.ok }">
          {{ testResult.message }}
        </p>
        <span class="spacer" />
        <button class="secondary" @click="emit('cancel')">{{ t('common.cancel') }}</button>
        <button class="primary" @click="save">{{ t('common.save') }}</button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.72);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  z-index: 50;
}

.dialog {
  display: flex;
  flex-direction: column;
  width: min(680px, 100%);
  max-height: 100%;
  background: var(--ss-surface);
  border: 1px solid var(--ss-border);
  border-radius: var(--ss-radius-card);
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px;
  border-bottom: 1px solid var(--ss-border-soft);
}

.head .ss-wordmark {
  font-size: 22px;
}

.close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--ss-touch-target);
  height: var(--ss-touch-target);
  border: 1px solid var(--ss-border-strong);
  border-radius: var(--ss-radius-pill);
}

.body {
  padding: 8px 24px 20px;
  flex-grow: 1;
  min-height: 0;
}

.foot {
  display: flex;
  align-items: center;
  /* Umbrechen erlaubt: im Bearbeiten-Dialog kommt „Quelle entfernen" dazu,
     und dann bleibt neben den vier Schaltflaechen kaum Platz. Ohne diese
     Zeile quetschte sich die Meldung am Geraet auf vier Zeilen in eine
     handbreite Spalte. */
  flex-wrap: wrap;
  gap: 10px;
  padding: 16px 24px;
  border-top: 1px solid var(--ss-border-soft);
}

/* Die Schaltflaechen behalten ihre Breite, die Meldung daneben gibt nach.
   Am Tablet brach sonst „Verbindung testen" auf zwei Zeilen um, sobald das
   Ergebnis danebenstand — der Beschriftung sieht man einen Umbruch als
   Versehen an, einem Fliesstext nicht. */
.foot > button {
  flex: 0 0 auto;
  white-space: nowrap;
}

.spacer {
  flex-grow: 1;
}

/* ── Abrufstand und Protokoll (Wartung F5–F7) ───────────────────────────── */

/* Der Steuerbereich einer Einstellungszeile ist eine Reihe. Ohne diesen Block
   standen Statuszeile, Schaltflaeche, Umschalter und Protokoll nebeneinander
   und brachen einzeln um — am Geraet vier Spalten aus je zwei Zeilen. */
.fetch-block {
  display: flex;
  flex-direction: column;
  width: 100%;
  min-width: 0;
}

.fetch-status {
  margin: 0 0 10px;
  font-size: 14px;
  color: var(--ss-text);
}

/* Ein fehlgeschlagener Abruf faellt sonst zwischen den uebrigen Zeilen nicht
   auf — und er ist die einzige, die zum Handeln auffordert. */
.fetch-status.bad {
  color: var(--ss-error);
}

.fetch-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.fetch-actions .link {
  padding: 0;
  border: none;
  background: none;
  font: inherit;
  font-size: 13px;
  color: var(--ss-text-dim);
  cursor: pointer;
}

.resync {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 10px;
  font-size: 13px;
}

.resync .link {
  padding: 0;
  border: none;
  background: none;
  font: inherit;
  color: var(--ss-text-dim);
  cursor: pointer;
}

.resync .progress {
  color: var(--ss-accent);
  font-variant-numeric: tabular-nums;
}

.fetch-log {
  margin-top: 12px;
}

.fetch-log .hint {
  margin: 0 0 8px;
  font-size: 12px;
  color: var(--ss-text-dim);
}

.fetch-log ol {
  list-style: none;
  margin: 0;
  padding: 0;
  /* Begrenzt, damit 50 Zeilen den Dialog nicht sprengen. */
  max-height: 220px;
  overflow-y: auto;
  font-size: 12px;
}

.fetch-log li {
  display: flex;
  gap: 10px;
  padding: 4px 0;
  border-top: 1px solid var(--ss-border-soft);
  color: var(--ss-text-dim);
}

.fetch-log li.bad {
  color: var(--ss-error);
}

.fetch-log .when {
  flex: 0 0 90px;
}

.fetch-log .trigger {
  flex: 0 0 72px;
}

/* Nicht `.result`: so heisst im selben Dialog schon die Meldung des
   Verbindungstests, und die ist rot, solange sie nicht `ok` ist. Am Geraet
   stand deshalb jede Protokollzeile in Fehlerfarbe, auch die geglueckten. */
.fetch-log .outcome {
  flex: 1 1 auto;
  min-width: 0;
  overflow-wrap: anywhere;
}

.senders {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.senders li {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 8px;
  background: var(--ss-surface-2, rgba(255, 255, 255, 0.03));
}

/* Lange Adressen kuerzen statt umbrechen — die Zeile bleibt so hoch wie die
   Schaltflaeche daneben. */
.sender-address {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sender-count {
  flex: 0 0 auto;
  font-size: 13px;
  color: var(--ss-text-dim);
}

.sender-remove {
  flex: 0 0 auto;
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  padding: 0;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--ss-text-dim);
  cursor: pointer;
}

.sender-remove:hover:not(:disabled) {
  color: var(--ss-error);
}

.sender-remove:disabled {
  opacity: 0.4;
}

.senders-empty {
  margin: 0;
  font-size: 13px;
  color: var(--ss-text-dim);
}

.kinds {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 18px 0 8px;
  padding: 0;
  border: none;
}

.kinds legend {
  padding: 0;
  margin-bottom: 10px;
}

.kind {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 13px 16px;
  border: 1px solid var(--ss-border);
  border-radius: var(--ss-radius-nav);
  cursor: pointer;
  min-height: var(--ss-touch-target);
}

.kind:has(input:checked) {
  background: var(--ss-surface-accent);
  border-color: var(--ss-border-strong);
}

.kind input {
  margin-top: 3px;
  accent-color: var(--ss-accent);
}

.kind-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.kind-title {
  font-size: 15px;
  color: var(--ss-text-strong);
}

.kind-hint {
  font-size: 13px;
  color: var(--ss-text-dim);
}

.album-row {
  display: flex;
  gap: 10px;
}

.dimensions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.times {
  color: var(--ss-text-faint);
}

.narrow {
  width: 110px;
}

.primary,
.secondary {
  padding: 0 22px;
  border-radius: var(--ss-radius-pill);
  font-size: 15px;
  font-weight: 500;
  transition: background var(--ss-transition), color var(--ss-transition);
}

.primary {
  background: var(--ss-accent);
  color: var(--ss-bg);
}

.primary:active {
  background: var(--ss-accent-hover);
}

.secondary {
  border: 1px solid var(--ss-border-strong);
  color: var(--ss-text-body);
}

.secondary:active:not(:disabled) {
  background: var(--ss-surface-accent);
  color: var(--ss-accent);
}

.secondary:disabled {
  opacity: 0.5;
  cursor: default;
}

.danger {
  padding: 0 18px;
  border-radius: var(--ss-radius-pill);
  color: var(--ss-error);
  font-size: 15px;
}

.danger:active {
  background: rgba(196, 102, 134, 0.12);
}

.error,
/* Sitzt in der Fusszeile neben „Verbindung testen" (E-33). Die Grundbreite
   von 240 px ist die Schwelle: passt sie daneben, steht die Meldung dort;
   sonst rutscht sie auf eine eigene Zeile und bleibt lesbar, statt sich in
   eine schmale Spalte zu quetschen. Servermeldungen koennen lang werden. */
.result {
  flex: 1 1 240px;
  min-width: 0;
  font-size: 14px;
  line-height: 1.35;
  color: var(--ss-error);
}

.result.ok {
  color: var(--ss-accent);
}

/* Flache Ansichten — ein Smartphone im Querformat hat rund 390 px Höhe.
   Kein Zielformat (RB-02 nennt Tablets), aber das übliche Testgerät. Ohne
   diese Regel bliebe vom scrollbaren Bereich des Dialogs kaum etwas übrig. */
@media (max-height: 520px) {
  .backdrop {
    padding: 8px;
  }

  .head {
    padding: 10px 20px;
  }

  .head .ss-wordmark {
    font-size: 18px;
  }

  .body {
    padding: 4px 20px 12px;
  }

  .foot {
    padding: 10px 20px;
  }

  .kinds {
    margin: 10px 0 4px;
  }

  .kind {
    padding: 8px 14px;
  }
}
</style>
