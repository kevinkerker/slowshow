<script setup lang="ts">
/**
 * Auswahl der Unterordner einer Quelle (FA-29, E-70).
 *
 * Ersetzt das freie Textfeld „Nur diese Unterordner". Dort ergab ein
 * Tippfehler still 0 Fotos, „alles außer `Sent`" war nicht ausdrückbar, und
 * ein Tester hielt den Platzhalter für einen eingetragenen Wert (E-68).
 *
 * Jede Zeile trägt den Schalter aus dem Entwurf (E-58). Ebenen werden erst
 * beim Aufklappen gelesen: den ganzen Baum eines NAS vorab zu holen hieße
 * minutenlang warten, bevor sich der Dialog bedienen lässt.
 */
import { computed, reactive, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import SsButton from './SsButton.vue'
import SsIconButton from './SsIconButton.vue'
import ToggleSwitch from './ToggleSwitch.vue'
import * as api from '@/lib/api'
import { dirOn, hasOwnDecision, setDir, within, type FolderSelection } from '@/lib/subfolders'
import type { Source, SubfolderEntry } from '@/lib/types'

const props = defineProps<{
  /** Entwurf der Quelle zum Auflisten. `null`, solange Ordner oder Adresse fehlen. */
  source: Source | null
  /** Passwort aus dem Formular; leer heißt „das gespeicherte". */
  password: string
  /** Name des gewählten Ordners, für die Zeile „Bilder direkt in …". */
  rootName: string
  /**
   * `.nomedia`-Ordner beim ersten Lesen abschalten (E-70). Nur für einen frisch
   * gewählten Ordner: bestehende Quellen sollen sich nicht still ändern.
   */
  hideNomedia: boolean
  /** Sofort lesen? Beim lokalen Ordner ja; beim NAS erst auf Wunsch, weil die Adresse noch getippt wird. */
  autoLoad: boolean
}>()

const selection = defineModel<FolderSelection>({ required: true })

const { t } = useI18n()

type Level =
  | { status: 'loading' }
  | { status: 'ok'; entries: SubfolderEntry[] }
  | { status: 'error'; error: string }

const levels = reactive<Record<string, Level>>({})
const open = ref(new Set<string>())

/** Ändert sich Ordner oder Adresse, gilt nichts Gelesenes mehr. */
const sourceKey = computed(() => (props.source ? JSON.stringify(props.source.kind) : ''))

watch(
  sourceKey,
  () => {
    for (const key of Object.keys(levels)) delete levels[key]
    open.value = new Set()
    if (props.source && props.autoLoad) void load('')
  },
  { immediate: true },
)

async function load(path: string) {
  const source = props.source
  if (!source) return
  levels[path] = { status: 'loading' }
  try {
    const entries = await api.listSubfolders(source, props.password, path)
    // Inzwischen ein anderer Ordner gewählt? Dann gehört das Ergebnis nicht hierher.
    if (sourceKey.value !== JSON.stringify(source.kind)) return
    levels[path] = { status: 'ok', entries }
    if (props.hideNomedia) {
      let next = selection.value
      for (const e of entries) {
        if (e.nomedia && !hasOwnDecision(next, e.path) && dirOn(next, e.path)) {
          next = setDir(next, e.path, false)
        }
      }
      if (next !== selection.value) selection.value = next
    }
  } catch (e) {
    levels[path] = { status: 'error', error: e instanceof Error ? e.message : String(e) }
  }
}

function toggleOpen(entry: SubfolderEntry) {
  const next = new Set(open.value)
  if (next.has(entry.path)) {
    next.delete(entry.path)
  } else {
    next.add(entry.path)
    if (!levels[entry.path] || levels[entry.path].status === 'error') void load(entry.path)
  }
  open.value = next
}

function set(path: string, on: boolean) {
  selection.value = setDir(selection.value, path, on)
}

type Row =
  | { type: 'entry'; entry: SubfolderEntry; depth: number }
  | { type: 'loading'; path: string; depth: number }
  | { type: 'error'; path: string; error: string; depth: number }

/** Der sichtbare Baum als flache Liste — so bleibt die Vorlage ohne Rekursion. */
const rows = computed<Row[]>(() => {
  const out: Row[] = []
  const walk = (path: string, depth: number) => {
    const level = levels[path]
    if (!level) return
    if (level.status === 'loading') out.push({ type: 'loading', path, depth })
    else if (level.status === 'error') out.push({ type: 'error', path, error: level.error, depth })
    else {
      for (const entry of level.entries) {
        out.push({ type: 'entry', entry, depth })
        if (open.value.has(entry.path)) walk(entry.path, depth + 1)
      }
    }
  }
  walk('', 0)
  return out
})

const rootLevel = computed(() => levels[''])

/**
 * Entscheidungen für Ordner, die es in einer gelesenen Ebene nicht (mehr)
 * gibt — umbenannt, gelöscht oder aus einem alten Textfeld vertippt. Ohne
 * diese Zeilen hinge eine unsichtbare Einstellung an der Quelle.
 */
const stale = computed(() => {
  const decisions = [...selection.value.include, ...selection.value.exclude]
  return decisions.filter((path) => {
    const parent = path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : ''
    const level = levels[parent]
    return level?.status === 'ok' && !level.entries.some((e) => e.path === path)
  })
})

function forget(path: string) {
  selection.value = {
    ...selection.value,
    include: selection.value.include.filter((d) => !within(d, path)),
    exclude: selection.value.exclude.filter((d) => !within(d, path)),
  }
}

function indent(depth: number) {
  return depth > 0 ? { paddingLeft: `calc(${depth} * var(--ss-space-3))` } : undefined
}
</script>

<template>
  <div class="picker">
    <p v-if="!source" class="hint">
      {{ autoLoad ? t('sourceForm.subfoldersNeedFolder') : t('sourceForm.subfoldersNeedUrl') }}
    </p>

    <template v-else>
      <div class="row root">
        <span class="name">{{ t('sourceForm.subfoldersRoot', { name: rootName }) }}</span>
        <ToggleSwitch
          :model-value="selection.includeRoot"
          :label="t('sourceForm.subfoldersRoot', { name: rootName })"
          @update:model-value="set('', $event)"
        />
      </div>

      <div v-if="!rootLevel" class="row">
        <SsButton variant="secondary" @click="load('')">{{ t('sourceForm.subfoldersLoad') }}</SsButton>
      </div>
      <p v-else-if="rootLevel.status === 'ok' && rootLevel.entries.length === 0" class="hint">
        {{ t('sourceForm.subfoldersNone') }}
      </p>

      <ul class="tree">
        <template v-for="row in rows" :key="row.type === 'entry' ? row.entry.path : `${row.type}:${row.path}`">
          <li v-if="row.type === 'entry'" class="row" :style="indent(row.depth)">
            <SsIconButton
              v-if="row.entry.hasChildren"
              class="expand"
              :class="{ open: open.has(row.entry.path) }"
              :label="
                open.has(row.entry.path)
                  ? t('sourceForm.subfoldersCollapse', { name: row.entry.name })
                  : t('sourceForm.subfoldersExpand', { name: row.entry.name })
              "
              @click="toggleOpen(row.entry)"
            >
              <svg width="16" height="16" viewBox="0 0 20 20" fill="none" stroke="currentColor"
                   stroke-width="1.5" stroke-linecap="round" aria-hidden="true">
                <path d="M8 4 L14 10 L8 16" />
              </svg>
            </SsIconButton>
            <span v-else class="expand-space" aria-hidden="true" />
            <span class="name">{{ row.entry.name }}</span>
            <span v-if="row.entry.nomedia" class="badge">{{ t('sourceForm.subfoldersNomedia') }}</span>
            <ToggleSwitch
              :model-value="dirOn(selection, row.entry.path)"
              :label="row.entry.name"
              @update:model-value="set(row.entry.path, $event)"
            />
          </li>
          <li v-else-if="row.type === 'loading'" class="row status" :style="indent(row.depth)">
            {{ t('sourceForm.subfoldersLoading') }}
          </li>
          <li v-else class="row status bad" :style="indent(row.depth)">
            <span class="name">{{ t('sourceForm.subfoldersError', { error: row.error }) }}</span>
            <SsButton variant="link" @click="load(row.path)">{{ t('sourceForm.subfoldersRetry') }}</SsButton>
          </li>
        </template>

        <li v-for="path in stale" :key="`stale:${path}`" class="row stale">
          <span class="expand-space" aria-hidden="true" />
          <span class="name">{{ path }}</span>
          <span class="badge">{{ t('sourceForm.subfoldersMissing') }}</span>
          <SsIconButton :label="t('sourceForm.subfoldersForget', { name: path })" @click="forget(path)">
            <svg width="16" height="16" viewBox="0 0 20 20" fill="none" stroke="currentColor"
                 stroke-width="1.5" stroke-linecap="round" aria-hidden="true">
              <path d="M5 5 L15 15 M15 5 L5 15" />
            </svg>
          </SsIconButton>
        </li>
      </ul>
    </template>
  </div>
</template>

<style scoped>
.picker {
  display: flex;
  flex-direction: column;
  width: 100%;
  min-width: 0;
}

.tree {
  list-style: none;
  margin: 0;
  padding: 0;
}

.row {
  display: flex;
  align-items: center;
  gap: var(--ss-space-1);
  min-height: var(--ss-touch-target);
  border-top: 1px solid var(--ss-border-soft);
  font-size: var(--ss-fs-m);
  color: var(--ss-text-body);
}

.row.root {
  border-top: none;
}

.name {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Platzhalter in der Breite des Aufklappknopfs, damit die Namen fluchten. */
.expand-space {
  flex: 0 0 var(--ss-touch-target);
}

.expand svg {
  transition: transform var(--ss-transition);
}

.expand.open svg {
  transform: rotate(90deg);
}

.badge {
  flex: 0 0 auto;
  font-size: var(--ss-fs-s);
  color: var(--ss-text-muted);
}

.status {
  color: var(--ss-text-dim);
}

.status.bad {
  color: var(--ss-error);
}

.stale .name {
  color: var(--ss-text-muted);
}

.hint {
  margin: 0;
  padding: var(--ss-space-1) 0;
  font-size: var(--ss-fs-s);
  color: var(--ss-text-dim);
}
</style>
