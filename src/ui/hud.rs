//! In-run HUD: timer, currencies, XP/HP bars, weapon & item trays, boss bar,
//! banners, interact prompt, powerup timers, hurt vignette.

use super::*;
use crate::content::weapons::WeaponKind;
use crate::enemies::{Boss, Enemy};
use crate::interact::InteractPrompt;
use crate::messages::BannerMsg;
use crate::run::{PlayerState, RunState, xp_needed};
use bevy::prelude::*;

#[derive(Component)]
pub struct HudRoot;
#[derive(Component)]
pub struct TimerText;
#[derive(Component)]
pub struct KillsText;
#[derive(Component)]
pub struct GoldText;
#[derive(Component)]
pub struct SilverText;
#[derive(Component)]
pub struct LevelText;
#[derive(Component)]
pub struct XpFill;
#[derive(Component)]
pub struct HpFill;
#[derive(Component)]
pub struct HpText;
#[derive(Component)]
pub struct WeaponRow;
#[derive(Component)]
pub struct BannerText;
#[derive(Component)]
pub struct PromptText;
#[derive(Component)]
pub struct BossBarWrap;
#[derive(Component)]
pub struct BossBarFill;
#[derive(Component)]
pub struct BossBarName;
#[derive(Component)]
pub struct PowerupText;
#[derive(Component)]
pub struct Vignette;
#[derive(Component)]
pub struct CometText;
#[derive(Component)]
pub struct DustOverlay;
/// One slot in the off-screen indicator pool (colored squares hugging the screen edge).
#[derive(Component)]
pub struct EdgeMarker;

#[derive(Resource, Default)]
pub struct BannerQueue {
    pub current: Option<(String, f32)>,
    pub queue: Vec<String>,
}

pub fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            // hurt vignette
            root.spawn((
                Vignette,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 0.1, 0.1, 0.0)),
                Pickable::IGNORE,
            ));
            // off-screen indicator pool: colored squares that hug the screen edge,
            // pointing at bosses / chests / shrines / the teleporter over the horizon
            for _ in 0..24 {
                root.spawn((
                    EdgeMarker,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(-100.0),
                        top: Val::Px(-100.0),
                        width: Val::Px(12.0),
                        height: Val::Px(12.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    BorderColor::all(Color::NONE),
                    Pickable::IGNORE,
                ));
            }

            // dust-storm haze (Mars) — fades in while you're inside the storm
            root.spawn((
                DustOverlay,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.72, 0.48, 0.30, 0.0)),
                Pickable::IGNORE,
            ));

            // timer, top center
            root.spawn((Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },))
                .with_children(|c| {
                    c.spawn((TimerText, txt("10:00", 44.0, Color::WHITE)));
                });

            // comet combo readout, just under the timer
            root.spawn((Node {
                position_type: PositionType::Absolute,
                top: Val::Px(52.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },))
                .with_children(|c| {
                    c.spawn((CometText, txt("", FONT_MED, Color::srgb(1.0, 0.8, 0.3))));
                });

            // boss bar under the timer
            root.spawn((
                BossBarWrap,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(64.0),
                    left: Val::Percent(25.0),
                    width: Val::Percent(50.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
                Visibility::Hidden,
            ))
                .with_children(|c| {
                    c.spawn((BossBarName, txt("BOSS", FONT_SMALL, Color::srgb(1.0, 0.5, 0.5))));
                    c.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(14.0),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                        BorderColor::all(Color::srgb(0.6, 0.2, 0.2)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            BossBarFill,
                            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                            BackgroundColor(Color::srgb(0.9, 0.2, 0.25)),
                        ));
                    });
                });

            // counters, top right
            root.spawn((Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                right: Val::Px(16.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(2.0),
                ..default()
            },))
                .with_children(|c| {
                    c.spawn((KillsText, txt("BONKS 0", FONT_MED, Color::srgb(0.9, 0.9, 1.0))));
                    c.spawn((GoldText, txt("GOLD 0", FONT_MED, Color::srgb(1.0, 0.85, 0.3))));
                    c.spawn((SilverText, txt("SILVER +0", FONT_MED, Color::srgb(0.75, 0.85, 1.0))));
                    c.spawn((PowerupText, txt("", FONT_SMALL, Color::srgb(0.85, 0.5, 1.0))));
                });

            // banner center
            root.spawn((Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(22.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },))
                .with_children(|c| {
                    c.spawn((BannerText, txt("", 30.0, Color::srgb(1.0, 0.9, 0.4))));
                });

            // interact prompt
            // first-run Mission-Control tutorial line
            root.spawn((Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(210.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },))
                .with_children(|c| {
                    c.spawn((crate::tutorial::TutorialText, txt("", FONT_MED, Color::srgb(0.55, 0.95, 1.0))));
                });

            root.spawn((Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(150.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },))
                .with_children(|c| {
                    c.spawn((PromptText, txt("", FONT_MED, Color::srgb(0.6, 1.0, 0.8))));
                });

            // bottom cluster: hp bar, xp bar, weapon row
            root.spawn((Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(12.0),
                left: Val::Percent(18.0),
                width: Val::Percent(64.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },))
                .with_children(|c| {
                    // weapons tray
                    c.spawn((
                        WeaponRow,
                        Node {
                            column_gap: Val::Px(6.0),
                            align_items: AlignItems::Center,
                            height: Val::Px(34.0),
                            ..default()
                        },
                    ));
                    // hp
                    c.spawn((Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(16.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                        BorderColor::all(Color::srgb(0.3, 0.1, 0.1)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            HpFill,
                            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                            BackgroundColor(Color::srgb(0.85, 0.25, 0.3)),
                        ));
                        bar.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Percent(100.0),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                        ))
                        .with_children(|t| {
                            t.spawn((HpText, txt("100/100", 13.0, Color::WHITE)));
                        });
                    });
                    // xp
                    c.spawn((Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(12.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                        BorderColor::all(Color::srgb(0.1, 0.3, 0.15)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            XpFill,
                            Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
                            BackgroundColor(Color::srgb(0.3, 0.95, 0.45)),
                        ));
                    });
                    c.spawn((Node { justify_content: JustifyContent::Center, ..default() },))
                        .with_children(|t| {
                            t.spawn((LevelText, txt("LV 1", FONT_MED, Color::srgb(0.5, 1.0, 0.6))));
                        });
                });
        });
}

