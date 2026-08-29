import { createRouter, createWebHashHistory } from 'vue-router'
import SlideshowView from '@/views/SlideshowView.vue'

/**
 * Nur zwei Ansichten: die Diashow und die Einstellungen.
 *
 * `createWebHashHistory`, weil das Frontend im Release als Dateien aus dem
 * APK geladen wird — ein History-Router bräuchte einen Server, der alle Pfade
 * auf index.html abbildet.
 */
export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', name: 'slideshow', component: SlideshowView },
    {
      path: '/settings',
      name: 'settings',
      // Nachgeladen: die Einstellungen werden selten geöffnet (Lastenheft 1.4),
      // brauchen also nicht im Startbündel zu liegen.
      component: () => import('@/views/SettingsView.vue'),
    },
    { path: '/:pathMatch(.*)*', redirect: '/' },
  ],
})
