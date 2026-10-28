// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// src/blobs.rs
// Klonten herkennen en volgen in de dichtheidskaart (stats_compute.wgsl, `grid_main`),
// en daaruit bijzondere gebeurtenissen halen voor de drone:
//   - samensmelten: twee gevolgde klonten worden er één
//   - uiteenvallen: één gevolgde klont wordt er twee
//   - naderen/nazinderen en uiteendrijven: doorlopend, zolang het in beeld te zien is
// Daarnaast krijgt elke grote zichtbare klont een eigen stem (BlobVoice): plaats,
// grootte, beweging als geheel, vaart t.o.v. de omgeving en onrust binnen de klont.
//
// De cellen van de kaart liggen vast in de wereld: een cel is op elk zoomniveau even
// groot ten opzichte van de particles. Ingezoomd zijn er minder cellen in beeld, net
// zoals er minder particles in beeld zijn — zo is een klont op elke zoom dezelfde klont.

use crate::dsp::{BlobVoice, DroneParams, BLOB_VOICES};

/// Cellen per as over de hele wereld (3240 / 144 = 22.5 world-units per cel, ongeveer
/// een half particle). Zo fijn, zodat samensmelten pas telt als de particles van twee
/// klonten elkaar (bijna) raken, en uiteenvallen zodra er een spleet van een half
/// particle tussen zit — met cellen van 45 kwam samensmelten te vroeg en uiteenvallen
/// te laat. Moet gelijk blijven aan WORLD_CELLS in stats_compute.wgsl.
pub const WORLD_CELLS: usize = 144;
/// Hooguit zoveel cellen per as in beeld (de hele wereld plus twee halve randcellen),
/// en de kop vóór de cellen in de GPU-buffer. Gelijk houden met stats_compute.wgsl.
pub const GRID_MAX: usize = 146;
pub const GRID_HEADER: usize = 8;
/// Lengte van de GPU-buffer in i32's
pub const GRID_BUFFER_LEN: usize = GRID_HEADER + GRID_MAX * GRID_MAX * 4;
/// Vaste-kommafactor van de snelheidssommen. Moet gelijk blijven aan VEL_SCALE in
/// stats_compute.wgsl.
pub const VEL_SCALE: f32 = 16.0;

/// Een cel hoort bij een klont als er particles in zitten én hij met zijn 8 buren (een
/// venster van 67.5 world-units) zoveel keer dichter is dan gemiddeld over de hele
/// wereld (niet over wat toevallig in beeld is: dat hangt van de zoom af) ...
pub const DENSE_FACTOR: f32 = 2.5;
/// ... en dat venster minstens zoveel particles bevat (dezelfde dichtheid als 3 per
/// cel van 45 world-units, waarop eerder geijkt is).
pub const DENSE_MIN_WINDOW: u32 = 7;
/// Achtergrond (particles buiten klonten) telt als referentie voor beweging als hij
/// minstens dit deel van alles in beeld is.
const BACKGROUND_MIN_SHARE: f32 = 0.1;
/// Kleinste klont die meetelt: zoveel particles (kern + rand). Bewust geen deel van
/// alles in beeld: dat zou weer van de zoom afhangen.
const BLOB_MIN_MASS: f32 = 12.0;
/// Een klont moet zoveel metingen (~0.1 s elk) bestaan voordat zijn samensmelten of
/// uiteenvallen telt, en het resultaat moet zoveel metingen blijven bestaan.
const MIN_PARENT_AGE: u32 = 3;
const CONFIRM_UPDATES: u32 = 2;
/// Deel van een klont dat in/naast een andere moet liggen om als "dezelfde" te tellen.
const LINK_SHARE: f32 = 0.3;
/// Een deelnemer aan samensmelten/uiteenvallen moet minstens dit deel van de grote
/// klont zijn (kruimels tellen niet, maar een kleine klont die opgaat in een grote wel:
/// dat zie je gebeuren).
const PART_SHARE: f32 = 0.08;
/// Rustpauze per soort gebeurtenis (s), tegen een reeks gebeurtenissen door flakkeren.
const COOLDOWN: f32 = 1.5;
/// Beweging als geheel: onder dit deel van de gemiddelde particle-snelheid ligt een
/// klont "stil"; bij deze snelheid (viewport-breedtes per seconde) beweegt hij vol.
/// Geijkt op de echte simulatie: klonten drijven typisch ~1–2% van de beeldbreedte
/// per seconde (met 0.15 kwam de beweging nooit boven ~0.13).
const MOTION_FLOOR: f32 = 0.1;
const MOTION_FULL: f32 = 0.03;
/// Onrust: gemiddelde eigen beweging van de particles in de klont (wat niet bij de
/// beweging als geheel hoort) t.o.v. de gemiddelde particle-snelheid in beeld,
/// logaritmisch van AGITATION_LOW (rustig) tot AGITATION_HIGH (vol). Nog niet geijkt
/// sinds de beweging als geheel er niet meer in zit (daarvoor 0.5–3.0).
const AGITATION_LOW: f32 = 0.2;
const AGITATION_HIGH: f32 = 1.5;
/// Twee klonten "werken op elkaar in" als hun tussenruimte kleiner is dan dit (cellen).
const NEAR_GAP: f32 = 10.0;
/// Vaart: snelheid t.o.v. de omgeving (viewport-breedtes per seconde) waarbij een
/// klont begint te zoeven, en waarbij hij vol langszoeft (logaritmisch daartussen).
/// Een klont drijft typisch 1–2% van de beeldbreedte per seconde.
const RUSH_LOW:  f32 = 0.04;
const RUSH_HIGH: f32 = 0.35;
/// Snel zoevende klonten krijgen voorrang op een stem, alsof ze zoveel keer zwaarder zijn.
const RUSH_PRIORITY: f32 = 3.0;
/// Snelheid waarmee de tussenruimte verandert die als volle beweging telt (cellen/s).
const GAP_SPEED: f32 = 1.6;
/// Nazinderen na samensmelten: hoe snel het wegebt (s) en hoe lang hooguit (s).
const SETTLE_DECAY: f32 = 0.6;
const SETTLE_MAX: f32 = 5.0;
/// Verandert de viewport meer dan dit (zoom of verschuiving), dan begint het volgen
/// opnieuw zonder gebeurtenissen.
const VIEWPORT_RESET_ZOOM: f32 = 0.03;
const VIEWPORT_RESET_SHIFT: f32 = 0.02;

