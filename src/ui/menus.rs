//! Out-of-run screens: main menu (with tome shop + quest log), astronaut select,
//! planet/tier select, and the results screen.

use super::*;
use crate::content::characters::AstronautKind;
use crate::content::planets::PlanetKind;
use crate::content::quests::QuestKind;
use crate::content::tomes::TomeKind;
use crate::director::ResultsData;
use crate::messages::{Sfx, SfxMsg};
use crate::run::RunState;
use crate::save::MetaSave;
use crate::AppState;
use bevy::app::AppExit;
use bevy::prelude::*;

#[derive(Resource, Clone, Copy)]
pub struct Selected {
    pub character: AstronautKind,
    pub planet: PlanetKind,
    pub tier: u32,
    pub daily: bool,
    /// Picking a hero to JOIN someone's run with: the pick returns to the address entry
    /// instead of going on to the planet select (the host picks the world).
    pub joining: bool,
}

impl Default for Selected {
    fn default() -> Self {
        Self { character: AstronautKind::Buzz, planet: PlanetKind::Moon, tier: 1, daily: false, joining: false }
    }
}

#[derive(Component)]
pub struct MenuRoot;
#[derive(Component)]
pub struct LaunchBtn;
#[derive(Component)]
pub struct TomesBtn;
#[derive(Component)]
pub struct QuestsBtn;
#[derive(Component)]
pub struct QuitBtn;
#[derive(Component)]
pub struct SettingsBtn;
#[derive(Component)]
pub struct DailyBtn;

/// One enum-tagged component for all main-menu buttons — keeps the input system under
/// Bevy's 16-param cap (one query instead of six).
#[derive(Component, Clone, Copy, PartialEq)]
pub enum MenuBtn {
    Launch,
    Daily,
    Tomes,
    Quests,
    Settings,
    Quit,
    /// Open this machine to joiners, then play as normal — it is a listen server, so the
    /// host is a player too.
    HostCoop,
    /// Open the address entry.
    JoinCoop,
    JoinConfirm,
    JoinCancel,
}

/// Is the address entry showing?
#[derive(Resource, Default)]
pub struct JoinOpen(pub bool);

/// A line of feedback under the co-op buttons — "HOSTING on port 5011", "JOIN FAILED: ...".
/// Without this, a failed host (port already taken) is completely silent to the player.
#[derive(Resource, Default)]
pub struct CoopNote(pub String);

#[derive(Component)]
pub struct CoopNoteText;

/// The address-entry overlay shown after JOIN CO-OP.
#[derive(Component)]
pub struct JoinPanel;
#[derive(Component)]
pub struct JoinAddrText;

/// What the player has typed so far. Pre-filled with localhost because the overwhelmingly
/// common first test is two instances on one machine.
#[derive(Resource)]
pub struct JoinAddr(pub String);

impl Default for JoinAddr {
    fn default() -> Self {
        Self("127.0.0.1".into())
    }
}
#[derive(Component)]
pub struct SidePanel;
#[derive(Component)]
pub struct TomePlus(pub TomeKind);
#[derive(Component)]
pub struct TomeToggle(pub TomeKind);

#[derive(Resource, Default, Clone, Copy, PartialEq)]
pub enum MenuTab {
    #[default]
    None,
    Tomes,
    Quests,
}

#[derive(Component)]
pub struct CharCard(pub AstronautKind);
#[derive(Component)]
pub struct PlanetCard(pub PlanetKind, pub u32);
#[derive(Component)]
pub struct BackBtn;
#[derive(Component)]
pub struct ContinueBtn;

// ------------------------------------------------------------- main menu

