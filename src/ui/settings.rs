//! Settings overlay — a tabbed stepper-row panel reachable from the main menu and the pause
//! menu (GDD §13: every setting is reachable mid-run, so nobody has to quit a run to fix the
//! thing hurting their eyes). Adjusts persisted MetaSave fields live and saves on every
//! change. Works in any AppState since it's an overlay, not a screen.
//!
//! Tabs: GENERAL (audio, mouse, shake) · DISPLAY (UI scale, damage numbers) · VISION
//! (colorblind palettes, high-contrast telegraphs, flash reduction, photosensitivity) ·
//! ASSIST (difficulty as options).

use super::*;
use crate::config::*;
use crate::content::palettes::Palette;
use crate::content::Rarity;
use crate::save::{MetaSave, NumberMode};
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct SettingsOpen(pub bool);

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsTab {
    #[default]
    General,
    Display,
    Vision,
    Assist,
}

impl SettingsTab {
    const ALL: [SettingsTab; 4] = [SettingsTab::General, SettingsTab::Display, SettingsTab::Vision, SettingsTab::Assist];

    fn label(&self) -> &'static str {
        match self {
            SettingsTab::General => "GENERAL",
            SettingsTab::Display => "DISPLAY",
            SettingsTab::Vision => "VISION",
            SettingsTab::Assist => "ASSIST",
        }
    }

    fn blurb(&self) -> &'static str {
        match self {
            SettingsTab::General => "Sound, mouse and camera.",
            SettingsTab::Display => "Interface size and how damage numbers are drawn.",
            SettingsTab::Vision => "Danger always reads by shape and motion too: pulsing rings,\nmarching aim dashes, cracking ground.",
            SettingsTab::Assist => "Eases the run on top of the normal difficulty. Assisted runs\nstill earn Silver, are flagged on results and rank apart.",
        }
    }

    fn settings(&self) -> &'static [Setting] {
        match self {
            SettingsTab::General => &[Setting::Master, Setting::Music, Setting::Sfx, Setting::Sensitivity, Setting::Shake],
            SettingsTab::Display => &[Setting::UiScale, Setting::Numbers, Setting::NumberSize],
            SettingsTab::Vision => &[Setting::Palette, Setting::HighContrast, Setting::FlashReduction, Setting::Photosensitive],
            SettingsTab::Assist => &[Setting::Density, Setting::EnemyDamage, Setting::ReviveToken],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Setting {
    Master,
    Music,
    Sfx,
    Sensitivity,
    Shake,
    UiScale,
    Numbers,
    NumberSize,
    Palette,
    HighContrast,
    FlashReduction,
    Photosensitive,
    Density,
    EnemyDamage,
    ReviveToken,
}

/// How a row steps: a bounded slider, an on/off toggle, or a cycle through named choices.
/// Every kind is read and written as an f32 (toggles 0/1, choices an index) so one stepper
/// row drives them all.
enum Kind {
    Slider { min: f32, max: f32, step: f32 },
    Toggle,
    Choice(usize),
}

impl Setting {
    fn label(&self) -> &'static str {
        match self {
            Setting::Master => "MASTER VOLUME",
            Setting::Music => "MUSIC",
            Setting::Sfx => "SOUND FX",
            Setting::Sensitivity => "MOUSE SENSITIVITY",
            Setting::Shake => "SCREEN SHAKE",
            Setting::UiScale => "UI SCALE",
            Setting::Numbers => "DAMAGE NUMBERS",
            Setting::NumberSize => "NUMBER SIZE",
            Setting::Palette => "COLOR PALETTE",
            Setting::HighContrast => "DANGER OUTLINES",
            Setting::FlashReduction => "FLASH REDUCTION",
            Setting::Photosensitive => "PHOTOSENSITIVITY",
            Setting::Density => "ENEMY DENSITY",
            Setting::EnemyDamage => "ENEMY DAMAGE",
            Setting::ReviveToken => "ONE MORE CHANCE",
        }
    }
    fn kind(&self) -> Kind {
        match self {
            Setting::Master | Setting::Music | Setting::Sfx => Kind::Slider { min: 0.0, max: 1.0, step: 0.1 },
            Setting::Sensitivity => Kind::Slider { min: 0.25, max: 3.0, step: 0.15 },
            // §13: 0–100%, where 100% is the canon budget that respects the 1.8° clamp
            Setting::Shake => Kind::Slider { min: 0.0, max: 1.0, step: 0.1 },
            Setting::UiScale => Kind::Slider { min: UI_SCALE_MIN, max: UI_SCALE_MAX, step: 0.05 },
            Setting::NumberSize => Kind::Slider { min: NUMBER_SIZE_MIN, max: NUMBER_SIZE_MAX, step: 0.1 },
            Setting::Density => Kind::Slider { min: ASSIST_DENSITY_MIN, max: 1.0, step: 0.05 },
            Setting::EnemyDamage => Kind::Slider { min: ASSIST_DAMAGE_MIN, max: 1.0, step: 0.05 },
            Setting::Numbers => Kind::Choice(NumberMode::ALL.len()),
            Setting::Palette => Kind::Choice(Palette::ALL.len()),
            Setting::HighContrast | Setting::FlashReduction | Setting::Photosensitive | Setting::ReviveToken => Kind::Toggle,
        }
    }
    fn get(&self, s: &MetaSave) -> f32 {
        let flag = |b: bool| if b { 1.0 } else { 0.0 };
        let a = &s.accessibility;
        match self {
            Setting::Master => s.volume,
            Setting::Music => s.music_volume,
            Setting::Sfx => s.sfx_volume,
            Setting::Sensitivity => s.sensitivity,
            Setting::Shake => s.shake_scale,
            Setting::UiScale => a.ui_scale,
            Setting::Numbers => NumberMode::ALL.iter().position(|m| *m == a.numbers).unwrap_or(0) as f32,
            Setting::NumberSize => a.number_size,
            Setting::Palette => Palette::ALL.iter().position(|p| *p == a.palette).unwrap_or(0) as f32,
            Setting::HighContrast => flag(a.high_contrast),
            Setting::FlashReduction => flag(a.flash_reduction),
            Setting::Photosensitive => flag(a.photosensitive),
            Setting::Density => s.assist.enemy_density,
            Setting::EnemyDamage => s.assist.enemy_damage,
            Setting::ReviveToken => flag(s.assist.revive_token),
        }
    }
    fn set(&self, s: &mut MetaSave, v: f32) {
        let on = v > 0.5;
        let idx = v.round().max(0.0) as usize;
        let a = &mut s.accessibility;
        match self {
            Setting::Master => s.volume = v,
            Setting::Music => s.music_volume = v,
            Setting::Sfx => s.sfx_volume = v,
            Setting::Sensitivity => s.sensitivity = v,
            Setting::Shake => s.shake_scale = v,
            Setting::UiScale => a.ui_scale = v,
            Setting::Numbers => a.numbers = NumberMode::ALL[idx % NumberMode::ALL.len()],
            Setting::NumberSize => a.number_size = v,
            Setting::Palette => a.palette = Palette::ALL[idx % Palette::ALL.len()],
            Setting::HighContrast => a.high_contrast = on,
            Setting::FlashReduction => a.flash_reduction = on,
            Setting::Photosensitive => a.photosensitive = on,
            Setting::Density => s.assist.enemy_density = v,
            Setting::EnemyDamage => s.assist.enemy_damage = v,
            Setting::ReviveToken => s.assist.revive_token = on,
        }
    }
    /// One press of `-` (dir -1) or `+` (dir +1). Sliders clamp at their ends; toggles flip;
    /// choices wrap around.
    fn step(&self, s: &mut MetaSave, dir: f32) {
        let v = self.get(s);
        let next = match self.kind() {
            // snapped to the step grid so repeated presses never drift (0.1 + 0.1 + ...)
            Kind::Slider { min, max, step } => (((v + dir * step) / step).round() * step).clamp(min, max),
            Kind::Toggle => 1.0 - v,
            Kind::Choice(n) => (v.round() as i32 + dir as i32).rem_euclid(n as i32) as f32,
        };
        self.set(s, next);
    }
    fn display(&self, v: f32) -> String {
        match self {
            Setting::Sensitivity => format!("{v:.2}x"),
            Setting::NumberSize => format!("{v:.1}x"),
            Setting::Numbers => NumberMode::ALL[v.round() as usize % NumberMode::ALL.len()].name().to_string(),
            Setting::Palette => Palette::ALL[v.round() as usize % Palette::ALL.len()].name().to_string(),
            _ => match self.kind() {
                Kind::Toggle => if v > 0.5 { "ON" } else { "OFF" }.to_string(),
                _ => format!("{:.0}%", v * 100.0),
            },
        }
    }
    /// The middle readout: a fill bar for sliders, the value itself otherwise.
    fn gauge(&self, v: f32) -> String {
        match self.kind() {
            Kind::Slider { min, max, .. } => bar((v - min) / (max - min).max(1e-3)),
            _ => self.display(v),
        }
    }
}

