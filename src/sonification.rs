// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// src/sonification.rs
// Vertaalt simulatieparameters en (bij hoge zoom) GPU-statistieken
// naar een SonificationState met 7 stemmen — één per particle-type.
//
// Architectuur:
//   zoom 1x–5x   → alleen SimParams (geen readback)
//   zoom 5x–12x  → SimParams + GpuTypeStats (readback 10 Hz)
//
// De bijdrage van GpuTypeStats schaalt proportioneel met (zoom − 5) / 7.

use crate::dsp::DroneParams;
use crate::SimulationParams;

/// Ruwe statistieken per particle-type (stats_out[0..6]).
/// Layout: vec4(viewport_count, energy, order, centroid_x)
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuTypeStats {
    /// Aantal particles van dit type zichtbaar in de viewport
    pub viewport_count: f32,
    /// Gemiddelde snelheidsgrootte in world-units/s
    pub energy: f32,
    /// Clusteringmaat [0,1]: 1 = volledig geclusterd, 0 = uniform verspreid
    pub order: f32,
    /// Gewogen centroid X in viewport [0,1] (voor stereo panning)
    pub centroid_x: f32,
}

impl GpuTypeStats {
    pub fn from_floats(data: &[f32; 4]) -> Self {
        Self {
            viewport_count: data[0],
            energy:         data[1],
            order:          data[2],
            centroid_x:     data[3],
        }
    }
}

/// Globale viewport-statistieken (stats_out[7]).
/// Layout: vec4(total_viewport_count, cluster_count, avg_cluster_size, 0)
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuGlobalStats {
    /// Totaal aantal actieve particles in de viewport (alle types)
    pub total_viewport_count: f32,
    /// Aantal niet-lege cellen in 8×8 viewport-grid (benadering clusteraantal)
    pub cluster_count: f32,
    /// Gemiddelde clustergrootte: total_viewport_count / cluster_count
    pub avg_cluster_size: f32,
}

impl GpuGlobalStats {
    pub fn from_floats(data: &[f32; 4]) -> Self {
        Self {
            total_viewport_count: data[0],
            cluster_count:        data[1],
            avg_cluster_size:     data[2],
        }
    }
}

/// Volledig sonische toestand van één stem (= één particle-type).
#[derive(Debug, Clone, Copy)]
pub struct StemState {
    /// Grondfrequentie in Hz (basstemmen: 40–200 Hz)
    pub frequency: f32,
    /// Detune-factor: lichte pitch-spread voor supersaw richheid [0, 1]
    pub detune: f32,
    /// Gate-openheid [0, 1]: stuurt cutoff-freq van de lowpass gate
    /// 0 = gesloten (dof, donker), 1 = volledig open (helder, raspig)
    pub gate: f32,
    /// Ruisniveau dat gemengd wordt in de stem [0, 1]
    pub noise: f32,
    /// Saturatie-drive [0, 1]: 0 = schoon, 1 = maximale rasp
    pub saturation: f32,
    /// Stereo panning [-1, 1]: -1 = volledig links, 1 = volledig rechts
    pub pan: f32,
    /// Amplitude [0, 1]
    pub amplitude: f32,
}

impl Default for StemState {
    fn default() -> Self {
        Self {
            frequency:  55.0,
            detune:     0.2,
            gate:       0.3,
            noise:      0.1,
            saturation: 0.2,
            pan:        0.0,
            amplitude:  0.5,
        }
    }
}

/// Volledige sonische toestand van de simulatie, elke frame bijgewerkt.
#[derive(Debug, Clone)]
pub struct SonificationState {
    pub stems: [StemState; 7],
    /// Globale amplitude-schaal [0, 1] — daalt bij lage zoom
    pub master_amplitude: f32,
    /// Hoeveel GPU-stats bijdragen (0 = puur SimParams, 1 = volledig GPU-gestuurd)
    pub gpu_blend: f32,
    /// Huidig zoomniveau (kopie voor downstream gebruik)
    pub zoom: f32,
    /// Besturing van de ambient drone (dsp::DroneSynth): temperatuur altijd,
    /// clusterstatistieken alleen boven 3× zoom (geschaald met gpu_blend)
    pub drone: DroneParams,
}

