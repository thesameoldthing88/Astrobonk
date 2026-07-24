//! Modal panels: level-up / shrine / moai / microwave choice cards,
//! chest reveal, Shady Guy shop, pause.

use super::*;
use crate::interact::{ChestPanel, Interactable, ShopPanel};
use crate::messages::{BannerMsg, Sfx, SfxMsg};
use crate::run::{roll_upgrades, ChoicePanel, RunPhase, RunResult, RunState, UpgradeOption};
use crate::save::MetaSave;
use bevy::prelude::*;

#[derive(Component)]
pub struct ChoiceRoot;
#[derive(Component)]
pub struct ChoiceCard(pub usize);
#[derive(Component)]
pub struct RefreshBtn;
#[derive(Component)]
pub struct BanishBtn;
#[derive(Component)]
pub struct SkipBtn;

#[derive(Component)]
pub struct ChestRoot;
#[derive(Component)]
pub struct ChestTake;
#[derive(Component)]
pub struct ChestLeave;

#[derive(Component)]
pub struct ShopRoot;
#[derive(Component)]
pub struct ShopBuy(pub usize);
#[derive(Component)]
pub struct ShopClose;

#[derive(Component)]
pub struct PauseRoot;
#[derive(Component)]
pub struct ResumeBtn;
#[derive(Component)]
pub struct AbandonBtn;
#[derive(Component)]
pub struct PauseSettingsBtn;

/// (Re)build the choice panel whenever its contents change.
pub fn sync_choice_panel(
    mut commands: Commands,
    phase: Res<RunPhase>,
    panel: Res<ChoicePanel>,
    run: Res<RunState>,
    q_root: Query<Entity, With<ChoiceRoot>>,
) {
    let should_show = matches!(*phase, RunPhase::LevelUp) || (matches!(*phase, RunPhase::Modal) && !panel.options.is_empty());
    if !should_show {
        for e in &q_root {
            commands.entity(e).despawn();
        }
        return;
    }
    if !panel.is_changed() && !q_root.is_empty() {
        return;
    }
    for e in &q_root {
        commands.entity(e).despawn();
    }

    commands
        .spawn((ChoiceRoot, overlay_root(), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)), GlobalZIndex(10)))
        .with_children(|root| {
            let title_color = if panel.banishing { Color::srgb(1.0, 0.4, 0.4) } else { Color::srgb(0.6, 1.0, 0.8) };
            let title = if panel.banishing {
                format!("{} — PICK ONE TO BANISH", panel.title)
            } else {
                panel.title.clone()
            };
            root.spawn(txt(title, FONT_BIG, title_color));

            root.spawn((Node {
                column_gap: Val::Px(14.0),
                align_items: AlignItems::Stretch,
                ..default()
            },))
                .with_children(|row| {
                    for (i, opt) in panel.options.iter().enumerate() {
                        let rarity = opt.rarity();
                        row.spawn((
                            ChoiceCard(i),
                            Button,
                            Node {
                                width: Val::Px(220.0),
                                min_height: Val::Px(180.0),
                                padding: UiRect::all(Val::Px(12.0)),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(8.0),
                                border: UiRect::all(Val::Px(3.0)),
                                border_radius: BorderRadius::all(Val::Px(8.0)),
                                ..default()
                            },
                            BackgroundColor(CARD_BG),
                            BorderColor::all(rarity.color()),
                        ))
                        .with_children(|card| {
                            card.spawn(txt(rarity.name(), FONT_SMALL, rarity.color()));
                            card.spawn(txt(opt.title(), FONT_MED, Color::WHITE));
                            card.spawn(txt(opt.body(&run), FONT_SMALL, Color::srgb(0.8, 0.82, 0.9)));
                            card.spawn(txt(format!("[{}]", i + 1), FONT_SMALL, Color::srgb(0.5, 0.55, 0.7)));
                        });
                    }
                });

            if panel.is_levelup {
                root.spawn((Node { column_gap: Val::Px(10.0), ..default() },))
                    .with_children(|row| {
                        row.spawn((RefreshBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.4, 0.7, 1.0))))
                            .with_children(|b| {
                                b.spawn(txt(format!("[R]EFRESH ({})", run.refreshes), FONT_SMALL, Color::WHITE));
                            });
                        row.spawn((BanishBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(1.0, 0.4, 0.4))))
                            .with_children(|b| {
                                b.spawn(txt(format!("[B]ANISH ({})", run.banishes), FONT_SMALL, Color::WHITE));
                            });
                        row.spawn((SkipBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.7, 0.7, 0.7))))
                            .with_children(|b| {
                                b.spawn(txt("[S]KIP +10g", FONT_SMALL, Color::WHITE));
                            });
                    });
            }
        });
}

