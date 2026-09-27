//! Modal panels: level-up / shrine / moai / microwave choice cards,
//! chest reveal, Shady Guy shop, pause.

use super::*;
use crate::interact::{ChestPanel, Interactable, ShopPanel};
use crate::messages::{BannerMsg, Sfx, SfxMsg};
use crate::run::{roll_upgrades, ChoicePanel, PlayerState, RefreshPrice, RunPhase, RunResult, RunState};
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
/// Co-op only: LEAVE SESSION on a client, END SESSION on the host.
#[derive(Component)]
pub struct LeaveSessionBtn;

/// (Re)build the choice panel whenever its contents change.
pub fn sync_choice_panel(
    mut commands: Commands,
    phase: Res<RunPhase>,
    panel: Res<ChoicePanel>,
    q_run: Query<&PlayerState, With<crate::player::LocalPlayer>>,
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
    let Ok(run) = q_run.single() else { return };
    for e in &q_root {
        commands.entity(e).despawn();
    }

    commands
        .spawn((ChoiceRoot, overlay_root(), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)), GlobalZIndex(10)))
        .with_children(|root| {
            let title_color = if panel.banishing { Color::srgb(1.0, 0.4, 0.4) } else { Color::srgb(0.6, 1.0, 0.8) };
            let title = if panel.banishing {
                format!("{}: PICK ONE TO BANISH", panel.title)
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
                // Labels state the §3 economy outright: what a Refresh costs RIGHT NOW,
                // charges left, and exactly what a Skip pays.
                let dim = Color::srgb(0.45, 0.47, 0.55);
                let (rlabel, rcolor) = match run.refresh_price() {
                    RefreshPrice::Free if run.character == crate::content::characters::AstronautKind::Fortuna => {
                        ("[R]EFRESH (FREE)".to_string(), Color::WHITE)
                    }
                    RefreshPrice::Free => (format!("[R]EFRESH ({} FREE)", run.refreshes), Color::WHITE),
                    RefreshPrice::Gold(c) => (
                        format!("[R]EFRESH ({c}g)"),
                        if run.gold >= c { Color::srgb(1.0, 0.85, 0.3) } else { dim },
                    ),
                };
                let blabel = if panel.banishing {
                    "[B] CANCEL BANISH".to_string()
                } else {
                    format!("[B]ANISH ({})", run.banishes)
                };
                let bcolor = if run.banishes > 0 || panel.banishing { Color::WHITE } else { dim };
                let (skip_gold, _) = run.skip_reward();
                root.spawn((Node { column_gap: Val::Px(10.0), ..default() },))
                    .with_children(|row| {
                        row.spawn((RefreshBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.4, 0.7, 1.0))))
                            .with_children(|b| {
                                b.spawn(txt(rlabel, FONT_SMALL, rcolor));
                            });
                        row.spawn((BanishBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(1.0, 0.4, 0.4))))
                            .with_children(|b| {
                                b.spawn(txt(blabel, FONT_SMALL, bcolor));
                            });
                        row.spawn((SkipBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.7, 0.7, 0.7))))
                            .with_children(|b| {
                                b.spawn(txt(format!("[S]KIP +{skip_gold}g +XP"), FONT_SMALL, Color::WHITE));
                            });
                    });
                root.spawn(txt(
                    format!("gold {}      evolutions {}/{}", run.gold, run.evolutions_used(), run.evo_cap()),
                    FONT_SMALL,
                    Color::srgb(0.6, 0.65, 0.8),
                ));
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
    mut q_run: Query<&mut PlayerState, With<crate::player::LocalPlayer>>,
    mut global: ResMut<RunState>,
    save: Res<MetaSave>,
    mut phase: ResMut<RunPhase>,
    mut sfx: MessageWriter<SfxMsg>,
    mut banners: MessageWriter<BannerMsg>,
    mut hitstop: ResMut<crate::fx::Hitstop>,
) {
    if !matches!(*phase, RunPhase::LevelUp | RunPhase::Modal) || panel.options.is_empty() {
        return;
    }
    let Ok(mut run) = q_run.single_mut() else { return };

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
        if do_refresh {
            // Free refreshes first (Fortuna: always), then rising Gold (§3).
            if run.spend_refresh() {
                let mut rng = rand::thread_rng();
                panel.options = roll_upgrades(&run, &save, &mut rng);
                panel.banishing = false;
                sfx.write(SfxMsg(Sfx::Click));
            } else {
                banners.write(BannerMsg("NOT ENOUGH GOLD TO REFRESH".into()));
            }
            return;
        }
        if do_banish {
            // B toggles banish mode, so a misclick costs nothing.
            if panel.banishing || run.banishes > 0 {
                panel.banishing = !panel.banishing;
                sfx.write(SfxMsg(Sfx::Click));
            }
            return;
        }
        if do_skip {
            run.take_skip();
            finish_choice(&mut run, &mut panel, &mut phase, &save);
            sfx.write(SfxMsg(Sfx::Coin));
            return;
        }
    }

    let Some(idx) = pick else { return };
    if idx >= panel.options.len() {
        return;
    }

    if panel.banishing {
        let opt = panel.options[idx].clone();
        if run.banish(&opt) {
            panel.options.remove(idx);
            // An emptied hand (more banishes than cards) is dealt afresh, never a softlock.
            if panel.options.is_empty() {
                let mut rng = rand::thread_rng();
                panel.options = roll_upgrades(&run, &save, &mut rng);
            }
            sfx.write(SfxMsg(Sfx::Click));
        } else {
            banners.write(BannerMsg("THAT CARD CAN'T BE BANISHED".into()));
        }
        panel.banishing = false;
        return;
    }

    let opt = panel.options[idx].clone();
    let evolved = run.apply_upgrade(&opt, &save, global.greed_stacks);
    if evolved {
        // Never advanced before, so the "Evolve any weapon" quest could not complete.
        global.evolves += 1;
        banners.write(BannerMsg("WEAPON EVOLVED".into()));
        sfx.write(SfxMsg(Sfx::Evolve));
        hitstop.timer = 0.18;
    } else {
        sfx.write(SfxMsg(Sfx::Click));
    }
    finish_choice(&mut run, &mut panel, &mut phase, &save);
}