impl Default for SonificationState {
    fn default() -> Self {
        let mut stems = [StemState::default(); 7];
        // Basisfrequenties per type (kwint-interval reeks, alle in het bas-register)
        // Type 0 (blauw)   → A1  = 55.0 Hz
        // Type 1 (geel)    → E2  = 82.4 Hz
        // Type 2 (rood)    → B2  = 123.5 Hz  (hoog, klein, snel)
        // Type 3 (paars)   → G#1 = 51.9 Hz
        // Type 4 (groen)   → D2  = 73.4 Hz
        // Type 5 (olijf)   → A#1 = 58.3 Hz
        // Type 6 (oranje)  → C#2 = 69.3 Hz
        let base_freqs = [55.0f32, 82.4, 123.5, 51.9, 73.4, 58.3, 69.3];
        for (i, stem) in stems.iter_mut().enumerate() {
            stem.frequency = base_freqs[i];
        }
        Self {
            stems,
            master_amplitude: 0.65,
            gpu_blend: 0.0,
            zoom: 1.0,
            drone: DroneParams::default(),
        }
    }
}

/// Berekent een nieuwe SonificationState op basis van actuele SimulationParams
/// en optionele GPU-statistieken.
///
/// # Parameters
/// - `params`    : huidige SimulationParams
/// - `gpu_stats` : optioneel, `None` bij lage zoom of als readback nog niet klaar is
/// - `prev`      : vorige staat voor EMA-smoothing
pub fn compute_sonification(
    params: &SimulationParams,
    gpu_stats: Option<&[GpuTypeStats; 7]>,
    gpu_global: Option<&GpuGlobalStats>,
    prev: &SonificationState,
) -> SonificationState {
    let zoom = params.current_zoom_level;

    // Hoeveel draagt GPU bij? Lineair van 0 bij zoom≤2 naar 1 bij zoom=12
    let gpu_blend = if gpu_stats.is_some() {
        ((zoom - 2.0) / 10.0).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Globale GPU-stats: cluster_count [0,64] en avg_cluster_size
    // cluster_count hoog = veel kleine clusters = gefragmenteerd
    // cluster_count laag + avg_cluster_size hoog = weinig grote clusters = georganiseerd
    let (global_detune_mod, master_amp_mod) = if let Some(g) = gpu_global {
        // Weinig clusters met grote gemiddelde grootte = orde → strakke detune, voller geluid
        // Veel kleine clusters = versnipperd → breed, drukker
        let cluster_norm   = (g.cluster_count / 64.0).clamp(0.0, 1.0);
        let size_norm      = (g.avg_cluster_size / 200.0).clamp(0.0, 1.0); // 200 = verwacht max
        let organisation   = (size_norm * (1.0 - cluster_norm * 0.5)).clamp(0.0, 1.0);
        // Hoge organisatie → lage detune (strak); lage organisatie → brede detune
        let detune_mod     = 1.0 - organisation * 0.7;
        // Totaal viewport count: vol scherm → voller geluid
        let density_mod    = (g.total_viewport_count / 500.0).clamp(0.3, 1.2);
        (detune_mod, density_mod)
    } else {
        (1.0, 1.0)
    };

    // Master amplitude: bij uitzoomen zachter, geschaald door viewport-dichtheid
    // zoom 1 → 0.65, zoom 12 → 0.95
    let master_amplitude = ((0.65 + (zoom - 1.0) / 11.0 * 0.30) * master_amp_mod).clamp(0.5, 1.0);

    // Globale chaos-maat uit SimParams (0 = maximale orde, 1 = maximale chaos)
    // Combineert lenia_growth_sigma (hoog = chaos) en friction (laag = chaos)
    let sigma_chaos    = ((params.lenia_growth_sigma - 0.02) / 0.14).clamp(0.0, 1.0);
    let friction_chaos = (1.0 - params.friction / 0.98).clamp(0.0, 1.0);
    let _global_chaos  = (sigma_chaos * 0.6 + friction_chaos * 0.4).clamp(0.0, 1.0);

    // inter_type_attraction_scale: [-1, 3] → gate [0.05, 0.9]
    let global_gate       = ((params.inter_type_attraction_scale + 1.0) / 4.0).clamp(0.05, 0.9);

    // r_smooth [0.1, 20] → saturation [0.7, 0.1] (lage r_smooth = scherpe krachten = rasp)
    let global_saturation = (1.0 - params.r_smooth / 20.0).clamp(0.1, 0.8);

    let base_freqs = [55.0f32, 82.4, 123.5, 51.9, 73.4, 58.3, 69.3];

    // Per-stem gate-offset: gebaseerd op gouden ratio spreiding zodat elke stem
    // een unieke positie in het gate-spectrum heeft. Zorgt voor differentiatie
    // in de mix — niet alle stems klinken gelijk.
    const GOLDEN: f32 = 0.618_033_9;
    let stem_gate_offsets: [f32; 7] = std::array::from_fn(|i| {
        ((i as f32 * GOLDEN).fract() * 2.0 - 1.0) * 0.25  // ±0.25 spread rond global_gate
    });

    // Zoom-uitdunning: hoe meer ingezoomd, hoe sterker stems met lage gate wegvallen.
    // Bij zoom 1 speelt alles mee; bij zoom 12 zijn alleen de meest open stems hoorbaar.
    // zoom_thin: 0 bij zoom 1, 1 bij zoom 12
    let zoom_thin = ((zoom - 1.0) / 11.0).clamp(0.0, 1.0);

    // EMA-smoothing coëfficiënt (~150ms bij 10 Hz updaterate)
    const ALPHA: f32 = 0.25;

    let mut stems = prev.stems;

    for i in 0..7 {
        let base_freq = base_freqs[i];

        // Per-stem gate: global_gate ± unieke offset, zodat elke stem anders reageert
        let stem_gate = (global_gate + stem_gate_offsets[i]).clamp(0.05, 0.95);

        // SimParams-gebaseerde waarden
        let temp_mod  = 1.0 + friction_chaos * 0.08;
        let sim_freq  = base_freq * temp_mod;
        let sim_det   = 0.1 + _global_chaos * 0.4;
        let sim_gate  = stem_gate;
        let sim_noise = sigma_chaos * 0.5;
        let sim_sat   = global_saturation;
        let sim_pan   = (i as f32 / 6.0) * 2.0 - 1.0; // -1..1 over 7 typen

        // Zoom-uitdunning: stems met lage gate worden stil naarmate je inzoomt.
        // stem_gate^3 geeft een sterk niet-lineair verval — lage gates dalen snel,
        // hoge gates blijven hoorbaar. Exponent stijgt met zoom.
        let zoom_exp  = 1.0 + zoom_thin * 4.0;          // 1.0 bij zoom 1, 5.0 bij zoom 12
        let sim_amp   = stem_gate.powf(zoom_exp).clamp(0.01, 1.0);

        // GPU-gebaseerde correcties (alleen bij voldoende zoom)
        let (gpu_freq, gpu_det, gpu_gate, gpu_noise, gpu_sat, gpu_pan, gpu_amp) =
            if let Some(stats) = gpu_stats {
                let s = &stats[i];

                let energy_norm = (s.energy / 500.0).clamp(0.0, 1.0);

                // order × energy: de kern van de nieuwe sonificatie
                //   hoog order + laag energy  → stabiele cluster    → rustig, clean
                //   hoog order + hoog energy  → vibrerende cluster  → gespannen, buzz
                //   laag order + hoog energy  → chaos               → ruis, breed
                //   laag order + laag energy  → dood                → stil, ambient
                let vibration = s.order * energy_norm; // [0,1]: hoog = vibrerende cluster

                // Frequentie: energie drijft pitch omhoog
                let energy_freq  = sim_freq * (1.0 + energy_norm * 0.06);

                // Detune: vibrerende clusters klinken breder, gestabiliseerd door global_detune_mod
                let energy_det   = (sim_det + vibration * 0.4) * global_detune_mod;

                // Gate: hoge orde (cluster aanwezig) → opener geluid
                let order_gate   = (sim_gate + (s.order - 0.5) * 0.3).clamp(0.05, 0.95);

                // Noise: energie → meer ruis, maar getemperd door orde
                // Een vibrerende cluster klinkt anders dan losse chaos
                let order_noise  = (sim_noise + energy_norm * 0.25 - s.order * 0.1).clamp(0.0, 0.8);

                // Saturatie: vibratie binnen een cluster → buzz/drive
                let vib_sat      = (sim_sat + vibration * 0.35).clamp(0.0, 0.95);

                // Pan: centroid_x van het type
                let pan          = (s.centroid_x * 2.0 - 1.0).clamp(-1.0, 1.0);

                // Amplitude: proportioneel aan viewport_count van dit type
                // Normaliseer op verwacht gemiddelde (num_particles / 7)
                let count_norm   = (s.viewport_count / 50.0).clamp(0.0, 1.5);
                let count_amp    = (count_norm * (0.5 + s.order * 0.5)).clamp(0.05, 1.4);

                (energy_freq, energy_det, order_gate, order_noise, vib_sat, pan, count_amp)
            } else {
                (sim_freq, sim_det, sim_gate, sim_noise, sim_sat, sim_pan, sim_amp)
            };

        // Blend op basis van gpu_blend
        let lerp = |a: f32, b: f32| a + gpu_blend * (b - a);

        let target_freq  = lerp(sim_freq,  gpu_freq);
        let target_det   = lerp(sim_det,   gpu_det);
        let target_gate  = lerp(sim_gate,  gpu_gate);
        let target_noise = lerp(sim_noise, gpu_noise);
        let target_sat   = lerp(sim_sat,   gpu_sat);
        let target_pan   = lerp(sim_pan,   gpu_pan);
        let target_amp   = lerp(sim_amp,   gpu_amp);

        // EMA-smoothing
        let s = &mut stems[i];
        s.frequency  += ALPHA * (target_freq  - s.frequency);
        s.detune     += ALPHA * (target_det   - s.detune);
        s.gate       += ALPHA * (target_gate  - s.gate);
        s.noise      += ALPHA * (target_noise - s.noise);
        s.saturation += ALPHA * (target_sat   - s.saturation);
        s.pan        += ALPHA * (target_pan   - s.pan);
        s.amplitude  += ALPHA * (target_amp   - s.amplitude);
    }

    let drone = drone_params(params, gpu_stats, gpu_blend);

    SonificationState { stems, master_amplitude, gpu_blend, zoom, drone }
}

/// Normalisatie van GpuTypeStats::energy (world-units/s) — zelfde schaal als de stems.
const DRONE_ENERGY_NORM: f32 = 500.0;
/// Een type telt pas als "beweeglijkste" mee met minstens zoveel deeltjes in beeld,
/// anders springt de theremin achter een paar losse deeltjes aan.
const DRONE_MIN_LEAD_COUNT: f32 = 5.0;

/// Drone-besturing. Zonder GPU-stats (zoom ≤ 3) alleen temperatuur en hypothese — de drone
/// gaat dan over de algemene toestand van de simulatie. Ingezoomd mengen de
/// clusterwaarden geleidelijk in via gpu_blend.
fn drone_params(
    params: &SimulationParams,
    gpu_stats: Option<&[GpuTypeStats; 7]>,
    gpu_blend: f32,
) -> DroneParams {
    let mut p = DroneParams {
        temperature: temperature_norm(params),
        surface:     if params.is_wlp { 1.0 } else { 0.0 },
        // Zichtbare duisternis: de theremin zwijgt pas helemaal als het hele scherm
        // donker is, en klinkt weer zodra er ergens licht terugkomt
        night:       params.night_darkness,
        ..DroneParams::default()
    };
    let Some(stats) = gpu_stats else { return p };
    let total: f32 = stats.iter().map(|s| s.viewport_count).sum();
    if total < 1.0 {
        return p;
    }

    // Gemiddelden gewogen naar aantal deeltjes in beeld
    let (mut energy, mut order, mut center) = (0.0f32, 0.0f32, 0.0f32);
    for s in stats {
        let w = s.viewport_count / total;
        energy += w * (s.energy / DRONE_ENERGY_NORM).clamp(0.0, 1.0);
        order  += w * s.order.clamp(0.0, 1.0);
        center += w * (s.centroid_x * 2.0 - 1.0);
    }
    let lead = stats
        .iter()
        .filter(|s| s.viewport_count >= DRONE_MIN_LEAD_COUNT)
        .max_by(|a, b| a.energy.total_cmp(&b.energy))
        .map_or(center, |s| s.centroid_x * 2.0 - 1.0);

    p.chaos      = gpu_blend * energy * (1.0 - order);
    p.motion     = gpu_blend * energy;
    p.calm       = gpu_blend * order * (1.0 - energy);
    p.pan_center = gpu_blend * center.clamp(-1.0, 1.0);
    p.pan_lead   = gpu_blend * lead.clamp(-1.0, 1.0);
    p
}

/// Temperatuur [min, max] van de actieve hypothese → [0, 1].
pub fn temperature_norm(params: &SimulationParams) -> f32 {
    let (min, max) = if params.is_wlp {
        (crate::life_params_gen::wlp::SLIDER1_MIN, crate::life_params_gen::wlp::SLIDER1_MAX)
    } else {
        (crate::life_params_gen::htv::SLIDER1_MIN, crate::life_params_gen::htv::SLIDER1_MAX)
    };
    ((params.temperature_level - min) / (max - min)).clamp(0.0, 1.0)
}

// ─── Onweer ──────────────────────────────────────────────────────────────────

/// Seconden donderwolk-opbouw vóór een inslag. Moet gelijk blijven aan lead_time
/// in lightning_frag_buffer.wgsl en PRE_GEN_LEAD in lightning_compute.wgsl.
const STORM_LEAD: f32 = 3.0;
/// Bliksemradius (UV-eenheden) die als "maximaal dichtbij" klinkt. Een bliksem
/// bestaat uit segmenten van 0.02–0.03 UV (lightning_compute.wgsl); de omtrek van
/// een hele bliksem ligt daardoor meestal ruim onder de 0.2.
const STORM_FULL_RADIUS: f32 = 0.15;

/// Volgt de bliksemplanning van de GPU via de simulatieklok, zodat het onweer
/// precies samenvalt met wat op het scherm gebeurt — zonder op een GPU-readback
/// te wachten. De LightningBolt wordt maar af en toe teruggelezen (WASM 1×/s,
/// native rond de preview), maar next_lightning_time ligt vast zodra de vorige
/// inslag gevallen is (lightning_compute.wgsl), dus de CPU weet ruim op tijd
/// wanneer de verduistering begint en wanneer de flits komt.
#[derive(Debug, Clone, Default)]
pub struct StormTracker {
    /// Geplande tijd (simulatieklok) van de volgende inslag; 0 = onbekend
    next_time: f32,
    /// next_time waarvoor de inslag al geklonken heeft
    fired_for: f32,
    strikes:   u32,
    /// Grootte, positie en super-vlag van de komende inslag, uit de preview
    preview:   Option<(f32, f32, bool)>,
    /// Zelfde, van de laatste inslag
    last:      (f32, f32, bool),
}

impl StormTracker {
    /// Elke frame aanroepen ná compute_sonification. `bolt` is de laatst
    /// teruggelezen LightningBolt (mag oud zijn).
    pub fn update(&mut self, bolt: Option<&crate::LightningBolt>, params: &SimulationParams, drone: &mut DroneParams) {
        let now = params.time;

        // Bliksem uit (diepzee, of elektrische activiteit op 0): de shader wist
        // dan ook zijn planning — wij doen hetzelfde
        if params.lightning_frequency <= 0.0 {
            self.next_time = 0.0;
            self.preview = None;
        } else if let Some(b) = bolt {
            // Nieuwe planning overnemen — maar niet de oude die nog in een
            // verouderde readback staat terwijl die inslag al geweest is
            if b.next_lightning_time > 0.0 && b.next_lightning_time > self.fired_for
                && b.next_lightning_time != self.next_time
            {
                self.next_time = b.next_lightning_time;
                self.preview = None;
            }
            if b.preview_ready != 0 && b.next_lightning_time == self.next_time {
                self.preview = Some((
                    (b.preview_radius / STORM_FULL_RADIUS).clamp(0.0, 1.0),
                    (b.preview_center_x * 2.0 - 1.0).clamp(-1.0, 1.0),
                    b.preview_is_super != 0,
                ));
            }
        }

        // Opbouw: zelfde kwadratische curve als de verduistering op scherm
        let time_to_next = self.next_time - now;
        drone.storm = if self.next_time > 0.0 && (0.0..=STORM_LEAD).contains(&time_to_next) {
            let x = 1.0 - time_to_next / STORM_LEAD;
            x * x
        } else {
            0.0
        };

        // Inslag: de simulatieklok passeert de geplande tijd
        if self.next_time > 0.0 && now >= self.next_time && self.fired_for != self.next_time {
            self.fired_for = self.next_time;
            // Ver achter (tab was verborgen e.d.)? Dan niet alsnog laten knallen
            if now - self.next_time < 1.0 {
                self.strikes += 1;
                self.last = self.preview.unwrap_or((0.6, 0.0, false));
                crate::console_log!(
                    "⚡ Onweer: inslag {} — sterkte {:.2}{}, pan {:+.2}{}",
                    self.strikes,
                    self.last.0,
                    if self.preview.is_none() { " (geen preview)" } else { "" },
                    self.last.1,
                    if self.last.2 { ", super" } else { "" },
                );
            }
        }

        let (power, pan, is_super) = self.last;
        drone.strike_id    = self.strikes as f32;
        drone.strike_age   = if self.strikes > 0 { (now - self.fired_for).clamp(0.0, 10.0) } else { 10.0 };
        drone.strike_power = power;
        drone.strike_pan   = pan;
        drone.strike_super = if is_super { 1.0 } else { 0.0 };
        drone.lightning    = if params.lightning_frequency > 0.0 { 1.0 } else { 0.0 };
        drone.strike_eta   = if self.next_time > 0.0 && time_to_next > 0.0 { time_to_next.min(10.0) } else { 10.0 };
    }
}

/// Gemiddelde particle-snelheid in beeld (world-units/s), gewogen naar aantal per type.
pub fn mean_speed(stats: &[GpuTypeStats; 7]) -> f32 {
    let total: f32 = stats.iter().map(|s| s.viewport_count).sum();
    if total < 1.0 {
        return 0.0;
    }
    stats.iter().map(|s| s.energy * s.viewport_count).sum::<f32>() / total
}
