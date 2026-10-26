// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// src/blobs.rs
// Klonten herkennen en volgen in de dichtheidskaart van de viewport (stats_compute.wgsl,
// `grid_main`), en daaruit bijzondere gebeurtenissen halen voor de drone:
//   - samensmelten: twee gevolgde klonten worden er één
//   - uiteenvallen: één gevolgde klont wordt er twee
//   - naderen/nazinderen en uiteendrijven: doorlopend, zolang het in beeld te zien is
// Daarnaast krijgt elke grote zichtbare klont een eigen stem (BlobVoice): plaats,
// grootte, beweging als geheel en onrust binnen de klont.

use crate::dsp::{BlobVoice, DroneParams, BLOB_VOICES};

/// Cellen per as van de dichtheidskaart. Moet gelijk blijven aan GRID_DIM in
/// stats_compute.wgsl.
pub const GRID_DIM: usize = 24;
pub const GRID_CELLS: usize = GRID_DIM * GRID_DIM;
/// Vaste-kommafactor van de snelheidssommen. Moet gelijk blijven aan VEL_SCALE in
/// stats_compute.wgsl.
pub const VEL_SCALE: f32 = 16.0;

/// Een cel hoort bij een klont als hij zoveel keer dichter is dan gemiddeld.
const DENSE_FACTOR: f32 = 2.5;
/// ... en minstens zoveel particles bevat.
const DENSE_MIN: u32 = 3;
/// Achtergrond (particles buiten klonten) telt als referentie voor beweging als hij
/// minstens dit deel van alles in beeld is.
const BACKGROUND_MIN_SHARE: f32 = 0.1;
/// Kleinste klont die meetelt: zoveel particles, of dit deel van alles in beeld.
const BLOB_MIN_MASS: f32 = 12.0;
const BLOB_MIN_SHARE: f32 = 0.03;
/// Een klont moet zoveel metingen (~0.1 s elk) bestaan voordat zijn samensmelten of
/// uiteenvallen telt, en het resultaat moet zoveel metingen blijven bestaan.
const MIN_PARENT_AGE: u32 = 3;
const CONFIRM_UPDATES: u32 = 2;
/// Deel van een klont dat in/naast een andere moet liggen om als "dezelfde" te tellen.
const LINK_SHARE: f32 = 0.3;
/// Een deelnemer aan samensmelten/uiteenvallen moet minstens dit deel van de grote
/// klont zijn (kleine afsplinteringen tellen niet).
const PART_SHARE: f32 = 0.15;
/// Rustpauze per soort gebeurtenis (s), tegen een reeks gebeurtenissen door flakkeren.
const COOLDOWN: f32 = 2.5;
/// Beweging als geheel: onder dit deel van de gemiddelde particle-snelheid ligt een
/// klont "stil"; bij deze snelheid (viewport-breedtes per seconde) beweegt hij vol.
/// Geijkt op de echte simulatie: klonten drijven typisch ~1–2% van de beeldbreedte
/// per seconde (met 0.15 kwam de beweging nooit boven ~0.13).
const MOTION_FLOOR: f32 = 0.1;
const MOTION_FULL: f32 = 0.03;
/// Onrust: gemiddelde particle-snelheid in de klont t.o.v. die van alles in beeld,
/// logaritmisch van AGITATION_LOW (rustig) tot AGITATION_HIGH (vol). Een lineair smal
/// bereik gaf in de echte simulatie vrijwel alleen 0 of 1.
const AGITATION_LOW: f32 = 0.5;
const AGITATION_HIGH: f32 = 3.0;
/// Twee klonten "werken op elkaar in" als hun tussenruimte kleiner is dan dit (cellen).
const NEAR_GAP: f32 = 5.0;
/// Snelheid waarmee de tussenruimte verandert die als volle beweging telt (cellen/s).
const GAP_SPEED: f32 = 0.8;
/// Nazinderen na samensmelten: hoe snel het wegebt (s) en hoe lang hooguit (s).
const SETTLE_DECAY: f32 = 0.6;
const SETTLE_MAX: f32 = 5.0;
/// Verandert de viewport meer dan dit (zoom of verschuiving), dan begint het volgen
/// opnieuw zonder gebeurtenissen.
const VIEWPORT_RESET_ZOOM: f32 = 0.03;
const VIEWPORT_RESET_SHIFT: f32 = 0.5 / GRID_DIM as f32;

