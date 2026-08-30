# Slowshow in Home Assistant einbinden

Es gibt zwei Wege, beide nach FA-55. Alle Beispiele sind gegen die laufende App
auf einem echten Gerät geprüft.

| | MQTT (empfohlen) | REST |
|---|---|---|
| Einrichtung in Home Assistant | keine — die Entitäten erscheinen von allein | YAML aus Abschnitt 3 |
| Feste Adresse fürs Tablet | **nicht nötig** | nötig |
| Zustandsänderungen | sofort (Push) | beim nächsten Abruf |
| Ausfall erkennbar | sofort (Last Will) | erst beim fehlgeschlagenen Abruf |
| Voraussetzung | ein Broker (z. B. Mosquitto-Add-on) | keine |

**Nimm MQTT, wenn du einen Broker hast.** REST bleibt sinnvoll für schnelle
Tests mit `curl` und wenn kein Broker läuft. Beide Wege können gleichzeitig
aktiv sein und lösen dieselben Aktionen aus.

---

## MQTT — der kurze Weg

In Slowshow: **Einstellungen → System → MQTT** einschalten und eintragen:

| Feld | Beispiel | Bemerkung |
|---|---|---|
| Broker-Adresse | `homeassistant.local` | oder die IP des Brokers |
| Port | `1883` | Standard ohne Verschlüsselung |
| Benutzername / Passwort | wie im Broker angelegt | Passwort wird verschlüsselt abgelegt (NF-05) |
| Basistopic | `slowshow` | bei mehreren Rahmen je Gerät ein eigener |
| Automatisch anmelden | an | legt die Entitäten selbst an |

Das war es. Nach dem Verbinden erscheint in Home Assistant ein Gerät
**Slowshow** mit zwölf Entitäten:

- **Schalter:** Diashow, Bildschirm, Helligkeit vom Gerät
- **Knöpfe:** Nächstes Bild, Vorheriges Bild, Synchronisieren
- **Zahlen:** Anzeigedauer (5 s … 30 min), Helligkeit (1 … 100 %)
- **Sensoren:** Bilder im Cache, Cachegröße
- **Binärsensoren:** Synchronisiert, Aktivzeit

Kein YAML nötig. Abschnitt 3 brauchst du nur für den REST-Weg.

### Topics

Falls du selbst mitlesen oder ohne Discovery arbeiten willst:

| Topic | Richtung | Inhalt |
|---|---|---|
| `slowshow/availability` | Rahmen → HA | `online` / `offline` (retained, Last Will) |
| `slowshow/state` | Rahmen → HA | Gesamtzustand als JSON (retained) |
| `slowshow/cmd/slideshow` | HA → Rahmen | `ON` / `OFF` |
| `slowshow/cmd/screen` | HA → Rahmen | `ON` / `OFF` |
| `slowshow/cmd/next` | HA → Rahmen | beliebig |
| `slowshow/cmd/prev` | HA → Rahmen | beliebig |
| `slowshow/cmd/sync` | HA → Rahmen | beliebig |
| `slowshow/cmd/interval` | HA → Rahmen | Sekunden als Zahl |
| `slowshow/cmd/brightness` | HA → Rahmen | Prozent als Zahl |
| `slowshow/cmd/device_brightness` | HA → Rahmen | `ON` / `OFF` |
| `slowshow/cmd/config` | HA → Rahmen | Teilmenge als JSON (siehe Abschnitt 5) |

Schaltbefehle akzeptieren `ON`/`OFF`, `true`/`false`, `1`/`0` und `an`/`aus`.

Der Zustand wird gesendet, sobald sich etwas ändert (mit kurzer Sammelfrist,
damit ein Sync nicht pro Bild eine Nachricht erzeugt) und zusätzlich einmal pro
Minute.

### Automatisierung mit MQTT

Mit Discovery brauchst du keine `rest_command`s mehr — die Entitäten sind da:

```yaml
automation:
  - alias: "Slowshow wecken"
    triggers:
      - trigger: state
        entity_id: binary_sensor.wohnzimmer_bewegung
        to: "on"
    actions:
      - action: switch.turn_on
        target:
          entity_id: switch.slowshow_bildschirm

  - alias: "Slowshow schlafen legen"
    triggers:
      - trigger: state
        entity_id: binary_sensor.wohnzimmer_bewegung
        to: "off"
        for: "00:20:00"
    actions:
      - action: switch.turn_off
        target:
          entity_id: switch.slowshow_bildschirm
```

---

# REST — der Weg ohne Broker

Ab hier geht es um die REST-Anbindung. Wer MQTT nutzt, kann den Rest
überspringen.

---

## 1. Vorbereiten

**In Slowshow:** Einstellungen → System → *Steuerung im Heimnetz* einschalten.
Port bleibt 8127. Ein Token ist optional; im eigenen Heimnetz hinter dem Router
kann das Feld leer bleiben.

**Im Router:** dem Tablet eine feste Adresse geben (DHCP-Reservierung).
Ohne das zeigt die Konfiguration irgendwann ins Leere.

**Kurz prüfen** — von einem Rechner im selben Netz:

```bash
curl http://192.168.1.42:8127/api/status
```

Kommt JSON zurück, ist alles bereit. Kommt nichts:
Steuerung eingeschaltet? Gleiches WLAN? App im Vordergrund?

---

## 2. configuration.yaml

Adresse anpassen. Bei gesetztem Token überall
`Authorization: "Bearer DEIN_TOKEN"` unter `headers:` ergänzen.

```yaml
# ── Zustand lesen ──────────────────────────────────────────────────────────
# Ein Abruf, mehrere Entitäten — spart Anfragen gegenüber je einem
# rest-Sensor pro Wert.
rest:
  - resource: "http://192.168.1.42:8127/api/status"
    scan_interval: 30
    # Der Rahmen kann aus sein; dann soll HA nicht bei jedem Abruf meckern.
    timeout: 10
    sensor:
      - name: "Slowshow Bilder im Cache"
        unique_id: slowshow_cache_images
        value_template: "{{ value_json.cache.images }}"
        unit_of_measurement: "Bilder"
        state_class: measurement
        icon: mdi:image-multiple

      - name: "Slowshow Cachegröße"
        unique_id: slowshow_cache_size
        value_template: "{{ (value_json.cache.bytes / 1048576) | round(1) }}"
        unit_of_measurement: "MB"
        state_class: measurement

      - name: "Slowshow Anzeigedauer"
        unique_id: slowshow_interval
        value_template: "{{ value_json.intervalSeconds }}"
        unit_of_measurement: "s"

      - name: "Slowshow Helligkeit"
        unique_id: slowshow_brightness
        value_template: "{{ value_json.brightness }}"
        unit_of_measurement: "%"

    binary_sensor:
      - name: "Slowshow Diashow läuft"
        unique_id: slowshow_playing
        value_template: "{{ value_json.playing == true }}"
        icon: mdi:play-circle

      - name: "Slowshow synchronisiert"
        unique_id: slowshow_syncing
        value_template: "{{ value_json.syncing == true }}"
        device_class: running

      - name: "Slowshow Aktivzeit"
        unique_id: slowshow_active
        value_template: "{{ value_json.display.slideshowActive == true }}"
        icon: mdi:clock-outline

# ── Befehle senden ─────────────────────────────────────────────────────────
rest_command:
  slowshow_play:
    url: "http://192.168.1.42:8127/api/slideshow"
    method: POST
    content_type: "application/json"
    payload: '{"on": true}'

  slowshow_pause:
    url: "http://192.168.1.42:8127/api/slideshow"
    method: POST
    content_type: "application/json"
    payload: '{"on": false}'

  slowshow_screen_on:
    url: "http://192.168.1.42:8127/api/screen"
    method: POST
    content_type: "application/json"
    payload: '{"on": true}'

  slowshow_screen_off:
    url: "http://192.168.1.42:8127/api/screen"
    method: POST
    content_type: "application/json"
    payload: '{"on": false}'

  slowshow_next:
    url: "http://192.168.1.42:8127/api/next"
    method: POST

  slowshow_prev:
    url: "http://192.168.1.42:8127/api/prev"
    method: POST

  slowshow_sync:
    url: "http://192.168.1.42:8127/api/sync"
    method: POST
    # Ein Sync über tausende Bilder dauert; HA soll nicht vorher abbrechen.
    timeout: 300

  # Grundeinstellungen. Alle Felder sind einzeln setzbar (E-09).
  slowshow_set_interval:
    url: "http://192.168.1.42:8127/api/config"
    method: POST
    content_type: "application/json"
    payload: '{"intervalSeconds": {{ seconds | int }}}'

  slowshow_set_brightness:
    url: "http://192.168.1.42:8127/api/config"
    method: POST
    content_type: "application/json"
    payload: '{"brightness": {{ level | int }}}'

  slowshow_set_schedule:
    url: "http://192.168.1.42:8127/api/config"
    method: POST
    content_type: "application/json"
    payload: >-
      {"scheduleEnabled": {{ enabled | lower }},
       "activeFrom": "{{ start }}", "activeTo": "{{ end }}"}

# ── Bedienelemente ─────────────────────────────────────────────────────────
switch:
  - platform: template
    switches:
      slowshow_diashow:
        friendly_name: "Slowshow"
        unique_id: slowshow_switch
        value_template: "{{ is_state('binary_sensor.slowshow_diashow_lauft', 'on') }}"
        icon_template: >-
          {{ 'mdi:play-circle' if is_state('binary_sensor.slowshow_diashow_lauft', 'on')
             else 'mdi:pause-circle' }}
        turn_on:
          service: rest_command.slowshow_play
        turn_off:
          service: rest_command.slowshow_pause
```