pub fn spawn_main_menu(mut commands: Commands, save: Res<MetaSave>) {
    commands
        .spawn((MenuRoot, overlay_root(), BackgroundColor(Color::srgb(0.02, 0.02, 0.05))))
        .with_children(|root| {
            root.spawn(txt("ASTROBONK", 72.0, Color::srgb(1.0, 0.8, 0.2)));
            root.spawn(txt(
                "tiny planets. big swarms. one wrench.",
                FONT_MED,
                Color::srgb(0.6, 0.65, 0.8),
            ));
            root.spawn(txt(format!("SILVER: {}", save.silver), FONT_MED, Color::srgb(0.75, 0.85, 1.0)));

            let buttons: [(&str, fn() -> ()); 0] = [];
            let _ = buttons;

            root.spawn((LaunchBtn, MenuBtn::Launch, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.4, 1.0, 0.6))))
                .with_children(|b| {
                    b.spawn(txt("LAUNCH", FONT_BIG, Color::WHITE));
                });

            // ---- CO-OP ----
            root.spawn((Node { column_gap: Val::Px(10.0), ..default() },)).with_children(|row| {
                row.spawn((
                    MenuBtn::HostCoop,
                    Button,
                    button_node(),
                    BackgroundColor(BTN_BG),
                    BorderColor::all(Color::srgb(0.4, 0.9, 1.0)),
                ))
                .with_children(|b| {
                    b.spawn(txt("HOST CO-OP", FONT_MED, Color::WHITE));
                });
                row.spawn((
                    MenuBtn::JoinCoop,
                    Button,
                    button_node(),
                    BackgroundColor(BTN_BG),
                    BorderColor::all(Color::srgb(0.4, 0.9, 1.0)),
                ))
                .with_children(|b| {
                    b.spawn(txt("JOIN CO-OP", FONT_MED, Color::WHITE));
                });
            });
            // Feedback line: without it a failed host (port already in use) or a bad address
            // is completely silent and the player just sees nothing happen.
            // Centred: a failed join explains itself over two lines.
            root.spawn((
                CoopNoteText,
                txt("", FONT_SMALL, Color::srgb(0.5, 0.9, 1.0)),
                TextLayout::new_with_justify(Justify::Center),
            ));

            // Daily seeded planet — same tiny world for everyone today.
            let day = crate::run::today();
            let dname = crate::run::daily_name(crate::run::daily_seed(day));
            let dbest = if save.daily_day == day { save.daily_best } else { 0 };
            root.spawn((DailyBtn, MenuBtn::Daily, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(1.0, 0.55, 0.85))))
                .with_children(|b| {
                    b.spawn(txt(format!("DAILY: {dname}   (best today: {dbest})"), FONT_MED, Color::WHITE));
                });

            root.spawn((Node { column_gap: Val::Px(10.0), ..default() },)).with_children(|row| {
                row.spawn((TomesBtn, MenuBtn::Tomes, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.6, 0.6, 1.0))))
                    .with_children(|b| {
                        b.spawn(txt("TOMES", FONT_MED, Color::WHITE));
                    });
                row.spawn((QuestsBtn, MenuBtn::Quests, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(1.0, 0.8, 0.4))))
                    .with_children(|b| {
                        b.spawn(txt("QUESTS", FONT_MED, Color::WHITE));
                    });
                row.spawn((SettingsBtn, MenuBtn::Settings, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.5, 0.8, 1.0))))
                    .with_children(|b| {
                        b.spawn(txt("SETTINGS", FONT_MED, Color::WHITE));
                    });
                row.spawn((QuitBtn, MenuBtn::Quit, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.7, 0.4, 0.4))))
                    .with_children(|b| {
                        b.spawn(txt("QUIT", FONT_MED, Color::WHITE));
                    });
            });
            // side panel placeholder
            root.spawn((
                SidePanel,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    max_height: Val::Percent(46.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.06, 0.1, 0.8)),
            ));
        });
}

