<script setup lang="ts">
/**
 * Anlegen und Bearbeiten einer Quelle (FA-20, FA-21, FA-23, FA-29).
 *
 * Drei Quellenarten in einem Formular. Der Verbindungstest vor dem Speichern
 * ist bewusst prominent: eine Quelle, die erst beim nächtlichen Sync scheitert,
 * fällt auf einem unbeaufsichtigten Gerät niemandem auf.
 */
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import ToggleSwitch from './ToggleSwitch.vue'
import SettingRow from './SettingRow.vue'
import * as api from '@/lib/api'
import type { Album, Source, SourceKind } from '@/lib/types'

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

const albums = ref<Album[]>([])
const loadingAlbums = ref(false)
const testing = ref(false)
const testResult = ref<{ ok: boolean; message: string } | null>(null)
const formError = ref('')
const safAvailable = ref(true)

const isEdit = computed(() => props.source !== null)
const isRemote = computed(() => kind.value !== 'local')

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
    await api.testSource(draft, password.value)
    testResult.value = { ok: true, message: t('sourceForm.testOk') }
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
          <label v-for="option in (['local', 'webDav', 'nextcloud'] as const)" :key="option" class="kind">
            <input v-model="kind" type="radio" :value="option" />
            <span class="kind-body">
              <span class="kind-title">
                {{ t(option === 'local' ? 'sourceForm.kindLocal' : option === 'webDav' ? 'sourceForm.kindWebdav' : 'sourceForm.kindNextcloud') }}
              </span>
              <span class="kind-hint">
                {{ t(option === 'local' ? 'sourceForm.kindLocalHint' : option === 'webDav' ? 'sourceForm.kindWebdavHint' : 'sourceForm.kindNextcloudHint') }}
              </span>
            </span>
          </label>
        </fieldset>

        <SettingRow :label="t('sourceForm.name')" stacked>
          <input v-model="name" type="text" :placeholder="t('sourceForm.namePlaceholder')" />
        </SettingRow>

        <!-- Lokaler Ordner (FA-20) -->
        <template v-if="kind === 'local'">
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

          <SettingRow :label="t('sourceForm.syncInterval')">
            <select v-model.number="syncIntervalMinutes" class="narrow">
              <option :value="15">15 min</option>
              <option :value="60">1 h</option>
              <option :value="360">6 h</option>
              <option :value="1440">24 h</option>
            </select>
          </SettingRow>
        </template>

        <!-- Filter (FA-29) -->
        <SettingRow
          v-if="kind !== 'nextcloud'"
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
        <p v-if="testResult" class="result" :class="{ ok: testResult.ok }">
          {{ testResult.message }}
        </p>
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
        <button v-if="isRemote" class="secondary" :disabled="testing" @click="testConnection">
          {{ testing ? t('sourceForm.testing') : t('sourceForm.test') }}
        </button>
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
  gap: 10px;
  padding: 16px 24px;
  border-top: 1px solid var(--ss-border-soft);
}

.spacer {
  flex-grow: 1;
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
.result {
  margin-top: 14px;
  font-size: 14px;
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
