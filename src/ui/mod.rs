pub mod hud;
pub mod menus;
pub mod numbers;
pub mod panels;

use bevy::prelude::*;

pub const FONT_BIG: f32 = 34.0;
pub const FONT_MED: f32 = 20.0;
pub const FONT_SMALL: f32 = 15.0;

pub const PANEL_BG: Color = Color::srgba(0.05, 0.06, 0.10, 0.92);
pub const CARD_BG: Color = Color::srgba(0.10, 0.11, 0.18, 0.96);
pub const BTN_BG: Color = Color::srgba(0.16, 0.18, 0.28, 1.0);
pub const BTN_HOVER: Color = Color::srgba(0.24, 0.28, 0.44, 1.0);

/// A text bundle with size + color.
pub fn txt(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont { font_size: size, ..default() },
        TextColor(color),
    )
}

/// Full-screen centered overlay container.
pub fn overlay_root() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(14.0),
        ..default()
    }
}

pub fn button_node() -> Node {
    Node {
        padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(2.0)),
        border_radius: BorderRadius::all(Val::Px(6.0)),
        ..default()
    }
}

/// Standard hover tint for every UI button.
pub fn button_hover(
    mut q: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
) {
    for (i, mut bg) in &mut q {
        bg.0 = match i {
            Interaction::Hovered => BTN_HOVER,
            _ => BTN_BG,
        };
    }
}