pub fn despawn_menu(mut commands: Commands, q: Query<Entity, With<MenuRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

#[allow(clippy::too_many_arguments)]
pub fn main_menu_input(
    mut commands: Commands,
    menu_btns: Query<(&Interaction, &MenuBtn), Changed<Interaction>>,
    plus: Query<(&Interaction, &TomePlus), Changed<Interaction>>,
    toggles: Query<(&Interaction, &TomeToggle), Changed<Interaction>>,
    mut tab: ResMut<MenuTab>,
    mut save: ResMut<MetaSave>,
    mut selected: ResMut<Selected>,
    mut settings_open: ResMut<crate::ui::settings::SettingsOpen>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
    mut sfx: MessageWriter<SfxMsg>,
    panel: Query<Entity, With<SidePanel>>,
    mut dirty: Local<bool>,
    // Bundled: this system is at Bevy's 16-system-param cap and already uses this trick.
    mut coop: (
        Res<bevy_replicon::prelude::RepliconChannels>,
        Res<JoinAddr>,
        ResMut<JoinOpen>,
        ResMut<CoopNote>,
        Res<crate::net::NetRole>,
        MessageWriter<crate::net::LeaveSession>,
    ),
) {
    let (channels, addr, join_open, coop_note, role, leave) = &mut coop;
    let mut changed = false;
    for (i, btn) in &menu_btns {
        if *i != Interaction::Pressed {
            continue;
        }
        // A full-screen overlay does not block the buttons behind it — Bevy still delivers
        // Interaction to them. Without this guard, clicking CONNECT also pressed HOST CO-OP
        // underneath, and the machine became a client AND a host at once.
        if join_open.0 && !matches!(btn, MenuBtn::JoinConfirm | MenuBtn::JoinCancel) {
            continue;
        }
        match btn {
            // The same button again stops what it started: a host that backed out of the
            // hero pick stops listening, a joiner still waiting for a world gives up.
            MenuBtn::HostCoop if **role == crate::net::NetRole::Host => {
                leave.write(crate::net::LeaveSession);
            }
            MenuBtn::JoinCoop if **role == crate::net::NetRole::Client => {
                leave.write(crate::net::LeaveSession);
            }
            MenuBtn::HostCoop | MenuBtn::JoinCoop if role.is_networked() => {
                coop_note.0 = "already in a co-op session: stop it first with the same button".into();
            }
            // A joiner plays the HOST'S run, entered when its seed arrives. Starting one of
            // our own here would build a world from a local seed while still connected, and
            // the host's run would never replace it.
            MenuBtn::Launch | MenuBtn::Daily if **role == crate::net::NetRole::Client => {
                coop_note.0 = "You are in the host's co-op session: the host picks the world.\n\
                               JOIN CO-OP again leaves the session, then LAUNCH plays solo."
                    .into();
                sfx.write(SfxMsg(Sfx::Click));
            }
            MenuBtn::HostCoop => {
                selected.joining = false;
                selected.daily = false;
                // Start listening, then fall through to the normal flow — the host picks a
                // character and planet as usual and joiners arrive once it is in a run.
                match crate::net::start_host(&mut commands, &channels, crate::net::DEFAULT_PORT) {
                    Ok(()) => {
                        coop_note.0 = crate::net::hosting_note(crate::net::DEFAULT_PORT);
                        next.set(AppState::CharSelect);
                    }
                    Err(e) => coop_note.0 = format!("HOST FAILED: {e}"),
                }
            }
            MenuBtn::JoinCoop => {
                // Hero first, like the host: the joiner plays whoever it picks, and the host
                // seats (and every client draws) that hero. The pick comes back here with
                // the address entry open.
                selected.joining = true;
                selected.daily = false;
                next.set(AppState::CharSelect);
                sfx.write(SfxMsg(Sfx::Click));
                return;
            }
            MenuBtn::JoinCancel => {
                join_open.0 = false;
                selected.joining = false;
            }
            MenuBtn::JoinConfirm => {
                let a = addr.0.clone();
                try_join(&mut commands, channels, &a, selected.character, coop_note, join_open, **role);
            }
            MenuBtn::Launch => {
                selected.daily = false;
                selected.joining = false;
                next.set(AppState::CharSelect);
                sfx.write(SfxMsg(Sfx::Click));
                return;
            }
            MenuBtn::Daily => {
                // daily = fixed Moon T1 on today's shared seed; player still picks a hero
                selected.daily = true;
                selected.joining = false;
                selected.planet = crate::content::planets::PlanetKind::Moon;
                selected.tier = 1;
                next.set(AppState::CharSelect);
                sfx.write(SfxMsg(Sfx::Click));
                return;
            }
            MenuBtn::Settings => {
                settings_open.0 = true;
                sfx.write(SfxMsg(Sfx::Click));
                return;
            }
            MenuBtn::Quit => {
                exit.write(AppExit::Success);
            }
            MenuBtn::Tomes => {
                *tab = if *tab == MenuTab::Tomes { MenuTab::None } else { MenuTab::Tomes };
                changed = true;
            }
            MenuBtn::Quests => {
                *tab = if *tab == MenuTab::Quests { MenuTab::None } else { MenuTab::Quests };
                changed = true;
            }
        }
    }
    for (i, p) in &plus {
        if *i == Interaction::Pressed {
            let lvl = save.tome_level(p.0);
            let cost = p.0.cost(lvl);
            if lvl < p.0.def().max_level && save.silver >= cost {
                save.silver -= cost;
                *save.tome_levels.entry(p.0).or_insert(0) += 1;
                save.save();
                sfx.write(SfxMsg(Sfx::Coin));
                changed = true;
            }
        }
    }
    for (i, t) in &toggles {
        if *i == Interaction::Pressed {
            if let Some(idx) = save.tome_loadout.iter().position(|x| *x == t.0) {
                save.tome_loadout.remove(idx);
            } else if (save.tome_loadout.len() as u32) < save.tome_slots {
                save.tome_loadout.push(t.0);
            }
            save.save();
            sfx.write(SfxMsg(Sfx::Click));
            changed = true;
        }
    }

    if !changed && !*dirty {
        return;
    }
    *dirty = false;

    // rebuild side panel
    let Ok(panel_e) = panel.single() else { return };
    commands.entity(panel_e).despawn_related::<Children>();
    match *tab {
        MenuTab::None => {}
        MenuTab::Tomes => {
            commands.entity(panel_e).with_children(|c| {
                c.spawn(txt(
                    format!("TOMES — loadout {}/{} (click name to equip)", save.tome_loadout.len(), save.tome_slots),
                    FONT_MED,
                    Color::srgb(0.7, 0.7, 1.0),
                ));
                for t in TomeKind::ALL {
                    let d = t.def();
                    let lvl = save.tome_level(t);
                    let equipped = save.tome_loadout.contains(&t);
                    c.spawn((Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() },))
                        .with_children(|row| {
                            row.spawn((
                                TomeToggle(t),
                                Button,
                                Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)), ..default() },
                                BackgroundColor(if equipped { Color::srgba(0.2, 0.5, 0.3, 1.0) } else { BTN_BG }),
                            ))
                            .with_children(|b| {
                                b.spawn(txt(
                                    format!("{} {} Lv{}", if equipped { "[x]" } else { "[ ]" }, d.name, lvl),
                                    FONT_SMALL,
                                    Color::WHITE,
                                ));
                            });
                            row.spawn(txt(d.desc, FONT_SMALL, Color::srgb(0.6, 0.65, 0.8)));
                            if lvl < d.max_level {
                                row.spawn((
                                    TomePlus(t),
                                    Button,
                                    Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)), ..default() },
                                    BackgroundColor(BTN_BG),
                                ))
                                .with_children(|b| {
                                    b.spawn(txt(format!("+1 ({}s)", t.cost(lvl)), FONT_SMALL, Color::srgb(0.75, 0.85, 1.0)));
                                });
                            }
                        });
                }
            });
        }
        MenuTab::Quests => {
            commands.entity(panel_e).with_children(|c| {
                c.spawn(txt("QUESTS", FONT_MED, Color::srgb(1.0, 0.85, 0.4)));
                for q in QuestKind::ALL {
                    let d = q.def();
                    let done = save.quests_done.contains(&q);
                    let progress = save
                        .quest_progress(q)
                        .map(|(a, b)| format!(" ({a}/{b})"))
                        .unwrap_or_default();
                    c.spawn(txt(
                        format!("{} {} — {}{}", if done { "[DONE]" } else { "[    ]" }, d.name, d.desc, progress),
                        FONT_SMALL,
                        if done { Color::srgb(0.45, 0.9, 0.5) } else { Color::srgb(0.75, 0.78, 0.9) },
                    ));
                }
            });
        }
    }
}

