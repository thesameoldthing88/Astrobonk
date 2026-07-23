//! Pooled floating damage numbers, projected from world space onto the UI.

use crate::config::DAMAGE_NUMBER_POOL;
use crate::messages::{NumKind, NumberMsg};
use crate::player::PlayerRig;
use bevy::prelude::*;

#[derive(Component)]
pub struct DamageNumber {
    pub active: bool,
    pub world: Vec3,
    pub life: f32,
    pub max_life: f32,
}

#[derive(Resource, Default)]
pub struct NumberCursor(pub usize);

pub fn spawn_number_pool(mut commands: Commands) {
    for _ in 0..DAMAGE_NUMBER_POOL {
        commands.spawn((
            DamageNumber { active: false, world: Vec3::ZERO, life: 0.0, max_life: 0.7 },
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
        cursor.0 = (cursor.0 + 1) % n;
        if let Some((mut dn, mut text, mut font, mut color)) = q.iter_mut().nth(cursor.0) {
            dn.active = true;
            dn.world = msg.pos;
            dn.life = 0.7;
            dn.max_life = 0.7;
            match msg.kind {
                NumKind::Hit => {
                    text.0 = format!("{:.0}", msg.amount.max(1.0));
                    font.font_size = 17.0;
                    color.0 = Color::WHITE;
                }
                NumKind::Crit => {
                    text.0 = format!("{:.0}!", msg.amount.max(1.0));
                    font.font_size = 24.0;
                    color.0 = Color::srgb(1.0, 0.85, 0.2);
                }
                NumKind::Heal => {
                    text.0 = format!("+{:.0}", msg.amount);
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
