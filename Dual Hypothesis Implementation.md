# Dual Hypothesis Implementation

We ondersteunen momenteel de "Hydrothermal vents" hypothese, die zegt dat leven ontstond op de bodem van de oceaan, dus onder hoge druk, bij 100 tot 120 graden, pH 10 en electrische activiteit van ongeveer 2.
Dat is op zich prima zo, maar de vorige meest-populaire hypothese was de "warm little ponds" hypothese, die er vanuit ging dat het leven ontstond in getijdenpoelen, dus ongeveer op zeeniveau (diepte = 0 meter), bij temperaturen van rond de 40 tot 60 graden. En hier was pH misschien net iets minder belangrijk dan UV licht.

Dus wat ik nu wil gaan doen, is beide hypotheses implementeren. Dat betekent dat we de diepte-slider tweede "goede" gebieden (dus groen) moeten gaan geven: het huidige bereik van zo'n 400 tot 1000 meter, aangevuld met een bereik van zeg 20 tot 0 meter (en misschien moeten we de slider 'clampen' naar 0). Zo wordt de diepte/druk slider eigenlijk een soort 'schakelaar' die ofwel de WLP hypothese of de HTV hypothese activeert.

## HTV Hypothese

Deze werkt al goed, dus deze laten we zoals'ie nu is.

## WLP Hypothese

Deze is nieuw en wordt geactiveerd wanneer de diepte/druk slider waarde kleiner is dan 20. In deze situatie gaan we de visuele representatie van de meters aanpassen en de vertaalfuncties die de waarden van de 4 sliders converteren naar de onderliggende parameters die de simulatie aansturen.

### Visuele representatie

Bij de WLP was de invloed van UV-licht van groot belang. UV kwam in periode waarin het leven ontstond, 4 tot 3,5 miljard jaar geleden, vrijwel ongefilterd op het aardoppervlak. En het was zowel een positieve kracht als een negatieve: teveel UV breekt DNA af en doodt daardoor het leven, maar UV licht is ook een bron van energie en van variatie in DNA materiaal. Dat betekent dat we in de WLP 'toestand' de pH-slider visueel veranderen in een UV-slider. En dit is slechts een cosmetische wijziging die we moeten doen op alle plaatsen waar we de sliders of meters weergeven:

* het Dashboard project (ESP32 P4 in ESP-IDF)
* de twee verschillende webpagina's die we gebouwd hebben rondom de WASM module

Voor deze locaties moeten we:

* Naam van de 'pH' meter veranderen in 'UV', ander numeriek bereik en units,
* Andere functies voor het kleuren van de value-label of de slider track en thumb

### Mathematische representatie

We hebben voor de HTV een aantal functies die de waarden van de 4 hoofdsliders vertalen naar de waarden van de onderliggende parameters van de simulatie, zoals de 'friction', 'R-smooth', etc. Nu moeten we dus 2 verschillende sets functies hebben, een voor de HTV en een voor WLP.