// ------------------------------------------------------------- char select

pub fn spawn_char_select(mut commands: Commands, save: Res<MetaSave>, selected: Res<Selected>) {
    commands
        .spawn((MenuRoot, overlay_root(), BackgroundColor(Color::srgb(0.02, 0.02, 0.05))))
        .with_children(|root| {
            root.spawn(txt(
                if selected.joining { "CHOOSE YOUR ASTRONAUT (JOINING CO-OP)" } else { "CHOOSE YOUR ASTRONAUT" },
                FONT_BIG,
                Color::WHITE,
            ));
            root.spawn((Node { column_gap: Val::Px(12.0), flex_wrap: FlexWrap::Wrap, justify_content: JustifyContent::Center, row_gap: Val::Px(12.0), ..default() },))
                .with_children(|row| {
                    for c in AstronautKind::ALL {
                        let d = c.def();
                        let unlocked = save.unlocked_chars.contains(&c);
                        let is_sel = selected.character == c;
                        let mut card = row.spawn((
                            Node {
                                width: Val::Px(190.0),
                                padding: UiRect::all(Val::Px(12.0)),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(6.0),
                                border: UiRect::all(Val::Px(3.0)),
                                border_radius: BorderRadius::all(Val::Px(8.0)),
                                ..default()
                            },
                            BackgroundColor(if unlocked { CARD_BG } else { Color::srgba(0.04, 0.04, 0.05, 0.95) }),
                            BorderColor::all(if is_sel {
                                Color::srgb(0.4, 1.0, 0.6)
                            } else if unlocked {
                                d.suit
                            } else {
                                Color::srgb(0.25, 0.25, 0.3)
                            }),
                        ));
                        if unlocked {
                            card.insert((Button, CharCard(c)));
                            card.with_children(|cc| {
                                cc.spawn(txt(d.name, FONT_MED, d.visor));
                                cc.spawn(txt(d.agency, FONT_SMALL, Color::srgb(0.6, 0.65, 0.8)));
                                cc.spawn(txt(d.desc, FONT_SMALL, Color::srgb(0.8, 0.82, 0.9)));
                                cc.spawn(txt(format!("Weapon: {}", d.weapon.def().name), FONT_SMALL, d.weapon.def().color));
                                cc.spawn(txt(d.passive_desc, FONT_SMALL, Color::srgb(0.5, 1.0, 0.7)));
                            });
                        } else {
                            card.with_children(|cc| {
                                cc.spawn(txt("???", FONT_MED, Color::srgb(0.4, 0.4, 0.5)));
                                cc.spawn(txt(d.unlock_desc, FONT_SMALL, Color::srgb(0.55, 0.55, 0.65)));
                            });
                        }
                    }
                });
            root.spawn((BackBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.6, 0.6, 0.7))))
                .with_children(|b| {
                    b.spawn(txt("BACK", FONT_MED, Color::WHITE));
                });
        });
}

