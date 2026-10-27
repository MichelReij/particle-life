// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

//! Hydrothermale uitbarstingen (alleen diepzee / HTV).
//!
//! De simulatie is de klok: hier wordt elke uitbarsting gepland, en dezelfde
//! gebeurtenis stuurt het vents-geluid (dsp.rs), de opstijgende bubbels op het
//! scherm (bubbles_frag.wgsl) en de oranjerode lava op de neopixel-ring (via het
//! EruptionPacket naar de liaison-ESP32). Uitbarstingen komen alleen uit de
//! onderste twee derde van het beeld — de bubbels stijgen daarvandaan omhoog.
//! Op de plek van de bron gloeit een rood-oranje waas, zoals de lava op de ring.

use crate::dsp::DroneParams;
use crate::simulation_params::SimulationParams;
use crate::sonification::temperature_norm;

/// Maximaal aantal uitbarstingen waarvan tegelijk bubbels te zien zijn.
pub const MAX_ERUPTIONS: usize = 4;
/// Bellen per uitbarsting (hooguit). Moet gelijk blijven aan MAX_BUBBLES in
/// bubbles.wgsl.
pub const MAX_BUBBLES: usize = 88;
/// Aantal vierkantjes dat de renderer tekent: per uitbarsting een gloed en
/// MAX_BUBBLES bellen (zie bubbles.wgsl).
pub const GPU_INSTANCES: u32 = (MAX_ERUPTIONS * (1 + MAX_BUBBLES)) as u32;
/// Grootte van gpu_data(): drie vec4's per uitbarsting.
pub const GPU_FLOATS_PER_ERUPTION: usize = 12;
pub const GPU_FLOATS: usize = MAX_ERUPTIONS * GPU_FLOATS_PER_ERUPTION;
/// Bronnen liggen binnen deze afstand van het midden van de wereld (UV), binnen
/// de rand van het ronde scherm bij 1× zoom (de vignettering dekt de uiterste
/// rand af)...
const SOURCE_RADIUS: f32 = 0.38;
/// ...en niet hoger dan deze y (wereld-UV, y naar beneden): de onderste twee
/// derde van het beeld.
const SOURCE_MIN_Y: f32 = 1.0 / 3.0;
/// Tijd (s) tussen uitbarstingen: klokvormig verdeeld binnen dit bereik
/// (bell_range met SPACING_SAMPLES), gedeeld door (0.6 + 0.8 × warmte) — heet
/// gemiddeld ~40 s, koud ~90 s, nooit korter dan ~14 s.
const SPACING: (f32, f32) = (20.0, 90.0);
const SPACING_SAMPLES: u32 = 3;
/// De eerste uitbarsting na het (opnieuw) beginnen van de diepzee.
const FIRST: (f32, f32) = (5.0, 20.0);
/// Hoe lang de laatste bubbels van een uitbarsting nog opstijgen (s). Moet
/// gelijk blijven aan RISE_TIME in bubbles_frag.wgsl.
pub const BUBBLE_RISE_TIME: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Eruption {
    pub id: u32,
    /// Begin (simulatieklok, s)
    pub start: f32,
    /// Bron in wereld-UV (0..1, y naar beneden)
    pub x: f32,
    pub y: f32,
    /// Grootte [0, 1]: volume van het geluid, helderheid op de ring, bubbels
    pub size: f32,
    /// Hitte [0, 1]: hoe ver de lava op de ring van rood naar geel gaat
    pub heat: f32,
    /// Breedte op de ring (pixels)
    pub reach: u8,
    /// Opkomen, aanhouden en uitdoven (s)
    pub ramp: f32,
    pub hold: f32,
    pub fade: f32,
    /// Variatie in het bubbelpatroon [0, 1)
    pub seed: f32,
}

impl Eruption {
    /// Bron in wereld-UV (0..1, y naar beneden).
    pub fn source(&self) -> (f32, f32) {
        (self.x, self.y)
    }

    /// Richting van de bron vanuit het midden (rad, atan2 met y naar beneden).
    pub fn angle(&self) -> f32 {
        (self.y - 0.5).atan2(self.x - 0.5)
    }

