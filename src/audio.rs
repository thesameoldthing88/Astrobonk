//! Zero-asset audio: a tiny WAV synthesizer builds every SFX at startup.

use crate::messages::{Sfx, SfxMsg};
use bevy::audio::{AudioPlayer, AudioSource, PlaybackMode, PlaybackSettings, Volume};
use bevy::prelude::*;
use std::collections::HashMap;

const RATE: u32 = 22050;

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Square,
    Saw,
    Noise,
}

/// Render a swept tone with exponential decay into samples.
fn tone(buf: &mut Vec<f32>, wave: Wave, f0: f32, f1: f32, dur: f32, amp: f32, offset: f32) {
    let n = (dur * RATE as f32) as usize;
    let start = (offset * RATE as f32) as usize;
    if buf.len() < start + n {
        buf.resize(start + n, 0.0);
    }
    let mut phase = 0.0f32;
    let mut noise_state = 0x12345678u32;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let f = f0 + (f1 - f0) * t;
        phase += f / RATE as f32;
        let env = (1.0 - t).powf(1.8);
        let s = match wave {
            Wave::Sine => (phase * std::f32::consts::TAU).sin(),
            Wave::Square => {
                if phase.fract() < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Wave::Saw => phase.fract() * 2.0 - 1.0,
            Wave::Noise => {
                noise_state = noise_state.wrapping_mul(1664525).wrapping_add(1013904223);
                (noise_state >> 16) as f32 / 32768.0 - 1.0
            }
        };
        buf[start + i] += s * env * amp;
    }
}

fn to_wav(samples: &[f32]) -> Vec<u8> {
    let n = samples.len() as u32;
    let data_len = n * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 30000.0) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

#[derive(Resource)]
pub struct SfxBank {
    pub map: HashMap<Sfx, Handle<AudioSource>>,
}