/// Handle clicks + number keys on the choice panel.
#[allow(clippy::too_many_arguments)]
pub fn choice_input(
    keys: Res<ButtonInput<KeyCode>>,
    cards: Query<(&Interaction, &ChoiceCard), Changed<Interaction>>,
    refresh: Query<&Interaction, (Changed<Interaction>, With<RefreshBtn>)>,
    banish: Query<&Interaction, (Changed<Interaction>, With<BanishBtn>)>,
    skip: Query<&Interaction, (Changed<Interaction>, With<SkipBtn>)>,
    mut panel: ResMut<ChoicePanel>,
    mut run: ResMut<RunState>,
    save: Res<MetaSave>,
    mut phase: ResMut<RunPhase>,
    mut sfx: MessageWriter<SfxMsg>,
    mut banners: MessageWriter<BannerMsg>,
    mut hitstop: ResMut<crate::fx::Hitstop>,
) {
    if !matches!(*phase, RunPhase::LevelUp | RunPhase::Modal) || panel.options.is_empty() {
        return;
    }

    let mut pick: Option<usize> = None;
    for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4].iter().enumerate() {
        if keys.just_pressed(*key) && i < panel.options.len() {
            pick = Some(i);
        }
    }
    for (interaction, card) in &cards {
        if *interaction == Interaction::Pressed {
            pick = Some(card.0);
        }
    }

    let mut do_refresh = keys.just_pressed(KeyCode::KeyR);
    for i in &refresh {
        if *i == Interaction::Pressed {
            do_refresh = true;
        }
    }
    let mut do_banish = keys.just_pressed(KeyCode::KeyB);
    for i in &banish {
        if *i == Interaction::Pressed {
            do_banish = true;
        }
    }
    let mut do_skip = keys.just_pressed(KeyCode::KeyS);
    for i in &skip {
        if *i == Interaction::Pressed {
            do_skip = true;
        }
    }

    if panel.is_levelup {
        if do_refresh && run.refreshes > 0 {
            run.refreshes -= 1;
            let mut rng = rand::thread_rng();
            panel.options = roll_upgrades(&run, &save, &mut rng);
            panel.banishing = false;
            sfx.write(SfxMsg(Sfx::Click));
            return;
        }
        if do_banish && run.banishes > 0 && !panel.banishing {
            panel.banishing = true;
            sfx.write(SfxMsg(Sfx::Click));
            return;
        }
        if do_skip {
            run.gold += 10;
            finish_choice(&mut run, &mut panel, &mut phase, &save);
            sfx.write(SfxMsg(Sfx::Click));
            return;
        }
    }

    let Some(idx) = pick else { return };
    if idx >= panel.options.len() {
        return;
    }

    if panel.banishing {
        run.banishes = run.banishes.saturating_sub(1);
        let opt = panel.options.remove(idx);
        if let UpgradeOption::NewItem(i) | UpgradeOption::ItemUp(i) = opt {
            run.banned_items.insert(i);
        }
        panel.banishing = false;
        sfx.write(SfxMsg(Sfx::Click));
        return;
    }

    let opt = panel.options[idx].clone();
    let evolved = run.apply_upgrade(&opt, &save);
    if evolved {
        banners.write(BannerMsg("WEAPON EVOLVED".into()));
        sfx.write(SfxMsg(Sfx::Evolve));
        hitstop.timer = 0.18;
    } else {
        sfx.write(SfxMsg(Sfx::Click));
    }
    finish_choice(&mut run, &mut panel, &mut phase, &save);
}

