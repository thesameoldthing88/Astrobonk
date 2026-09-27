//! The co-op HUD (GDD §11, P18): the squad strip (every teammate's suit, level, HP — or
//! their Beacon's meters), the "you're down" panel, markers pointing at teammates' Beacons
//! over the horizon, the revive you are giving, STATIC CASCADE's link, and a drop-in's
//! autopilot grace. All of it is read from `net::PlayerVitals` (the host writes its own
//! astronauts'; a joiner reads the replicated ones), so host and joiner draw one path.
//! Plain ASCII only: the game font has the 95 printable glyphs and nothing else.

use super::*;
use crate::config::*;
use crate::content::characters::AstronautKind;
use crate::net::{NetHero, NetRole, PlayerVitals, VITALS_CLAIMED};
use crate::player::{LocalPlayer, PlayerId};
use crate::run::{PlayerState, RunState};

#[derive(Component)]
pub struct CoopHudRoot;
/// One row of the squad strip, top-left.
#[derive(Component)]
pub struct SquadRow(usize);
#[derive(Component)]
pub struct CascadeLine;
/// Drop-in grace, Hero's Adrenaline, a friendly chill: what is happening to US.
#[derive(Component)]
pub struct StatusLine;
#[derive(Component)]
pub struct DownPanel;
#[derive(Component)]
pub struct DownTitle;
#[derive(Component)]
pub struct DownHint;
#[derive(Component)]
pub struct StaticFill;
#[derive(Component)]
pub struct ReviveFill;
/// The screen souring toward The Static while we are down.
#[derive(Component)]
pub struct DownTint;
/// "REVIVING P2 [#####-----]" while we stand in a teammate's ring.
#[derive(Component)]
pub struct RescueLine;
/// A marker at the screen's edge for a teammate's Beacon beyond it.
#[derive(Component)]
pub struct BeaconMarker(usize);

const MAX_MATES: usize = crate::net::MAX_PLAYERS - 1;
const SQUAD_X: f32 = 14.0;
const SQUAD_Y: f32 = 14.0;
const BEACON_AMBER: Color = Color::srgb(1.0, 0.62, 0.15);
const STATIC_VIOLET: Color = Color::srgb(0.66, 0.42, 1.0);

pub fn spawn_coop_hud(mut commands: Commands) {
    commands
        .spawn((
            CoopHudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            root.spawn((
                DownTint,
                Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                BackgroundColor(Color::NONE),
                Pickable::IGNORE,
            ));
            // squad strip, cascade link and our own status, top-left
            root.spawn(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(SQUAD_X),
                top: Val::Px(SQUAD_Y),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|c| {
                for i in 0..MAX_MATES {
                    c.spawn((SquadRow(i), txt("", FONT_SMALL, Color::WHITE), Visibility::Hidden));
                }
                c.spawn((CascadeLine, txt("", FONT_SMALL, Color::srgb(0.6, 0.85, 1.0))));
                c.spawn((StatusLine, txt("", FONT_SMALL, Color::srgb(0.6, 1.0, 0.75))));
            });
            // the down panel, upper middle — clear of the banner line (124 px) and of the
            // card band a level-up draws lower down
            root.spawn((
                DownPanel,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(170.0),
                    left: Val::Percent(30.0),
                    width: Val::Percent(40.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.02, 0.07, 0.6)),
                Visibility::Hidden,
                Pickable::IGNORE,
            ))
            .with_children(|p| {
                p.spawn((DownTitle, txt("YOU'RE DOWN", FONT_BIG, BEACON_AMBER)));
                p.spawn((DownHint, txt("", FONT_SMALL, Color::srgb(0.85, 0.85, 0.95)), TextLayout::new_with_justify(Justify::Center)));
                for (label, fill, color) in [("THE STATIC", 0, STATIC_VIOLET), ("REVIVE", 1, Color::srgb(0.35, 1.0, 0.6))] {
                    p.spawn(Node { width: Val::Percent(100.0), column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() })
                        .with_children(|row| {
                            row.spawn((txt(label, FONT_SMALL, color), Node { width: Val::Px(96.0), ..default() }));
                            row.spawn((
                                Node { flex_grow: 1.0, height: Val::Px(12.0), border: UiRect::all(Val::Px(2.0)), ..default() },
                                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                                BorderColor::all(color.darker(0.3)),
                            ))
                            .with_children(|bar| {
                                let node = Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() };
                                if fill == 0 {
                                    bar.spawn((StaticFill, node, BackgroundColor(color)));
                                } else {
                                    bar.spawn((ReviveFill, node, BackgroundColor(color)));
                                }
                            });
                        });
                }
            });
            // the revive we are giving, just above the tutorial / prompt lines
            root.spawn(Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(250.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|c| {
                c.spawn((RescueLine, txt("", FONT_MED, Color::srgb(0.35, 1.0, 0.6))));
            });
            for i in 0..MAX_MATES {
                root.spawn((
                    BeaconMarker(i),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(-200.0),
                        top: Val::Px(-200.0),
                        padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.03, 0.0, 0.7)),
                    BorderColor::all(BEACON_AMBER),
                    Visibility::Hidden,
                    Pickable::IGNORE,
                ))
                .with_children(|m| {
                    m.spawn(txt("", 13.0, BEACON_AMBER));
                });
            }
        });
}