Nach dem Neustart von Home Assistant sind die Entitäten da.

> **Zu den Namen:** Der `rest`-Sensor `"Slowshow Diashow läuft"` wird zu
> `binary_sensor.slowshow_diashow_lauft` — Umlaute fallen weg. Wenn dein
> Template-Schalter „unbekannt" bleibt, ist meist genau das die Ursache; die
> tatsächliche Entitäts-Id stehen in den Entwicklerwerkzeugen.

---

## 3. Dashboard

```yaml
type: entities
title: Bilderrahmen
entities:
  - entity: switch.slowshow_diashow
  - entity: binary_sensor.slowshow_aktivzeit
  - entity: sensor.slowshow_bilder_im_cache
  - entity: sensor.slowshow_cachegrosse
  - entity: binary_sensor.slowshow_synchronisiert
  - type: buttons
    entities:
      - entity: switch.slowshow_diashow
        name: Zurück
        tap_action:
          action: call-service
          service: rest_command.slowshow_prev
      - entity: switch.slowshow_diashow
        name: Weiter
        tap_action:
          action: call-service
          service: rest_command.slowshow_next
      - entity: switch.slowshow_diashow
        name: Sync
        tap_action:
          action: call-service
          service: rest_command.slowshow_sync
```

---

## 4. Automatisierungen

### Bewegungsmelder weckt den Rahmen

Das ist der Fall, für den die Kamera-Präsenzerkennung gestrichen wurde
(Entscheidung E-05): der Bewegungsmelder im Raum übernimmt das Aufwecken.

```yaml
automation:
  - alias: "Slowshow wecken"
    triggers:
      - trigger: state
        entity_id: binary_sensor.wohnzimmer_bewegung
        to: "on"
    actions:
      - action: rest_command.slowshow_screen_on

  - alias: "Slowshow schlafen legen"
    triggers:
      - trigger: state
        entity_id: binary_sensor.wohnzimmer_bewegung
        to: "off"
        for: "00:20:00"
    actions:
      - action: rest_command.slowshow_screen_off
```

### Abends dunkler

Der Zeitplan der App kann das auch allein (FA-52/FA-53). Über Home Assistant
lohnt es sich, wenn die Helligkeit von etwas anderem abhängen soll — etwa
davon, ob der Fernseher läuft.

```yaml
  - alias: "Slowshow abends dimmen"
    triggers:
      - trigger: sun
        event: sunset
    actions:
      - action: rest_command.slowshow_set_brightness
        data:
          level: 35
```

### Nachts synchronisieren

Die App synchronisiert selbst nach dem eingestellten Intervall (FA-28). Eine
zusätzliche nächtliche Runde lohnt, wenn dein NAS tagsüber schläft.

