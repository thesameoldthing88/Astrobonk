//! In-run HUD: timer, currencies, XP/HP bars, weapon & item trays, boss bar,
//! banners, interact prompt, powerup timers, hurt vignette.

use super::*;
use crate::content::weapons::WeaponKind;
use crate::enemies::{Boss, Enemy};
use crate::interact::InteractPrompt;
use crate::messages::BannerMsg;
use crate::config::{HURT_TINT_PEAK, HURT_TINT_PEAK_REDUCED, HURT_TINT_SECS};
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
/// What the local astronaut's conditional items are doing right now (jammed, hovering,
/// encircled, the Widow's bonus, the tether, the eaten sun).
#[derive(Component)]
pub struct ItemStatusText;
/// The HP bar, XP bar and weapon tray at the bottom of the screen (its height sets the
/// in-run panels' bottom band, `keep_panels_clear_of_hud`).
#[derive(Component)]
pub struct HudBottomCluster;
/// The bottom cluster's distance from the screen's bottom edge, and the clear gap the
/// in-run panels keep above it, in UI units.
const HUD_BOTTOM_CLUSTER_Y: f32 = 12.0;
const HUD_BAND_GAP: f32 = 8.0;
#[derive(Component)]
pub struct Vignette;
#[derive(Component)]
pub struct CometText;
#[derive(Component)]
pub struct DustOverlay;
/// §13 assists in force (and the "one more chance" token), under the counters.
#[derive(Component)]
pub struct AssistText;
/// One slot in the off-screen indicator pool (colored squares hugging the screen edge).
#[derive(Component)]
pub struct EdgeMarker;
/// The antipode dial's distance from the screen's left and bottom edges, in UI units.
const ANTIPODE_DIAL_X: f32 = 14.0;
const ANTIPODE_DIAL_Y: f32 = 44.0;
/// The antipode dial (§4: the HUD tell for Antipode Blink): who waits on the far side of
/// the planet, and whether the blink is charged. Shown while the local astronaut carries a
/// blink (Antipode Blink or Boomerang Insurance).
#[derive(Component)]
pub struct AntipodeDial;
/// The dial's ring: its border is the far side's band colour, pulsing when it's a wall.
#[derive(Component)]
pub struct AntipodeRing;
#[derive(Component)]
pub struct AntipodeCount;
#[derive(Component)]
pub struct AntipodeBandText;
#[derive(Component)]
pub struct AntipodeBlinkText;

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

            // the antipode dial, bottom-left: a ring naming the far side's crowd, over the
            // blink's charge. Clear of the bottom cluster (which starts at 18% across); it
            // steps aside while a card panel is up, like the other mid-screen lines.
            root.spawn((
                AntipodeDial,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(ANTIPODE_DIAL_X),
                    // above the bottom line the off-screen edge markers run along (34 px
                    // in); the left one's markers step round it (`update_edge_markers`)
                    bottom: Val::Px(ANTIPODE_DIAL_Y),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(1.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                // a dark backing: the dial sits over the day side's bright regolith too
                BackgroundColor(Color::srgba(0.03, 0.04, 0.08, 0.55)),
                Visibility::Hidden,
                Pickable::IGNORE,
            ))
            .with_children(|d| {
                d.spawn((txt("ANTIPODE", 11.0, Color::srgb(0.7, 0.8, 0.9)),));
                d.spawn((
                    AntipodeRing,
                    Node {
                        width: Val::Px(52.0),
                        height: Val::Px(52.0),
                        border: UiRect::all(Val::Px(4.0)),
                        border_radius: BorderRadius::MAX,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.03, 0.04, 0.08, 0.75)),
                    BorderColor::all(Color::WHITE),
                ))
                .with_children(|r| {
                    r.spawn((AntipodeCount, txt("0", FONT_MED, Color::WHITE)));
                });
                d.spawn((AntipodeBandText, txt("CLEAR", 12.0, Color::WHITE)));
                d.spawn((AntipodeBlinkText, txt("", 12.0, Color::WHITE)));
            });

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

            // boss bar under the timer — and under the comet readout (52 px + a 20 px line),
            // which it used to overprint: a boss fight is exactly when a comet tail forms
            root.spawn((
                BossBarWrap,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(80.0),
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
                    c.spawn((ItemStatusText, txt("", FONT_SMALL, Color::srgb(1.0, 0.75, 0.45))));
                    c.spawn((AssistText, txt("", FONT_SMALL, Color::srgb(0.55, 0.9, 1.0)), TextLayout::new_with_justify(Justify::Right)));
                });

            // banner center, clear of the boss bar (80 px + a name line + the bar). In px, not
            // a share of the screen height, so it keeps its distance at every UI scale
            root.spawn((Node {
                position_type: PositionType::Absolute,
                top: Val::Px(124.0),
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
            root.spawn((HudBottomCluster, Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(HUD_BOTTOM_CLUSTER_Y),
                left: Val::Percent(18.0),
                width: Val::Percent(64.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },))
                .with_children(|c| {
                    // weapons tray, then item chips. It WRAPS, growing upward from the bars:
                    // a late build holds 20+ distinct items, and one row ran off the screen.
                    c.spawn((
                        WeaponRow,
                        Node {
                            width: Val::Percent(100.0),
                            min_height: Val::Px(34.0),
                            flex_wrap: FlexWrap::Wrap,
                            column_gap: Val::Px(6.0),
                            row_gap: Val::Px(4.0),
                            align_items: AlignItems::Center,
                            align_content: AlignContent::FlexEnd,
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
    mut vignette: Query<(&mut BackgroundColor, Ref<Vignette>)>,
    (save, time): (Res<crate::save::MetaSave>, Res<Time>),
    // (HP + shield last frame, seconds of hurt tint left)
    mut hurt: Local<(f32, f32)>,
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
    if let Ok((mut bg, vignette)) = vignette.single_mut() {
        // The hurt tint marks what a hit TOOK — HP or shield — seen here, on the HUD's own
        // machine: a joiner gets it from the vitals the host streams, and a revive's grace
        // (HP going up) never tints. A fresh HUD (new run, other hero) starts from its own
        // numbers, not the last run's. Flash reduction keeps it to a faint wash.
        let pool = ps.hp.max(0.0) + ps.shield.max(0.0);
        if vignette.is_added() {
            *hurt = (pool, 0.0);
        } else if pool < hurt.0 - 0.01 {
            hurt.1 = HURT_TINT_SECS;
        }
        hurt.0 = pool;
        hurt.1 = (hurt.1 - time.delta_secs()).max(0.0);
        let peak = if save.accessibility.flash_reduction { HURT_TINT_PEAK_REDUCED } else { HURT_TINT_PEAK };
        bg.0 = save.accessibility.palette.danger().with_alpha(peak * hurt.1 / HURT_TINT_SECS);
    }
}

/// The HUD's mid-screen lines (the Mission Control tutorial, the interact prompt) and the
/// item status line (P03; long enough to reach under the rightmost card) sit where a choice
/// panel or the pause menu draws its cards — at a large UI scale right across them. While
/// one is open those lines step aside; they are back the moment play resumes.
pub fn hide_mid_hud_under_panels(
    phase: Res<crate::run::RunPhase>,
    mut q: Query<&mut Visibility, Or<(With<crate::tutorial::TutorialText>, With<PromptText>, With<ItemStatusText>)>>,
) {
    use crate::run::RunPhase;
    let want = if matches!(*phase, RunPhase::LevelUp | RunPhase::Modal | RunPhase::Paused) {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut v in &mut q {
        if *v != want {
            *v = want;
        }
    }
}

/// Keep the in-run panels' bands clear of the HUD: the top band grows while a boss bar is
/// showing, so a level-up or the pause menu never prints over a boss's name, and the bottom
/// band grows with the weapon tray, whose item chips wrap onto more rows as a build fills
/// out (P03) — so a card, a button or the panel's status line never lands on a chip.
pub fn keep_panels_clear_of_hud(
    boss_bar: Query<&Visibility, With<BossBarWrap>>,
    cluster: Query<&ComputedNode, With<HudBottomCluster>>,
    mut q: Query<&mut Node, With<super::RunOverlay>>,
) {
    let boss = boss_bar.iter().any(|v| *v != Visibility::Hidden);
    let top = Val::Px(if boss { crate::config::HUD_TOP_BAND_BOSS } else { crate::config::HUD_TOP_BAND });
    // ComputedNode sizes are physical pixels; × inverse_scale_factor gives the UI units
    // Val::Px is written in (the same units at every UI scale). Whole units, so layout
    // jitter of a fraction of a pixel never re-lays the panel.
    let tray = cluster
        .iter()
        .next()
        .map_or(0.0, |c| (c.size().y * c.inverse_scale_factor()).ceil() + HUD_BOTTOM_CLUSTER_Y + HUD_BAND_GAP);
    let bottom = Val::Px(crate::config::HUD_BOTTOM_BAND.max(tray));
    for mut n in &mut q {
        if n.padding.top != top {
            n.padding.top = top;
        }
        if n.padding.bottom != bottom {
            n.padding.bottom = bottom;
        }
    }
}

/// The assists in force, so nobody mistakes an eased run for a canon one — including a
/// joiner, whose run is the host's (`RunState::assist` arrives in RunSnapMsg) — and the
/// "one more chance" token while it is still in hand.
pub fn update_assist_hud(
    run: Res<RunState>,
    q_ps: Query<&PlayerState, With<crate::player::LocalPlayer>>,
    mut q: Query<&mut Text, With<AssistText>>,
) {
    let Ok(mut t) = q.single_mut() else { return };
    let token = match q_ps.single() {
        Ok(ps) if run.assist.revive_token => {
            if ps.revive_token_ready(&run.assist) { "\nONE MORE CHANCE: READY" } else { "\nONE MORE CHANCE: USED" }
        }
        _ => "",
    };
    let line = if run.assisted { format!("ASSISTED{token}") } else { String::new() };
    if t.0 != line {
        t.0 = line;
    }
}

/// The antipode dial (§4 "the off-screen threat ring shows antipode density"): the count
/// the host takes at the local astronaut's far pole every quarter second, named in a band
/// word (shape + text + colour, §13 — never colour alone), The Static or a boss called out,
/// and the blink's charge. It reads only the local body's `NetItemVis` — written by the host
/// for its own body, adopted by a joiner from the host's copy of it — so both HUDs are one
/// path. The ring pulses (well under 3 Hz, and never a flash) while the far side is a wall.
#[allow(clippy::type_complexity)]
pub fn update_antipode_dial(
    time: Res<Time<Real>>,
    save: Res<crate::save::MetaSave>,
    phase: Res<crate::run::RunPhase>,
    q_ps: Query<(&PlayerState, &crate::net::NetItemVis), With<crate::player::LocalPlayer>>,
    mut dial: Query<&mut Visibility, With<AntipodeDial>>,
    mut ring: Query<&mut BorderColor, With<AntipodeRing>>,
    mut texts: ParamSet<(
        Query<(&mut Text, &mut TextColor), With<AntipodeCount>>,
        Query<(&mut Text, &mut TextColor), With<AntipodeBandText>>,
        Query<(&mut Text, &mut TextColor), With<AntipodeBlinkText>>,
    )>,
) {
    use crate::content::items::ItemKind;
    use crate::net::{ITEMVIS_ANTIPODE_BOSS, ITEMVIS_ANTIPODE_STATIC, ITEMVIS_INSURED};
    use crate::techs::AntipodeBand;
    let Ok(mut vis) = dial.single_mut() else { return };
    let Ok((ps, net)) = q_ps.single() else {
        *vis = Visibility::Hidden;
        return;
    };
    let blink = ps.has_item(ItemKind::AntipodeBlink);
    let insured = ps.has_item(ItemKind::BoomerangInsurance);
    use crate::run::RunPhase;
    let panel = matches!(*phase, RunPhase::LevelUp | RunPhase::Modal | RunPhase::Paused);
    let want = if (blink || insured) && !panel { Visibility::Inherited } else { Visibility::Hidden };
    if *vis != want {
        *vis = want;
    }
    if want == Visibility::Hidden {
        return;
    }
    let danger = save.accessibility.palette.danger();
    let count = net.antipode as u32;
    let band = AntipodeBand::of(count);
    let (label, color) = if net.flags & ITEMVIS_ANTIPODE_STATIC != 0 {
        ("THE STATIC", Color::srgb(0.85, 0.65, 1.0))
    } else if net.flags & ITEMVIS_ANTIPODE_BOSS != 0 {
        ("BOSS", danger)
    } else {
        match band {
            AntipodeBand::Clear => (band.label(), Color::srgb(0.45, 1.0, 0.8)),
            AntipodeBand::Thin => (band.label(), Color::srgb(0.85, 0.95, 1.0)),
            AntipodeBand::Crowded => (band.label(), Color::srgb(1.0, 0.75, 0.3)),
            AntipodeBand::Wall => (band.label(), danger),
        }
    };
    let alarm = band == AntipodeBand::Wall || net.flags & (ITEMVIS_ANTIPODE_STATIC | ITEMVIS_ANTIPODE_BOSS) != 0;
    let pulse = if alarm { 0.7 + 0.3 * (time.elapsed_secs() * std::f32::consts::TAU * 1.2).sin() } else { 1.0 };
    if let Ok(mut b) = ring.single_mut() {
        *b = BorderColor::all(color.with_alpha(pulse));
    }
    let count_line = if count >= 255 { "255+".to_string() } else { count.to_string() };
    let charged = net.blink_cd == 0;
    let blink_line = match (blink, insured) {
        (true, _) if charged => "[Q] BLINK".to_string(),
        (true, _) => format!("BLINK {}s", net.blink_cd),
        (false, _) if !charged => format!("RECHARGE {}s", net.blink_cd),
        (false, _) => String::new(),
    };
    let policy = if !insured {
        ""
    } else if net.flags & ITEMVIS_INSURED != 0 {
        "INSURED"
    } else {
        "POLICY PAID"
    };
    let blink_line = match (blink_line.is_empty(), policy.is_empty()) {
        (_, true) => blink_line,
        (true, false) => policy.to_string(),
        (false, false) => format!("{blink_line}\n{policy}"),
    };
    let ready = Color::srgb(0.5, 0.95, 1.0);
    let waiting = Color::srgb(0.75, 0.78, 0.85);
    set_text(&mut texts.p0(), &count_line, color);
    set_text(&mut texts.p1(), label, color);
    set_text(&mut texts.p2(), &blink_line, if charged { ready } else { waiting });
}

/// Write a HUD line only when it changed (a text edit re-lays the node).
fn set_text<F: bevy::ecs::query::QueryFilter>(q: &mut Query<(&mut Text, &mut TextColor), F>, s: &str, c: Color) {
    if let Ok((mut t, mut tc)) = q.single_mut() {
        if t.0 != s {
            t.0 = s.to_string();
        }
        if tc.0 != c {
            tc.0 = c;
        }
    }
}

/// What the weapon row was last built from (it is rebuilt only when this changes).
#[derive(PartialEq)]
pub struct WeaponRowKey {
    weapons: Vec<(WeaponKind, u32)>,
    items: Vec<(crate::content::items::ItemKind, u32, crate::content::Rarity)>,
    palette: crate::content::palettes::Palette,
}

/// Rebuild the weapon tray when loadout changes.
pub fn update_weapon_row(
    mut commands: Commands,
    q_ps: Query<&PlayerState, With<crate::player::LocalPlayer>>,
    save: Res<crate::save::MetaSave>,
    mut cache: Local<Option<WeaponRowKey>>,
    q_row: Query<Entity, With<WeaponRow>>,
) {
    let Ok(run) = q_ps.single() else { return };
    let current: Vec<(WeaponKind, u32)> = run.weapons.iter().map(|w| (w.kind, w.level)).collect();
    let palette = save.accessibility.palette;
    // rebuilt on any change to the loadout — a new copy of an item, or a better grade of it —
    // or to the palette its grade colors are drawn in
    let key = WeaponRowKey {
        weapons: current.clone(),
        items: run.items.iter().map(|s| (s.kind, s.count(), s.best())).collect(),
        palette,
    };
    if cache.as_ref() == Some(&key) {
        return;
    }
    *cache = Some(key);
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
        // item chips, coloured by the best grade held of each
        for stack in run.items.iter() {
            let d = stack.kind.def();
            let color = stack.best().color(palette);
            c.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.7)),
                BorderColor::all(color),
            ))
            .with_children(|chip| {
                chip.spawn(txt(format!("{}{}", d.tag, stack.count()), 11.0, color));
            });
        }
    });
}

/// One line of what the local astronaut's conditional items and tomes are doing — the
/// numbers they deal damage from, so "+12% from the descent" is something you can read.
pub fn update_item_status(
    run: Res<RunState>,
    q_ps: Query<(&PlayerState, &crate::items::ItemProcs), With<crate::player::LocalPlayer>>,
    mut q: Query<&mut Text, With<ItemStatusText>>,
) {
    use crate::config::*;
    use crate::content::items::ItemKind;
    let Ok(mut text) = q.single_mut() else { return };
    let Ok((ps, procs)) = q_ps.single() else { return };
    let mut parts: Vec<String> = Vec::new();
    if procs.jam > 0.0 {
        parts.push("JAMMED".into());
    }
    if ps.has_item(ItemKind::AntiGravBoots) && ps.airborne {
        parts.push(format!("HOVER {:.1}s", procs.hover_left.max(0.0)));
    }
    if ps.has_item(ItemKind::IcarusBoots) {
        parts.push(if ps.airborne { "ICARUS UP".into() } else { "ICARUS GROUNDED".into() });
    }
    let dirs = ps.encircle_dirs.count_ones();
    if dirs > 0 {
        let pct = ENCIRCLE_DMG_PER_DIR * dirs as f32 * ps.item_power(ItemKind::EncirclementBonus) * 100.0;
        parts.push(format!("ENCIRCLED {dirs}/8 +{pct:.0}%"));
    }
    let downhill = DOWNHILL_DMG_PER_M * ps.descent_m * ps.item_power(ItemKind::DownhillMomentum) * 100.0;
    if downhill >= 1.0 {
        parts.push(format!("DOWNHILL +{downhill:.0}%"));
    }
    if ps.widow_active() {
        parts.push(format!("WIDOW +{:.0}%", WIDOW_STAT_BONUS * 100.0));
    } else if ps.has_item(ItemKind::WidowsRing) && procs.widow_cd > 0.0 {
        parts.push(format!("RING {:.0}s", procs.widow_cd));
    }
    if ps.has_item(ItemKind::DeadMansTether) {
        parts.push(if ps.tether_used { "TETHER SPENT".into() } else { "TETHER READY".into() });
    }
    if run.sun_shrink > 0.0 {
        parts.push(format!("SUN -{:.0}%", run.sun_shrink * 100.0));
    }
    // the tomes' conditions (Encirclement, Nightfall, Momentum)
    if ps.crowd_bonus() >= 0.005 {
        parts.push(format!("CROWD {} +{:.0}%", ps.crowd, ps.crowd_bonus() * 100.0));
    }
    if ps.night_bonus() > 0.0 {
        parts.push(format!("NIGHT +{:.0}%", ps.night_bonus() * 100.0));
    }
    if ps.momentum_bonus() >= 0.005 {
        parts.push(format!("MOMENTUM +{:.0}%", ps.momentum_bonus() * 100.0));
    }
    let line = parts.join("  ");
    if text.0 != line {
        text.0 = line;
    }
}

/// Point edge markers at important things beyond the screen/horizon: bosses (red),
/// the teleporter (green), chests (gold), the Shady Guy (purple), charge shrines
/// (cyan), the cage (brown). On a sphere most objectives are below the horizon —
/// this is how you navigate to them.
#[allow(clippy::type_complexity)]
pub fn update_edge_markers(
    ui_scale: Res<UiScale>,
    save: Res<crate::save::MetaSave>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::player::PlayerRig>>,
    q_boss: Query<&Transform, With<crate::enemies::Boss>>,
    q_inter: Query<(&Transform, &crate::interact::Interactable)>,
    q_charge: Query<(&Transform, &crate::interact::ChargeShrine)>,
    dial: Query<(&ComputedNode, &Visibility), With<AntipodeDial>>,
    mut markers: Query<(&mut Node, &mut BackgroundColor, &mut BorderColor), With<EdgeMarker>>,
) {
    use crate::interact::InteractKind;
    let Ok((cam, cam_tf)) = camera.single() else { return };
    let Some(size) = cam.logical_viewport_size() else { return };
    let center = size / 2.0;
    let margin = 34.0;
    let ui = ui_scale.0.max(0.01);
    // The antipode dial sits on the left edge's marker line: while it shows, a marker that
    // would land on it steps up the edge to just above it, so neither hides the other.
    // (UI units, like the markers' own Val::Px; ComputedNode sizes are physical pixels.)
    let dial_rect = dial.iter().find(|(_, v)| **v != Visibility::Hidden).map(|(c, _)| {
        let dial = c.size() * c.inverse_scale_factor();
        let bottom = size.y / ui - ANTIPODE_DIAL_Y;
        Rect::new(ANTIPODE_DIAL_X, bottom - dial.y, ANTIPODE_DIAL_X + dial.x, bottom)
    });

    // collect targets: (world pos, color, marker px size), priority order
    let mut targets: Vec<(Vec3, Color, f32)> = Vec::new();
    for tf in &q_boss {
        targets.push((tf.translation, save.accessibility.palette.danger(), 16.0));
    }
    for (tf, inter) in &q_inter {
        if inter.used {
            continue;
        }
        let (color, px) = match inter.kind {
            InteractKind::Teleporter => (Color::srgb(0.3, 1.0, 0.8), 16.0),
            InteractKind::RewardChest => (Color::srgb(1.0, 0.85, 0.2), 15.0),
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
        // Logical pixels -> UI units: UiScale multiplies every Val::Px.
        let mut pos = (center + d * scale_x.min(scale_y)) / ui;
        if let Some(r) = dial_rect {
            let h = px / 2.0;
            // a marker on the bottom edge's line passes under the dial: only the left
            // column's can land on it
            if pos.x - h < r.max.x + 2.0 && pos.y + h > r.min.y - 2.0 && pos.y - h < r.max.y {
                pos.y = r.min.y - 2.0 - h;
            }
        }

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
/// Plain ASCII on purpose: the game font (Bevy's built-in FiraMono subset) has only the 95
/// printable ASCII glyphs, so block characters drew a row of identical missing-glyph boxes
/// and the meter never visibly filled.
pub fn update_comet_hud(
    comet: Res<crate::comet::Comet>,
    mut q: Query<(&mut Text, &mut TextColor), With<CometText>>,
) {
    let Ok((mut text, mut color)) = q.single_mut() else { return };
    if comet.flash > 0.0 {
        text.0 = "COMET!".into();
        color.0 = Color::srgb(1.0, 0.9, 0.4);
    } else if comet.active {
        let filled = (comet.progress * 12.0).round() as usize;
        let bar: String = "#".repeat(filled) + &"-".repeat(12 - filled);
        text.0 = format!("COMET x{}  [{bar}]", comet.count);
        // warm up from amber to white-hot as the charge fills
        let t = comet.progress;
        color.0 = Color::srgb(1.0, 0.8 + 0.2 * t, 0.3 + 0.5 * t);
    } else {
        text.0 = String::new();
    }
}

pub fn update_boss_bar(
    save: Res<crate::save::MetaSave>,
    q_boss: Query<(&Enemy, &Boss)>,
    mut wrap: Query<&mut Visibility, With<BossBarWrap>>,
    mut fill: Query<(&mut Node, &mut BackgroundColor), With<BossBarFill>>,
    mut name: Query<(&mut Text, &mut TextColor), With<BossBarName>>,
) {
    let danger = save.accessibility.palette.danger();
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
            if let Ok((mut f, mut bg)) = fill.single_mut() {
                f.width = Val::Percent((hp / max * 100.0).clamp(0.0, 100.0));
                bg.0 = danger;
            }
            if let Ok((mut t, mut c)) = name.single_mut() {
                t.0 = n.to_string();
                c.0 = danger.mix(&Color::WHITE, 0.35);
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
