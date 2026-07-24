//! Settings overlay — a stepper-row panel reachable from the main menu and the pause
//! menu. Adjusts persisted MetaSave fields live (volumes, sensitivity, screenshake) and
//! saves on every change. Works in any AppState since it's an overlay, not a screen.

use super::*;
use crate::save::MetaSave;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct SettingsOpen(pub bool);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Master,
    Music,
    Sfx,
    Sensitivity,
    Shake,
}

impl Setting {
    const ALL: [Setting; 5] = [Setting::Master, Setting::Music, Setting::Sfx, Setting::Sensitivity, Setting::Shake];

    fn label(&self) -> &'static str {
        match self {
            Setting::Master => "MASTER VOLUME",
            Setting::Music => "MUSIC",
            Setting::Sfx => "SOUND FX",
            Setting::Sensitivity => "MOUSE SENSITIVITY",
            Setting::Shake => "SCREEN SHAKE",
        }
    }
    fn bounds(&self) -> (f32, f32, f32) {
        // (min, max, step)
        match self {
            Setting::Master | Setting::Music | Setting::Sfx => (0.0, 1.0, 0.1),
            Setting::Sensitivity => (0.25, 3.0, 0.15),
            Setting::Shake => (0.0, 1.5, 0.15),
        }
    }
    fn get(&self, s: &MetaSave) -> f32 {
        match self {
            Setting::Master => s.volume,
            Setting::Music => s.music_volume,
            Setting::Sfx => s.sfx_volume,
            Setting::Sensitivity => s.sensitivity,
            Setting::Shake => s.shake_scale,
        }
    }
    fn set(&self, s: &mut MetaSave, v: f32) {
        match self {
            Setting::Master => s.volume = v,
            Setting::Music => s.music_volume = v,
            Setting::Sfx => s.sfx_volume = v,
            Setting::Sensitivity => s.sensitivity = v,
            Setting::Shake => s.shake_scale = v,
        }
    }
    fn display(&self, v: f32) -> String {
        match self {
            Setting::Sensitivity => format!("{v:.2}x"),
            Setting::Shake => format!("{:.0}%", v / 1.5 * 100.0),
            _ => format!("{:.0}%", v * 100.0),
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
pub struct SettingsClose;

fn bar(v: f32, max: f32) -> String {
    let filled = (v / max * 14.0).round().clamp(0.0, 14.0) as usize;
    "\u{2588}".repeat(filled) + &"\u{2591}".repeat(14 - filled)
}

/// Build / tear down / drive the overlay based on `SettingsOpen`.
#[allow(clippy::too_many_arguments)]
pub fn settings_panel(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<SettingsOpen>,
    mut save: ResMut<MetaSave>,
    q_root: Query<Entity, With<SettingsRoot>>,
    steps: Query<(&Interaction, &SettingsStep), Changed<Interaction>>,
    closes: Query<&Interaction, (Changed<Interaction>, With<SettingsClose>)>,
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
            let (min, max, step) = s.setting.bounds();
            let v = (s.setting.get(&save) + s.dir * step).clamp(min, max);
            s.setting.set(&mut save, v);
            changed = true;
        }
    }
    if changed {
        save.save();
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

    commands
        .spawn((
            SettingsRoot,
            overlay_root(),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(50),
        ))
        .with_children(|root| {
            root.spawn(txt("SETTINGS", FONT_BIG, Color::srgb(0.7, 0.85, 1.0)));
            for setting in Setting::ALL {
                let (_, max, _) = setting.bounds();
                let v = setting.get(&save);
                root.spawn((Node {
                    width: Val::Px(560.0),
                    column_gap: Val::Px(12.0),
                    align_items: AlignItems::Center,
                    ..default()
                },))
                    .with_children(|row| {
                        row.spawn((
                            Node { width: Val::Px(230.0), ..default() },
                            Text::new(setting.label()),
                            TextFont { font_size: FONT_MED, ..default() },
                            TextColor(Color::srgb(0.85, 0.87, 0.95)),
                        ));
                        for (dir, glyph) in [(-1.0f32, "-"), (1.0f32, "+")] {
                            // the value readout sits between the two steppers
                            if dir > 0.0 {
                                row.spawn((
                                    Node { width: Val::Px(180.0), justify_content: JustifyContent::Center, ..default() },
                                    Text::new(bar(v, max)),
                                    TextFont { font_size: FONT_MED, ..default() },
                                    TextColor(Color::srgb(0.4, 0.9, 0.6)),
                                ));
                            }
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
                        row.spawn((
                            Node { width: Val::Px(60.0), justify_content: JustifyContent::FlexEnd, ..default() },
                            Text::new(setting.display(v)),
                            TextFont { font_size: FONT_MED, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });
            }
            root.spawn((SettingsClose, Button, button_node(), BackgroundColor(BTN_BG), BorderColor::all(Color::srgb(0.4, 1.0, 0.6))))
                .with_children(|b| {
                    b.spawn(txt("[ESC] DONE", FONT_MED, Color::WHITE));
                });
        });
}
