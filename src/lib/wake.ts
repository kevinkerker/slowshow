/**
 * Bildschirm wachhalten (FA-50).
 *
 * Die verlässliche Umsetzung ist nativ: `FLAG_KEEP_SCREEN_ON` in
 * `MainActivity.kt` gilt für das ganze Fenster und überlebt auch dann, wenn die
 * WebView die Wake-Lock-API nicht kennt. Diese Datei ist die Ergänzung für den
 * Desktop-Build und für WebViews, die den nativen Weg unterlaufen.
 *
 * Ein Fehlschlag ist bewusst folgenlos — der native Teil trägt die Anforderung.
 */

type WakeLockSentinelLike = { release: () => Promise<void>; released: boolean }

let sentinel: WakeLockSentinelLike | null = null

function wakeLockApi(): { request: (type: 'screen') => Promise<WakeLockSentinelLike> } | null {
  const nav = navigator as Navigator & {
    wakeLock?: { request: (type: 'screen') => Promise<WakeLockSentinelLike> }
  }
  return nav.wakeLock ?? null
}

export async function keepAwake(): Promise<boolean> {
  const api = wakeLockApi()
  if (!api) return false

  try {
    sentinel = await api.request('screen')
    // Android gibt die Sperre frei, wenn die App in den Hintergrund geht.
    // Beim Zurückkehren muss sie neu angefordert werden, sonst schläft das
    // Tablet im Dauerbetrieb doch irgendwann ein.
    document.addEventListener('visibilitychange', onVisibilityChange)
    return true
  } catch {
    return false
  }
}

async function onVisibilityChange() {
  if (document.visibilityState !== 'visible') return
  if (sentinel && !sentinel.released) return

  const api = wakeLockApi()
  if (!api) return
  try {
    sentinel = await api.request('screen')
  } catch {
    /* nächster Versuch beim nächsten Sichtbarwerden */
  }
}

export async function releaseAwake(): Promise<void> {
  document.removeEventListener('visibilitychange', onVisibilityChange)
  if (sentinel && !sentinel.released) {
    try {
      await sentinel.release()
    } catch {
      /* egal — die App wird ohnehin beendet */
    }
  }
  sentinel = null
}