#[allow(clippy::too_many_arguments)]
pub fn char_select_input(
    cards: Query<(&Interaction, &CharCard), Changed<Interaction>>,
    back: Query<&Interaction, (Changed<Interaction>, With<BackBtn>)>,
    mut selected: ResMut<Selected>,
    save: Res<MetaSave>,
    mut run: ResMut<RunState>,
    mut next: ResMut<NextState<AppState>>,
    mut sfx: MessageWriter<SfxMsg>,
    mut join_open: ResMut<JoinOpen>,
    role: Res<crate::net::NetRole>,
) {
    for (i, c) in &cards {
        if *i == Interaction::Pressed {
            selected.character = c.0;
            if *role == crate::net::NetRole::Client {
                // Backstop for main_menu_input's LAUNCH/DAILY guard: a connected joiner
                // plays the host's run and never starts one of its own.
                next.set(AppState::MainMenu);
            } else if selected.joining {
                // A fresh sheet in the picked hero; the seed, world and clock all arrive
                // from the host's snapshots once connected, but the hero is ours.
                *run = RunState::new(c.0, crate::content::planets::PlanetKind::Moon, 1, &save);
                join_open.0 = true;
                next.set(AppState::MainMenu);
                sfx.write(SfxMsg(Sfx::Click));
            } else if selected.daily {
                // build the daily run directly: today's shared seed, fixed Moon T1
                *run = RunState::new(c.0, crate::content::planets::PlanetKind::Moon, 1, &save);
                run.run_seed = crate::run::daily_seed(crate::run::today());
                run.is_daily = true;
                next.set(AppState::InRun);
                sfx.write(SfxMsg(Sfx::Teleport));
            } else {
                next.set(AppState::PlanetSelect);
                sfx.write(SfxMsg(Sfx::Click));
            }
        }
    }
    for i in &back {
        if *i == Interaction::Pressed {
            selected.joining = false;
            next.set(AppState::MainMenu);
            sfx.write(SfxMsg(Sfx::Click));
        }
    }
}

// ------------------------------------------------------------- planet select