/// Teruggelezen dichtheidskaart: per cel het aantal particles en de som van hun
/// snelheden (world-units/s).
#[derive(Debug, Clone)]
pub struct DensityGrid {
    pub counts: Vec<u32>,
    pub sum_vx: Vec<f32>,
    pub sum_vy: Vec<f32>,
    /// Som van de snelheidsgrootte per cel (world-units/s)
    pub sum_speed: Vec<f32>,
    /// Volgnummer van de meting, zodat elke kaart maar één keer verwerkt wordt
    pub seq:    u64,
}

impl DensityGrid {
    /// Uit de ruwe GPU-buffer: per cel (aantal, Σvx, Σvy, Σ|v|) als i32, snelheden × VEL_SCALE.
    pub fn from_raw(raw: &[i32], seq: u64) -> Self {
        let mut g = Self {
            counts: vec![0; GRID_CELLS],
            sum_vx: vec![0.0; GRID_CELLS],
            sum_vy: vec![0.0; GRID_CELLS],
            sum_speed: vec![0.0; GRID_CELLS],
            seq,
        };
        for (i, cell) in raw.chunks_exact(4).take(GRID_CELLS).enumerate() {
            g.counts[i] = cell[0].max(0) as u32;
            g.sum_vx[i] = cell[1] as f32 / VEL_SCALE;
            g.sum_vy[i] = cell[2] as f32 / VEL_SCALE;
            g.sum_speed[i] = cell[3] as f32 / VEL_SCALE;
        }
        g
    }
}

/// Een klont in één meting.
#[derive(Debug, Clone)]
struct Blob {
    id:    u64,
    age:   u32,
    mass:  f32,
    /// Zwaartepunt in de viewport [0, 1]
    cx:    f32,
    cy:    f32,
    /// Gezamenlijke snelheid t.o.v. alles in beeld (viewport-breedtes per seconde)
    vx:    f32,
    vy:    f32,
    /// Gemiddelde snelheidsgrootte van de particles in de klont (world-units/s)
    speed: f32,
    cells: Vec<usize>,
}

/// Een gebeurtenis die nog moet blijken te blijven bestaan.
#[derive(Debug, Clone)]
struct Pending {
    split:    bool,
    /// Id's van de klont(en) die moeten blijven bestaan
    ids:      Vec<u64>,
    updates:  u32,
    strength: f32,
    pan:      f32,
}

/// Volgt klonten over metingen heen en zet gebeurtenissen in DroneParams.
#[derive(Debug, Clone, Default)]
pub struct BlobTracker {
    blobs:       Vec<Blob>,
    /// Vorige meting: particles per cel (voor overlap)
    prev_counts: Vec<u32>,
    last_seq:    Option<u64>,
    next_id:     u64,
    pending:     Vec<Pending>,
    viewport:    Option<(f32, f32, f32)>,
    time:        f32,
    /// Tijdstip van de laatste gebeurtenis per soort (voor de rustpauze)
    last_merge:  Option<f32>,
    last_split:  Option<f32>,
    merges:      u32,
    splits:      u32,
    merge:       (f32, f32, f32), // (tijdstip, sterkte, pan)
    split:       (f32, f32, f32),
    /// Stemmen van de grootste klonten (zonder zoom-weging)
    voices:      [BlobVoice; BLOB_VOICES],
    /// Tussenruimte (cellen) per paar klonten bij de vorige meting
    pair_gaps:   Vec<((u64, u64), f32)>,
    last_process: Option<f32>,
    /// Nazinderen van de laatst samengesmolten klont: (activiteit, pan, id, cellen, start)
    settle:      Option<(f32, f32, u64, usize, f32)>,
    /// Doorlopend: naar elkaar toe (naderen + nazinderen) en uit elkaar, (activiteit, pan)
    converge:    (f32, f32),
    diverge:     (f32, f32),
    last_log:    f32,
}

