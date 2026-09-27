//! Procedural adaptive music — zero assets, same in-code synthesis as our SFX. A full
//! 32-second (16-bar) A-minor synthwave piece with real structure: an 8-bar VERSE, then
//! an 8-bar CHORUS with a lead-melody hook and a lift in the chords. Six phase-locked
//! looped stems (bass, pad, arp, lead, drums, static) whose volumes are driven live by
//! the swarm: there's always a beat, it swells as the horde tightens, and on THE STATIC
//! it detunes flat under a noise wash. "Beautiful becoming broken."

use crate::enemies::Enemy;
use crate::run::{RunPhase, RunState};
use crate::save::MetaSave;
use bevy::audio::{AudioPlayer, AudioSink, AudioSource, PlaybackMode, PlaybackSettings, Volume};
use bevy::prelude::*;

const RATE: u32 = 22050;
const BAR: f32 = 2.0; // 120 BPM, 4 beats/bar
const BARS: usize = 16;
const LOOP_SECS: f32 = BAR * BARS as f32; // 32s
const MUSIC_MIX: f32 = 0.12; // music slider 100% now == the old ~20% level (it was too loud)

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stem {
    Bass,
    Pad,
    Arp,
    Lead,
    Drums,
    Static,
}

#[derive(Component)]
pub struct MusicStem {
    pub stem: Stem,
    pub cur: f32,
}

#[derive(Resource)]
pub struct MusicBank {
    pub stems: Vec<(Stem, Handle<AudioSource>)>,
}

// ---- Chords: Am, F, C, G (root + triad) ----
const ROOTS: [f32; 4] = [110.00, 87.31, 130.81, 98.00];
const TRIADS: [[f32; 3]; 4] = [
    [220.00, 261.63, 329.63], // Am
    [174.61, 220.00, 261.63], // F
    [261.63, 329.63, 392.00], // C
    [196.00, 246.94, 293.66], // G
];
// 16-bar arrangement: verse (Am F C G ×2), then chorus (C G Am F ×2) for a lift.
const PROG: [usize; BARS] = [0, 1, 2, 3, 0, 1, 2, 3, 2, 3, 0, 1, 2, 3, 0, 1];
const CHORUS_START: usize = 8;

// Lead hook, only over the chorus: (chorus_bar, beat_secs, freq, dur). A-minor pentatonic.
const LEAD: &[(usize, f32, f32, f32)] = &[
    (0, 0.0, 523.25, 0.5), (0, 0.5, 659.25, 0.5), (0, 1.0, 783.99, 0.5), (0, 1.5, 659.25, 0.5),
    (1, 0.0, 587.33, 0.5), (1, 0.5, 783.99, 0.5), (1, 1.0, 493.88, 0.5), (1, 1.5, 587.33, 0.5),
    (2, 0.0, 440.00, 0.5), (2, 0.5, 523.25, 0.5), (2, 1.0, 659.25, 0.75), (2, 1.75, 440.00, 0.25),
    (3, 0.0, 440.00, 0.5), (3, 0.5, 523.25, 0.5), (3, 1.0, 440.00, 1.0),
    (4, 0.0, 523.25, 0.5), (4, 0.5, 659.25, 0.5), (4, 1.0, 783.99, 0.5), (4, 1.5, 880.00, 0.5),
    (5, 0.0, 587.33, 0.5), (5, 0.5, 783.99, 0.5), (5, 1.0, 493.88, 0.5), (5, 1.5, 587.33, 0.5),
    (6, 0.0, 440.00, 0.5), (6, 0.5, 523.25, 0.5), (6, 1.0, 659.25, 0.75), (6, 1.75, 523.25, 0.25),
    (7, 0.0, 523.25, 0.5), (7, 0.5, 440.00, 0.5), (7, 1.0, 440.00, 1.0),
];

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Tri,
    Saw,
    Square,
    Noise,
}