fn finish_choice(run: &mut RunState, panel: &mut ChoicePanel, phase: &mut RunPhase, save: &MetaSave) {
    if panel.is_levelup {
        run.pending_levelups = run.pending_levelups.saturating_sub(1);
        if run.pending_levelups > 0 {
            let mut rng = rand::thread_rng();
            panel.options = roll_upgrades(run, save, &mut rng);
            panel.title = format!("LEVEL {}", run.level);
            return;
        }
    }
    panel.options.clear();
    *phase = RunPhase::Playing;
}

/// Chest reveal panel lifecycle.
#[allow(clippy::too_many_arguments)]
pub fn chest_panel(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut chest: ResMut<ChestPanel>,
    mut run: ResMut<RunState>,
    save: Res<MetaSave>,
    mut phase: ResMut<RunPhase>,
    q_root: Query<Entity, With<ChestRoot>>,
    take: Query<&Interaction, (Changed<Interaction>, With<ChestTake>)>,
    leave: Query<&Interaction, (Changed<Interaction>, With<ChestLeave>)>,
    mut q_inter: Query<&mut Interactable>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    if !chest.open {
        for e in &q_root {
            commands.entity(e).despawn();
        }
        return;
    }
    if q_root.is_empty() {
        let Some(item) = chest.item else { return };
        let d = item.def();
        let cost = chest.cost;
        commands
            .spawn((ChestRoot, overlay_root(), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)), GlobalZIndex(10)))
            .with_children(|root| {
                root.spawn(txt("CHEST", FONT_BIG, Color::srgb(1.0, 0.8, 0.3)));
                root.spawn((
                    Node {
                        width: Val::Px(260.0),
                        padding: UiRect::all(Val::Px(14.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        border: UiRect::all(Val::Px(3.0)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(CARD_BG),
                    BorderColor::all(d.rarity.color()),
                ))
                .with_children(|card| {
                    card.spawn(txt(d.rarity.name(), FONT_SMALL, d.rarity.color()));
                    card.spawn(txt(d.name, FONT_MED, Color::WHITE));
                    card.spawn(txt(d.desc, FONT_SMALL, Color::srgb(0.8, 0.82, 0.9)));
                    let stats: Vec<String> = d.boosts.iter().map(|(k, v)| k.label(*v)).collect();
                    card.spawn(txt(stats.join(", "), FONT_SMALL, Color::srgb(0.6, 1.0, 0.7)));
                });
                root.spawn((Node { column_gap: Val::Px(10.0), ..default() },))
                    .with_children(|row| {
                        row.spawn((ChestTake, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.4, 1.0, 0.6))))
                            .with_children(|b| {
                                b.spawn(txt(format!("[1] TAKE (-{cost}g)"), FONT_SMALL, Color::WHITE));
                            });
                        row.spawn((ChestLeave, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.7, 0.7, 0.7))))
                            .with_children(|b| {
                                b.spawn(txt("[2] LEAVE", FONT_SMALL, Color::WHITE));
                            });
                    });
            });
        return;
    }

    let mut do_take = keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::KeyE);
    for i in &take {
        if *i == Interaction::Pressed {
            do_take = true;
        }
    }
    let mut do_leave = keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Escape);
    for i in &leave {
        if *i == Interaction::Pressed {
            do_leave = true;
        }
    }

    if do_take {
        if let Some(item) = chest.item {
            if run.gold >= chest.cost {
                run.gold -= chest.cost;
                if run.item_count(item) == 0 {
                    run.items.push((item, 1));
                } else if let Some(e) = run.items.iter_mut().find(|(k, _)| *k == item) {
                    e.1 += 1;
                }
                run.chest_opens += 1;
                run.chests_opened += 1;
                let save_c = save.clone();
                run.recompute_stats(&save_c);
                if let Some(ent) = chest.chest {
                    if let Ok(mut i) = q_inter.get_mut(ent) {
                        i.used = true;
                    }
                }
                sfx.write(SfxMsg(Sfx::Chest));
            }
        }
        chest.open = false;
        *phase = RunPhase::Playing;
    } else if do_leave {
        chest.open = false;
        *phase = RunPhase::Playing;
        sfx.write(SfxMsg(Sfx::Click));
    }
}