    /// Positie voor de ESP32's: 0–255 over 360°, dezelfde canonieke hoek als
    /// de bliksem (zie native_minimal.rs).
    pub fn position_byte(&self) -> u8 {
        (((self.angle() + std::f32::consts::PI) / std::f32::consts::TAU) * 255.0) as u8
    }

    /// Links-rechts [-1, 1] voor het geluid.
    pub fn pan(&self) -> f32 {
        ((self.x - 0.5) / SOURCE_RADIUS).clamp(-1.0, 1.0)
    }

    /// Piekhelderheid op de ring [0, 1]: nooit helemaal vol, anders te wit.
    pub fn ring_intensity(&self) -> f32 {
        0.4 + 0.45 * self.size
    }

    /// Hoe lang de straal spuit: zo lang komen er bubbels uit de bron.
    pub fn emit_duration(&self) -> f32 {
        self.ramp + self.hold
    }

    fn finished(&self, now: f32) -> bool {
        now - self.start > self.emit_duration() + BUBBLE_RISE_TIME
    }
}

#[derive(Debug, Default)]
pub struct EruptionScheduler {
    /// Geplande tijd van de volgende uitbarsting; None = nog niet gepland
    next_time: Option<f32>,
    next_id: u32,
    active: Vec<Eruption>,
    /// Laatst begonnen uitbarsting, ook als de bubbels al weg zijn (het geluid
    /// herkent een nieuwe aan een ander id)
    last: Option<Eruption>,
    rng: u32,
}

impl EruptionScheduler {
    /// Elke frame aanroepen. Geeft een uitbarsting terug op het moment dat die
    /// begint (om naar de ESP32's te sturen).
    pub fn update(&mut self, params: &SimulationParams) -> Option<Eruption> {
        let now = params.time;
        if params.is_wlp {
            // Getijdenpoel: geen bronnen, en de bubbels verdwijnen meteen mee
            self.next_time = None;
            self.active.clear();
            return None;
        }
        self.active.retain(|e| !e.finished(now) && e.start <= now);

        let warmth = temperature_norm(params);
        if self.next_time.is_none() {
            self.next_time = Some(now + self.bell_range(FIRST.0, FIRST.1, SPACING_SAMPLES));
        }
        if now < self.next_time.unwrap_or(now) {
            return None;
        }
        self.next_time = Some(now + self.bell_range(SPACING.0, SPACING.1, SPACING_SAMPLES) / (0.6 + 0.8 * warmth));

        // Willekeurige plek in de onderste twee derde, binnen de cirkel
        let (x, y) = loop {
            let (x, y) = (self.range(0.5 - SOURCE_RADIUS, 0.5 + SOURCE_RADIUS), self.range(SOURCE_MIN_Y, 0.5 + SOURCE_RADIUS));
            if (x - 0.5).hypot(y - 0.5) <= SOURCE_RADIUS {
                break (x, y);
            }
        };
        let size = self.unit();
        // Meestal rood tot dieporanje, soms een oranje kern, zelden geel
        let roll = self.unit();
        let heat = if roll < 0.88 {
            self.range(0.0, 0.4)
        } else if roll < 0.985 {
            self.range(0.4, 0.65)
        } else {
            self.range(0.65, 0.85)
        };
        self.next_id = self.next_id.wrapping_add(1);
        let eruption = Eruption {
            id: self.next_id,
            start: now,
            x,
            y,
            size,
            heat,
            reach: (4.0 + 14.0 * size + self.range(-2.0, 2.0)).round().clamp(2.0, 20.0) as u8,
            ramp: self.range(1.0, 3.0),
            hold: self.range(0.3, 0.9),
            fade: self.range(0.8, 2.0),
            seed: self.unit(),
        };
        if self.active.len() == MAX_ERUPTIONS {
            self.active.remove(0);
        }
        self.active.push(eruption);
        self.last = Some(eruption);
        Some(eruption)
    }