impl BlobTracker {
    /// Elke frame aanroepen. `grid`: laatst teruggelezen kaart (mag dezelfde zijn als
    /// vorige keer); `viewport`: (center_x, center_y, breedte) in world-units;
    /// `mean_speed`: gemiddelde particle-snelheid in beeld (world-units/s);
    /// `blend`: hoe zwaar de klont-gebeurtenissen meewegen (0 uitgezoomd .. 1).
    pub fn update(
        &mut self,
        grid: Option<&DensityGrid>,
        viewport: (f32, f32, f32),
        mean_speed: f32,
        blend: f32,
        dt: f32,
        drone: &mut DroneParams,
    ) {
        self.time += dt;

        match grid {
            Some(g) if blend > 0.0 && self.last_seq != Some(g.seq) => {
                self.last_seq = Some(g.seq);
                self.process(g, viewport, mean_speed);
            }
            Some(_) if blend > 0.0 => {}
            _ => {
                // Uitgezoomd: niets volgen, geen gebeurtenissen
                self.reset();
                self.converge = (0.0, 0.0);
                self.diverge = (0.0, 0.0);
            }
        }

        drone.merge_id       = self.merges as f32;
        drone.merge_age      = (self.time - self.merge.0).clamp(0.0, 10.0);
        drone.merge_strength = self.merge.1 * blend;
        drone.merge_pan      = self.merge.2;
        drone.split_id       = self.splits as f32;
        drone.split_age      = (self.time - self.split.0).clamp(0.0, 10.0);
        drone.split_strength = self.split.1 * blend;
        drone.split_pan      = self.split.2;
        drone.voices         = self.voices.map(|v| BlobVoice { level: v.level * blend, ..v });
        drone.converge       = self.converge.0 * blend;
        drone.converge_pan   = self.converge.1;
        drone.diverge        = self.diverge.0 * blend;
        drone.diverge_pan    = self.diverge.1;
    }

    fn reset(&mut self) {
        self.blobs.clear();
        self.prev_counts.clear();
        self.pending.clear();
        self.pair_gaps.clear();
        self.voices = [BlobVoice::default(); BLOB_VOICES];
        self.settle = None;
        self.last_process = None;
        self.viewport = None;
    }

    fn process(&mut self, g: &DensityGrid, viewport: (f32, f32, f32), mean_speed: f32) {
        // Zoom of verschuiving: alles lijkt dan te bewegen — opnieuw beginnen
        if let Some((px, py, pw)) = self.viewport {
            let zoomed = (viewport.2 / pw.max(1e-3) - 1.0).abs() > VIEWPORT_RESET_ZOOM;
            let shifted = ((viewport.0 - px).abs().max((viewport.1 - py).abs()))
                / viewport.2.max(1e-3) > VIEWPORT_RESET_SHIFT;
            if zoomed || shifted {
                self.reset();
            }
        }
        let fresh = self.viewport.is_none();
        self.viewport = Some(viewport);

        let mut found = find_blobs(g, viewport.2);
        self.link(&mut found, fresh, &g.counts);
        self.set_voices(&found, mean_speed, viewport.2);
        self.confirm(&found);
        self.interactions(&found);

        self.prev_counts = g.counts.clone();
        self.blobs = found;
    }

    /// Koppel klonten aan die van de vorige meting (id's, leeftijd) en zoek
    /// samensmelten en uiteenvallen.
    fn link(&mut self, found: &mut [Blob], fresh: bool, counts: &[u32]) {
        let prev = &self.blobs;
        // share[p][c]: deel van p's massa in of naast c, en van c's massa in of naast p
        let mut p_in_c = vec![vec![0.0f32; found.len()]; prev.len()];
        let mut c_in_p = vec![vec![0.0f32; found.len()]; prev.len()];
        for (pi, p) in prev.iter().enumerate() {
            let p_near = dilate(&p.cells);
            for (ci, c) in found.iter().enumerate() {
                let c_near = dilate(&c.cells);
                let pm: f32 = p.cells.iter().filter(|&&k| c_near[k])
                    .map(|&k| self.prev_counts.get(k).copied().unwrap_or(0) as f32).sum();
                let cm: f32 = c.cells.iter().filter(|&&k| p_near[k])
                    .map(|&k| counts[k] as f32).sum();
                p_in_c[pi][ci] = pm / p.mass.max(1.0);
                c_in_p[pi][ci] = cm / c.mass.max(1.0);
            }
        }

        // Id's: elke nieuwe klont erft van de vorige waar hij het meest mee overlapt
        let mut taken = vec![false; prev.len()];
        let mut order: Vec<usize> = (0..found.len()).collect();
        order.sort_by(|&a, &b| found[b].mass.total_cmp(&found[a].mass));
        for &ci in &order {
            let best = (0..prev.len())
                .filter(|&pi| !taken[pi] && c_in_p[pi][ci] >= LINK_SHARE)
                .max_by(|&a, &b| c_in_p[a][ci].total_cmp(&c_in_p[b][ci]));
            match best {
                Some(pi) => {
                    taken[pi] = true;
                    found[ci].id = prev[pi].id;
                    found[ci].age = prev[pi].age + 1;
                }
                None => {
                    self.next_id += 1;
                    found[ci].id = self.next_id;
                    found[ci].age = 0;
                }
            }
        }
        if fresh {
            return;
        }

        // Samensmelten: een klont waar minstens twee gevestigde klonten in opgaan
        for (ci, c) in found.iter().enumerate() {
            let parents: Vec<usize> = (0..prev.len())
                .filter(|&pi| p_in_c[pi][ci] >= LINK_SHARE
                    && prev[pi].age >= MIN_PARENT_AGE
                    && prev[pi].mass >= PART_SHARE * c.mass)
                .collect();
            if parents.len() >= 2 && cooled(self.last_merge, self.time) {
                let total: f32 = parents.iter().map(|&pi| prev[pi].mass).sum();
                self.pending.push(Pending {
                    split: false,
                    ids: vec![c.id],
                    updates: 0,
                    strength: strength(total),
                    pan: c.cx * 2.0 - 1.0,
                });
                self.last_merge = Some(self.time);
            }
        }

        // Uiteenvallen: een gevestigde klont die in minstens twee klonten opgaat
        for (pi, p) in prev.iter().enumerate() {
            if p.age < MIN_PARENT_AGE {
                continue;
            }
            let children: Vec<usize> = (0..found.len())
                .filter(|&ci| c_in_p[pi][ci] >= LINK_SHARE && found[ci].mass >= PART_SHARE * p.mass)
                .collect();
            if children.len() >= 2 && cooled(self.last_split, self.time) {
                self.pending.push(Pending {
                    split: true,
                    ids: children.iter().map(|&ci| found[ci].id).collect(),
                    updates: 0,
                    strength: strength(p.mass),
                    pan: p.cx * 2.0 - 1.0,
                });
                self.last_split = Some(self.time);
            }
        }
    }