fn osc(wave: Wave, phase: f32, noise: &mut u32) -> f32 {
    match wave {
        Wave::Sine => (phase * std::f32::consts::TAU).sin(),
        Wave::Tri => 2.0 * (2.0 * (phase - (phase + 0.5).floor())).abs() - 1.0,
        Wave::Saw => 2.0 * (phase - (phase + 0.5).floor()),
        Wave::Square => {
            if phase.fract() < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Wave::Noise => {
            *noise = noise.wrapping_mul(1664525).wrapping_add(1013904223);
            (*noise >> 16) as f32 / 32768.0 - 1.0
        }
    }
}

fn note(buf: &mut [f32], f: f32, t0: f32, dur: f32, wave: Wave, amp: f32, atk: f32, rel: f32) {
    let start = (t0 * RATE as f32) as usize;
    let n = (dur * RATE as f32) as usize;
    let mut phase = 0.0f32;
    let mut noise = 0x9e3779b9u32;
    for i in 0..n {
        let idx = start + i;
        if idx >= buf.len() {
            break;
        }
        let t = i as f32 / RATE as f32;
        let env = if t < atk {
            t / atk
        } else if t > dur - rel {
            ((dur - t) / rel).max(0.0)
        } else {
            1.0
        };
        phase += f / RATE as f32;
        buf[idx] += osc(wave, phase, &mut noise) * env * amp;
    }
}

fn kick(buf: &mut [f32], t0: f32) {
    let start = (t0 * RATE as f32) as usize;
    let n = (0.14 * RATE as f32) as usize;
    let mut phase = 0.0f32;
    for i in 0..n {
        let idx = start + i;
        if idx >= buf.len() {
            break;
        }
        let t = i as f32 / n as f32;
        let f = 135.0 * (1.0 - t) + 48.0 * t;
        phase += f / RATE as f32;
        buf[idx] += (phase * std::f32::consts::TAU).sin() * (1.0 - t).powf(2.0) * 0.8;
    }
}

fn snare(buf: &mut [f32], t0: f32) {
    let start = (t0 * RATE as f32) as usize;
    let n = (0.10 * RATE as f32) as usize;
    let mut noise = 0x2545f491u32;
    let mut phase = 0.0f32;
    for i in 0..n {
        let idx = start + i;
        if idx >= buf.len() {
            break;
        }
        let t = i as f32 / n as f32;
        let env = (1.0 - t).powf(1.5);
        noise = noise.wrapping_mul(1664525).wrapping_add(1013904223);
        let nz = (noise >> 16) as f32 / 32768.0 - 1.0;
        phase += 190.0 / RATE as f32;
        buf[idx] += (nz * 0.7 + (phase * std::f32::consts::TAU).sin() * 0.3) * env * 0.45;
    }
}

fn build(stem: Stem) -> Vec<f32> {
    let len = (LOOP_SECS * RATE as f32) as usize;
    let mut buf = vec![0.0f32; len];
    match stem {
        Stem::Bass => {
            for bar in 0..BARS {
                let root = ROOTS[PROG[bar]];
                for e in 0..8 {
                    let t = bar as f32 * BAR + e as f32 * 0.25;
                    // octave lift on the off-beats for movement
                    let f = if e % 2 == 1 { root * 2.0 } else { root };
                    note(&mut buf, f, t, 0.2, Wave::Square, 0.30, 0.005, 0.05);
                }
            }
        }
        Stem::Pad => {
            for bar in 0..BARS {
                for &f in &TRIADS[PROG[bar]] {
                    note(&mut buf, f, bar as f32 * BAR, BAR, Wave::Tri, 0.11, 0.3, 0.4);
                }
            }
        }
        Stem::Arp => {
            for bar in 0..BARS {
                let tri = TRIADS[PROG[bar]];
                let seq = [tri[0], tri[1], tri[2], tri[1]];
                // busier in the chorus (16ths) than the verse (8ths)
                let steps = if bar >= CHORUS_START { 16 } else { 8 };
                for s in 0..steps {
                    let t = bar as f32 * BAR + s as f32 * (BAR / steps as f32);
                    note(&mut buf, seq[s % 4] * 2.0, t, BAR / steps as f32 * 0.9, Wave::Saw, 0.11, 0.004, 0.05);
                }
            }
        }
        Stem::Lead => {
            for &(cb, bt, f, d) in LEAD {
                let t = (CHORUS_START + cb) as f32 * BAR + bt;
                note(&mut buf, f, t, d, Wave::Saw, 0.16, 0.01, 0.06);
                note(&mut buf, f * 1.004, t, d, Wave::Saw, 0.08, 0.01, 0.06); // detune thickness
            }
        }
        Stem::Drums => {
            for bar in 0..BARS {
                let b = bar as f32 * BAR;
                kick(&mut buf, b);
                kick(&mut buf, b + 1.0);
                kick(&mut buf, b + 1.5); // "and of 3" push
                snare(&mut buf, b + 0.5);
                snare(&mut buf, b + 1.5);
                for h in 0..8 {
                    note(&mut buf, 8000.0, b + h as f32 * 0.25, 0.025, Wave::Noise, 0.07, 0.001, 0.02);
                }
                if bar % 4 == 0 {
                    note(&mut buf, 6000.0, b, 0.3, Wave::Noise, 0.10, 0.002, 0.28); // crash on 4-bar phrase
                }
            }
        }
        Stem::Static => {
            for bar in 0..BARS {
                note(&mut buf, 55.0, bar as f32 * BAR, BAR, Wave::Saw, 0.09, 0.2, 0.2);
                note(&mut buf, 55.6, bar as f32 * BAR, BAR, Wave::Saw, 0.09, 0.2, 0.2);
            }
            let mut noise = 0x1234567u32;
            for s in buf.iter_mut() {
                noise = noise.wrapping_mul(1664525).wrapping_add(1013904223);
                *s += ((noise >> 16) as f32 / 32768.0 - 1.0) * 0.06;
            }
        }
    }
    buf
}

fn to_wav(samples: &[f32]) -> Vec<u8> {
    let n = samples.len() as u32;
    let data_len = n * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
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

pub fn build_music_bank(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let stems = [Stem::Bass, Stem::Pad, Stem::Arp, Stem::Lead, Stem::Drums, Stem::Static]
        .into_iter()
        .map(|s| {
            let bytes = to_wav(&build(s));
            (s, sources.add(AudioSource { bytes: bytes.into() }))
        })
        .collect();
    commands.insert_resource(MusicBank { stems });
}

pub fn start_music(mut commands: Commands, bank: Res<MusicBank>) {
    for (stem, handle) in &bank.stems {
        commands.spawn((
            AudioPlayer(handle.clone()),
            PlaybackSettings {
                mode: PlaybackMode::Loop,
                volume: Volume::Linear(0.0),
                ..default()
            },
            MusicStem { stem: *stem, cur: 0.0 },
        ));
    }
}

pub fn stop_music(mut commands: Commands, q: Query<Entity, With<MusicStem>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

pub fn update_music(
    time: Res<Time<Real>>,
    save: Res<MetaSave>,
    run: Option<Res<RunState>>,
    phase: Res<RunPhase>,
    // the horde, not the scenery: pots share `Enemy` but never chase anyone (L16)
    enemies: Query<(), (With<Enemy>, Without<crate::interact::Pot>, Without<crate::enemies::Boss>)>,
    mut q: Query<(&mut AudioSink, &mut MusicStem)>,
) {
    let dt = time.delta_secs();
    let count = enemies.iter().count();
    let density = (count as f32 / 260.0).clamp(0.0, 1.0);
    let static_on = run.as_ref().map(|r| r.static_active).unwrap_or(false);
    let paused = !matches!(*phase, RunPhase::Playing | RunPhase::Dead);
    let master = save.volume * save.music_volume * MUSIC_MIX;

    for (mut sink, mut ms) in &mut q {
        let mut target = match ms.stem {
            Stem::Bass => 0.85,
            Stem::Pad => 0.65,
            Stem::Arp => 0.2 + 0.6 * density,
            Stem::Lead => 0.7,
            Stem::Drums => 0.5 + 0.5 * density, // always a beat, harder as it heats up
            Stem::Static => {
                if static_on {
                    1.0
                } else {
                    0.0
                }
            }
        };
        if static_on && ms.stem != Stem::Static {
            target *= 0.5;
        }
        if paused {
            target *= 0.35;
        }
        target *= master;

        let k = 1.0 - (-2.5 * dt).exp();
        ms.cur += (target - ms.cur) * k;
        sink.set_volume(Volume::Linear(ms.cur));
        sink.set_speed(if static_on { 0.92 } else { 1.0 });
    }
}