/// Teruggelezen dichtheidskaart: de wereldcellen in beeld, per cel het aantal particles
/// en de som van hun snelheden (world-units/s). Cel `k` ligt op wereldcel
/// (x0 + k % nx, y0 + k / nx).
#[derive(Debug, Clone)]
pub struct DensityGrid {
    /// Eerste wereldcel in beeld
    pub x0:     i32,
    pub y0:     i32,
    /// Aantal cellen in beeld per as
    pub nx:     usize,
    pub ny:     usize,
    /// Afmeting van een cel (world-units)
    pub cell:   f32,
    /// Actieve particles in de hele wereld (voor de gemiddelde dichtheid), en het
    /// aantal cellen per as over de hele wereld
    pub world_total: u32,
    pub world_cells: usize,
    pub counts: Vec<u32>,
    pub sum_vx: Vec<f32>,
    pub sum_vy: Vec<f32>,
    /// Som van de snelheidsgrootte per cel (world-units/s)
    pub sum_speed: Vec<f32>,
    /// Volgnummer van de meting, zodat elke kaart maar één keer verwerkt wordt
    pub seq:    u64,
}

impl DensityGrid {
    /// Uit de ruwe GPU-buffer: kop (x0, y0, nx, ny, particles in de wereld, celgrootte
    /// als f32-bits), dan per cel (aantal, Σvx, Σvy, Σ|v|) als i32, snelheden × VEL_SCALE.
    pub fn from_raw(raw: &[i32], seq: u64) -> Self {
        let nx = raw.get(2).copied().unwrap_or(0).clamp(0, GRID_MAX as i32) as usize;
        let ny = raw.get(3).copied().unwrap_or(0).clamp(0, GRID_MAX as i32) as usize;
        let mut g = Self::empty(raw[0], raw[1], nx, ny, f32::from_bits(raw[5] as u32), seq);
        g.world_total = raw[4].max(0) as u32;
        for (i, cell) in raw[GRID_HEADER..].chunks_exact(4).take(nx * ny).enumerate() {
            g.counts[i] = cell[0].max(0) as u32;
            g.sum_vx[i] = cell[1] as f32 / VEL_SCALE;
            g.sum_vy[i] = cell[2] as f32 / VEL_SCALE;
            g.sum_speed[i] = cell[3] as f32 / VEL_SCALE;
        }
        g
    }

    pub fn empty(x0: i32, y0: i32, nx: usize, ny: usize, cell: f32, seq: u64) -> Self {
        Self {
            x0, y0, nx, ny, cell, world_total: 0, world_cells: WORLD_CELLS,
            counts: vec![0; nx * ny],
            sum_vx: vec![0.0; nx * ny],
            sum_vy: vec![0.0; nx * ny],
            sum_speed: vec![0.0; nx * ny],
            seq,
        }
    }

    pub fn len(&self) -> usize {
        self.counts.len()
    }

    /// Wereldcel van cel `k`.
    pub fn world_cell(&self, k: usize) -> (i32, i32) {
        (self.x0 + (k % self.nx.max(1)) as i32, self.y0 + (k / self.nx.max(1)) as i32)
    }

    /// Cel in beeld voor wereldcel (x, y), of None als die buiten beeld ligt.
    pub fn index(&self, x: i32, y: i32) -> Option<usize> {
        let (lx, ly) = (x - self.x0, y - self.y0);
        let inside = lx >= 0 && ly >= 0 && (lx as usize) < self.nx && (ly as usize) < self.ny;
        inside.then(|| ly as usize * self.nx + lx as usize)
    }