    /// Een gebeurtenis telt pas als het resultaat even blijft bestaan.
    fn confirm(&mut self, found: &[Blob]) {
        let alive = |id: u64| found.iter().any(|b| b.id == id);
        let mut keep = Vec::new();
        for mut ev in std::mem::take(&mut self.pending) {
            if !ev.ids.iter().all(|&id| alive(id)) {
                continue; // flakkeren: verdwenen voordat het bevestigd was
            }
            ev.updates += 1;
            if ev.updates < CONFIRM_UPDATES {
                keep.push(ev);
                continue;
            }
            if ev.split {
                self.splits += 1;
                self.split = (self.time, ev.strength, ev.pan);
                crate::console_log!("🫧 Uiteenvallen: sterkte {:.2}, pan {:+.2}", ev.strength, ev.pan);
            } else {
                self.merges += 1;
                self.merge = (self.time, ev.strength, ev.pan);
                // Na het samensmelten zindert de nieuwe klont nog na zolang hij van
                // vorm verandert
                let cells = found.iter().find(|b| b.id == ev.ids[0]).map_or(0, |b| b.cells.len());
                self.settle = Some((ev.strength, ev.pan, ev.ids[0], cells, self.time));
                crate::console_log!("🫧 Samensmelten: sterkte {:.2}, pan {:+.2}", ev.strength, ev.pan);
            }
        }
        self.pending = keep;
    }

