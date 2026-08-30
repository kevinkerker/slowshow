<script setup lang="ts">
/**
 * Einstellungen (Artboard „Einstellungen · Quellen").
 *
 * Kopfzeile mit Wortmarke und Schließen-Knopf, links die vier Bereiche, rechts
 * der Inhalt. Alle Einstellungen sind direkt auf dem Tablet erreichbar (FA-40)
 * und werden sofort gespeichert (FA-42) — es gibt bewusst keinen
 * „Speichern"-Knopf, den man auf einem Bilderrahmen vergessen könnte.
 */
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import SourcesPane from '@/components/panes/SourcesPane.vue'
import ShowPane from '@/components/panes/ShowPane.vue'
import SchedulePane from '@/components/panes/SchedulePane.vue'
import SystemPane from '@/components/panes/SystemPane.vue'

type Pane = 'sources' | 'show' | 'schedule' | 'system'

const router = useRouter()
const { t } = useI18n()
const pane = ref<Pane>('sources')

const NAV: Array<{ key: Pane; icon: string[] }> = [
  {
    key: 'sources',
    icon: [
      'M3 7 L3 18 A2 2 0 0 0 5 20 L19 20 A2 2 0 0 0 21 18 L21 9 A2 2 0 0 0 19 7 L12 7 L10 4.5 L5 4.5 A2 2 0 0 0 3 6.5 Z',
    ],
  },
  { key: 'show', icon: ['M7 15 L10.5 10.5 L13 13.5 L15.5 10.8 L17 12.6'] },
  { key: 'schedule', icon: ['M12 7.5 L12 12 L15 14'] },
  {
    key: 'system',
    icon: [
      'M12 3.5 L12 5.5 M12 18.5 L12 20.5 M3.5 12 L5.5 12 M18.5 12 L20.5 12 M6 6 L7.4 7.4 M16.6 16.6 L18 18 M18 6 L16.6 7.4 M7.4 16.6 L6 18',
    ],
  },
]

const title = computed(() => t(`nav.${pane.value}`))
</script>

<template>
  <div class="settings">
    <header class="head">
      <div class="titles">
        <span class="ss-wordmark">{{ t('app.name') }}</span>
        <span class="ss-label">{{ title }}</span>
      </div>
      <button class="close" :aria-label="t('nav.close')" @click="router.push('/')">
        <svg width="18" height="18" viewBox="0 0 20 20" fill="none" stroke="var(--ss-icon-soft)" stroke-width="1.5" stroke-linecap="round">
          <path d="M4 4 L16 16 M16 4 L4 16" />
        </svg>
      </button>
    </header>

    <div class="main">
      <nav class="nav">
        <button
          v-for="item in NAV"
          :key="item.key"
          class="nav-item"
          :class="{ active: pane === item.key }"
          @click="pane = item.key"
        >
          <svg
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            :stroke="pane === item.key ? 'var(--ss-accent)' : 'var(--ss-icon-muted)'"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <rect v-if="item.key === 'show'" x="3" y="5" width="18" height="14" rx="2" />
            <circle v-if="item.key === 'schedule'" cx="12" cy="12" r="8.5" />
            <circle v-if="item.key === 'system'" cx="12" cy="12" r="3" />
            <path v-for="(d, i) in item.icon" :key="i" :d="d" />
          </svg>
          <span class="nav-label">{{ t(`nav.${item.key}`) }}</span>
        </button>
      </nav>

      <div class="content">
        <SourcesPane v-if="pane === 'sources'" />
        <ShowPane v-else-if="pane === 'show'" />
        <SchedulePane v-else-if="pane === 'schedule'" />
        <SystemPane v-else />
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  background: var(--ss-bg);
  color: var(--ss-text-body);
  /* Anders als die Diashow darf die Bedienung nicht unter eine
     Display-Aussparung geraten. Im Querformat liegen die auf den kurzen
     Kanten, deshalb zaehlen hier vor allem links und rechts. */
  padding: env(safe-area-inset-top) env(safe-area-inset-right)
    env(safe-area-inset-bottom) env(safe-area-inset-left);
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 24px 40px;
  border-bottom: 1px solid var(--ss-border-soft);
  flex-shrink: 0;
}

.titles {
  display: flex;
  align-items: baseline;
  gap: 14px;
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

.main {
  display: flex;
  flex-grow: 1;
  min-height: 0;
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: var(--ss-nav-width);
  padding: 28px 20px;
  border-right: 1px solid var(--ss-border-soft);
  flex-shrink: 0;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 13px 16px;
  border-radius: var(--ss-radius-nav);
  color: var(--ss-text-muted);
  text-align: left;
  transition: background var(--ss-transition), color var(--ss-transition);
}

.nav-item.active {
  background: var(--ss-surface-accent);
  color: var(--ss-text-accent);
}

.nav-label {
  font-size: 15px;
}

.nav-item.active .nav-label {
  font-weight: 500;
}

.content {
  flex-grow: 1;
  min-width: 0;
  padding: 32px 40px;
  overflow: hidden;
}

@media (max-width: 900px) {
  .head {
    padding: 16px 20px;
  }

  .nav {
    padding: 16px 10px;
  }

  .nav-label {
    font-size: 13px;
  }

  .content {
    padding: 20px;
  }
}
</style>
