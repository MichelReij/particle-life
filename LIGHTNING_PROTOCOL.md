# Lightning Event Protocol — PC → Liaison ESP32

## Overzicht

Wanneer er een bliksemflits optreedt in de particle-life simulatie, stuurt de Ubuntu-PC een kort pakketje naar de liaison ESP32 over de seriele verbinding. De liaison kan dit signaal gebruiken om een bliksemeffect te triggeren in de glazen bol (of verder door te sturen naar andere ESP32's in het netwerk).

## Verbinding

- **Interface**: USB-serieel (dezelfde verbinding waarover de ESP32 ook sensordata stuurt)
- **Baud rate**: 115200
- **Richting**: PC → ESP32 (unidirectioneel, geen ACK verwacht)

## Pakketformaat

Een ASCII-regel, afgesloten met `\n` (LF, `0x0A`):

```
LIGHTNING:<flash_id>,<type>,<start_time>,<intensity>\n
```

### Velden

| Veld | Type | Voorbeeldwaarde | Betekenis |
|------|------|-----------------|-----------|
| `flash_id` | integer | `42` | Uniek oplopend ID per bliksemflits |
| `type` | integer | `0` of `1` | `0` = normale bliksem, `1` = superbliksem |
| `start_time` | float | `127.35` | Interne simulatietijd in seconden (zie noot) |
| `intensity` | float | `0.70` of `1.00` | `0.70` voor normaal, `1.00` voor super |

### Voorbeelden

```
LIGHTNING:42,0,127.35,0.70\n    ← normale bliksem
LIGHTNING:43,1,132.80,1.00\n    ← superbliksem
```

### Noot over start_time

`start_time` is de interne simulatietijd (seconden sinds opstart). Deze waarde heeft geen relatie met een realtime klok en is niet bruikbaar voor synchronisatie. Gebruik het moment van ontvangst als trigger. `start_time` is puur diagnostisch.

## Timing

- Het pakketje wordt verstuurd **zodra** een nieuwe bliksem gedetecteerd wordt
- Elke flits genereert **exact één pakketje**
- Er is geen rate-limiting; pakketjes komen zo snel als bliksemflitsen optreden (typisch elke paar seconden, soms sneller bij hoge elektrische activiteit)

## Wat de liaison moet doen

1. **Parsen**: lees seriele input regel voor regel tot `\n`
2. **Herkennen**: check of de regel begint met `LIGHTNING:`
3. **Dedupliceren**: sla het laatste ontvangen `flash_id` op; negeer pakketjes met een `flash_id` dat je al gezien hebt
4. **Reageren**:
   - `type == 0`: trigger een **normaal bliksemeffect** in de bol
   - `type == 1`: trigger een **superbliksemeffect** (intenser/langer) in de bol
5. **Doorsturen** (optioneel): stuur het signaal verder naar andere ESP32's in het netwerk indien nodig

## Voorbeeldcode (Arduino/C++)

```cpp
String serialBuffer = "";

void loop() {
    // Lees seriele data
    while (Serial.available()) {
        char c = Serial.read();
        if (c == '\n') {
            handleSerialLine(serialBuffer);
            serialBuffer = "";
        } else {
            serialBuffer += c;
        }
    }
    // ... rest van loop
}

uint32_t lastFlashId = 0;

void handleSerialLine(String line) {
    if (!line.startsWith("LIGHTNING:")) return;

    line.remove(0, 10); // verwijder "LIGHTNING:"

    uint32_t flash_id = line.substring(0, line.indexOf(',')).toInt();
    line = line.substring(line.indexOf(',') + 1);
    int type = line.substring(0, line.indexOf(',')).toInt();
    // start_time en intensity kunnen worden genegeerd

    // Deduplicatie
    if (flash_id <= lastFlashId) return;
    lastFlashId = flash_id;

    if (type == 1) {
        triggerSuperLightning();
    } else {
        triggerNormalLightning();
    }
}
```