pub fn spawn_planet_select(mut commands: Commands, save: Res<MetaSave>) {
    commands
        .spawn((MenuRoot, overlay_root(), BackgroundColor(Color::srgb(0.02, 0.02, 0.05))))
        .with_children(|root| {
            root.spawn(txt("PICK A WORLD TO SAVE", FONT_BIG, Color::WHITE));
            root.spawn((Node { column_gap: Val::Px(16.0), ..default() },)).with_children(|row| {
                for p in [PlanetKind::Moon, PlanetKind::Mars] {
                    let d = p.def();
                    let unlocked = save.unlocked_planets.contains(&p);
                    row.spawn((
                        Node {
                            width: Val::Px(260.0),
                            padding: UiRect::all(Val::Px(14.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(8.0),
                            border: UiRect::all(Val::Px(3.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(if unlocked { CARD_BG } else { Color::srgba(0.04, 0.04, 0.05, 0.95) }),
                        BorderColor::all(if unlocked { d.ground } else { Color::srgb(0.25, 0.25, 0.3) }),
                    ))
                    .with_children(|card| {
                        if unlocked {
                            card.spawn(txt(d.name, FONT_MED, d.ground_high));
                            card.spawn(txt(d.desc, FONT_SMALL, Color::srgb(0.8, 0.82, 0.9)));
                            card.spawn((Node { column_gap: Val::Px(8.0), ..default() },)).with_children(|tiers| {
                                for tier in 1..=PlanetKind::max_tier(p) {
                                    // tier N unlocked when tier N-1 of this planet is cleared
                                    let tier_ok = tier == 1 || save.counters.cleared.contains(&(p, tier - 1));
                                    let mut b = tiers.spawn((
                                        Node {
                                            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                                            border: UiRect::all(Val::Px(2.0)),
                                            border_radius: BorderRadius::all(Val::Px(4.0)),
                                            ..default()
                                        },
                                        BackgroundColor(if tier_ok { BTN_BG } else { Color::srgba(0.05, 0.05, 0.06, 1.0) }),
                                        BorderColor::all(if tier_ok { Color::srgb(0.4, 1.0, 0.6) } else { Color::srgb(0.3, 0.3, 0.35) }),
                                    ));
                                    if tier_ok {
                                        b.insert((Button, PlanetCard(p, tier)));
                                    }
                                    b.with_children(|bb| {
                                        bb.spawn(txt(
                                            format!("TIER {tier}"),
                                            FONT_SMALL,
                                            if tier_ok { Color::WHITE } else { Color::srgb(0.4, 0.4, 0.5) },
                                        ));
                                    });
                                }
                            });
                            let chain: Vec<&str> = PlanetKind::chain_from(p, PlanetKind::max_tier(p))
                                .iter()
                                .map(|x| x.def().name)
                                .collect();
                            card.spawn(txt(format!("T3 route: {}", chain.join(" > ")), FONT_SMALL, Color::srgb(0.55, 0.6, 0.75)));
                        } else {
                            card.spawn(txt("???", FONT_MED, Color::srgb(0.4, 0.4, 0.5)));
                            card.spawn(txt("Clear MOON Tier 2 to chart this world", FONT_SMALL, Color::srgb(0.55, 0.55, 0.65)));
                        }
                    });
                }
            });
            root.spawn((BackBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.6, 0.6, 0.7))))
                .with_children(|b| {
                    b.spawn(txt("BACK", FONT_MED, Color::WHITE));
                });
        });
}

pub fn planet_select_input(
    cards: Query<(&Interaction, &PlanetCard), Changed<Interaction>>,
    back: Query<&Interaction, (Changed<Interaction>, With<BackBtn>)>,
    mut selected: ResMut<Selected>,
    save: Res<MetaSave>,
    mut run: ResMut<RunState>,
    mut next: ResMut<NextState<AppState>>,
    mut sfx: MessageWriter<SfxMsg>,
    role: Res<crate::net::NetRole>,
) {
    for (i, c) in &cards {
        if *i == Interaction::Pressed {
            if *role == crate::net::NetRole::Client {
                // Same backstop as char_select_input: the host picks a joiner's world.
                next.set(AppState::MainMenu);
                continue;
            }
            selected.planet = c.0;
            selected.tier = c.1;
            *run = RunState::new(selected.character, selected.planet, selected.tier, &save);
            next.set(AppState::InRun);
            sfx.write(SfxMsg(Sfx::Teleport));
        }
    }
    for i in &back {
        if *i == Interaction::Pressed {
            next.set(AppState::CharSelect);
            sfx.write(SfxMsg(Sfx::Click));
        }
    }
}

// ------------------------------------------------------------- results

pub fn spawn_results(mut commands: Commands, data: Res<ResultsData>) {
    commands
        .spawn((MenuRoot, overlay_root(), BackgroundColor(Color::srgb(0.02, 0.02, 0.05))))
        .with_children(|root| {
            if data.victory {
                root.spawn(txt("PLANET SAVED", 56.0, Color::srgb(0.4, 1.0, 0.6)));
            } else {
                root.spawn(txt("YOU GOT BONKED", 56.0, Color::srgb(1.0, 0.35, 0.3)));
            }
            let m = (data.time / 60.0) as u32;
            let s = (data.time % 60.0) as u32;
            root.spawn(txt(
                format!(
                    "BONKS {}   LEVEL {}   GOLD {}   TIME {m}:{s:02}",
                    data.kills, data.level, data.gold
                ),
                FONT_MED,
                Color::srgb(0.85, 0.87, 0.95),
            ));
            root.spawn(txt(format!("SILVER EARNED: +{}", data.silver_earned), FONT_BIG, Color::srgb(0.75, 0.85, 1.0)));
            // The §10 formula, term by term.
            if !data.silver_lines.is_empty() {
                root.spawn((Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Px(440.0),
                    row_gap: Val::Px(2.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                }, BackgroundColor(CARD_BG), BorderColor::all(Color::srgb(0.35, 0.42, 0.6))))
                    .with_children(|col| {
                        for (label, amount) in &data.silver_lines {
                            col.spawn((Node { width: Val::Percent(100.0), justify_content: JustifyContent::SpaceBetween, ..default() },))
                                .with_children(|row| {
                                    row.spawn(txt(label.clone(), FONT_SMALL, Color::srgb(0.7, 0.75, 0.88)));
                                    let c = if amount.starts_with('x') { Color::srgb(1.0, 0.8, 0.4) } else { Color::srgb(0.75, 0.85, 1.0) };
                                    row.spawn(txt(amount.clone(), FONT_SMALL, c));
                                });
                        }
                    });
            }
            if let Some((name, best, new_best)) = &data.daily {
                root.spawn(txt(
                    format!("DAILY {name} — score {}   (best today: {best})", data.silver_earned),
                    FONT_MED,
                    Color::srgb(1.0, 0.6, 0.85),
                ));
                if *new_best {
                    root.spawn(txt("NEW DAILY BEST!", FONT_MED, Color::srgb(1.0, 0.85, 0.3)));
                }
            }
            for q in &data.quests_completed {
                root.spawn(txt(format!("QUEST COMPLETE: {q}"), FONT_SMALL, Color::srgb(1.0, 0.85, 0.4)));
            }
            root.spawn((ContinueBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.4, 1.0, 0.6))))
                .with_children(|b| {
                    b.spawn(txt("[SPACE] CONTINUE", FONT_MED, Color::WHITE));
                });
        });
}

pub fn results_input(
    keys: Res<ButtonInput<KeyCode>>,
    cont: Query<&Interaction, (Changed<Interaction>, With<ContinueBtn>)>,
    mut next: ResMut<NextState<AppState>>,
) {
    let mut go = keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter);
    for i in &cont {
        if *i == Interaction::Pressed {
            go = true;
        }
    }
    if go {
        next.set(AppState::MainMenu);
    }
}