pub fn build_sfx_bank(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let mut map = HashMap::new();
    let mut add = |sfx: Sfx, samples: Vec<f32>| {
        let bytes: Vec<u8> = to_wav(&samples);
        let handle = sources.add(AudioSource { bytes: bytes.into() });
        map.insert(sfx, handle);
    };

    let mut b = Vec::new();
    tone(&mut b, Wave::Square, 220.0, 170.0, 0.07, 0.35, 0.0);
    add(Sfx::Hit, std::mem::take(&mut b));

    tone(&mut b, Wave::Square, 460.0, 320.0, 0.09, 0.4, 0.0);
    tone(&mut b, Wave::Sine, 920.0, 640.0, 0.09, 0.2, 0.0);
    add(Sfx::Crit, std::mem::take(&mut b));

    tone(&mut b, Wave::Saw, 130.0, 70.0, 0.18, 0.5, 0.0);
    tone(&mut b, Wave::Noise, 0.0, 0.0, 0.1, 0.25, 0.0);
    add(Sfx::Hurt, std::mem::take(&mut b));

    tone(&mut b, Wave::Sine, 760.0, 1050.0, 0.07, 0.3, 0.0);
    add(Sfx::Pickup, std::mem::take(&mut b));

    tone(&mut b, Wave::Sine, 1180.0, 1180.0, 0.05, 0.3, 0.0);
    tone(&mut b, Wave::Sine, 1560.0, 1560.0, 0.07, 0.25, 0.05);
    add(Sfx::Coin, std::mem::take(&mut b));

    tone(&mut b, Wave::Sine, 523.0, 523.0, 0.09, 0.35, 0.0);
    tone(&mut b, Wave::Sine, 659.0, 659.0, 0.09, 0.35, 0.08);
    tone(&mut b, Wave::Sine, 784.0, 784.0, 0.14, 0.35, 0.16);
    add(Sfx::LevelUp, std::mem::take(&mut b));

    tone(&mut b, Wave::Sine, 392.0, 392.0, 0.1, 0.3, 0.0);
    tone(&mut b, Wave::Sine, 523.0, 523.0, 0.16, 0.3, 0.09);
    add(Sfx::Chest, std::mem::take(&mut b));

    tone(&mut b, Wave::Square, 220.0, 880.0, 0.35, 0.3, 0.0);
    tone(&mut b, Wave::Sine, 440.0, 1760.0, 0.35, 0.2, 0.05);
    add(Sfx::Evolve, std::mem::take(&mut b));

    tone(&mut b, Wave::Square, 65.0, 45.0, 0.4, 0.45, 0.0);
    tone(&mut b, Wave::Noise, 0.0, 0.0, 0.3, 0.3, 0.0);
    add(Sfx::BossRoar, std::mem::take(&mut b));

    tone(&mut b, Wave::Sine, 1000.0, 900.0, 0.035, 0.25, 0.0);
    add(Sfx::Click, std::mem::take(&mut b));

    tone(&mut b, Wave::Noise, 0.0, 0.0, 0.07, 0.4, 0.0);
    tone(&mut b, Wave::Sine, 300.0, 180.0, 0.05, 0.2, 0.0);
    add(Sfx::Pot, std::mem::take(&mut b));

    tone(&mut b, Wave::Sine, 880.0, 120.0, 0.32, 0.35, 0.0);
    add(Sfx::Teleport, std::mem::take(&mut b));

    tone(&mut b, Wave::Sine, 660.0, 700.0, 0.22, 0.3, 0.0);
    tone(&mut b, Wave::Sine, 990.0, 1020.0, 0.18, 0.2, 0.06);
    add(Sfx::Shrine, std::mem::take(&mut b));

    // slide: dusty whoosh
    tone(&mut b, Wave::Noise, 0.0, 0.0, 0.20, 0.22, 0.0);
    tone(&mut b, Wave::Sine, 320.0, 140.0, 0.18, 0.15, 0.0);
    add(Sfx::Slide, std::mem::take(&mut b));

    // bhop: quick uplift chirp — "kept it!"
    tone(&mut b, Wave::Square, 520.0, 940.0, 0.09, 0.28, 0.0);
    add(Sfx::Bhop, std::mem::take(&mut b));

    // comet cash-out: big boom-sweep, the signature payoff
    tone(&mut b, Wave::Saw, 220.0, 55.0, 0.5, 0.45, 0.0);
    tone(&mut b, Wave::Noise, 0.0, 0.0, 0.35, 0.3, 0.0);
    tone(&mut b, Wave::Sine, 880.0, 1760.0, 0.4, 0.2, 0.05);
    add(Sfx::Comet, std::mem::take(&mut b));

    // slam: a low body-blow thud under a gravel spray — the crater you just made
    tone(&mut b, Wave::Saw, 120.0, 38.0, 0.34, 0.5, 0.0);
    tone(&mut b, Wave::Sine, 70.0, 30.0, 0.3, 0.45, 0.0);
    tone(&mut b, Wave::Noise, 0.0, 0.0, 0.22, 0.3, 0.02);
    add(Sfx::Slam, std::mem::take(&mut b));

    // grind: the metal-on-rock "shhk" of catching a rail
    tone(&mut b, Wave::Noise, 0.0, 0.0, 0.16, 0.22, 0.0);
    tone(&mut b, Wave::Square, 1480.0, 1320.0, 0.12, 0.12, 0.0);
    tone(&mut b, Wave::Sine, 2960.0, 2700.0, 0.1, 0.08, 0.01);
    add(Sfx::Grind, std::mem::take(&mut b));

    // blink: an inhale-and-pop — a rising suck, then the far side
    tone(&mut b, Wave::Sine, 180.0, 1400.0, 0.16, 0.3, 0.0);
    tone(&mut b, Wave::Square, 1600.0, 900.0, 0.08, 0.2, 0.15);
    tone(&mut b, Wave::Noise, 0.0, 0.0, 0.08, 0.15, 0.15);
    add(Sfx::Blink, std::mem::take(&mut b));

    // flashlight: a small switch click
    tone(&mut b, Wave::Square, 2400.0, 2200.0, 0.018, 0.2, 0.0);
    tone(&mut b, Wave::Noise, 0.0, 0.0, 0.015, 0.15, 0.0);
    add(Sfx::Flashlight, std::mem::take(&mut b));

    // thorns: a dry scratch and a little yelp of fabric — you are wading through it
    tone(&mut b, Wave::Noise, 0.0, 0.0, 0.12, 0.28, 0.0);
    tone(&mut b, Wave::Saw, 1900.0, 1300.0, 0.06, 0.12, 0.0);
    tone(&mut b, Wave::Saw, 1700.0, 1100.0, 0.05, 0.1, 0.07);
    add(Sfx::Thorns, std::mem::take(&mut b));

    // spore cap primed: a wet, rising hiss — something is about to go
    tone(&mut b, Wave::Noise, 0.0, 0.0, 0.5, 0.18, 0.0);
    tone(&mut b, Wave::Sine, 180.0, 520.0, 0.5, 0.16, 0.0);
    add(Sfx::Spore, std::mem::take(&mut b));

    // spore cap bursts: a soft thump and a puff
    tone(&mut b, Wave::Sine, 150.0, 60.0, 0.22, 0.4, 0.0);
    tone(&mut b, Wave::Noise, 0.0, 0.0, 0.3, 0.26, 0.02);
    add(Sfx::SporePop, std::mem::take(&mut b));

    commands.insert_resource(SfxBank { map });
}

/// Rate-limit identical sounds so 40 hits a frame don't clip the mixer.
#[derive(Resource, Default)]
pub struct SfxThrottle {
    pub last: HashMap<Sfx, f32>,
}

pub fn play_sfx(
    mut commands: Commands,
    time: Res<Time<Real>>,
    bank: Option<Res<SfxBank>>,
    save: Res<crate::save::MetaSave>,
    mut throttle: ResMut<SfxThrottle>,
    mut reader: MessageReader<SfxMsg>,
) {
    let Some(bank) = bank else {
        reader.clear();
        return;
    };
    let now = time.elapsed_secs();
    for msg in reader.read() {
        let min_gap = match msg.0 {
            Sfx::Hit => 0.05,
            Sfx::Pickup | Sfx::Coin => 0.04,
            _ => 0.02,
        };
        if let Some(last) = throttle.last.get(&msg.0) {
            if now - last < min_gap {
                continue;
            }
        }
        throttle.last.insert(msg.0, now);
        if let Some(handle) = bank.map.get(&msg.0) {
            commands.spawn((
                AudioPlayer(handle.clone()),
                PlaybackSettings {
                    mode: PlaybackMode::Despawn,
                    volume: Volume::Linear(save.volume * save.sfx_volume),
                    ..default()
                },
            ));
        }
    }
}