/// Shady Guy shop panel lifecycle.
#[allow(clippy::too_many_arguments)]
pub fn shop_panel(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut shop: ResMut<ShopPanel>,
    mut run: ResMut<RunState>,
    save: Res<MetaSave>,
    mut phase: ResMut<RunPhase>,
    q_root: Query<Entity, With<ShopRoot>>,
    buys: Query<(&Interaction, &ShopBuy), Changed<Interaction>>,
    closes: Query<&Interaction, (Changed<Interaction>, With<ShopClose>)>,
    mut q_inter: Query<&mut Interactable>,
    mut sfx: MessageWriter<SfxMsg>,
    mut rebuild: Local<bool>,
) {
    if !shop.open {
        for e in &q_root {
            commands.entity(e).despawn();
        }
        return;
    }
    if q_root.is_empty() || *rebuild {
        *rebuild = false;
        for e in &q_root {
            commands.entity(e).despawn();
        }
        let offers = shop.offers.clone();
        let gold = run.gold;
        commands
            .spawn((ShopRoot, overlay_root(), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)), GlobalZIndex(10)))
            .with_children(|root| {
                root.spawn(txt("SHADY GUY", FONT_BIG, Color::srgb(0.8, 0.7, 1.0)));
                root.spawn(txt(format!("your gold: {gold}"), FONT_SMALL, Color::srgb(1.0, 0.85, 0.3)));
                root.spawn((Node { column_gap: Val::Px(12.0), ..default() },))
                    .with_children(|row| {
                        for (i, (item, price, sold)) in offers.iter().enumerate() {
                            let d = item.def();
                            let mut card = row.spawn((
                                Node {
                                    width: Val::Px(200.0),
                                    padding: UiRect::all(Val::Px(12.0)),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(6.0),
                                    border: UiRect::all(Val::Px(3.0)),
                                    border_radius: BorderRadius::all(Val::Px(8.0)),
                                    ..default()
                                },
                                BackgroundColor(if *sold { Color::srgba(0.05, 0.05, 0.06, 0.9) } else { CARD_BG }),
                                BorderColor::all(if *sold { Color::srgb(0.3, 0.3, 0.3) } else { d.rarity.color() }),
                            ));
                            card.with_children(|c| {
                                c.spawn(txt(d.rarity.name(), FONT_SMALL, d.rarity.color()));
                                c.spawn(txt(d.name, FONT_MED, Color::WHITE));
                                c.spawn(txt(d.desc, FONT_SMALL, Color::srgb(0.8, 0.82, 0.9)));
                                if *sold {
                                    c.spawn(txt("SOLD", FONT_MED, Color::srgb(0.6, 0.4, 0.4)));
                                }
                            });
                            if !*sold {
                                card.insert(Button).insert(ShopBuy(i)).with_children(|c| {
                                    c.spawn(txt(format!("[{}] BUY {price}g", i + 1), FONT_SMALL, Color::srgb(1.0, 0.85, 0.3)));
                                });
                            }
                        }
                    });
                root.spawn((ShopClose, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.7, 0.7, 0.7))))
                    .with_children(|b| {
                        b.spawn(txt("[E] WALK AWAY", FONT_SMALL, Color::WHITE));
                    });
            });
        return;
    }

    let mut buy_idx: Option<usize> = None;
    for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3].iter().enumerate() {
        if keys.just_pressed(*key) {
            buy_idx = Some(i);
        }
    }
    for (interaction, b) in &buys {
        if *interaction == Interaction::Pressed {
            buy_idx = Some(b.0);
        }
    }
    if let Some(i) = buy_idx {
        if i < shop.offers.len() {
            let (item, price, sold) = shop.offers[i];
            if !sold && run.gold >= price {
                run.gold -= price;
                if run.item_count(item) == 0 {
                    run.items.push((item, 1));
                } else if let Some(e) = run.items.iter_mut().find(|(k, _)| *k == item) {
                    e.1 += 1;
                }
                let save_c = save.clone();
                run.recompute_stats(&save_c);
                shop.offers[i].2 = true;
                if let Some(v) = shop.vendor {
                    if let Ok(mut inter) = q_inter.get_mut(v) {
                        if i < inter.stock.len() {
                            inter.stock[i].2 = true;
                        }
                    }
                }
                sfx.write(SfxMsg(Sfx::Coin));
                *rebuild = true;
            }
        }
    }

    let mut close = keys.just_pressed(KeyCode::KeyE) || keys.just_pressed(KeyCode::Escape);
    for i in &closes {
        if *i == Interaction::Pressed {
            close = true;
        }
    }
    if close {
        shop.open = false;
        *phase = RunPhase::Playing;
        sfx.write(SfxMsg(Sfx::Click));
    }
}