    /// Doorlopende activiteit tussen klonten: naderen (en nazinderen na samensmelten)
    /// en uiteendrijven. Duurt zolang het in beeld te zien is.
    fn interactions(&mut self, found: &[Blob]) {
        let dt = self.last_process.map_or(0.0, |t| self.time - t);
        self.last_process = Some(self.time);

        let mut converge = (0.0f32, 0.0f32);
        let mut diverge = (0.0f32, 0.0f32);
        let mut gaps = Vec::new();
        let grid = GRID_DIM as f32;
        for (i, a) in found.iter().enumerate() {
            for b in found.iter().skip(i + 1) {
                if a.age < 1 || b.age < 1 {
                    continue;
                }
                let radius = |x: &Blob| (x.cells.len() as f32 / std::f32::consts::PI).sqrt();
                let dist = ((a.cx - b.cx).powi(2) + (a.cy - b.cy).powi(2)).sqrt() * grid;
                let gap = dist - radius(a) - radius(b);
                if gap > NEAR_GAP {
                    continue;
                }
                let key = (a.id.min(b.id), a.id.max(b.id));
                if let Some(&(_, prev)) = self.pair_gaps.iter().find(|(k, _)| *k == key) {
                    if dt > 0.0 {
                        let rate = (gap - prev) / dt;
                        let closeness = 1.0 - gap.max(0.0) / NEAR_GAP;
                        let weight = strength(a.mass.min(b.mass));
                        let pan = (a.cx + b.cx) - 1.0;
                        // Wortel: ook rustig naderen/uiteendrijven moet hoorbaar zijn
                        let toward = (closeness * (-rate / GAP_SPEED).clamp(0.0, 1.0) * weight).sqrt();
                        let apart = (closeness * (rate / GAP_SPEED).clamp(0.0, 1.0) * weight).sqrt();
                        if toward > converge.0 {
                            converge = (toward, pan);
                        }
                        if apart > diverge.0 {
                            diverge = (apart, pan);
                        }
                    }
                }
                gaps.push((key, gap));
            }
        }
        self.pair_gaps = gaps;

        // Nazinderen: zolang de samengesmolten klont van vorm verandert
        if let Some((act, pan, id, cells, start)) = self.settle {
            match found.iter().find(|b| b.id == id) {
                Some(b) if self.time - start < SETTLE_MAX => {
                    let change = (b.cells.len() as f32 - cells as f32).abs() / b.cells.len().max(1) as f32;
                    let decay = if dt > 0.0 { (-dt / SETTLE_DECAY).exp() } else { 1.0 };
                    let act = (act * decay).max((change * 4.0).min(1.0) * strength(b.mass));
                    self.settle = if act > 0.02 { Some((act, pan, id, b.cells.len(), start)) } else { None };
                    if act > converge.0 {
                        converge = (act, pan);
                    }
                }
                _ => self.settle = None,
            }
        }

        self.converge = converge;
        self.diverge = diverge;

        // Diagnose om de drempels op de echte simulatie te kunnen ijken
        if self.time - self.last_log >= LOG_INTERVAL {
            self.last_log = self.time;
            let mut masses: Vec<u32> = found.iter().map(|b| b.mass as u32).collect();
            masses.sort_unstable_by(|a, b| b.cmp(a));
            masses.truncate(6);
            let close = self.pair_gaps.len();
            let voices: Vec<String> = self.voices.iter().filter(|v| v.level > 0.0)
                .map(|v| format!("pan {:+.1} groot {:.1} beweging {:.2} onrust {:.2}", v.pan, v.size, v.motion, v.agitation))
                .collect();
            crate::console_log!(
                "🫧 klonten: {} (massa {:?}), paren dichtbij: {}, naderen {:.2}, uiteen {:.2} | {}",
                found.len(), masses, close, converge.0, diverge.0, voices.join(" · ")
            );
        }
    }

    /// Stemmen voor de grootste zichtbare klonten: plaats, grootte, beweging als
    /// geheel (t.o.v. de rest) en onrust binnen de klont.
    fn set_voices(&mut self, found: &[Blob], mean_speed: f32, viewport_w: f32) {
        // Gemiddelde particle-snelheid in viewport-breedtes per seconde
        let typical = (mean_speed / viewport_w.max(1.0)).max(1e-4);
        let mut order: Vec<&Blob> = found.iter().filter(|b| b.age >= 1).collect();
        order.sort_by(|a, b| b.mass.total_cmp(&a.mass));
        self.voices = [BlobVoice::default(); BLOB_VOICES];
        for (v, b) in self.voices.iter_mut().zip(order) {
            let speed = (b.vx * b.vx + b.vy * b.vy).sqrt();
            let moving = ((speed / typical - MOTION_FLOOR) / (1.0 - MOTION_FLOOR)).clamp(0.0, 1.0);
            let restless = b.speed / mean_speed.max(1e-3);
            *v = BlobVoice {
                id:        b.id as f32,
                level:     strength(b.mass),
                pan:       b.cx * 2.0 - 1.0,
                // 12 particles = klein (0), ~480 = groot (1), logaritmisch
                size:      ((b.mass / BLOB_MIN_MASS).ln() / 40f32.ln()).clamp(0.0, 1.0),
                motion:    moving * (speed / MOTION_FULL).min(1.0),
                agitation: ((restless / AGITATION_LOW).max(1e-3).ln()
                    / (AGITATION_HIGH / AGITATION_LOW).ln()).clamp(0.0, 1.0),
            };
        }
    }
}

fn cooled(last: Option<f32>, now: f32) -> bool {
    last.is_none_or(|t| now - t > COOLDOWN)
}

/// Sterkte van een gebeurtenis naar de massa van de betrokken klonten.
fn strength(mass: f32) -> f32 {
    (mass / 150.0).sqrt().clamp(0.4, 1.0)
}

/// Hoe zwaar klont-gebeurtenissen meewegen bij een zoomniveau: vanaf 3× (waar de
/// statistieken beginnen) in 2 stappen naar vol bij 5×. Bewust steiler dan de
/// algemene gpu_blend (vol pas bij 12×): wat je ziet gebeuren moet je ook horen.
pub fn event_blend(zoom: f32) -> f32 {
    ((zoom - 3.0) / 2.0).clamp(0.0, 1.0)
}

