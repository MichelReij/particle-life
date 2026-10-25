// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// examples/render_drone.rs
// Rendert de ambient drone offline naar een 16-bit stereo WAV, met een lineaire
// temperatuur-sweep zodat het hele tempobereik hoorbaar is.
//
//   cargo run --release --example render_drone -- <uit.wav> [seconden] [temp_start] [temp_eind] [chaos] [beweging] [rust] [branding] [onweer_interval] [mix] [nacht]
//
// Temperaturen zijn genormaliseerd: 0 = koudst, 1 = heetst. Chaos, beweging en rust
// [0, 1] simuleren de clusterstatistieken die de simulatie boven 3× zoom levert;
// ze zwellen in de tweede helft van de render lineair aan van 0 naar de opgegeven waarde.
// Branding 1 = getijdenpoel-hypothese (WLP), vanaf het begin.
// Onweer-interval > 0: elke zoveel seconden een inslag met 3 s donderwolk vooraf;
// grootte wisselt, elke vierde is een superbliksem.
// Mix: komma-gescheiden lineaire gains in de volgorde van dsp::mix (cellos,
// theremin, brainwaves, droplets, stream, surf, thunder, lightning, sonar, reverb,
// master, en optioneel de bliksem-sample-and-hold), bijv. "1,0,0,0,0,0,0,0,0,1,1"
// om alleen de cello's (met galm) te horen.
// Nacht 1: dag-nachtcyclus van de getijdenpoel (80 s dag, 10 s schemering, 20 s
// nacht, 10 s ochtend), zoals SimulationParams::update_night_alpha.

use std::fs::File;
use std::io::{BufWriter, Write};

use particle_life_wasm::dsp::{DroneParams, DroneSynth, SAMPLE_RATE};

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path    = args.get(1).map(String::as_str).unwrap_or("drone.wav");
    let seconds = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(120.0f32);
    let t_start = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0f32);
    let t_end   = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(1.0f32);
    let arg = |i: usize| args.get(i).and_then(|s| s.parse().ok()).unwrap_or(0.0f32);
    let (chaos, motion, calm, surface) = (arg(5), arg(6), arg(7), arg(8));
    let storm_interval = arg(9);
    let night_cycle = arg(11) > 0.0;

    let frames = (seconds * SAMPLE_RATE as f32) as u32;
    let mut synth = DroneSynth::new(SAMPLE_RATE as f32);
    if let Some(mix) = args.get(10) {
        let gains: Vec<f32> = mix.split(',').filter_map(|g| g.trim().parse().ok()).collect();
        synth.set_mix(&gains);
    }

    let mut out = BufWriter::new(File::create(path)?);
    let data_len = frames * 4;
    out.write_all(b"RIFF")?;
    out.write_all(&(36 + data_len).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;           // PCM
    out.write_all(&2u16.to_le_bytes())?;           // stereo
    out.write_all(&SAMPLE_RATE.to_le_bytes())?;
    out.write_all(&(SAMPLE_RATE * 4).to_le_bytes())?;
    out.write_all(&4u16.to_le_bytes())?;
    out.write_all(&16u16.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&data_len.to_le_bytes())?;

    let (mut peak, mut sum_sq) = (0.0f32, 0.0f64);
    let started = std::time::Instant::now();
    for i in 0..frames {
        if i % 4410 == 0 {
            let x = i as f32 / frames as f32;
            let cluster = (2.0 * x - 1.0).max(0.0);
            synth.set_params(&DroneParams {
                temperature: t_start + (t_end - t_start) * x,
                chaos:       chaos * cluster,
                motion:      motion * cluster,
                calm:        calm * cluster,
                surface,
                night: if night_cycle { night(i as f32 / SAMPLE_RATE as f32) } else { 0.0 },
                ..storm_params(i as f32 / SAMPLE_RATE as f32, storm_interval)
            });
        }
        let (l, r) = synth.render();
        peak = peak.max(l.abs()).max(r.abs());
        sum_sq += (l * l + r * r) as f64 * 0.5;
        out.write_all(&((l * 32767.0) as i16).to_le_bytes())?;
        out.write_all(&((r * 32767.0) as i16).to_le_bytes())?;
    }
    out.flush()?;

    let rms = (sum_sq / frames as f64).sqrt();
    println!(
        "{path}: {seconds:.0} s, piek {:.1} dBFS, rms {:.1} dBFS, render {:.1}× realtime",
        20.0 * peak.log10(),
        20.0 * rms.log10(),
        seconds / started.elapsed().as_secs_f32(),
    );
    Ok(())
}

/// Nacht [0, 1] volgens de cyclus van SimulationParams::update_night_alpha.
fn night(t: f32) -> f32 {
    let t = t % 120.0;
    if t < 80.0 { 0.0 } else if t < 90.0 { (t - 80.0) / 10.0 } else if t < 110.0 { 1.0 } else { 1.0 - (t - 110.0) / 10.0 }
}

/// Onweer zoals StormTracker het zou aanleveren: inslag n op n × interval.
fn storm_params(t: f32, interval: f32) -> DroneParams {
    if interval <= 0.0 {
        return DroneParams::default();
    }
    let n = (t / interval).floor();
    let to_next = (n + 1.0) * interval - t;
    let build = if to_next <= 3.0 { (1.0 - to_next / 3.0).powi(2) } else { 0.0 };
    DroneParams {
        storm:        build,
        strike_id:    n,
        strike_age:   if n > 0.0 { (t - n * interval).min(10.0) } else { 10.0 },
        strike_power: [0.9, 0.4, 0.65, 1.0][n as usize % 4],
        strike_pan:   [-0.5, 0.6, 0.0, 0.2][n as usize % 4],
        strike_super: if n as usize % 4 == 3 { 1.0 } else { 0.0 },
        strike_eta:   to_next.min(10.0),
        lightning:    1.0,
        ..DroneParams::default()
    }
}