    /// De (hooguit 8) buren van cel `k` die in beeld liggen.
    pub fn neighbours(&self, k: usize) -> impl Iterator<Item = usize> + '_ {
        let (x, y) = self.world_cell(k);
        (-1..=1).flat_map(move |dy| (-1..=1).map(move |dx| (dx, dy)))
            .filter(|&d| d != (0, 0))
            .filter_map(move |(dx, dy)| self.index(x + dx, y + dy))
    }

    /// Gemiddeld aantal particles per cel over de hele wereld.
    pub fn world_mean(&self) -> f32 {
        self.world_total as f32 / (self.world_cells * self.world_cells).max(1) as f32
    }

    /// Vanaf zoveel particles is een cel dicht: `factor` × het wereldgemiddelde, en
    /// minstens `min`. Hangt niet van de zoom af.
    pub fn dense_threshold(&self, factor: f32, min: u32) -> f32 {
        (factor * self.world_mean()).max(min as f32)
    }

    /// Welke cellen bij een klont horen: er zitten particles in, en de cel met zijn 8
    /// buren is dicht (zie DENSE_FACTOR). Het venster geeft een betrouwbare dichtheid;
    /// dat de cel zelf bezet moet zijn houdt de grenzen van een klont scherp.
    pub fn dense_cells(&self) -> Vec<bool> {
        let threshold = self.window_threshold();
        (0..self.len()).map(|k| {
            self.counts[k] > 0
                && self.counts[k] + self.neighbours(k).map(|n| self.counts[n]).sum::<u32>() >= threshold.ceil() as u32
        }).collect()
    }

    /// Drempel voor een venster van 3 × 3 cellen (zie dense_cells).
    pub fn window_threshold(&self) -> f32 {
        (DENSE_FACTOR * 9.0 * self.world_mean()).max(DENSE_MIN_WINDOW as f32)
    }

    /// Grovere kaart: `f` × `f` cellen samengenomen (wereldcellen blijven op hun plek).
    pub fn coarsen(&self, f: usize) -> Self {
        let f_i = f as i32;
        let x0 = self.x0.div_euclid(f_i);
        let y0 = self.y0.div_euclid(f_i);
        let nx = ((self.x0 + self.nx as i32 - 1).div_euclid(f_i) - x0 + 1).max(0) as usize;
        let ny = ((self.y0 + self.ny as i32 - 1).div_euclid(f_i) - y0 + 1).max(0) as usize;
        let mut g = Self::empty(x0, y0, nx, ny, self.cell * f as f32, self.seq);
        g.world_total = self.world_total;
        g.world_cells = self.world_cells / f;
        for k in 0..self.len() {
            let (x, y) = self.world_cell(k);
            if let Some(c) = g.index(x.div_euclid(f_i), y.div_euclid(f_i)) {
                g.counts[c] += self.counts[k];
                g.sum_vx[c] += self.sum_vx[k];
                g.sum_vy[c] += self.sum_vy[k];
                g.sum_speed[c] += self.sum_speed[k];
            }
        }
        g
    }

    /// De (hooguit 4) buren van cel `k` die er met een zijde aan grenzen.
    pub fn sides(&self, k: usize) -> impl Iterator<Item = usize> + '_ {
        let (x, y) = self.world_cell(k);
        [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter()
            .filter_map(move |(dx, dy)| self.index(x + dx, y + dy))
    }

    /// Masker over de cellen in beeld: de gegeven wereldcellen plus hun 8 buren.
    fn near_mask(&self, cells: impl Iterator<Item = (i32, i32)>) -> Vec<bool> {
        let mut near = vec![false; self.len()];
        for (x, y) in cells {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if let Some(k) = self.index(x + dx, y + dy) {
                        near[k] = true;
                    }
                }
            }
        }
        near
    }
}

/// Een klont in één meting.
#[derive(Debug, Clone)]
struct Blob {
    id:    u64,
    age:   u32,
    /// Particles in de klont: de dichte kern plus de bezette cellen eromheen. Ver
    /// ingezoomd is een cel klein en ligt een flink deel van een zichtbare klont in
    /// minder dichte randcellen — alleen de kern tellen liet klonten van 20+ particles
    /// onder de ondergrens vallen.
    mass:  f32,
    /// Alleen de dichte kern (de cellen in `cells`), voor het koppelen over metingen
    core:  f32,
    /// Zwaartepunt in de viewport [0, 1] (voor de plek in stereo)
    cx:    f32,
    cy:    f32,
    /// Zwaartepunt in wereldcellen (voor afstanden tussen klonten)
    gx:    f32,
    gy:    f32,
    /// Gezamenlijke snelheid t.o.v. alles in beeld (viewport-breedtes per seconde)
    vx:    f32,
    vy:    f32,
    /// Eigen snelheid (cellen per seconde), om hem bij de volgende meting terug te
    /// vinden ook als hij flink verschoven is
    ax:    f32,
    ay:    f32,
    /// Onrust: gemiddelde snelheid van de particles in de klont die níet bij de
    /// beweging van de klont als geheel hoort (world-units/s). Een klont die als geheel
    /// voortschuift is daardoor rustig; een kolkende klont onrustig.
    speed: f32,
    /// De dichte cellen (wereldcellen) en hun aantal particles
    cells: Vec<((i32, i32), u32)>,
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
    /// Bij de laatste meting, voor de diagnose: particles en cellen in beeld, drempel
    /// voor een dichte cel, drukste cel
    in_view:     u32,
    view_cells:  (usize, usize),
    threshold:   f32,
    busiest:     u32,
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
    /// Id van de samengesmolten klont, en van de twee brokken na uiteenvallen
    merge_voice: f32,
    split_voices: (f32, f32),
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
    /// vorige keer); `viewport`: (center_x, center_y, breedte, hoogte) in world-units;
    /// `mean_speed`: gemiddelde particle-snelheid in beeld (world-units/s);
    /// `blend`: hoe zwaar de klont-gebeurtenissen meewegen (0 uitgezoomd .. 1).
    pub fn update(
        &mut self,
        grid: Option<&DensityGrid>,
        viewport: (f32, f32, f32, f32),
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
        drone.merge_voice    = self.merge_voice;
        drone.split_voice_a  = self.split_voices.0;
        drone.split_voice_b  = self.split_voices.1;
        drone.voices         = self.voices.map(|v| BlobVoice { level: v.level * blend, ..v });
        drone.converge       = self.converge.0 * blend;
        drone.converge_pan   = self.converge.1;
        drone.diverge        = self.diverge.0 * blend;
        drone.diverge_pan    = self.diverge.1;
    }