pub fn despawn_hud(mut commands: Commands, q: Query<Entity, With<HudRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

#[allow(clippy::type_complexity)]
pub fn update_hud(
    run: Res<RunState>,
    q_ps: Query<&PlayerState, With<crate::player::LocalPlayer>>,
    prompt: Res<InteractPrompt>,
    mut sets: ParamSet<(
        Query<&mut Text, With<TimerText>>,
        Query<&mut Text, With<KillsText>>,
        Query<&mut Text, With<GoldText>>,
        Query<&mut Text, With<SilverText>>,
        Query<&mut Text, With<LevelText>>,
        Query<&mut Text, With<HpText>>,
        Query<&mut Text, With<PromptText>>,
        Query<&mut Text, With<PowerupText>>,
    )>,
    mut fills: ParamSet<(
        Query<&mut Node, With<XpFill>>,
        Query<&mut Node, With<HpFill>>,
    )>,
    mut vignette: Query<&mut BackgroundColor, With<Vignette>>,
) {
    let Ok(ps) = q_ps.single() else { return };
    if let Ok(mut t) = sets.p0().single_mut() {
        if run.static_active {
            let m = (run.static_timer / 60.0) as u32;
            let s = (run.static_timer % 60.0) as u32;
            t.0 = format!("THE STATIC {m}:{s:02}");
        } else {
            let m = (run.timer / 60.0) as u32;
            let s = (run.timer % 60.0) as u32;
            t.0 = format!("{m}:{s:02}");
        }
    }
    if let Ok(mut t) = sets.p1().single_mut() {
        t.0 = format!("BONKS {}", run.kills);
    }
    if let Ok(mut t) = sets.p2().single_mut() {
        t.0 = format!("GOLD {}", ps.gold);
    }
    if let Ok(mut t) = sets.p3().single_mut() {
        t.0 = format!("SILVER +{}", run.silver_run);
    }
    if let Ok(mut t) = sets.p4().single_mut() {
        t.0 = format!("LV {}", ps.level);
    }
    if let Ok(mut t) = sets.p5().single_mut() {
        t.0 = format!("{:.0}/{:.0}", ps.hp.max(0.0), ps.stats.max_hp);
    }
    if let Ok(mut t) = sets.p6().single_mut() {
        t.0 = prompt.0.clone().unwrap_or_default();
    }
    if let Ok(mut t) = sets.p7().single_mut() {
        let s: Vec<String> = ps
            .powerups
            .iter()
            .map(|(k, secs)| format!("{:?} {:.0}s", k, secs))
            .collect();
        t.0 = s.join("  ");
    }
    if let Ok(mut n) = fills.p0().single_mut() {
        n.width = Val::Percent((ps.xp / xp_needed(ps.level).max(0.001) * 100.0).clamp(0.0, 100.0));
    }
    if let Ok(mut n) = fills.p1().single_mut() {
        n.width = Val::Percent((ps.hp / ps.stats.max_hp * 100.0).clamp(0.0, 100.0));
    }
    if let Ok(mut bg) = vignette.single_mut() {
        bg.0 = Color::srgba(1.0, 0.1, 0.1, (ps.iframes * 0.55).clamp(0.0, 0.4));
    }
}

/// Rebuild the weapon tray when loadout changes.
pub fn update_weapon_row(
    mut commands: Commands,
    q_ps: Query<&PlayerState, With<crate::player::LocalPlayer>>,
    mut cache: Local<Vec<(WeaponKind, u32)>>,
    q_row: Query<Entity, With<WeaponRow>>,
) {
    let Ok(run) = q_ps.single() else { return };
    let current: Vec<(WeaponKind, u32)> = run.weapons.iter().map(|w| (w.kind, w.level)).collect();
    if *cache == current {
        return;
    }
    *cache = current.clone();
    let Ok(row) = q_row.single() else { return };
    commands.entity(row).despawn_related::<Children>();
    commands.entity(row).with_children(|c| {
        for (kind, level) in current {
            let def = kind.def();
            c.spawn((
                Node {
                    width: Val::Px(60.0),
                    height: Val::Px(30.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.8)),
                BorderColor::all(def.color),
            ))
            .with_children(|slot| {
                let tag: String = def.name.chars().take(3).collect();
                slot.spawn(txt(format!("{} {}", tag.to_uppercase(), level), 13.0, def.color));
            });
        }
        // item chips
        for (item, count) in run.items.iter() {
            let d = item.def();
            c.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.7)),
                BorderColor::all(d.rarity.color()),
            ))
            .with_children(|chip| {
                let tag: String = d.name.chars().take(2).collect();
                chip.spawn(txt(format!("{tag}{count}"), 11.0, d.rarity.color()));
            });
        }
    });
}

