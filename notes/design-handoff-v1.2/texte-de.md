# Neue und geänderte Texte (de.json)

Englische Fassung in `en.json` jeweils ergänzen.

## Neu (E-59, Rückfragedialog)

| Schlüssel (Vorschlag) | Text |
| --- | --- |
| `sourceForm.removeSenderTitle` | `{sender} entfernen?` |
| `sourceForm.removeSenderBody` | `Künftige Mails warten dann wieder auf Freigabe. Von dieser Adresse sind schon {n} Fotos da.` |
| `sourceForm.removeSenderKeep` | `Entfernen · Fotos bleiben sichtbar` |
| `sourceForm.removeSenderRequarantine` | `Entfernen · {n} Fotos zurück in die Quarantäne` |
| `sources.removeTitle` | `Quelle „{name}“ entfernen?` |
| `sources.removeBody` | `Die Bilder im Cache werden gelöscht, an der Quelle bleibt alles unverändert.` |

Die übrigen Rückfragen (`resyncAsk`, `resetHistoryAsk`, `databaseRepairAsk`) werden genauso in Titel (erste Zeile) und Text (Rest) geteilt. Die bestätigende Aktion trägt den Namen der Handlung, z. B. „Postfach neu abgleichen“, „Anzeige-Historie zurücksetzen“, „Aufräumen“.

## Entfällt

- `sourceForm.removeSenderAsk`: ersetzt durch die vier Schlüssel oben (enthielt „OK = … / Abbrechen = …“).

## Zu korrigieren

- `sourceForm.allowedSendersEmpty` nennt den Knopf „Absender vertrauen“, der heißt aber „Alle von {Adresse}“.

## Offen (Phase 3, F14 Anrede)

Die Mischung aus „du“ und „Sie“ bleibt, bis F14 entschieden ist.
