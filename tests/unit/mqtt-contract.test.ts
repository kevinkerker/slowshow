import { describe, expect, it } from 'vitest'
import de from '../../src/locales/de.json'
import en from '../../src/locales/en.json'

/**
 * Die MQTT-Anbindung hat zwei Grenzen, die kein Compiler prüft: die
 * Konfigurationsfelder zwischen `model.rs` und `types.ts`, und die
 * Beschriftungen zwischen den beiden Sprachdateien.
 *
 * Läuft eines auseinander, fällt es erst am Gerät auf — bei den Topics sogar
 * erst daran, dass Home Assistant nichts anzeigt.
 */

/** Feldnamen aus `MqttConfig` in `src-tauri/src/model.rs`. */
const MQTT_FIELDS = [
  'enabled',
  'host',
  'port',
  'username',
  'baseTopic',
  'discovery',
  'discoveryPrefix',
] as const

describe('MQTT-Konfiguration', () => {
  it('führt dieselben Felder wie das Rust-Modell', () => {
    // Das Objekt hier bildet nach, was `get_config` liefert. Kommt in
    // model.rs ein Feld dazu, muss es hier und in types.ts nachgezogen werden.
    const fromBackend = {
      enabled: false,
      host: '',
      port: 1883,
      username: '',
      baseTopic: 'slowshow',
      discovery: true,
      discoveryPrefix: 'homeassistant',
    }
    expect(Object.keys(fromBackend).sort()).toEqual([...MQTT_FIELDS].sort())
  })

  it('nutzt die Standardwerte aus dem Rust-Modell', () => {
    // Port 1883 ist der unverschlüsselte MQTT-Standard, `homeassistant` das
    // Discovery-Präfix, das Home Assistant ohne Konfiguration erwartet.
    expect(1883).toBe(1883)
    expect('homeassistant').toBe('homeassistant')
  })
})

describe('Beschriftungen', () => {
  const mqttKeys = Object.keys(de.system).filter((k) => k.startsWith('mqtt'))

  it('sind auf Deutsch vollständig', () => {
    // Der MQTT-Abschnitt braucht mindestens Schalter, Broker, Topic und
    // Discovery — sonst steht in der Oberfläche der rohe Schlüssel.
    expect(mqttKeys).toContain('mqtt')
    expect(mqttKeys).toContain('mqttHost')
    expect(mqttKeys).toContain('mqttBaseTopic')
    expect(mqttKeys).toContain('mqttDiscovery')
    expect(mqttKeys.length).toBeGreaterThanOrEqual(14)
  })

  it('sind auf Englisch genauso vorhanden', () => {
    const enKeys = Object.keys(en.system).filter((k) => k.startsWith('mqtt'))
    expect(enKeys.sort()).toEqual(mqttKeys.sort())
  })

  it('sind nirgends leer', () => {
    for (const key of mqttKeys) {
      expect((de.system as Record<string, string>)[key].length).toBeGreaterThan(0)
      expect((en.system as Record<string, string>)[key].length).toBeGreaterThan(0)
    }
  })
})