fn finish_choice(run: &mut PlayerState, panel: &mut ChoicePanel, phase: &mut RunPhase, save: &MetaSave) {
    // Banish mode belongs to the hand it was armed on. Carried into the next queued hand
    // (B, then Skip) it would turn the player's first click there into a spent charge.
    panel.banishing = false;
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
    mut q_run: Query<&mut PlayerState, With<crate::player::LocalPlayer>>,
    mut global: ResMut<RunState>,
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
    let Ok(mut run) = q_run.single_mut() else { return };
    if q_root.is_empty() {
        let Some(item) = chest.item else { return };
        let d = item.def();
        let cost = chest.cost;
        let cat = crate::run::catalyst_line(item, &run);
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
                    if !cat.is_empty() {
                        card.spawn(txt(cat.trim_start(), FONT_SMALL, Color::srgb(1.0, 0.75, 0.3)));
                    }
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
                let save_c = save.clone();
                run.recompute_stats(&save_c, global.greed_stacks);
                // Both were never advanced: chest prices never rose (CHEST_COST_GROWTH was
                // dead) and the "Open 10 chests" quest could not complete.
                global.chest_opens += 1;
                global.chests_opened += 1;
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
    mut q_run: Query<&mut PlayerState, With<crate::player::LocalPlayer>>,
    global: Res<RunState>,
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
    let Ok(mut run) = q_run.single_mut() else { return };
    if q_root.is_empty() || *rebuild {
        *rebuild = false;
        for e in &q_root {
            commands.entity(e).despawn();
        }
        let offers = shop.offers.clone();
        let gold = run.gold;
        let cats: Vec<String> =
            offers.iter().map(|(item, _, _)| crate::run::catalyst_line(*item, &run)).collect();
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
                                let cat = &cats[i];
                                if !cat.is_empty() {
                                    c.spawn(txt(cat.trim_start(), FONT_SMALL, Color::srgb(1.0, 0.75, 0.3)));
                                }
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
                run.recompute_stats(&save_c, global.greed_stacks);
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

/// Leaving the run strips every modal overlay. The panel systems only run in a run, so
/// they cannot tidy up after it — and a co-op client can be pulled out mid-panel (the host
/// ended the session, or it left from the pause menu) straight onto the main menu.
#[allow(clippy::type_complexity)]
pub fn despawn_panels(
    mut commands: Commands,
    q: Query<Entity, Or<(With<ChoiceRoot>, With<ChestRoot>, With<ShopRoot>, With<PauseRoot>)>>,
) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Escape toggles pause; pause menu buttons.
#[allow(clippy::too_many_arguments)]
pub fn pause_panel(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut phase: ResMut<RunPhase>,
    mut run: ResMut<RunState>,
    q_ps: Query<&PlayerState, With<crate::player::LocalPlayer>>,
    mut settings_open: ResMut<crate::ui::settings::SettingsOpen>,
    q_root: Query<Entity, With<PauseRoot>>,
    resume: Query<&Interaction, (Changed<Interaction>, With<ResumeBtn>)>,
    abandon: Query<&Interaction, (Changed<Interaction>, With<AbandonBtn>)>,
    psettings: Query<&Interaction, (Changed<Interaction>, With<PauseSettingsBtn>)>,
    leave: Query<&Interaction, (Changed<Interaction>, With<LeaveSessionBtn>)>,
    role: Res<crate::net::NetRole>,
    mut leave_out: MessageWriter<crate::net::LeaveSession>,
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
                let Ok(ps) = q_ps.single() else { return };
                let stats = ps.stats.clone();
                // The §3 model made visible: what the horde is scaled to right now.
                let sc = crate::run::scaling::Scaling::for_run(&run, 1);
                let delta = crate::run::scaling::difficulty_points(&run);
                let refresh = match ps.refresh_price() {
                    RefreshPrice::Free if ps.character == crate::content::characters::AstronautKind::Fortuna => "free".to_string(),
                    RefreshPrice::Free => format!("{} free", ps.refreshes),
                    RefreshPrice::Gold(c) => format!("{c}g"),
                };
                let rules = format!(
                    "THREAT {:.1}   ENEMY HP x{:.2}  DMG x{:.2}  ELITE {:.0}%\nEVOLUTIONS {}/{}   REFRESH {}   BANISH {}",
                    delta,
                    sc.hp,
                    sc.dmg,
                    sc.elite_chance * 100.0,
                    ps.evolutions_used(),
                    ps.evo_cap(),
                    refresh,
                    ps.banishes
                );
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
                        root.spawn(txt(rules, FONT_SMALL, Color::srgb(1.0, 0.7, 0.55)));
                        root.spawn((ResumeBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.4, 1.0, 0.6))))
                            .with_children(|b| {
                                b.spawn(txt("[ESC] RESUME", FONT_MED, Color::WHITE));
                            });
                        root.spawn((PauseSettingsBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.5, 0.8, 1.0))))
                            .with_children(|b| {
                                b.spawn(txt("SETTINGS", FONT_MED, Color::WHITE));
                            });
                        // A joiner's run is the HOST'S run: it cannot abandon it (nothing on a
                        // client ends a run), only leave it — and the host plays on.
                        if *role != crate::net::NetRole::Client {
                            root.spawn((AbandonBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(1.0, 0.4, 0.4))))
                                .with_children(|b| {
                                    b.spawn(txt(
                                        // It ends the run for everyone; the session itself
                                        // stays open, and the squad follows us into the next.
                                        if role.is_networked() { "ABANDON RUN (WHOLE SQUAD)" } else { "ABANDON RUN" },
                                        FONT_MED,
                                        Color::WHITE,
                                    ));
                                });
                        }
                        if role.is_networked() {
                            let label = if *role == crate::net::NetRole::Host {
                                "END SESSION (PLAY ON SOLO)"
                            } else {
                                "LEAVE SESSION"
                            };
                            root.spawn((LeaveSessionBtn, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(1.0, 0.7, 0.3))))
                                .with_children(|b| {
                                    b.spawn(txt(label, FONT_MED, Color::WHITE));
                                });
                            root.spawn(txt(
                                if *role == crate::net::NetRole::Host {
                                    "you host this run: pausing freezes it for your whole squad"
                                } else {
                                    "the host's world keeps running while this menu is open"
                                },
                                FONT_SMALL,
                                Color::srgb(0.6, 0.65, 0.8),
                            ));
                        }
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
                    run.result = Some(RunResult::Abandoned);
                    *phase = RunPhase::Dead;
                }
            }
            for i in &leave {
                if *i == Interaction::Pressed {
                    leave_out.write(crate::net::LeaveSession);
                }
            }
        }
        _ => {}
    }
}