#[derive(Component)]
pub struct SettingsRoot;
#[derive(Component)]
pub struct SettingsStep {
    setting: Setting,
    dir: f32,
}
#[derive(Component)]
pub struct SettingsTabBtn(SettingsTab);
#[derive(Component)]
pub struct SettingsClose;

/// A 14-cell ASCII meter. Plain ASCII on purpose: the built-in font has only the printable
/// ASCII glyphs, so block characters draw as missing-glyph boxes.
fn bar(frac: f32) -> String {
    let filled = (frac * 14.0).round().clamp(0.0, 14.0) as usize;
    "#".repeat(filled) + &"-".repeat(14 - filled)
}

fn stepper_button(row: &mut ChildSpawnerCommands, setting: Setting, dir: f32, glyph: &str) {
    row.spawn((
        SettingsStep { setting, dir },
        Button,
        Node {
            width: Val::Px(38.0),
            height: Val::Px(34.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(BTN_BG),
        BorderColor::all(Color::srgb(0.5, 0.6, 0.8)),
    ))
    .with_children(|b| {
        b.spawn(txt(glyph, FONT_MED, Color::WHITE));
    });
}

/// Build / tear down / drive the overlay based on `SettingsOpen`.
#[allow(clippy::too_many_arguments)]
pub fn settings_panel(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<SettingsOpen>,
    mut tab: ResMut<SettingsTab>,
    mut save: ResMut<MetaSave>,
    role: Res<crate::net::NetRole>,
    q_root: Query<Entity, With<SettingsRoot>>,
    steps: Query<(&Interaction, &SettingsStep), Changed<Interaction>>,
    tabs: Query<(&Interaction, &SettingsTabBtn), Changed<Interaction>>,
    closes: Query<&Interaction, (Changed<Interaction>, With<SettingsClose>)>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut rebuild: Local<bool>,
) {
    if !open.0 {
        for e in &q_root {
            commands.entity(e).despawn();
        }
        return;
    }

    // apply any step clicks first, then rebuild to reflect new values
    let mut changed = false;
    for (i, s) in &steps {
        if *i == Interaction::Pressed {
            s.setting.step(&mut save, s.dir);
            changed = true;
        }
    }
    if changed {
        save.save();
        *rebuild = true;
    }
    for (i, t) in &tabs {
        if *i == Interaction::Pressed && *tab != t.0 {
            *tab = t.0;
            *rebuild = true;
        }
    }
    if keys.just_pressed(KeyCode::Tab) {
        let i = SettingsTab::ALL.iter().position(|t| *t == *tab).unwrap_or(0);
        *tab = SettingsTab::ALL[(i + 1) % SettingsTab::ALL.len()];
        *rebuild = true;
    }

    let mut do_close = keys.just_pressed(KeyCode::Escape);
    for i in &closes {
        if *i == Interaction::Pressed {
            do_close = true;
        }
    }
    if do_close {
        open.0 = false;
        save.save();
        return;
    }

    if !q_root.is_empty() && !*rebuild {
        return;
    }
    *rebuild = false;
    for e in &q_root {
        commands.entity(e).despawn();
    }

    let current = *tab;
    let ui_fit = effective_ui_scale(save.accessibility.ui_scale, windows.iter().next());
    commands
        .spawn((
            SettingsRoot,
            overlay_root(),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(50),
            // Swallow every click that misses the card's own buttons: the main menu and the
            // pause menu underneath must never see them (a stray click on blank card space
            // used to hit LAUNCH or QUIT).
            bevy::ui::FocusPolicy::Block,
        ))
        .with_children(|overlay| {
            // a solid card: whatever screen is underneath (menu or a live run) must not
            // show through the text
            overlay
                .spawn((
                    // One fixed size for every tab (the tallest page, GENERAL's five rows),
                    // so the tab strip never jumps under the pointer when the page changes.
                    // Where the UI scale makes it taller than the window, it is fitted
                    // (`FitToScreen::Panel`).
                    Node {
                        width: Val::Px(720.0),
                        height: Val::Px(496.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(10.0),
                        padding: UiRect::axes(Val::Px(26.0), Val::Px(18.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(PANEL_BG.with_alpha(1.0)),
                    BorderColor::all(Color::srgb(0.35, 0.45, 0.7)),
                    FitToScreen::Panel,
                ))
                .with_children(|root| settings_card(root, current, &save, *role, ui_fit));
        });
}

/// The panel's contents for one tab.
fn settings_card(root: &mut ChildSpawnerCommands, current: SettingsTab, save: &MetaSave, role: crate::net::NetRole, ui_fit: f32) {
    root.spawn(txt("SETTINGS", FONT_BIG, Color::srgb(0.7, 0.85, 1.0)));
    // tab strip ([TAB] cycles)
    root.spawn((Node { column_gap: Val::Px(8.0), ..default() },)).with_children(|row| {
        for t in SettingsTab::ALL {
            let active = t == current;
            row.spawn((
                SettingsTabBtn(t),
                Button,
                button_node(),
                BackgroundColor(BTN_BG),
                BorderColor::all(if active { Color::srgb(0.4, 1.0, 0.6) } else { Color::srgb(0.3, 0.35, 0.5) }),
            ))
            .with_children(|b| {
                let c = if active { Color::WHITE } else { Color::srgb(0.6, 0.64, 0.75) };
                b.spawn(txt(t.label(), FONT_SMALL, c));
            });
        }
    });
    root.spawn((
        txt(current.blurb(), FONT_SMALL, Color::srgb(0.6, 0.65, 0.8)),
        TextLayout::new_with_justify(Justify::Center),
    ));
    for &setting in current.settings() {
        let v = setting.get(save);
        root.spawn((Node {
            width: Val::Px(660.0),
            column_gap: Val::Px(12.0),
            align_items: AlignItems::Center,
            ..default()
        },))
            .with_children(|row| {
                row.spawn((
                    Node { width: Val::Px(260.0), ..default() },
                    Text::new(setting.label()),
                    TextFont { font_size: FONT_MED, ..default() },
                    TextColor(Color::srgb(0.85, 0.87, 0.95)),
                ));
                stepper_button(row, setting, -1.0, "-");
                row.spawn((
                    Node { width: Val::Px(200.0), ..default() },
                    Text::new(setting.gauge(v)),
                    TextLayout::new_with_justify(Justify::Center),
                    TextFont { font_size: FONT_MED, ..default() },
                    TextColor(Color::srgb(0.4, 0.9, 0.6)),
                ));
                stepper_button(row, setting, 1.0, "+");
                // sliders read out their value here; the other kinds already show it in
                // the gauge, but keep the column so every row lines up
                let value = if matches!(setting.kind(), Kind::Slider { .. }) { setting.display(v) } else { String::new() };
                row.spawn((
                    Node { width: Val::Px(70.0), ..default() },
                    Text::new(value),
                    TextFont { font_size: FONT_MED, ..default() },
                    TextColor(Color::WHITE),
                ));
            });
    }
    match current {
        SettingsTab::Display if ui_fit < save.accessibility.ui_scale - 0.004 => {
            root.spawn(txt(
                format!("This window holds the UI at up to {:.0}%; the rest applies on a bigger one.", ui_fit * 100.0),
                FONT_SMALL,
                Color::srgb(1.0, 0.75, 0.4),
            ));
        }
        SettingsTab::Vision => palette_preview(root, save.accessibility.palette),
        SettingsTab::Assist => {
            let (note, color) = if role == crate::net::NetRole::Client {
                ("In co-op the HOST's assists govern the run.".to_string(), Color::srgb(1.0, 0.75, 0.4))
            } else if save.assist.is_assisted() {
                (format!("Active: {}", save.assist.summary()), Color::srgb(0.55, 0.9, 1.0))
            } else {
                ("All assists off: canon difficulty.".to_string(), Color::srgb(0.6, 0.65, 0.8))
            };
            root.spawn(txt(note, FONT_SMALL, color));
        }
        _ => {}
    }
    // spacer: pins DONE to the bottom of the fixed-size card
    root.spawn(Node { flex_grow: 1.0, ..default() });
    root.spawn((SettingsClose, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.4, 1.0, 0.6))))
        .with_children(|b| {
            b.spawn(txt("[ESC] DONE    [TAB] NEXT PAGE", FONT_MED, Color::WHITE));
        });
}

/// Swatches of the colors the palette remaps, drawn in it, so a change is visible before
/// the next telegraph lands.
fn palette_preview(root: &mut ChildSpawnerCommands, palette: Palette) {
    root.spawn((Node { column_gap: Val::Px(14.0), align_items: AlignItems::Center, ..default() },))
        .with_children(|row| {
            let mut swatch = |label: &str, color: Color| {
                row.spawn((Node { column_gap: Val::Px(5.0), align_items: AlignItems::Center, ..default() },))
                    .with_children(|s| {
                        s.spawn((
                            Node {
                                width: Val::Px(16.0),
                                height: Val::Px(16.0),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(color),
                            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.5)),
                        ));
                        s.spawn(txt(label.to_string(), FONT_SMALL, color));
                    });
            };
            swatch("DANGER", palette.danger());
            for r in [Rarity::Common, Rarity::Rare, Rarity::Epic, Rarity::Legendary] {
                swatch(r.name(), r.color(palette));
            }
        });
}

/// The UI scale this window can actually give: the setting, unless the window is too small
/// to hold the UI_FIT_CANVAS at it (then the largest scale that does).
pub fn effective_ui_scale(setting: f32, window: Option<&Window>) -> f32 {
    let want = setting.clamp(UI_SCALE_MIN, UI_SCALE_MAX);
    let Some(w) = window else { return want };
    let fit = (w.width() / UI_FIT_CANVAS.0).min(w.height() / UI_FIT_CANVAS.1);
    if fit.is_finite() && fit > 0.0 { want.min(fit) } else { want }
}

/// Keep Bevy's `UiScale` on the setting (§13: UI scale 75–150%), as far as the window allows
/// (and, on a menu screen, as far as that screen fits — `MenuFit`).
/// Every Val::Px in the game scales with it; the two world-projected overlays (damage
/// numbers, edge markers) divide it back out of their screen positions.
pub fn apply_ui_scale(
    save: Res<MetaSave>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    menu_fit: Res<MenuFit>,
    mut ui_scale: ResMut<UiScale>,
) {
    let want = effective_ui_scale(save.accessibility.ui_scale, windows.iter().next()) * menu_fit.0;
    if (ui_scale.0 - want).abs() > 1e-4 {
        ui_scale.0 = want;
    }
}

/// Headless self-check for the window fit: 1280x720 and the Steam Deck's 1280x800 both hold
/// the full 150%, a smaller window caps the scale at what holds the UI_FIT_CANVAS, and the
/// setting itself is never exceeded.
pub fn ui_scale_self_check() -> Result<(), String> {
    let window = |w: f32, h: f32| {
        let mut win = Window::default();
        win.resolution.set(w, h);
        win
    };
    for (w, h) in [(1280.0, 720.0), (1280.0, 800.0), (1920.0, 1080.0)] {
        let got = effective_ui_scale(UI_SCALE_MAX, Some(&window(w, h)));
        if (got - UI_SCALE_MAX).abs() > 1e-3 {
            return Err(format!("{w}x{h} should hold the UI at {UI_SCALE_MAX}, got {got}"));
        }
    }
    let small = effective_ui_scale(UI_SCALE_MAX, Some(&window(1024.0, 576.0)));
    if (small - 1.2).abs() > 1e-3 {
        return Err(format!("1024x576 should cap 150% at 120%, got {small}"));
    }
    if (effective_ui_scale(0.9, Some(&window(1920.0, 1080.0))) - 0.9).abs() > 1e-4
        || (effective_ui_scale(1.25, None) - 1.25).abs() > 1e-4
    {
        return Err("the window fit raised or ignored the setting".into());
    }
    Ok(())
}