/// Point edge markers at important things beyond the screen/horizon: bosses (red),
/// the teleporter (green), chests (gold), the Shady Guy (purple), charge shrines
/// (cyan), the cage (brown). On a sphere most objectives are below the horizon —
/// this is how you navigate to them.
#[allow(clippy::type_complexity)]
pub fn update_edge_markers(
    camera: Query<(&Camera, &GlobalTransform), With<crate::player::PlayerRig>>,
    q_boss: Query<&Transform, With<crate::enemies::Boss>>,
    q_inter: Query<(&Transform, &crate::interact::Interactable)>,
    q_charge: Query<(&Transform, &crate::interact::ChargeShrine)>,
    mut markers: Query<(&mut Node, &mut BackgroundColor, &mut BorderColor), With<EdgeMarker>>,
) {
    use crate::interact::InteractKind;
    let Ok((cam, cam_tf)) = camera.single() else { return };
    let Some(size) = cam.logical_viewport_size() else { return };
    let center = size / 2.0;
    let margin = 34.0;

    // collect targets: (world pos, color, marker px size), priority order
    let mut targets: Vec<(Vec3, Color, f32)> = Vec::new();
    for tf in &q_boss {
        targets.push((tf.translation, Color::srgb(1.0, 0.25, 0.2), 16.0));
    }
    for (tf, inter) in &q_inter {
        if inter.used {
            continue;
        }
        let (color, px) = match inter.kind {
            InteractKind::Teleporter => (Color::srgb(0.3, 1.0, 0.8), 16.0),
            InteractKind::Chest => (Color::srgb(1.0, 0.8, 0.25), 11.0),
            InteractKind::ShadyGuy => (Color::srgb(0.75, 0.5, 1.0), 11.0),
            InteractKind::Cage => (Color::srgb(0.75, 0.55, 0.35), 12.0),
            _ => continue, // shrines/moai/microwave handled below or skipped to limit noise
        };
        targets.push((tf.translation, color, px));
    }
    for (tf, shrine) in &q_charge {
        if !shrine.done {
            targets.push((tf.translation, Color::srgb(0.4, 1.0, 0.95), 11.0));
        }
    }

    let inv = cam_tf.affine().inverse();
    let mut it = markers.iter_mut();
    for (world, color, px) in targets.into_iter().take(24) {
        let Some((mut node, mut bg, mut border)) = it.next() else { break };

        // on-screen and in front? then no marker needed
        let mut visible_on_screen = false;
        if let Ok(v) = cam.world_to_viewport(cam_tf, world) {
            if v.x >= 0.0 && v.y >= 0.0 && v.x <= size.x && v.y <= size.y {
                visible_on_screen = true;
            }
        }
        if visible_on_screen {
            node.left = Val::Px(-100.0);
            node.top = Val::Px(-100.0);
            continue;
        }

        // view-space direction → screen-edge position
        let local = inv.transform_point3(world);
        let mut d = Vec2::new(local.x, -local.y); // screen: +x right, +y down
        if local.z > 0.0 {
            d = -d; // target is behind the camera — flip so the arrow still points at it
        }
        if d.length_squared() < 1e-6 {
            d = Vec2::Y;
        }
        let d = d.normalize();
        let half = center - Vec2::splat(margin);
        let scale_x = if d.x.abs() > 1e-4 { half.x / d.x.abs() } else { f32::MAX };
        let scale_y = if d.y.abs() > 1e-4 { half.y / d.y.abs() } else { f32::MAX };
        let pos = center + d * scale_x.min(scale_y);

        node.left = Val::Px(pos.x - px / 2.0);
        node.top = Val::Px(pos.y - px / 2.0);
        node.width = Val::Px(px);
        node.height = Val::Px(px);
        bg.0 = color.with_alpha(0.85);
        *border = BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.6));
    }
    // park the unused markers
    for (mut node, mut bg, _) in it {
        node.left = Val::Px(-100.0);
        node.top = Val::Px(-100.0);
        bg.0 = Color::NONE;
    }
}