pub fn despawn_coop_hud(mut commands: Commands, q: Query<Entity, With<CoopHudRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// One teammate as the HUD sees them.
struct Mate {
    pid: u8,
    hero: AstronautKind,
    v: PlayerVitals,
    pos: Option<Vec3>,
}

/// Our PlayerId, and every teammate. On the host each astronaut carries the vitals the host
/// writes; on a joiner the replicated entities do (the server's copy of US included — our
/// own predicted body's vitals are zeroed, so it is skipped).
#[allow(clippy::type_complexity)]
fn squad_view(
    role: &NetRole,
    mine: &crate::net::MyPlayerId,
    q: &Query<(&PlayerId, &PlayerVitals, Option<&NetHero>, Option<&GlobalTransform>, Has<LocalPlayer>)>,
) -> (Option<u8>, Vec<Mate>) {
    let client = *role == NetRole::Client;
    let me = if client { mine.0 } else { q.iter().find(|(.., local)| *local).map(|(pid, ..)| pid.0) };
    let mut mates: Vec<Mate> = q
        .iter()
        .filter(|(pid, _, _, _, local)| !(client && *local) && Some(pid.0) != me)
        .map(|(pid, v, hero, tf, _)| Mate {
            pid: pid.0,
            hero: hero.map(|h| crate::net::hero_from_code(h.0)).unwrap_or(AstronautKind::Buzz),
            v: *v,
            pos: tf.map(|t| t.translation()),
        })
        .collect();
    mates.sort_by_key(|m| m.pid);
    (me, mates)
}

fn bar(frac: f32, cells: usize) -> String {
    let n = (frac.clamp(0.0, 1.0) * cells as f32).round() as usize;
    format!("[{}{}]", "#".repeat(n), "-".repeat(cells - n))
}

/// The squad strip, STATIC CASCADE's link, and our own co-op status line. Also the one
/// place the squad's down / claimed / back-up moments become banners, from the vitals'
/// edges — the same on every machine, however the news arrived.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn update_squad_hud(
    role: Res<NetRole>,
    mine: Res<crate::net::MyPlayerId>,
    run: Res<RunState>,
    q: Query<(&PlayerId, &PlayerVitals, Option<&NetHero>, Option<&GlobalTransform>, Has<LocalPlayer>)>,
    q_me: Query<&PlayerState, With<LocalPlayer>>,
    mut rows: Query<(&SquadRow, &mut Text, &mut TextColor, &mut Visibility), (Without<CascadeLine>, Without<StatusLine>)>,
    mut lines: ParamSet<(Query<&mut Text, With<CascadeLine>>, Query<(&mut Text, &mut TextColor), With<StatusLine>>)>,
    mut banners: MessageWriter<crate::messages::BannerMsg>,
    mut seen: Local<std::collections::HashMap<u8, (bool, bool)>>,
) {
    let (me, mates) = squad_view(&role, &mine, &q);
    for (row, mut text, mut color, mut vis) in &mut rows {
        let Some(m) = mates.get(row.0) else {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        };
        *vis = Visibility::Inherited;
        let name = format!("P{} {}", m.pid + 1, m.hero.def().name);
        let (line, c) = if m.v.status & VITALS_CLAIMED != 0 {
            (format!("{name}  CLAIMED BY THE STATIC - back at the teleporter"), STATIC_VIOLET)
        } else if m.v.down {
            (
                format!(
                    "{name}  DOWN!  static {:.0}%  revive {:.0}%",
                    m.v.static_meter as f32 / 2.55,
                    m.v.revive as f32 / 2.55
                ),
                BEACON_AMBER,
            )
        } else {
            let frac = if m.v.max_hp > 0.0 { m.v.hp / m.v.max_hp } else { 1.0 };
            let grace = if m.v.grace > 0 { format!("  autopilot {}s", m.v.grace) } else { String::new() };
            (format!("{name}  LV {}  {} {:.0}{grace}", m.v.level, bar(frac, 10), m.v.hp.max(0.0)), m.hero.def().visor.mix(&Color::WHITE, 0.4))
        };
        if text.0 != line {
            text.0 = line;
        }
        color.0 = c;
    }
    // banners from the vitals' edges, for everyone in the squad (ourselves included)
    let everyone: Vec<(u8, bool, bool)> = q
        .iter()
        .filter(|(_, _, _, _, local)| !(*role == NetRole::Client && *local))
        .map(|(pid, v, ..)| (pid.0, v.down, v.status & VITALS_CLAIMED != 0))
        .collect();
    if everyone.len() > 1 {
        for (pid, down, claimed) in &everyone {
            let was = seen.insert(*pid, (*down, *claimed)).unwrap_or((false, false));
            let who = if Some(*pid) == me { None } else { Some(pid + 1) };
            if *down && !was.0 {
                banners.write(crate::messages::BannerMsg(match who {
                    Some(n) => format!("P{n} IS DOWN! STAND IN THEIR RING TO REVIVE"),
                    None => "YOU'RE DOWN! HOLD ON FOR A TEAMMATE".into(),
                }));
            }
            if *claimed && !was.1 {
                banners.write(crate::messages::BannerMsg(match who {
                    Some(n) => format!("THE STATIC CLAIMED P{n}"),
                    None => "THE STATIC HAS YOU".into(),
                }));
            }
        }
    }
    seen.retain(|pid, _| everyone.iter().any(|(p, ..)| p == pid));

    if let Ok(mut t) = lines.p0().single_mut() {
        let line = if run.cascade_charge > 0.0 {
            format!("STATIC CASCADE LINKING {}", bar(run.cascade_charge, 12))
        } else {
            String::new()
        };
        if t.0 != line {
            t.0 = line;
        }
    }
    if let (Ok((mut t, mut c)), Ok(ps)) = (lines.p1().single_mut(), q_me.single()) {
        let mut parts: Vec<String> = Vec::new();
        if ps.grace > 0.0 && !ps.dead {
            parts.push(format!("DROP-IN GRACE {:.0}s: AUTOPILOT WHILE YOU'RE IDLE", ps.grace.ceil()));
        }
        if ps.adrenaline > 0.0 {
            parts.push("HERO'S ADRENALINE +20% SPEED".into());
        }
        if ps.chill > 0.0 && !ps.dead {
            parts.push("CHILLED BY A TEAMMATE'S CRYO".into());
        }
        let line = parts.join("\n");
        if t.0 != line {
            t.0 = line;
        }
        c.0 = if ps.chill > 0.0 { Color::srgb(0.55, 0.85, 1.0) } else { Color::srgb(0.6, 1.0, 0.75) };
    }
}