/// Escape toggles pause; pause menu buttons.
#[allow(clippy::too_many_arguments)]
pub fn pause_panel(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut phase: ResMut<RunPhase>,
    mut run: ResMut<RunState>,
    mut settings_open: ResMut<crate::ui::settings::SettingsOpen>,
    q_root: Query<Entity, With<PauseRoot>>,
    resume: Query<&Interaction, (Changed<Interaction>, With<ResumeBtn>)>,
    abandon: Query<&Interaction, (Changed<Interaction>, With<AbandonBtn>)>,
    psettings: Query<&Interaction, (Changed<Interaction>, With<PauseSettingsBtn>)>,
) {
    // While the settings overlay is up, it owns input (incl. ESC) — don't also resume.
    if settings_open.0 {
        return;
    }
    for i in &psettings {
        if *i == Interaction::Pressed {
            settings_open.0 = true;
            return;
        }
    }
    match *phase {
        RunPhase::Playing => {
            if keys.just_pressed(KeyCode::Escape) {
                *phase = RunPhase::Paused;
            }
            for e in &q_root {
                commands.entity(e).despawn();
            }
        }
        RunPhase::Paused => {
            if q_root.is_empty() {
                let stats = run.stats.clone();
                commands
                    .spawn((PauseRoot, overlay_root(), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)), GlobalZIndex(20)))
                    .with_children(|root| {
                        root.spawn(txt("PAUSED", FONT_BIG, Color::WHITE));
                        root.spawn(txt(
                            format!(
                                "DMG x{:.2}  AS x{:.2}  CRIT {:.0}%  SPD x{:.2}\nARMOR {:.0}  EVA {:.0}  LUCK +{:.0}%  DIFF +{:.0}%",
                                stats.damage,
                                stats.attack_speed,
                                stats.crit_chance * 100.0,
                                stats.move_speed,
                                stats.armor,
                                stats.evasion,
                                stats.luck * 100.0,
                                stats.difficulty * 100.0
                            ),
                            FONT_SMALL,
                            Color::srgb(0.8, 0.85, 0.95),
                        ));
                        root.spawn((ResumeBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.4, 1.0, 0.6))))
                            .with_children(|b| {
                                b.spawn(txt("[ESC] RESUME", FONT_MED, Color::WHITE));
                            });
                        root.spawn((PauseSettingsBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.5, 0.8, 1.0))))
                            .with_children(|b| {
                                b.spawn(txt("SETTINGS", FONT_MED, Color::WHITE));
                            });
                        root.spawn((AbandonBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(1.0, 0.4, 0.4))))
                            .with_children(|b| {
                                b.spawn(txt("ABANDON RUN", FONT_MED, Color::WHITE));
                            });
                    });
            }
            let mut do_resume = keys.just_pressed(KeyCode::Escape);
            for i in &resume {
                if *i == Interaction::Pressed {
                    do_resume = true;
                }
            }
            if do_resume {
                *phase = RunPhase::Playing;
            }
            for i in &abandon {
                if *i == Interaction::Pressed {
                    run.result = Some(RunResult::Death);
                    *phase = RunPhase::Dead;
                }
            }
        }
        _ => {}
    }
}