/// Fade the dust haze in/out based on whether the player is inside the Mars storm.
pub fn update_dust_overlay(
    time: Res<Time<Real>>,
    storm: Res<crate::events_world::DustStorm>,
    mut q: Query<&mut BackgroundColor, With<DustOverlay>>,
    mut cur: Local<f32>,
) {
    let target = if storm.player_inside { 0.4 } else { 0.0 };
    let k = 1.0 - (-4.0 * time.delta_secs()).exp();
    *cur += (target - *cur) * k;
    if let Ok(mut bg) = q.single_mut() {
        bg.0 = Color::srgba(0.72, 0.48, 0.30, *cur);
    }
}

/// Comet combo indicator: a growing tail counter + a charge meter drawn in text bars.
pub fn update_comet_hud(
    comet: Res<crate::comet::Comet>,
    mut q: Query<(&mut Text, &mut TextColor), With<CometText>>,
) {
    let Ok((mut text, mut color)) = q.single_mut() else { return };
    if comet.flash > 0.0 {
        text.0 = "\u{2604} COMET!".into();
        color.0 = Color::srgb(1.0, 0.9, 0.4);
    } else if comet.active {
        let filled = (comet.progress * 12.0).round() as usize;
        let bar: String = "\u{2588}".repeat(filled) + &"\u{2591}".repeat(12 - filled);
        text.0 = format!("\u{2604} x{}  {bar}", comet.count);
        // warm up from amber to white-hot as the charge fills
        let t = comet.progress;
        color.0 = Color::srgb(1.0, 0.8 + 0.2 * t, 0.3 + 0.5 * t);
    } else {
        text.0 = String::new();
    }
}

pub fn update_boss_bar(
    q_boss: Query<(&Enemy, &Boss)>,
    mut wrap: Query<&mut Visibility, With<BossBarWrap>>,
    mut fill: Query<&mut Node, With<BossBarFill>>,
    mut name: Query<&mut Text, With<BossBarName>>,
) {
    let Ok(mut vis) = wrap.single_mut() else { return };
    // show the beefiest live boss
    let mut best: Option<(f32, f32, &'static str)> = None;
    for (e, b) in q_boss.iter() {
        let d = b.kind.def();
        if best.map(|(_, m, _)| e.max_hp > m).unwrap_or(true) {
            best = Some((e.hp, e.max_hp, d.name));
        }
    }
    match best {
        Some((hp, max, n)) => {
            *vis = Visibility::Visible;
            if let Ok(mut f) = fill.single_mut() {
                f.width = Val::Percent((hp / max * 100.0).clamp(0.0, 100.0));
            }
            if let Ok(mut t) = name.single_mut() {
                t.0 = n.to_string();
            }
        }
        None => {
            *vis = Visibility::Hidden;
        }
    }
}

/// Banner queue: one line at a time, 2.6 s each.
pub fn update_banners(
    time: Res<Time<Real>>,
    mut queue: ResMut<BannerQueue>,
    mut reader: MessageReader<BannerMsg>,
    mut q: Query<(&mut Text, &mut TextColor), With<BannerText>>,
) {
    for msg in reader.read() {
        queue.queue.push(msg.0.clone());
    }
    let dt = time.delta_secs();
    if let Some((_, t)) = queue.current.as_mut() {
        *t -= dt;
        if *t <= 0.0 {
            queue.current = None;
        }
    }
    if queue.current.is_none() && !queue.queue.is_empty() {
        queue.current = Some((queue.queue.remove(0), 2.6));
    }
    if let Ok((mut text, mut color)) = q.single_mut() {
        match &queue.current {
            Some((s, t)) => {
                text.0 = s.clone();
                color.0 = Color::srgba(1.0, 0.9, 0.4, (*t / 0.5).clamp(0.0, 1.0));
            }
            None => {
                text.0 = String::new();
            }
        }
    }
}