/// Seconden tussen twee diagnoseregels in de console.
const LOG_INTERVAL: f32 = 2.0;

/// Cellen van een klont plus hun 8 buren.
fn dilate(cells: &[usize]) -> Vec<bool> {
    let mut near = vec![false; GRID_CELLS];
    for &k in cells {
        let (x, y) = ((k % GRID_DIM) as i32, (k / GRID_DIM) as i32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0 && ny >= 0 && (nx as usize) < GRID_DIM && (ny as usize) < GRID_DIM {
                    near[ny as usize * GRID_DIM + nx as usize] = true;
                }
            }
        }
    }
    near
}

/// Aaneengesloten (8-buren) dichte cellen → klonten.
fn find_blobs(g: &DensityGrid, viewport_w: f32) -> Vec<Blob> {
    let total: u32 = g.counts.iter().sum();
    if total == 0 {
        return Vec::new();
    }
    let threshold = ((total as f32 / GRID_CELLS as f32) * DENSE_FACTOR).max(DENSE_MIN as f32);
    let dense: Vec<bool> = g.counts.iter().map(|&c| c as f32 >= threshold).collect();
    // Referentiesnelheid (bijv. drift): die van de achtergrond, de particles buiten de
    // klonten — zodat een klont alleen beweegt als hij t.o.v. zijn omgeving beweegt, en
    // een snelle klont een stilliggende buurman niet "meesleept". Is er nauwelijks
    // achtergrond, dan het gemiddelde van alles in beeld.
    let (mut bn, mut bvx, mut bvy) = (0.0f32, 0.0f32, 0.0f32);
    for k in (0..GRID_CELLS).filter(|&k| !dense[k]) {
        bn += g.counts[k] as f32;
        bvx += g.sum_vx[k];
        bvy += g.sum_vy[k];
    }
    let (mean_vx, mean_vy) = if bn >= BACKGROUND_MIN_SHARE * total as f32 {
        (bvx / bn, bvy / bn)
    } else {
        (g.sum_vx.iter().sum::<f32>() / total as f32, g.sum_vy.iter().sum::<f32>() / total as f32)
    };
    let min_mass = BLOB_MIN_MASS.max(BLOB_MIN_SHARE * total as f32);

    let mut seen = vec![false; GRID_CELLS];
    let mut blobs = Vec::new();
    for start in 0..GRID_CELLS {
        if !dense[start] || seen[start] {
            continue;
        }
        let mut stack = vec![start];
        let mut cells = Vec::new();
        seen[start] = true;
        while let Some(k) = stack.pop() {
            cells.push(k);
            let (x, y) = ((k % GRID_DIM) as i32, (k / GRID_DIM) as i32);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx as usize >= GRID_DIM || ny as usize >= GRID_DIM {
                        continue;
                    }
                    let n = ny as usize * GRID_DIM + nx as usize;
                    if dense[n] && !seen[n] {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
        }
        let mass: f32 = cells.iter().map(|&k| g.counts[k] as f32).sum();
        if mass < min_mass {
            continue;
        }
        let (mut sx, mut sy, mut svx, mut svy, mut sspeed) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for &k in &cells {
            let c = g.counts[k] as f32;
            sx += c * ((k % GRID_DIM) as f32 + 0.5);
            sy += c * ((k / GRID_DIM) as f32 + 0.5);
            svx += g.sum_vx[k];
            svy += g.sum_vy[k];
            sspeed += g.sum_speed[k];
        }
        blobs.push(Blob {
            id: 0,
            age: 0,
            mass,
            cx: sx / mass / GRID_DIM as f32,
            cy: sy / mass / GRID_DIM as f32,
            vx: (svx / mass - mean_vx) / viewport_w.max(1.0),
            vy: (svy / mass - mean_vy) / viewport_w.max(1.0),
            speed: sspeed / mass,
            cells,
        });
    }
    blobs
}

#[cfg(test)]
mod tests {
    use super::*;

    const VP: (f32, f32, f32) = (500.0, 500.0, 1000.0);

    /// Kaart met een paar klonten: (cx, cy, straal in cellen, particles per cel, vx).
    fn grid(blobs: &[(usize, usize, i32, u32, f32)], seq: u64) -> DensityGrid {
        let mut g = DensityGrid {
            counts: vec![0; GRID_CELLS],
            sum_vx: vec![0.0; GRID_CELLS],
            sum_vy: vec![0.0; GRID_CELLS],
            sum_speed: vec![0.0; GRID_CELLS],
            seq,
        };
        // Wat verspreide achtergrond, met gewone (gemiddelde) snelheid
        for k in (0..GRID_CELLS).step_by(7) {
            g.counts[k] = 1;
            g.sum_speed[k] = 50.0;
        }
        for &(cx, cy, r, n, vx) in blobs {
            for dy in -r..=r {
                for dx in -r..=r {
                    let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                    if x < 0 || y < 0 || x >= GRID_DIM as i32 || y >= GRID_DIM as i32 {
                        continue;
                    }
                    let k = y as usize * GRID_DIM + x as usize;
                    g.counts[k] += n;
                    g.sum_vx[k] += vx * n as f32;
                    g.sum_speed[k] += (50.0 + vx.abs()) * n as f32;
                }
            }
        }
        g
    }

    fn run(t: &mut BlobTracker, frames: &[DensityGrid]) -> DroneParams {
        let mut p = DroneParams::default();
        for g in frames {
            t.update(Some(g), VP, 50.0, 1.0, 0.1, &mut p);
        }
        p
    }

    #[test]
    fn rustige_klonten_geven_geen_gebeurtenis() {
        let mut t = BlobTracker::default();
        let frames: Vec<_> = (0..20).map(|i| grid(&[(5, 5, 1, 8, 0.0), (17, 17, 1, 8, 0.0)], i)).collect();
        let p = run(&mut t, &frames);
        assert_eq!(p.merge_id, 0.0);
        assert_eq!(p.split_id, 0.0);
        assert!(p.voices.iter().all(|v| v.motion == 0.0), "stilliggende klonten bewegen niet");
        assert_eq!(p.voices.iter().filter(|v| v.level > 0.0).count(), 2, "twee stemmen");
    }

    #[test]
    fn samensmelten_wordt_herkend() {
        let mut t = BlobTracker::default();
        let mut frames = Vec::new();
        // Twee klonten naderen elkaar en smelten samen
        for (i, gap) in [8, 8, 8, 8, 6, 4, 3].iter().enumerate() {
            frames.push(grid(&[(12 - gap, 12, 1, 8, 0.0), (12 + gap, 12, 1, 8, 0.0)], i as u64));
        }
        for i in 0..5 {
            frames.push(grid(&[(12, 12, 2, 8, 0.0)], 10 + i));
        }
        let p = run(&mut t, &frames);
        assert_eq!(p.merge_id, 1.0, "precies één samensmelting");
        assert_eq!(p.split_id, 0.0);
        assert!(p.merge_strength > 0.2);
    }

    #[test]
    fn naderen_en_uiteendrijven_klinken_zolang_ze_bezig_zijn() {
        let step = |t: &mut BlobTracker, p: &mut DroneParams, gap: usize, seq: u64| {
            t.update(Some(&grid(&[(12 - gap, 12, 1, 8, 0.0), (12 + gap, 12, 1, 8, 0.0)], seq)), VP, 50.0, 1.0, 0.1, p);
        };

        // Naderen: alleen zolang de tussenruimte kleiner wordt
        let mut t = BlobTracker::default();
        let mut p = DroneParams::default();
        let mut toward = Vec::new();
        for (i, gap) in [3, 3, 3, 2, 2].iter().enumerate() {
            step(&mut t, &mut p, *gap, i as u64);
            toward.push(p.converge);
        }
        assert_eq!(toward[2], 0.0, "dicht bij elkaar maar stil: niets ({:?})", toward);
        assert!(toward[3] > 0.1, "naderend: activiteit ({:?})", toward);
        assert_eq!(toward[4], 0.0, "weer stil: voorbij ({:?})", toward);
        assert_eq!(p.diverge, 0.0);

        // Uiteendrijven na uiteenvallen
        let mut t = BlobTracker::default();
        let mut p = DroneParams::default();
        for i in 0..5 {
            t.update(Some(&grid(&[(12, 12, 2, 8, 0.0)], i)), VP, 50.0, 1.0, 0.1, &mut p);
        }
        let mut apart = Vec::new();
        for (i, gap) in [2, 2, 3, 3].iter().enumerate() {
            step(&mut t, &mut p, *gap, 10 + i as u64);
            apart.push(p.diverge);
        }
        assert!(apart[2] > 0.1, "uiteendrijvend: activiteit ({:?})", apart);
        assert_eq!(apart[3], 0.0, "stil: voorbij ({:?})", apart);
        assert_eq!(p.converge, 0.0);
    }

    #[test]
    fn uiteenvallen_wordt_herkend() {
        let mut t = BlobTracker::default();
        let mut frames = Vec::new();
        for i in 0..5 {
            frames.push(grid(&[(12, 12, 2, 8, 0.0)], i));
        }
        for (i, gap) in [3, 4, 6, 8, 8].iter().enumerate() {
            frames.push(grid(&[(12 - gap, 12, 1, 8, 0.0), (12 + gap, 12, 1, 8, 0.0)], 10 + i as u64));
        }
        let p = run(&mut t, &frames);
        assert_eq!(p.split_id, 1.0, "precies één keer uiteenvallen");
        assert_eq!(p.merge_id, 0.0);
    }

    #[test]
    fn flakkerende_klont_geeft_geen_gebeurtenis() {
        let mut t = BlobTracker::default();
        let mut frames = Vec::new();
        // Eén klont die om de meting heel even in tweeën "knippert"
        for i in 0..20u64 {
            if i % 2 == 0 {
                frames.push(grid(&[(12, 12, 2, 8, 0.0)], i));
            } else {
                frames.push(grid(&[(10, 12, 1, 8, 0.0), (14, 12, 1, 8, 0.0)], i));
            }
        }
        let p = run(&mut t, &frames);
        assert_eq!(p.split_id, 0.0, "knipperen is geen uiteenvallen");
        assert_eq!(p.merge_id, 0.0, "knipperen is geen samensmelten");
    }

    #[test]
    fn stemmen_volgen_plaats_grootte_en_beweging() {
        // Een grote klont links die hard beweegt, een kleine rechts die stilligt
        let mut t = BlobTracker::default();
        let frames: Vec<_> = (0..5).map(|i| grid(&[(5, 6, 2, 8, 300.0), (19, 18, 1, 8, 0.0)], i)).collect();
        let p = run(&mut t, &frames);
        let big = p.voices[0];
        let small = p.voices[1];
        assert!(big.pan < 0.0 && small.pan > 0.0, "plaats: {:?} / {:?}", big, small);
        assert!(big.size > small.size, "grootte: {:?} / {:?}", big, small);
        assert!(big.motion > 0.3, "beweging: {:?}", big);
        assert_eq!(small.motion, 0.0, "stil: {:?}", small);
        assert!(big.agitation > small.agitation, "onrust: {:?} / {:?}", big, small);
        assert_eq!(p.voices[2].level, 0.0, "maar twee klonten");

        // Alles beweegt samen (drift): geen beweging t.o.v. de rest
        let mut t = BlobTracker::default();
        let mut frames = Vec::new();
        for i in 0..5 {
            let mut g = grid(&[(5, 6, 2, 8, 300.0), (19, 18, 1, 8, 300.0)], i);
            for k in 0..GRID_CELLS {
                g.sum_vx[k] = g.counts[k] as f32 * 300.0;
            }
            frames.push(g);
        }
        let p = run(&mut t, &frames);
        assert!(p.voices.iter().all(|v| v.motion == 0.0), "meebewegen is geen beweging");
    }

    #[test]
    fn inzoomen_reset_zonder_gebeurtenis() {
        let mut t = BlobTracker::default();
        let mut p = DroneParams::default();
        for i in 0..5 {
            t.update(Some(&grid(&[(12, 12, 2, 8, 0.0)], i)), VP, 50.0, 1.0, 0.1, &mut p);
        }
        // Zoom: de klont lijkt plots in tweeën te vallen, maar dat is de camera
        for i in 0..5 {
            let vp = (500.0, 500.0, 600.0);
            t.update(Some(&grid(&[(6, 12, 1, 8, 0.0), (18, 12, 1, 8, 0.0)], 10 + i)), vp, 50.0, 1.0, 0.1, &mut p);
        }
        assert_eq!(p.split_id, 0.0);
    }

    #[test]
    fn ruwe_gpu_buffer_wordt_goed_gelezen() {
        let mut raw = vec![0i32; GRID_CELLS * 4];
        raw[4] = 4;
        raw[5] = (100.0 * VEL_SCALE) as i32;
        raw[6] = (-50.0 * VEL_SCALE) as i32;
        raw[7] = (120.0 * VEL_SCALE) as i32;
        let g = DensityGrid::from_raw(&raw, 7);
        assert_eq!(g.counts[1], 4);
        assert!((g.sum_vx[1] - 100.0).abs() < 1e-3);
        assert!((g.sum_vy[1] + 50.0).abs() < 1e-3);
        assert!((g.sum_speed[1] - 120.0).abs() < 1e-3);
        assert_eq!(g.seq, 7);
    }
}