```yaml
  - alias: "Slowshow nachts synchronisieren"
    triggers:
      - trigger: time
        at: "03:30:00"
    actions:
      - action: rest_command.slowshow_sync
```

---

## 5. Endpunkte im Überblick

| Endpunkt | Methode | Rumpf | Antwort |
| --- | --- | --- | --- |
| `/api/status` | GET | — | Gesamtzustand (siehe unten) |
| `/api/slideshow` | POST | `{"on": true}` | `{"playing": true}` |
| `/api/screen` | POST | `{"on": true}` | `{"screen": true}` |
| `/api/next` | POST | — | `{"slide": {...}}` |
| `/api/prev` | POST | — | `{"slide": {...}}` |
| `/api/sync` | POST | — | `{"reports": [...]}` |
| `/api/config` | GET | — | Grundeinstellungen |
| `/api/config` | POST | Teilmenge | die neuen Werte |

`POST /api/config` nimmt einzelne Felder — alles ist optional:

```json
{
  "intervalSeconds": 30,
  "scheduleEnabled": true,
  "activeFrom": "07:00",
  "activeTo": "22:00",
  "brightness": 80,
  "deviceBrightness": false
}
```

`deviceBrightness: true` gibt die Helligkeitsregelung an das Tablet zurück
(E-22). Die App setzt dann in **keinem** Zustand mehr eine Helligkeit — auch
nicht nachts und auch nicht auf `POST /api/screen {"on": false}`. Der Rahmen
wird außerhalb der Aktivzeit trotzdem schwarz, das erledigt die Oberfläche;
nur die Hintergrundbeleuchtung bleibt in der Hand des Geräts. `brightness`
wird währenddessen weiterhin gespeichert, wirkt aber erst wieder, wenn
`deviceBrightness` auf `false` steht.

Werte außerhalb der zulässigen Bereiche werden geklemmt, nicht abgewiesen: die
Anzeigedauer bleibt zwischen 5 Sekunden und 30 Minuten (FA-02), die Helligkeit
zwischen 1 und 100. Eine fehlerhafte Automatisierung kann den Rahmen also nicht
in einen unsinnigen Zustand bringen.

Antwort von `GET /api/status`:

```json
{
  "playing": true,
  "syncing": false,
  "intervalSeconds": 30,
  "brightness": 100,
  "deviceBrightness": false,
  "display": { "slideshowActive": true, "showNightClock": false, "brightness": 100 },
  "currentSlide": { "kind": "single", "id": "ea9c9c9a37489830" },
  "cache": { "images": 77, "bytes": 61341696, "maxBytes": 2147483648, "excluded": 0 },
  "sources": [
    { "id": "src-…", "name": "NAS Fotoarchiv", "enabled": true, "lastSync": 1788034487 }
  ]
}
```

`brightness` ist die *eingestellte* Grundhelligkeit und liegt immer zwischen 1
und 100 — der richtige Wert für einen Regler. `display.brightness` ist die
gerade wirksame: nachts `1` und bei gerätegesteuerter Helligkeit `0`. Wer sie
anzeigt, sollte beide Sonderfälle abfangen.

---

## 6. Grenzen

- **Polling, keine Push-Meldungen.** Home Assistant erfährt einen Bildwechsel
  erst beim nächsten Abruf. Wer den Zustand sofort will, nimmt MQTT.
- **Kein Autostart** (E-01). Nach einem Stromausfall antwortet der Rahmen erst,
  wenn die App wieder von Hand gestartet wurde.
- **Android kann den Server einfrieren.** Ohne Foreground-Service beendet die
  Energieverwaltung langlaufende Hintergrunddienste (R-04). Deshalb: die App in
  den Akku-Einstellungen auf „Nicht optimiert" setzen. Ob das über Tage trägt,
  muss der Dauertest zeigen.
- **Kein HTTPS und keine Authentifizierung per Voreinstellung.** Der Server ist
  für das Heimnetz gedacht. Wer ihn absichern will, setzt in den Einstellungen
  ein Token; nach außen gehört der Port ohnehin nicht freigegeben.
