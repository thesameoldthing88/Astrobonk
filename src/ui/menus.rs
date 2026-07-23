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
}

impl Default for Selected {
    fn default() -> Self {
        Self { character: AstronautKind::Buzz, planet: PlanetKind::Moon, tier: 1 }
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

            root.spawn((LaunchBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.4, 1.0, 0.6))))
                .with_children(|b| {
                    b.spawn(txt("LAUNCH", FONT_BIG, Color::WHITE));
                });
            root.spawn((Node { column_gap: Val::Px(10.0), ..default() },)).with_children(|row| {
                row.spawn((TomesBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.6, 0.6, 1.0))))
                    .with_children(|b| {
                        b.spawn(txt("TOMES", FONT_MED, Color::WHITE));
                    });
                row.spawn((QuestsBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(1.0, 0.8, 0.4))))
                    .with_children(|b| {
                        b.spawn(txt("QUESTS", FONT_MED, Color::WHITE));
                    });
                row.spawn((QuitBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.7, 0.4, 0.4))))
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
    launch: Query<&Interaction, (Changed<Interaction>, With<LaunchBtn>)>,
    tomes: Query<&Interaction, (Changed<Interaction>, With<TomesBtn>)>,
    quests: Query<&Interaction, (Changed<Interaction>, With<QuestsBtn>)>,
    quit: Query<&Interaction, (Changed<Interaction>, With<QuitBtn>)>,
    plus: Query<(&Interaction, &TomePlus), Changed<Interaction>>,
    toggles: Query<(&Interaction, &TomeToggle), Changed<Interaction>>,
    mut tab: ResMut<MenuTab>,
    mut save: ResMut<MetaSave>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
    mut sfx: MessageWriter<SfxMsg>,
    panel: Query<Entity, With<SidePanel>>,
    mut dirty: Local<bool>,
) {
    for i in &launch {
        if *i == Interaction::Pressed {
            next.set(AppState::CharSelect);
            sfx.write(SfxMsg(Sfx::Click));
            return;
        }
    }
    for i in &quit {
        if *i == Interaction::Pressed {
            exit.write(AppExit::Success);
        }
    }
    let mut changed = false;
    for i in &tomes {
        if *i == Interaction::Pressed {
            *tab = if *tab == MenuTab::Tomes { MenuTab::None } else { MenuTab::Tomes };
            changed = true;
        }
    }
    for i in &quests {
        if *i == Interaction::Pressed {
            *tab = if *tab == MenuTab::Quests { MenuTab::None } else { MenuTab::Quests };
            changed = true;
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
            root.spawn(txt("CHOOSE YOUR ASTRONAUT", FONT_BIG, Color::WHITE));
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

pub fn char_select_input(
    cards: Query<(&Interaction, &CharCard), Changed<Interaction>>,
    back: Query<&Interaction, (Changed<Interaction>, With<BackBtn>)>,
    mut selected: ResMut<Selected>,
    mut next: ResMut<NextState<AppState>>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    for (i, c) in &cards {
        if *i == Interaction::Pressed {
            selected.character = c.0;
            next.set(AppState::PlanetSelect);
            sfx.write(SfxMsg(Sfx::Click));
        }
    }
    for i in &back {
        if *i == Interaction::Pressed {
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
) {
    for (i, c) in &cards {
        if *i == Interaction::Pressed {
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
