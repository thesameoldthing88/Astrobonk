//! Pooled floating damage numbers, projected from world space onto the UI.
//! Nearby rapid hits MERGE into a running sum so swarm fights stay readable
//! instead of becoming number confetti. How much merges, which numbers show at all and how
//! big they are is the player's call (§13: Full / Merged-only / Crits-only / Off + size).

use crate::config::*;
use crate::messages::{NumKind, NumberMsg};
use crate::player::PlayerRig;
use crate::save::{MetaSave, NumberMode};
use bevy::prelude::*;

const NUM_LIFE: f32 = 0.7;

#[derive(Component)]
pub struct DamageNumber {
    pub active: bool,
    pub world: Vec3,
    pub life: f32,
    pub max_life: f32,
    pub value: f32,
    pub crit: bool,
    pub kind: NumKind,
}

#[derive(Resource, Default)]
pub struct NumberCursor(pub usize);

pub fn spawn_number_pool(mut commands: Commands) {
    for _ in 0..DAMAGE_NUMBER_POOL {
        commands.spawn((
            DamageNumber {
                active: false,
                world: Vec3::ZERO,
                life: 0.0,
                max_life: NUM_LIFE,
                value: 0.0,
                crit: false,
                kind: NumKind::Hit,
            },
            Text::new(""),
            TextFont { font_size: 18.0, ..default() },
            TextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-1000.0),
                top: Val::Px(-1000.0),
                ..default()
            },
            Pickable::IGNORE,
        ));
    }
    commands.init_resource::<NumberCursor>();
}

/// `size` is the player's damage-number size setting. Crits are shape-coded as well as
/// gold ("123!", bigger) so they read without color.
fn style_number(dn: &DamageNumber, size: f32, text: &mut Text, font: &mut TextFont, color: &mut TextColor) {
    match dn.kind {
        NumKind::Hit | NumKind::Crit => {
            let v = dn.value.max(1.0);
            if dn.crit {
                text.0 = format!("{v:.0}!");
                // merged sums grow a little so big trades read bigger
                font.font_size = (24.0 + (v / 120.0).min(8.0)).min(34.0) * size;
                color.0 = Color::srgb(1.0, 0.85, 0.2);
            } else {
                text.0 = format!("{v:.0}");
                font.font_size = (17.0 + (v / 150.0).min(6.0)).min(28.0) * size;
                color.0 = Color::WHITE;
            }
        }
        NumKind::Heal => {
            text.0 = format!("+{:.0}", dn.value);
            font.font_size = 18.0 * size;
            color.0 = Color::srgb(0.35, 1.0, 0.45);
        }
        NumKind::Dodge => {
            text.0 = "DODGE".into();
            font.font_size = 16.0 * size;
            color.0 = Color::srgb(0.4, 0.95, 1.0);
        }
        NumKind::Block => {
            text.0 = "BLOCK".into();
            font.font_size = 15.0 * size;
            color.0 = Color::srgb(0.7, 0.8, 0.95);
        }
        NumKind::Immune => {
            text.0 = "IMMUNE".into();
            font.font_size = 15.0 * size;
            color.0 = Color::srgb(0.45, 0.55, 1.0);
        }
    }
}

pub fn claim_numbers(
    mut reader: MessageReader<NumberMsg>,
    save: Res<MetaSave>,
    mut cursor: ResMut<NumberCursor>,
    mut q: Query<(&mut DamageNumber, &mut Text, &mut TextFont, &mut TextColor)>,
) {
    let n = q.iter().count();
    if n == 0 {
        return;
    }
    let mode = save.accessibility.numbers;
    let size = save.accessibility.number_size;
    // Full mode merges wide only "at high counts" (§13): once most of the pool is live.
    let crowded = q.iter().filter(|(dn, ..)| dn.active).count() as f32 >= n as f32 * NUMBER_CROWDED_FRACTION;
    for msg in reader.read() {
        let damage = matches!(msg.kind, NumKind::Hit | NumKind::Crit);
        let shown = match mode {
            NumberMode::Off => false,
            NumberMode::CritsOnly => !damage || msg.kind == NumKind::Crit,
            NumberMode::Full | NumberMode::Merged => true,
        };
        if !shown {
            continue;
        }
        // Damage merging: fold this hit into a live nearby number of the same family —
        // any live one in reach for Merged/Crits-only (and a crowded Full screen), only a
        // fresh one within 0.3 m / 0.1 s otherwise.
        if damage {
            let (radius, window) = if mode == NumberMode::Full && !crowded {
                (NUMBER_MERGE_RADIUS_FULL, NUMBER_MERGE_SECS_FULL)
            } else {
                (NUMBER_MERGE_RADIUS, f32::MAX)
            };
            let mut merged = false;
            for (mut dn, mut text, mut font, mut color) in q.iter_mut() {
                if dn.active
                    && matches!(dn.kind, NumKind::Hit | NumKind::Crit)
                    && dn.max_life - dn.life <= window
                    && dn.world.distance_squared(msg.pos) < radius * radius
                {
                    dn.value += msg.amount;
                    dn.crit |= msg.kind == NumKind::Crit;
                    if dn.crit {
                        dn.kind = NumKind::Crit;
                    }
                    dn.world = (dn.world + msg.pos) / 2.0;
                    dn.life = dn.life.max(NUM_LIFE * 0.6); // keep it alive while feeding
                    style_number(&dn, size, &mut text, &mut font, &mut color);
                    merged = true;
                    break;
                }
            }
            if merged {
                continue;
            }
        }

        cursor.0 = (cursor.0 + 1) % n;
        if let Some((mut dn, mut text, mut font, mut color)) = q.iter_mut().nth(cursor.0) {
            dn.active = true;
            dn.world = msg.pos;
            dn.life = NUM_LIFE;
            dn.max_life = NUM_LIFE;
            dn.value = msg.amount;
            dn.crit = msg.kind == NumKind::Crit;
            dn.kind = msg.kind;
            style_number(&dn, size, &mut text, &mut font, &mut color);
        }
    }
}

pub fn update_numbers(
    time: Res<Time<Real>>,
    ui_scale: Res<UiScale>,
    camera: Query<(&Camera, &GlobalTransform), With<PlayerRig>>,
    mut q: Query<(&mut DamageNumber, &mut Node, &mut TextColor)>,
) {
    let Ok((cam, cam_tf)) = camera.single() else { return };
    let dt = time.delta_secs();
    // The projection is in logical pixels; UiScale multiplies every Val::Px, so undo it or
    // numbers drift away from their enemies at any scale but 100%.
    let px = 1.0 / ui_scale.0.max(0.01);
    for (mut dn, mut node, mut color) in &mut q {
        if !dn.active {
            continue;
        }
        dn.life -= dt;
        if dn.life <= 0.0 {
            dn.active = false;
            node.left = Val::Px(-1000.0);
            node.top = Val::Px(-1000.0);
            continue;
        }
        let up = dn.world.normalize_or_zero();
        let rise = (1.0 - dn.life / dn.max_life) * 1.4;
        match cam.world_to_viewport(cam_tf, dn.world + up * (0.8 + rise)) {
            Ok(screen) => {
                node.left = Val::Px(screen.x * px - 12.0);
                node.top = Val::Px(screen.y * px - 12.0);
                let a = (dn.life / 0.25).clamp(0.0, 1.0);
                color.0 = color.0.with_alpha(a);
            }
            Err(_) => {
                node.left = Val::Px(-1000.0);
            }
        }
    }
}