    fn reset(&mut self) {
        self.blobs.clear();
        self.pending.clear();
        self.pair_gaps.clear();
        self.voices = [BlobVoice::default(); BLOB_VOICES];
        self.settle = None;
        self.last_process = None;
        self.viewport = None;
    }

    fn process(&mut self, g: &DensityGrid, viewport: (f32, f32, f32, f32), mean_speed: f32) {
        let viewport3 = (viewport.0, viewport.1, viewport.2);
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
        self.viewport = Some(viewport3);
        let step = self.last_process.map_or(0.0, |t| self.time - t).min(0.5);

        self.in_view = g.counts.iter().sum();
        self.view_cells = (g.nx, g.ny);
        self.threshold = g.window_threshold();
        self.busiest = g.counts.iter().copied().max().unwrap_or(0);
        let mut found = find_blobs(g, viewport);
        self.link(&mut found, fresh, g, step);
        self.set_voices(&found, mean_speed, viewport.2);
        self.confirm(&found);
        self.interactions(&found);

        self.blobs = found;
    }

    /// Koppel klonten aan die van de vorige meting (id's, leeftijd) en zoek
    /// samensmelten en uiteenvallen. Een vorige klont wordt eerst verschoven naar waar
    /// hij met zijn snelheid nu zou moeten zijn (`step` s later), zodat ook een snel
    /// langszoevende klont dezelfde blijft.
    fn link(&mut self, found: &mut [Blob], fresh: bool, g: &DensityGrid, step: f32) {
        let view = self.in_view;
        let prev = &self.blobs;
        // share[p][c]: deel van p's kern in of naast c, en van c's kern in of naast p
        let mut p_in_c = vec![vec![0.0f32; found.len()]; prev.len()];
        let mut c_in_p = vec![vec![0.0f32; found.len()]; prev.len()];
        let c_near: Vec<Vec<bool>> = found.iter()
            .map(|c| g.near_mask(c.cells.iter().map(|&(xy, _)| xy)))
            .collect();
        for (pi, p) in prev.iter().enumerate() {
            let (dx, dy) = ((p.ax * step).round() as i32, (p.ay * step).round() as i32);
            let moved = |(x, y): (i32, i32)| (x + dx, y + dy);
            let p_near = g.near_mask(p.cells.iter().map(|&(xy, _)| moved(xy)));
            for (ci, c) in found.iter().enumerate() {
                let pm: f32 = p.cells.iter()
                    .filter(|&&(xy, _)| { let (x, y) = moved(xy); g.index(x, y).is_some_and(|k| c_near[ci][k]) })
                    .map(|&(_, n)| n as f32).sum();
                let cm: f32 = c.cells.iter()
                    .filter(|&&((x, y), _)| g.index(x, y).is_some_and(|k| p_near[k]))
                    .map(|&(_, n)| n as f32).sum();
                p_in_c[pi][ci] = pm / p.core.max(1.0);
                c_in_p[pi][ci] = cm / c.core.max(1.0);
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

        // Samensmelten en uiteenvallen per groep die met elkaar verbonden is (vorige
        // klonten ↔ nieuwe klonten die in/naast elkaar liggen). Per groep tellen de
        // vorige en de nieuwe klonten van betekenis: worden het er minder, dan is het
        // samensmelten; meer, dan uiteenvallen. Evenveel (twee klonten die deeltjes
        // uitwisselen) is geen van beide — eerder klonk dan beide tegelijk.
        let n = prev.len() + found.len();
        let mut group: Vec<usize> = (0..n).collect();
        fn root(group: &mut [usize], mut i: usize) -> usize {
            while group[i] != i {
                group[i] = group[group[i]];
                i = group[i];
            }
            i
        }
        for pi in 0..prev.len() {
            for ci in 0..found.len() {
                if p_in_c[pi][ci] >= LINK_SHARE || c_in_p[pi][ci] >= LINK_SHARE {
                    let (a, b) = (root(&mut group, pi), root(&mut group, prev.len() + ci));
                    group[a] = b;
                }
            }
        }
        let roots: Vec<usize> = (0..n).map(|i| root(&mut group, i)).collect();
        let mut done = Vec::new();
        for &r in &roots {
            if done.contains(&r) {
                continue;
            }
            done.push(r);
            let parents: Vec<&Blob> = (0..prev.len()).filter(|&pi| roots[pi] == r).map(|pi| &prev[pi]).collect();
            let mut children: Vec<&Blob> = (0..found.len()).filter(|&ci| roots[prev.len() + ci] == r)
                .map(|ci| &found[ci]).collect();
            let biggest = parents.iter().chain(&children).map(|b| b.mass).fold(0.0f32, f32::max);
            // Van betekenis: geen kruimel naast de grootste; een vorige klont moet ook
            // al even bestaan
            let parents: Vec<&Blob> = parents.into_iter()
                .filter(|b| b.age >= MIN_PARENT_AGE && b.mass >= PART_SHARE * biggest).collect();
            children.retain(|b| b.mass >= PART_SHARE * biggest);
            children.sort_by(|a, b| b.mass.total_cmp(&a.mass));
            if parents.is_empty() || children.is_empty() {
                continue;
            }
            if parents.len() > children.len() && cooled(self.last_merge, self.time) {
                let total: f32 = parents.iter().map(|b| b.mass).sum();
                let c = children[0];
                crate::console_log!("🫧 mogelijk samensmelten: {:?} → {:?}",
                    parents.iter().map(|b| b.mass as u32).collect::<Vec<_>>(),
                    children.iter().map(|b| b.mass as u32).collect::<Vec<_>>());
                self.pending.push(Pending {
                    split: false,
                    ids: vec![c.id],
                    updates: 0,
                    strength: strength(total, view),
                    pan: c.cx * 2.0 - 1.0,
                });
                self.last_merge = Some(self.time);
            } else if children.len() > parents.len() && cooled(self.last_split, self.time) {
                let p = parents.iter().max_by(|a, b| a.mass.total_cmp(&b.mass)).unwrap();
                crate::console_log!("🫧 mogelijk uiteenvallen: {:?} → {:?}",
                    parents.iter().map(|b| b.mass as u32).collect::<Vec<_>>(),
                    children.iter().map(|b| b.mass as u32).collect::<Vec<_>>());
                self.pending.push(Pending {
                    split: true,
                    ids: children.iter().take(2).map(|b| b.id).collect(),
                    updates: 0,
                    strength: strength(p.mass, view),
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
                self.split_voices = (ev.ids[0] as f32, ev.ids.get(1).map_or(0.0, |&id| id as f32));
                crate::console_log!("🫧 Uiteenvallen: sterkte {:.2}, pan {:+.2}", ev.strength, ev.pan);
            } else {
                self.merges += 1;
                self.merge = (self.time, ev.strength, ev.pan);
                self.merge_voice = ev.ids[0] as f32;
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
        for (i, a) in found.iter().enumerate() {
            for b in found.iter().skip(i + 1) {
                if a.age < 1 || b.age < 1 {
                    continue;
                }
                let radius = |x: &Blob| (x.cells.len() as f32 / std::f32::consts::PI).sqrt();
                let dist = ((a.gx - b.gx).powi(2) + (a.gy - b.gy).powi(2)).sqrt();
                let gap = dist - radius(a) - radius(b);
                if gap > NEAR_GAP {
                    continue;
                }
                let key = (a.id.min(b.id), a.id.max(b.id));
                if let Some(&(_, prev)) = self.pair_gaps.iter().find(|(k, _)| *k == key) {
                    if dt > 0.0 {
                        let rate = (gap - prev) / dt;
                        let closeness = 1.0 - gap.max(0.0) / NEAR_GAP;
                        let weight = strength(a.mass.min(b.mass), self.in_view);
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
                    let act = (act * decay).max((change * 4.0).min(1.0) * strength(b.mass, self.in_view));
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
            let in_view = self.in_view;
            let mut masses: Vec<u32> = found.iter().map(|b| b.mass as u32).collect();
            masses.sort_unstable_by(|a, b| b.cmp(a));
            masses.truncate(6);
            let close = self.pair_gaps.len();
            let fastest = found.iter().map(|b| (b.vx * b.vx + b.vy * b.vy).sqrt())
                .fold(0.0f32, f32::max);
            let voices: Vec<String> = self.voices.iter().filter(|v| v.level > 0.0)
                .map(|v| format!("pan {:+.1} groot {:.1} beweging {:.2} vaart {:.2} onrust {:.2}", v.pan, v.size, v.motion, v.rush, v.agitation))
                .collect();
            crate::console_log!(
                "🫧 klonten: {} (massa {:?}, in beeld {} in {}×{} cellen, dicht vanaf {:.1} per 3×3, drukste cel {}), snelste {:.3} beeld/s, paren dichtbij: {}, naderen {:.2}, uiteen {:.2} | {}",
                found.len(), masses, in_view, self.view_cells.0, self.view_cells.1, self.threshold, self.busiest, fastest, close, converge.0, diverge.0, voices.join(" · ")
            );
        }
    }

    /// Stemmen voor de grootste zichtbare klonten: plaats, grootte, beweging als
    /// geheel (t.o.v. de rest) en onrust binnen de klont.
    fn set_voices(&mut self, found: &[Blob], mean_speed: f32, viewport_w: f32) {
        // Gemiddelde particle-snelheid in viewport-breedtes per seconde
        let typical = (mean_speed / viewport_w.max(1.0)).max(1e-4);
        // Vaart: snelheid t.o.v. de particles eromheen. Ver ingezoomd met stroming
        // schuift alles samen door beeld (0.13–0.17 beeld/s gemeten); dan zou over het
        // scherm gemeten elke klont voortdurend zoeven
        let rush = |b: &Blob| {
            let relative = (b.vx * b.vx + b.vy * b.vy).sqrt();
            ((relative / RUSH_LOW).max(1e-3).ln() / (RUSH_HIGH / RUSH_LOW).ln()).clamp(0.0, 1.0)
        };
        let priority = |b: &Blob| b.mass * (1.0 + RUSH_PRIORITY * rush(b));
        let mut order: Vec<&Blob> = found.iter().filter(|b| b.age >= 1).collect();
        order.sort_by(|a, b| priority(b).total_cmp(&priority(a)));
        self.voices = [BlobVoice::default(); BLOB_VOICES];
        for (v, b) in self.voices.iter_mut().zip(order) {
            let speed = (b.vx * b.vx + b.vy * b.vy).sqrt();
            let moving = ((speed / typical - MOTION_FLOOR) / (1.0 - MOTION_FLOOR)).clamp(0.0, 1.0);
            let restless = b.speed / mean_speed.max(1e-3);
            *v = BlobVoice {
                id:        b.id as f32,
                level:     strength(b.mass, self.in_view),
                pan:       b.cx * 2.0 - 1.0,
                // 5 particles = klein (0), ~400 = groot (1), logaritmisch
                size:      ((apparent(b.mass, self.in_view) / 5.0).ln() / 80f32.ln()).clamp(0.0, 1.0),
                motion:    moving * (speed / MOTION_FULL).min(1.0),
                rush:      rush(b),
                agitation: ((restless / AGITATION_LOW).max(1e-3).ln()
                    / (AGITATION_HIGH / AGITATION_LOW).ln()).clamp(0.0, 1.0),
            };
        }
    }
}

fn cooled(last: Option<f32>, now: f32) -> bool {
    last.is_none_or(|t| now - t > COOLDOWN)
}

/// Een klont van dit deel van de particles in beeld klinkt als een van VIEW_REF ×
/// dat deel: 10% van het beeld klinkt als 80 particles (vol).
const VIEW_REF: f32 = 800.0;

/// Hoe groot een klont in beeld lijkt (in particles). Ver ingezoomd zie je weinig
/// particles, en is ook een kleine klont een flink deel van wat je ziet. Het vinden van
/// klonten blijft op elk zoomniveau gelijk; alleen hoe sterk ze klinken hangt hiervan af.
fn apparent(mass: f32, in_view: u32) -> f32 {
    mass.max(mass / in_view.max(1) as f32 * VIEW_REF)
}

/// Sterkte van een gebeurtenis naar de (schijnbare) massa van de betrokken klonten.
/// Geijkt op de echte simulatie (klonten van 13–102 particles): ook een kleine klont
/// moet je duidelijk horen, een grote is vol.
fn strength(mass: f32, in_view: u32) -> f32 {
    (apparent(mass, in_view) / 80.0).sqrt().clamp(0.7, 1.0)
}

/// Hoe zwaar klont-gebeurtenissen meewegen bij een zoomniveau: vanaf 3× (waar de
/// statistieken beginnen) in 2 stappen naar vol bij 5×. Bewust steiler dan de
/// algemene gpu_blend (vol pas bij 12×): wat je ziet gebeuren moet je ook horen.
pub fn event_blend(zoom: f32) -> f32 {
    ((zoom - 3.0) / 2.0).clamp(0.0, 1.0)
}

/// Seconden tussen twee diagnoseregels in de console.
const LOG_INTERVAL: f32 = 2.0;

/// Aaneengesloten (via een zijde) dichte cellen → klonten. `viewport`: (center_x, center_y,
/// breedte, hoogte) in world-units.
fn find_blobs(g: &DensityGrid, viewport: (f32, f32, f32, f32)) -> Vec<Blob> {
    let total: u32 = g.counts.iter().sum();
    if total == 0 {
        return Vec::new();
    }
    let dense = g.dense_cells();
    // Referentiesnelheid (bijv. drift): die van de achtergrond, de particles buiten de
    // klonten — zodat een klont alleen beweegt als hij t.o.v. zijn omgeving beweegt, en
    // een snelle klont een stilliggende buurman niet "meesleept". Is er nauwelijks
    // achtergrond, dan het gemiddelde van alles in beeld.
    let (mut bn, mut bvx, mut bvy) = (0.0f32, 0.0f32, 0.0f32);
    for k in (0..g.len()).filter(|&k| !dense[k]) {
        bn += g.counts[k] as f32;
        bvx += g.sum_vx[k];
        bvy += g.sum_vy[k];
    }
    let (mean_vx, mean_vy) = if bn >= BACKGROUND_MIN_SHARE * total as f32 {
        (bvx / bn, bvy / bn)
    } else {
        (g.sum_vx.iter().sum::<f32>() / total as f32, g.sum_vy.iter().sum::<f32>() / total as f32)
    };
    let (vp_w, vp_h) = (viewport.2.max(1.0), viewport.3.max(1.0));
    let (vp_left, vp_top) = (viewport.0 - 0.5 * vp_w, viewport.1 - 0.5 * vp_h);
    let cell = g.cell.max(1e-3);

    let mut seen = vec![false; g.len()];
    let mut blobs = Vec::new();
    for start in 0..g.len() {
        if !dense[start] || seen[start] {
            continue;
        }
        let mut stack = vec![start];
        let mut cells = Vec::new();
        seen[start] = true;
        while let Some(k) = stack.pop() {
            cells.push(k);
            // Alleen via een zijde: via een hoek verbond één los particle in de spleet
            // twee klonten al
            for n in g.sides(k) {
                if dense[n] && !seen[n] {
                    seen[n] = true;
                    stack.push(n);
                }
            }
        }
        let core: f32 = cells.iter().map(|&k| g.counts[k] as f32).sum();
        // Rand: bezette, niet-dichte cellen direct eromheen (die van een andere klont
        // kunnen dubbel tellen; dat is voor de grootte niet erg)
        let fringe: f32 = g.near_mask(cells.iter().map(|&k| g.world_cell(k))).iter().enumerate()
            .filter(|&(k, &near)| near && !dense[k])
            .map(|(k, _)| g.counts[k] as f32).sum();
        let mass = core + fringe;
        if mass < BLOB_MIN_MASS {
            continue;
        }
        // Zwaartepunt (in wereldcellen) en snelheden over de kern
        let (mut sx, mut sy, mut svx, mut svy, mut sspeed) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for &k in &cells {
            let c = g.counts[k] as f32;
            let (x, y) = g.world_cell(k);
            sx += c * (x as f32 + 0.5);
            sy += c * (y as f32 + 0.5);
            svx += g.sum_vx[k];
            svy += g.sum_vy[k];
            sspeed += g.sum_speed[k];
        }
        let (gx, gy) = (sx / core, sy / core);
        blobs.push(Blob {
            id: 0,
            age: 0,
            mass,
            core,
            cx: ((gx * cell - vp_left) / vp_w).clamp(0.0, 1.0),
            cy: ((gy * cell - vp_top) / vp_h).clamp(0.0, 1.0),
            gx,
            gy,
            vx: (svx / core - mean_vx) / vp_w,
            vy: (svy / core - mean_vy) / vp_w,
            ax: svx / core / cell,
            ay: svy / core / cell,
            speed: (sspeed - (svx * svx + svy * svy).sqrt()).max(0.0) / core,
            cells: cells.iter().map(|&k| (g.world_cell(k), g.counts[k])).collect(),
        });
    }
    blobs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Viewport van 24 × 24 wereldcellen van 45 world-units, linksboven in de wereld
    const DIM: usize = 24;
    const VP: (f32, f32, f32, f32) = (540.0, 540.0, 1080.0, 1080.0);

    /// Kaart met een paar klonten: (cx, cy, straal in cellen, particles per cel, vx).
    fn grid(blobs: &[(usize, usize, i32, u32, f32)], seq: u64) -> DensityGrid {
        let mut g = DensityGrid::empty(0, 0, DIM, DIM, 45.0, seq);
        // Wereldgemiddelde laag: een cel is dicht vanaf DENSE_MIN particles
        g.world_total = 1000;
        // Wat verspreide achtergrond, met gewone (gemiddelde) snelheid
        for k in (0..g.len()).step_by(7) {
            g.counts[k] = 1;
            g.sum_speed[k] = 50.0;
        }
        for &(cx, cy, r, n, vx) in blobs {
            for dy in -r..=r {
                for dx in -r..=r {
                    let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                    let Some(k) = g.index(x, y) else { continue };
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
        assert!(big.agitation <= small.agitation, "als geheel bewegen is geen onrust: {:?} / {:?}", big, small);
        assert_eq!(p.voices[2].level, 0.0, "maar twee klonten");

        // Alles beweegt samen (drift): geen beweging t.o.v. de rest
        let mut t = BlobTracker::default();
        let mut frames = Vec::new();
        for i in 0..5 {
            let mut g = grid(&[(5, 6, 2, 8, 300.0), (19, 18, 1, 8, 300.0)], i);
            for k in 0..g.len() {
                g.sum_vx[k] = g.counts[k] as f32 * 300.0;
            }
            frames.push(g);
        }
        let p = run(&mut t, &frames);
        assert!(p.voices.iter().all(|v| v.motion == 0.0), "meebewegen is geen beweging");
    }

    #[test]
    fn snel_langszoevende_klont_houdt_zijn_stem() {
        // 4 cellen per meting (1.7 beeld/s): zonder voorspelling zou hij elke meting
        // een nieuwe klont zijn en nooit een stem krijgen
        let mut t = BlobTracker::default();
        let frames: Vec<_> = (0..5).map(|i| grid(&[(2 + 4 * i as usize, 12, 1, 8, 1800.0)], i)).collect();
        let p = run(&mut t, &frames);
        let v = p.voices[0];
        assert!(v.level > 0.0, "stem: {:?}", v);
        assert!(v.rush > 0.9, "vaart: {:?}", v);
        assert_eq!(p.merge_id + p.split_id, 0.0, "geen gebeurtenis");

        // Een stilliggende klont zoeft niet
        let mut t = BlobTracker::default();
        let frames: Vec<_> = (0..5).map(|i| grid(&[(12, 12, 1, 8, 0.0)], i)).collect();
        assert_eq!(run(&mut t, &frames).voices[0].rush, 0.0);
    }

    #[test]
    fn gebeurtenis_noemt_de_betrokken_klonten() {
        let mut t = BlobTracker::default();
        let mut frames = Vec::new();
        for i in 0..5 {
            frames.push(grid(&[(12, 12, 2, 8, 0.0)], i));
        }
        for (i, gap) in [3, 4, 6, 8, 8].iter().enumerate() {
            frames.push(grid(&[(12 - gap, 12, 1, 8, 0.0), (12 + gap, 12, 1, 8, 0.0)], 10 + i as u64));
        }
        let p = run(&mut t, &frames);
        let ids: Vec<f32> = p.voices.iter().filter(|v| v.level > 0.0).map(|v| v.id).collect();
        assert!(ids.contains(&p.split_voice_a) && ids.contains(&p.split_voice_b),
            "brokken {} en {} hebben een stem ({:?})", p.split_voice_a, p.split_voice_b, ids);
    }

    #[test]
    fn ver_ingezoomd_telt_een_uitgespreide_klont() {
        // Ver ingezoomd: een klont van 22 particles waarvan maar 6 in dichte cellen
        // (2 × 3) liggen en de rest verspreid over de cellen eromheen (1–2 per cel)
        let mut t = BlobTracker::default();
        let frames: Vec<_> = (0..5).map(|i| {
            let mut g = grid(&[], i);
            g.counts[12 * DIM + 12] = 3;
            g.counts[12 * DIM + 13] = 3;
            for (dx, dy, n) in [(-1, -1, 2), (0, -1, 2), (1, -1, 2), (2, -1, 1), (-1, 0, 2), (2, 0, 2),
                                (-1, 1, 2), (0, 1, 2), (1, 1, 2), (2, 1, 1)] {
                g.counts[(12 + dy) as usize * DIM + (12 + dx) as usize] = n;
            }
            g
        }).collect();
        let p = run(&mut t, &frames);
        assert!(p.voices[0].level > 0.0, "uitgespreide klont heeft een stem: {:?}", p.voices[0]);
        assert_eq!(p.voices[1].level, 0.0, "maar één klont");
    }

    #[test]
    fn kolkende_klont_is_onrustig() {
        // Links een kolkende klont (snelle particles, samen nergens heen), rechts een rustige
        let mut t = BlobTracker::default();
        let frames: Vec<_> = (0..5).map(|i| {
            let mut g = grid(&[(5, 12, 1, 8, 0.0), (19, 12, 1, 8, 0.0)], i);
            for y in 11..=13 {
                for x in 4..=6 {
                    g.sum_speed[y * DIM + x] = 8.0 * 200.0;
                }
            }
            g
        }).collect();
        let p = run(&mut t, &frames);
        let left = p.voices.iter().find(|v| v.pan < 0.0).unwrap();
        let right = p.voices.iter().find(|v| v.pan > 0.0).unwrap();
        assert!(left.agitation > 0.5 && right.agitation < left.agitation, "onrust: {:?} / {:?}", left, right);
    }

    #[test]
    fn uitwisselen_is_geen_samensmelten_of_uiteenvallen() {
        // Twee klonten vlak naast elkaar die deeltjes uitwisselen: de grens tussen
        // beide schuift heen en weer, maar het blijven er steeds twee. De linker raakt
        // daarbij beide vorige klonten (de oude herkenning hoorde dan samensmelten)
        let strip = |g: &mut DensityGrid, x0: usize, x1: usize| {
            for y in 11..=13 {
                for x in x0..=x1 {
                    g.counts[y * DIM + x] = 8;
                }
            }
        };
        let mut t = BlobTracker::default();
        let mut frames = Vec::new();
        for i in 0..16u64 {
            // Zonder achtergrond: een los particle in de spleet verbindt de klonten echt
            let mut g = grid(&[], i);
            g.counts.iter_mut().for_each(|c| *c = 0);
            if (i / 4) % 2 == 0 {
                strip(&mut g, 8, 10);
                strip(&mut g, 12, 14);
            } else {
                strip(&mut g, 9, 12);
                strip(&mut g, 14, 16);
            }
            frames.push(g);
        }
        let p = run(&mut t, &frames);
        assert_eq!((p.merge_id, p.split_id), (0.0, 0.0));
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
            let vp = (540.0, 540.0, 700.0, 700.0);
            t.update(Some(&grid(&[(6, 12, 1, 8, 0.0), (18, 12, 1, 8, 0.0)], 10 + i)), vp, 50.0, 1.0, 0.1, &mut p);
        }
        assert_eq!(p.split_id, 0.0);
    }

    #[test]
    fn ruwe_gpu_buffer_wordt_goed_gelezen() {
        let mut raw = vec![0i32; GRID_BUFFER_LEN];
        raw[..6].copy_from_slice(&[10, 20, 6, 5, 6400, 45.0f32.to_bits() as i32]);
        let c = GRID_HEADER + 4;
        raw[c] = 4;
        raw[c + 1] = (100.0 * VEL_SCALE) as i32;
        raw[c + 2] = (-50.0 * VEL_SCALE) as i32;
        raw[c + 3] = (120.0 * VEL_SCALE) as i32;
        let g = DensityGrid::from_raw(&raw, 7);
        assert_eq!((g.x0, g.y0, g.nx, g.ny, g.world_total, g.cell), (10, 20, 6, 5, 6400, 45.0));
        assert_eq!(g.len(), 30);
        assert_eq!(g.counts[1], 4);
        assert_eq!(g.world_cell(1), (11, 20));
        assert_eq!(g.index(11, 20), Some(1));
        assert_eq!(g.index(16, 20), None, "buiten beeld");
        assert!((g.sum_vx[1] - 100.0).abs() < 1e-3);
        assert!((g.sum_vy[1] + 50.0).abs() < 1e-3);
        assert!((g.sum_speed[1] - 120.0).abs() < 1e-3);
        assert_eq!(g.seq, 7);
    }

    #[test]
    fn klont_blijft_dezelfde_bij_verschuiven_van_het_beeld() {
        // Het beeld schuift een paar cellen op; de klont ligt stil in de wereld
        let mut t = BlobTracker::default();
        let mut p = DroneParams::default();
        let mut ids = Vec::new();
        for i in 0..6u64 {
            let shift = (i / 3) as i32 * 2;
            let mut g = grid(&[], i);
            g.x0 = shift;
            for (x, y) in [(12, 12), (13, 12), (12, 13), (13, 13)] {
                let k = g.index(x, y).unwrap();
                g.counts[k] = 5;
            }
            let vp = (540.0 + shift as f32 * 45.0 * 0.2, 540.0, 1080.0, 1080.0);
            t.update(Some(&g), vp, 50.0, 1.0, 0.1, &mut p);
            ids.push(p.voices[0].id);
        }
        assert!(ids[2] > 0.0 && ids.iter().skip(2).all(|&id| id == ids[2]), "zelfde stem: {:?}", ids);
    }

    #[test]
    fn kleine_klont_klinkt_ver_ingezoomd_vol() {
        // 15 particles van 60 in beeld (ver ingezoomd) tegen 15 van 3000
        assert_eq!(strength(15.0, 60), 1.0);
        assert_eq!(strength(15.0, 3000), 0.7);
        assert!(apparent(15.0, 60) > 15.0 * 10.0);
    }
}