/// Build / tear down the address entry, and show the co-op status line.
pub fn join_panel_sync(
    mut commands: Commands,
    open: Res<JoinOpen>,
    addr: Res<JoinAddr>,
    note: Res<CoopNote>,
    selected: Res<Selected>,
    panel: Query<Entity, With<JoinPanel>>,
    mut addr_text: Query<&mut Text, (With<JoinAddrText>, Without<CoopNoteText>)>,
    mut note_text: Query<&mut Text, (With<CoopNoteText>, Without<JoinAddrText>)>,
) {
    // status line under the co-op buttons
    if let Ok(mut t) = note_text.single_mut() {
        if t.0 != note.0 {
            t.0 = note.0.clone();
        }
    }

    let shown = !panel.is_empty();
    if open.0 && !shown {
        commands
            .spawn((
                JoinPanel,
                overlay_root(),
                BackgroundColor(Color::srgba(0.02, 0.02, 0.06, 0.94)),
            ))
            .with_children(|root| {
                root.spawn(txt("JOIN CO-OP", FONT_BIG, Color::srgb(0.4, 0.9, 1.0)));
                let hero = selected.character.def();
                root.spawn(txt(format!("joining as {}", hero.name), FONT_MED, hero.visor));
                root.spawn(txt(
                    "type the host's IP address, then ENTER",
                    FONT_MED,
                    Color::srgb(0.6, 0.65, 0.8),
                ));
                root.spawn((JoinAddrText, txt(addr.0.clone(), FONT_BIG, Color::WHITE)));
                root.spawn(txt(
                    "same machine: 127.0.0.1   |   same house: the host's local IP   |   ESC to cancel",
                    FONT_SMALL,
                    Color::srgb(0.5, 0.55, 0.7),
                ));
                root.spawn((Node { column_gap: Val::Px(10.0), ..default() },)).with_children(|row| {
                    row.spawn((
                        MenuBtn::JoinConfirm,
                        Button,
                        button_node(),
                        BackgroundColor(BTN_BG),
                        BorderColor::all(Color::srgb(0.4, 1.0, 0.6)),
                    ))
                    .with_children(|b| {
                        b.spawn(txt("CONNECT", FONT_MED, Color::WHITE));
                    });
                    row.spawn((
                        MenuBtn::JoinCancel,
                        Button,
                        button_node(),
                        BackgroundColor(BTN_BG),
                        BorderColor::all(Color::srgb(1.0, 0.5, 0.5)),
                    ))
                    .with_children(|b| {
                        b.spawn(txt("CANCEL", FONT_MED, Color::WHITE));
                    });
                });
            });
    } else if !open.0 && shown {
        for e in &panel {
            commands.entity(e).despawn();
        }
    }

    if open.0 {
        if let Ok(mut t) = addr_text.single_mut() {
            if t.0 != addr.0 {
                t.0 = addr.0.clone();
            }
        }
    }
}

