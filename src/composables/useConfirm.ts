/**
 * Rückfragen im eigenen Dialog statt `window.confirm()` (E-59).
 *
 * Der native Dialog brach den Stil und konnte nur OK und Abbrechen. Bei
 * „Absender entfernen" hieß OK deshalb „Fotos zurück in die Quarantäne" und
 * Abbrechen „sichtbar lassen" — entfernt wurde der Absender in beiden Fällen.
 * Hier trägt jeder Ausgang eine Aufschrift, die sagt, was passiert.
 *
 * Aufruf mit `await confirm({ title, body, actions })`. Das Ergebnis ist die
 * `id` der gewählten Aktion oder `'cancel'`. „Abbrechen" setzt der Dialog
 * selbst dazu; ebenso zählen ein Tipp auf den Hintergrund und die
 * Zurück-Taste von Android als Abbrechen.
 *
 * Es gibt nur eine offene Rückfrage zugleich. Kommt eine zweite, bevor die
 * erste beantwortet ist, gilt die erste als abgebrochen.
 */
import { readonly, shallowRef } from 'vue'
import type { Router } from 'vue-router'

export type ConfirmVariant = 'primary' | 'secondary' | 'danger'

export interface ConfirmAction {
  id: string
  label: string
  variant?: ConfirmVariant
}

export interface ConfirmRequest {
  title: string
  body?: string
  actions: ConfirmAction[]
}

interface Pending extends ConfirmRequest {
  resolve: (id: string) => void
}

export const CANCEL = 'cancel'

const pending = shallowRef<Pending | null>(null)

/** Die offene Rückfrage, für `SsConfirmHost`. */
export const confirmRequest = readonly(pending)

export function confirm(request: ConfirmRequest): Promise<string> {
  settle(CANCEL)
  return new Promise((resolve) => {
    pending.value = { ...request, resolve }
  })
}

/**
 * Kurzform für die häufigste Rückfrage: eine bestätigende Aktion, meist in
 * der Gefahrenfarbe, neben „Abbrechen".
 */
export async function confirmAction(
  title: string,
  body: string | undefined,
  label: string,
  variant: ConfirmVariant = 'danger',
): Promise<boolean> {
  const id = await confirm({ title, body, actions: [{ id: 'confirm', label, variant }] })
  return id === 'confirm'
}

/** Beantwortet die offene Rückfrage; ohne offene Rückfrage wirkungslos. */
export function settle(id: string) {
  const current = pending.value
  if (!current) return
  pending.value = null
  current.resolve(id)
}

/**
 * Zurück-Taste als Abbrechen.
 *
 * Android schickt „Zurück" an die WebView, die in der Verlaufsliste einen
 * Schritt zurückgeht — aus den Einstellungen also in die Diashow. Steht eine
 * Rückfrage offen, fängt dieser Wächter den Schritt ab, beantwortet sie mit
 * Abbrechen und bleibt, wo er ist; der Router stellt die Adresse dabei selbst
 * wieder her. Gibt die Abmeldefunktion zurück.
 */
export function installConfirmGuard(router: Router): () => void {
  return router.beforeEach(() => {
    if (!pending.value) return true
    settle(CANCEL)
    return false
  })
}