    /// De laatste uitbarsting in de drone-parameters (vents-geluid).
    pub fn apply_to_drone(&self, now: f32, drone: &mut DroneParams) {
        if let Some(e) = &self.last {
            drone.eruption_id = e.id as f32;
            drone.eruption_age = (now - e.start).clamp(0.0, 10.0);
            drone.eruption_strength = e.size;
            drone.eruption_pan = e.pan();
            drone.eruption_hold = e.emit_duration();
            drone.eruption_release = e.fade * 1.5;
        }
    }

    /// Voor bubbles.wgsl: per uitbarsting drie vec4's —
    /// (bron x, bron y, begin, grootte), (spuitduur, seed, hitte, actief) en
    /// (opkomen, aanhouden, uitdoven, helderheid) voor de gloed.
    pub fn gpu_data(&self) -> [f32; GPU_FLOATS] {
        let mut out = [0.0f32; GPU_FLOATS];
        for (chunk, e) in out.chunks_exact_mut(GPU_FLOATS_PER_ERUPTION).zip(&self.active) {
            let (x, y) = e.source();
            chunk.copy_from_slice(&[
                x, y, e.start, e.size,
                e.emit_duration(), e.seed, e.heat, 1.0,
                e.ramp, e.hold, e.fade, e.ring_intensity(),
            ]);
        }
        out
    }

    fn unit(&mut self) -> f32 {
        if self.rng == 0 {
            self.rng = 0x9E37_79B9;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }

    /// bellRandom() uit platformio/include/MathUtils.h: het gemiddelde van
    /// `samples` uniforme trekkingen — 1 = uniform, 2 = driehoek, ≥3 steeds meer
    /// een klokvorm rond het midden.
    fn bell_range(&mut self, lo: f32, hi: f32, samples: u32) -> f32 {
        let n = samples.max(1);
        let t = (0..n).map(|_| self.unit()).sum::<f32>() / n as f32;
        lo + (hi - lo) * t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn htv_params() -> SimulationParams {
        let mut p = SimulationParams::new();
        p.is_wlp = false;
        p
    }

    /// Simuleert `seconds` met 60 fps en verzamelt de uitbarstingen.
    fn run(s: &mut EruptionScheduler, p: &mut SimulationParams, seconds: f32) -> Vec<Eruption> {
        let mut out = Vec::new();
        for _ in 0..(seconds * 60.0) as usize {
            p.time += 1.0 / 60.0;
            out.extend(s.update(p));
        }
        out
    }

    #[test]
    fn erupts_in_htv_from_the_lower_two_thirds() {
        let mut s = EruptionScheduler::default();
        let mut p = htv_params();
        let eruptions = run(&mut s, &mut p, 1200.0);
        assert!(eruptions.len() >= 10, "te weinig uitbarstingen: {}", eruptions.len());
        for e in &eruptions {
            let (x, y) = e.source();
            assert!(y >= SOURCE_MIN_Y, "bron niet in de onderste twee derde: {e:?}");
            assert!((x - 0.5).hypot(y - 0.5) <= SOURCE_RADIUS + 1e-6);
            assert!((2..=20).contains(&e.reach));
        }
        assert!(eruptions.iter().any(|e| e.y < 0.5), "nooit een bron boven het midden");
        let ids: Vec<u32> = eruptions.iter().map(|e| e.id).collect();
        assert!(ids.windows(2).all(|w| w[1] == w[0] + 1));
    }

    #[test]
    fn silent_in_wlp() {
        let mut s = EruptionScheduler::default();
        let mut p = htv_params();
        run(&mut s, &mut p, 60.0);
        p.is_wlp = true;
        assert!(run(&mut s, &mut p, 120.0).is_empty());
        assert!(s.gpu_data().iter().all(|&v| v == 0.0));
    }

    #[test]
    fn drone_gets_the_latest_eruption() {
        let mut s = EruptionScheduler::default();
        let mut p = htv_params();
        let last = *run(&mut s, &mut p, 200.0).last().unwrap();
        let mut d = DroneParams::default();
        s.apply_to_drone(p.time, &mut d);
        assert_eq!(d.eruption_id, last.id as f32);
        assert!(d.eruption_age >= 0.0 && d.eruption_age < 30.0);
        assert!((d.eruption_pan - last.pan()).abs() < 1e-6);
    }
}
