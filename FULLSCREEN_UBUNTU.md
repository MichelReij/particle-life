# Fullscreen Mode op Ubuntu/Linux

## Over Fullscreen Mode
Op Linux wordt de particle-life applicatie automatisch in fullscreen mode geopend zonder titelbalk voor maximale immersie.

## Hoe de applicatie starten
```bash
./target/debug/native_minimal
```

## Hoe de applicatie stoppen
Omdat er geen titelbalk is in fullscreen mode, kun je de applicatie op de volgende manieren stoppen:

### Methode 1: Escape toets
- Druk op **Escape** om de applicatie direct af te sluiten

### Methode 2: Q toets  
- Druk op **Q** om de applicatie af te sluiten

### Methode 3: Terminal (noodgeval)
Als de applicatie vastloopt:
1. Open een nieuwe terminal (Ctrl+Alt+T)
2. Zoek het proces: `ps aux | grep native_minimal`
3. Stop het proces: `kill [process_id]`

### Methode 4: System shortcuts
- **Alt+F4** (werkt soms nog steeds in fullscreen)
- **Ctrl+Alt+F1** om naar TTY te switchen, dan terug met **Ctrl+Alt+F7**

## Platform Verschillen
- **Linux**: Automatisch fullscreen zonder titelbalk
- **macOS/Windows**: Normale window met titelbalk en resize mogelijkheden

## Features in Fullscreen
- ✅ Volledige lightning detection met ESP32 synchronisatie
- ✅ Smart polling systeem voor optimale prestaties  
- ✅ Alle particle physics en visualisaties
- ✅ FPS display en debugging informatie
- ✅ ESP32 communicatie voor externe sensoren

## Debugging
Als je debugging informatie wilt zien:
```bash
# Start vanuit terminal om console output te zien
./target/debug/native_minimal
```

De applicatie toont nuttige informatie zoals:
- Lightning detection status
- ESP32 communicatie 
- FPS en prestatie metrics
- Smart polling timing
