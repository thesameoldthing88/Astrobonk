//! First-run onboarding. Teaches movement, auto-weapons, the level-up, day/night, and the
//! boss tell through diegetic Mission-Control radio lines triggered by what the player is
//! actually doing — no wall of text, no modal. Only fires on a player's very first run.

use crate::enemies::Enemy;
use crate::messages::{Sfx, SfxMsg};
use crate::run::{PlayerState, RunState};
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct Tutorial {
    pub active: bool,
    pub step: usize,
    pub timer: f32, // seconds the current line stays up
}

const HOLD: f32 = 5.0;

/// Each line + the condition (by index) that lets it fire, checked in order.
const LINES: [&str; 6] = [
    "Welcome to the rock. WASD to move, mouse to look. Walk it off.",
    "Your weapon fires itself. Your only job is not getting cornered.",
    "Green gems are XP. Fill the bar, pick a card. Trust your gut.",
    "Shift to slide. Jump as you land to keep the speed — you'll need it.",
    "It gets dark on the far side. Bring a light. F works the one on your gun.",
    "Big one inbound. Kite it, don't trade. It'll drop a chest.",
];

#[derive(Component)]
pub struct TutorialText;

pub fn tutorial_system(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut tut: ResMut<Tutorial>,
    run: Res<RunState>,
    q_ps: Query<&PlayerState, With<crate::player::LocalPlayer>>,
    q_enemies: Query<(), With<Enemy>>,
    mut q_text: Query<(&mut Text, &mut TextColor), With<TutorialText>>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    // DEV (`--dev` only): press T to replay the first-run tutorial (veterans have it marked
    // done).
    if crate::dev_mode() && keys.just_pressed(KeyCode::KeyT) {
        *tut = Tutorial { active: true, step: 0, timer: 0.0 };
    }
    let Ok((mut text, mut color)) = q_text.single_mut() else { return };
    if !tut.active {
        if !text.0.is_empty() {
            text.0.clear();
        }
        return;
    }
    let dt = time.delta_secs();

    // A line is currently on screen: hold it, then clear.
    if tut.timer > 0.0 {
        tut.timer -= dt;
        let a = (tut.timer / 0.6).min(1.0).min(((HOLD - tut.timer) / 0.3).max(0.0));
        color.0 = Color::srgb(0.55, 0.95, 1.0).with_alpha(a.clamp(0.0, 1.0));
        if tut.timer <= 0.0 {
            text.0.clear();
        }
        return;
    }

    if tut.step >= LINES.len() {
        tut.active = false;
        return;
    }

    let alive = q_enemies.iter().count();
    let ready = match tut.step {
        0 => run.elapsed > 1.2,
        1 => run.elapsed > 6.0 || alive > 0,
        2 => run.elapsed > 14.0,
        3 => q_ps.single().map(|p| p.level >= 2).unwrap_or(false),
        4 => run.elapsed > 42.0,
        5 => run.minibosses_spawned[0],
        _ => false,
    };
    if ready {
        text.0 = format!("\u{260E} MISSION CONTROL:  {}", LINES[tut.step]);
        color.0 = Color::srgb(0.55, 0.95, 1.0).with_alpha(0.0);
        tut.timer = HOLD;
        tut.step += 1;
        sfx.write(SfxMsg(Sfx::Click));
    }
}
