//! Pooled floating damage numbers, projected from world space onto the UI.
//! Nearby rapid hits MERGE into a running sum so swarm fights stay readable
//! instead of becoming number confetti.

use crate::config::DAMAGE_NUMBER_POOL;
use crate::messages::{NumKind, NumberMsg};
use crate::player::PlayerRig;
use bevy::prelude::*;

const MERGE_RADIUS: f32 = 1.6; // world meters — hits this close coalesce
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

fn style_number(dn: &DamageNumber, text: &mut Text, font: &mut TextFont, color: &mut TextColor) {
    match dn.kind {
        NumKind::Hit | NumKind::Crit => {
            let v = dn.value.max(1.0);
            if dn.crit {
                text.0 = format!("{v:.0}!");
                // merged sums grow a little so big trades read bigger
                font.font_size = (24.0 + (v / 120.0).min(8.0)).min(34.0);
                color.0 = Color::srgb(1.0, 0.85, 0.2);
            } else {
                text.0 = format!("{v:.0}");
                font.font_size = (17.0 + (v / 150.0).min(6.0)).min(28.0);
                color.0 = Color::WHITE;
            }
        }
        NumKind::Heal => {
            text.0 = format!("+{:.0}", dn.value);
            font.font_size = 18.0;
            color.0 = Color::srgb(0.35, 1.0, 0.45);
        }
        NumKind::Dodge => {
            text.0 = "DODGE".into();
            font.font_size = 16.0;
            color.0 = Color::srgb(0.4, 0.95, 1.0);
        }
    }
}

pub fn claim_numbers(
    mut reader: MessageReader<NumberMsg>,
    mut cursor: ResMut<NumberCursor>,
    mut q: Query<(&mut DamageNumber, &mut Text, &mut TextFont, &mut TextColor)>,
) {
    let n = q.iter().count();
    if n == 0 {
        return;
    }
    for msg in reader.read() {
        // Damage merging: fold this hit into a live nearby number of the same family.
        if matches!(msg.kind, NumKind::Hit | NumKind::Crit) {
            let mut merged = false;
            for (mut dn, mut text, mut font, mut color) in q.iter_mut() {
                if dn.active
                    && matches!(dn.kind, NumKind::Hit | NumKind::Crit)
                    && dn.world.distance_squared(msg.pos) < MERGE_RADIUS * MERGE_RADIUS
                {
                    dn.value += msg.amount;
                    dn.crit |= msg.kind == NumKind::Crit;
                    if dn.crit {
                        dn.kind = NumKind::Crit;
                    }
                    dn.world = (dn.world + msg.pos) / 2.0;
                    dn.life = dn.life.max(NUM_LIFE * 0.6); // keep it alive while feeding
                    style_number(&dn, &mut text, &mut font, &mut color);
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
            style_number(&dn, &mut text, &mut font, &mut color);
        }
    }
}

pub fn update_numbers(
    time: Res<Time<Real>>,
    camera: Query<(&Camera, &GlobalTransform), With<PlayerRig>>,
    mut q: Query<(&mut DamageNumber, &mut Node, &mut TextColor)>,
) {
    let Ok((cam, cam_tf)) = camera.single() else { return };
    let dt = time.delta_secs();
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
                node.left = Val::Px(screen.x - 12.0);
                node.top = Val::Px(screen.y - 12.0);
                let a = (dn.life / 0.25).clamp(0.0, 1.0);
                color.0 = color.0.with_alpha(a);
            }
            Err(_) => {
                node.left = Val::Px(-1000.0);
            }
        }
    }
}