/// Shared by the CONNECT button and the ENTER key, so the two cannot drift apart.
pub fn try_join(
    commands: &mut Commands,
    channels: &bevy_replicon::prelude::RepliconChannels,
    addr: &str,
    // the hero picked for this join: the connect token names it, and the host seats it
    hero: AstronautKind,
    note: &mut CoopNote,
    open: &mut JoinOpen,
    role: crate::net::NetRole,
) {
    if role.is_networked() {
        note.0 = "already in a co-op session: stop it first with the same button".into();
        open.0 = false;
        return;
    }
    match addr.trim().parse::<std::net::IpAddr>() {
        Ok(ip) => match crate::net::start_join(commands, channels, ip, crate::net::DEFAULT_PORT, hero) {
            Ok(()) => {
                // Deliberately NO state change: a client must not build a world from its own
                // seed. `client_follow_host_run` enters the run once the host's snapshot lands.
                note.0 = format!("CONNECTING to {ip}: waiting for the host's world...   JOIN CO-OP again cancels");
                open.0 = false;
            }
            Err(e) => note.0 = format!("JOIN FAILED: {e}"),
        },
        Err(_) => note.0 = format!("'{}' is not a valid IP address", addr.trim()),
    }
}

/// Type an address. Digits and dots only — this is an IP field, so filtering here is
/// simpler than validating a mess later, and it makes a typo impossible to submit.
pub fn join_addr_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<JoinOpen>,
    mut addr: ResMut<JoinAddr>,
    mut note: ResMut<CoopNote>,
    channels: Res<bevy_replicon::prelude::RepliconChannels>,
    role: Res<crate::net::NetRole>,
    mut selected: ResMut<Selected>,
) {
    if !open.0 {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        open.0 = false;
        selected.joining = false;
        return;
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        let a = addr.0.clone();
        try_join(&mut commands, &channels, &a, selected.character, &mut note, &mut open, *role);
        return;
    }
    for k in keys.get_just_pressed() {
        let ch = match k {
            KeyCode::Digit0 | KeyCode::Numpad0 => Some('0'),
            KeyCode::Digit1 | KeyCode::Numpad1 => Some('1'),
            KeyCode::Digit2 | KeyCode::Numpad2 => Some('2'),
            KeyCode::Digit3 | KeyCode::Numpad3 => Some('3'),
            KeyCode::Digit4 | KeyCode::Numpad4 => Some('4'),
            KeyCode::Digit5 | KeyCode::Numpad5 => Some('5'),
            KeyCode::Digit6 | KeyCode::Numpad6 => Some('6'),
            KeyCode::Digit7 | KeyCode::Numpad7 => Some('7'),
            KeyCode::Digit8 | KeyCode::Numpad8 => Some('8'),
            KeyCode::Digit9 | KeyCode::Numpad9 => Some('9'),
            KeyCode::Period | KeyCode::NumpadDecimal => Some('.'),
            _ => None,
        };
        if let Some(c) = ch {
            if addr.0.len() < 15 {
                addr.0.push(c);
            }
        }
        if matches!(k, KeyCode::Backspace) {
            addr.0.pop();
        }
    }
}