/// Our own Beacon: the panel with The Static's meter and the revive, and the screen souring
/// toward The Static as it fills. Claimed, the panel says when we are back. Only in a squad
/// — solo, going down still ends the run a beat later.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn update_down_panel(
    time: Res<Time<Real>>,
    phase: Res<crate::run::RunPhase>,
    save: Res<crate::save::MetaSave>,
    role: Res<NetRole>,
    mine: Res<crate::net::MyPlayerId>,
    q: Query<(&PlayerId, &PlayerVitals, Option<&NetHero>, Option<&GlobalTransform>, Has<LocalPlayer>)>,
    q_me: Query<&PlayerState, With<LocalPlayer>>,
    mut panel: Query<&mut Visibility, With<DownPanel>>,
    mut texts: ParamSet<(Query<(&mut Text, &mut TextColor), With<DownTitle>>, Query<&mut Text, With<DownHint>>)>,
    mut fills: ParamSet<(Query<&mut Node, With<StaticFill>>, Query<&mut Node, With<ReviveFill>>)>,
    mut tint: Query<&mut BackgroundColor, With<DownTint>>,
) {
    let Ok(ps) = q_me.single() else { return };
    let (_, mates) = squad_view(&role, &mine, &q);
    // a card panel or the pause menu takes the middle of the screen: the tint stays, the
    // panel steps aside
    let behind = matches!(*phase, crate::run::RunPhase::LevelUp | crate::run::RunPhase::Modal | crate::run::RunPhase::Paused);
    let show = ps.dead && !mates.is_empty();
    if let Ok(mut vis) = panel.single_mut() {
        let want = if show && !behind { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
    if let Ok(mut bg) = tint.single_mut() {
        // a slow breath (well under 3 Hz), steady under photosensitivity
        let breathe = if save.accessibility.photosensitive { 1.0 } else { 0.85 + 0.15 * (time.elapsed_secs() * 1.2).sin() };
        let a = if !show {
            0.0
        } else if ps.claimed {
            0.32
        } else {
            (0.06 + 0.22 * ps.static_meter) * breathe
        };
        bg.0 = STATIC_VIOLET.darker(0.35).with_alpha(a);
    }
    if !show {
        return;
    }
    let standing = mates.iter().filter(|m| !m.v.down).count();
    if let Ok((mut t, mut c)) = texts.p0().single_mut() {
        let (title, color) = if ps.claimed { ("THE STATIC HAS YOU", STATIC_VIOLET) } else { ("YOU'RE DOWN", BEACON_AMBER) };
        if t.0 != title {
            t.0 = title.into();
        }
        c.0 = color;
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        let hint = if ps.claimed {
            "You rejoin the squad at the next teleporter, at half HP.".to_string()
        } else if standing == 0 {
            "Nobody is left standing to reach your Beacon...".to_string()
        } else {
            format!("Your Beacon is calling.\nA teammate in your ring for {REVIVE_SECS:.0}s brings you back.")
        };
        if t.0 != hint {
            t.0 = hint;
        }
    }
    if let Ok(mut n) = fills.p0().single_mut() {
        n.width = Val::Percent(ps.static_meter.clamp(0.0, 1.0) * 100.0);
    }
    if let Ok(mut n) = fills.p1().single_mut() {
        n.width = Val::Percent(ps.revive.clamp(0.0, 1.0) * 100.0);
    }
}

/// The revive WE are giving: standing in a teammate's ring, the line shows their progress.
#[allow(clippy::type_complexity)]
pub fn update_rescue_line(
    phase: Res<crate::run::RunPhase>,
    role: Res<NetRole>,
    mine: Res<crate::net::MyPlayerId>,
    planet: Res<crate::planet::CurrentPlanet>,
    q: Query<(&PlayerId, &PlayerVitals, Option<&NetHero>, Option<&GlobalTransform>, Has<LocalPlayer>)>,
    q_me: Query<(&crate::player::Player, &PlayerState), With<LocalPlayer>>,
    mut line: Query<&mut Text, With<RescueLine>>,
) {
    let Ok(mut t) = line.single_mut() else { return };
    let (_, mates) = squad_view(&role, &mine, &q);
    let text = q_me
        .single()
        .ok()
        .filter(|(_, ps)| !ps.dead && *phase == crate::run::RunPhase::Playing)
        .and_then(|(p, _)| {
            mates.iter().find(|m| {
                m.v.down
                    && m.v.status & VITALS_CLAIMED == 0
                    && m.pos.is_some_and(|pos| crate::sphere::arc_dist(p.dir, pos.normalize_or_zero(), planet.radius) <= REVIVE_RADIUS + 0.5)
            })
        })
        .map(|m| format!("REVIVING P{} {}", m.pid + 1, bar(m.v.revive as f32 / 255.0, 12)))
        .unwrap_or_default();
    if t.0 != text {
        t.0 = text;
    }
}

/// Markers at the screen's edge for teammates' Beacons beyond it — the far side of a
/// planet is exactly where a flare can't be seen (§11: "visible from anywhere"), and a
/// marker is how you find it. Labelled with who and how far round the planet.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn update_beacon_markers(
    ui_scale: Res<UiScale>,
    role: Res<NetRole>,
    mine: Res<crate::net::MyPlayerId>,
    planet: Res<crate::planet::CurrentPlanet>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::player::PlayerRig>>,
    q: Query<(&PlayerId, &PlayerVitals, Option<&NetHero>, Option<&GlobalTransform>, Has<LocalPlayer>)>,
    q_me: Query<&crate::player::Player, With<LocalPlayer>>,
    mut markers: Query<(&BeaconMarker, &mut Node, &mut Visibility, &Children)>,
    mut labels: Query<&mut Text>,
) {
    let Ok((cam, cam_tf)) = camera.single() else { return };
    let Some(size) = cam.logical_viewport_size() else { return };
    let (_, mates) = squad_view(&role, &mine, &q);
    let me = q_me.single().ok().map(|p| p.dir);
    let beacons: Vec<&Mate> = mates.iter().filter(|m| m.v.down && m.v.status & VITALS_CLAIMED == 0 && m.pos.is_some()).collect();
    let ui = ui_scale.0.max(0.01);
    let center = size / 2.0;
    let inv = cam_tf.affine().inverse();
    for (slot, mut node, mut vis, children) in &mut markers {
        let Some(m) = beacons.get(slot.0) else {
            *vis = Visibility::Hidden;
            continue;
        };
        let world = m.pos.unwrap_or(Vec3::ZERO);
        // the flare itself is the marker when it's on screen
        let on_screen = cam
            .world_to_viewport(cam_tf, world)
            .is_ok_and(|v| v.x >= 0.0 && v.y >= 0.0 && v.x <= size.x && v.y <= size.y);
        let ahead = inv.transform_point3(world).z < 0.0;
        if on_screen && ahead {
            *vis = Visibility::Hidden;
            continue;
        }
        let local = inv.transform_point3(world);
        let mut d = Vec2::new(local.x, -local.y);
        if local.z > 0.0 {
            d = -d;
        }
        let d = d.try_normalize().unwrap_or(Vec2::Y);
        let half = center - Vec2::new(70.0, 30.0);
        let k = (half.x / d.x.abs().max(1e-4)).min(half.y / d.y.abs().max(1e-4));
        let pos = (center + d * k) / ui;
        node.left = Val::Px(pos.x - 40.0);
        node.top = Val::Px(pos.y - 10.0);
        *vis = Visibility::Inherited;
        let dist = me.map(|d0| crate::sphere::arc_dist(d0, world.normalize_or_zero(), planet.radius)).unwrap_or(0.0);
        let text = format!("P{} DOWN {:.0}m", m.pid + 1, dist);
        for c in children.iter() {
            if let Ok(mut t) = labels.get_mut(c) {
                if t.0 != text {
                    t.0 = text.clone();
                }
            }
        }
    }
}
