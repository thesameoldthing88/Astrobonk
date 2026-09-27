pub mod hud;
pub mod menus;
pub mod numbers;
pub mod panels;
pub mod settings;

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
        // the margin `FitToScreen` keeps clear of the window edge
        padding: UiRect::all(Val::Px(8.0)),
        ..default()
    }
}

/// `overlay_root` for a panel shown over a live run: its content centers in, and fits to,
/// the space between the HUD's top and bottom bands, so a card never lands on the timer, the
/// boss bar or the HP bar at any UI scale (`hud::keep_panels_clear_of_hud` keeps the top band
/// matched to whether a boss bar is up).
pub fn run_overlay_root() -> impl Bundle {
    (
        RunOverlay,
        Node {
            padding: UiRect {
                left: Val::Px(8.0),
                right: Val::Px(8.0),
                top: Val::Px(crate::config::HUD_TOP_BAND),
                bottom: Val::Px(crate::config::HUD_BOTTOM_BAND),
            },
            ..overlay_root()
        },
    )
}

/// An overlay laid out by `run_overlay_root`.
#[derive(Component)]
pub struct RunOverlay;

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

// ------------------------------------------------------------ fitting the screen
//
// §13 offers the UI at up to 150%, which on a 1280x720 screen leaves 853x480 UI units, and a
// few screens hold more than that. Each layout reflows first (the choice cards narrow, the
// long lists scroll); what still does not fit is brought down to size one of two ways:
//  * a menu screen, the only thing on screen, eases the UI scale itself while it is up
//    (`MenuFit`): text stays crisp, and clipping and hit-testing stay exact;
//  * a panel over a live run shrinks just itself with a `UiTransform`, so the HUD behind it
//    keeps its size. Those panels hold no scroll lists: Bevy clips a scrolled node at its
//    untransformed size.

/// Marks the box a screen's content lives in — the one child of its overlay, so there is a
/// single thing to measure against the room the overlay leaves.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum FitToScreen {
    /// Over a live run (choice, chest, shop, pause, settings): shrinks itself.
    Panel,
    /// A whole menu screen: eases the UI scale.
    Menu,
}

/// How far the UI scale is eased below what the window allows so the current menu screen
/// fits (1 = not at all). `settings::apply_ui_scale` multiplies it in.
#[derive(Resource)]
pub struct MenuFit(pub f32);

impl Default for MenuFit {
    fn default() -> Self {
        Self(1.0)
    }
}

/// The column an in-run panel stacks its content in. Hidden until its first fit (the frame
/// after it is laid out), so an oversized panel never shows at full size first.
pub fn fit_column() -> impl Bundle {
    (FitToScreen::Panel, Visibility::Hidden, fit_column_node())
}

/// The column a menu screen stacks its content in. Bounded in height too, so a scroll list
/// inside (`flex_shrink`, a small `min_height`) gives up height before the whole screen has
/// to shrink — only for columns whose other children keep a fixed width: Taffy measures a
/// wrapping row's minimum height at its widest, and would squash it.
pub fn menu_column() -> impl Bundle {
    (FitToScreen::Menu, Visibility::Hidden, Node { max_height: Val::Percent(100.0), ..fit_column_node() })
}

fn fit_column_node() -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: Val::Px(14.0),
        // bounded by the overlay, so a row inside can be sized as a share of the screen
        max_width: Val::Percent(100.0),
        ..default()
    }
}

/// Room over need for a fit box, both in physical px at the layout just done: below 1, it
/// does not fit. The need is the box itself, or what its children take laid end to end with
/// its gaps, padding and border if that is more: a height-bounded column is clamped to the
/// room however much it holds (and Taffy's content size would count a scroll list's whole
/// scrolled content). `None` until it has been laid out.
fn fit_ratio(node: &ComputedNode, style: &Node, parent: &ComputedNode, kids: Option<&Children>, sizes: &Query<&ComputedNode>) -> Option<f32> {
    // the parent's padding is what it keeps clear: a margin at the window edge, and the HUD
    // bands for an in-run panel
    let room = parent.size - parent.padding.min_inset - parent.padding.max_inset;
    let mut stack = Vec2::ZERO;
    let mut n = 0u32;
    for kid in kids.into_iter().flatten() {
        let Ok(k) = sizes.get(*kid) else { continue };
        stack.x = stack.x.max(k.size.x);
        stack.y += k.size.y;
        n += 1;
    }
    let gap = match style.row_gap {
        Val::Px(g) => g / node.inverse_scale_factor.max(1e-4),
        _ => 0.0,
    };
    let frame = node.padding.min_inset + node.padding.max_inset + node.border.min_inset + node.border.max_inset;
    stack.y += gap * n.saturating_sub(1) as f32;
    let need = node.size.max(stack + frame);
    (need.min_element() > 0.0 && room.min_element() > 0.0).then(|| (room / need).min_element())
}

pub fn fit_panels(
    mut q: Query<(&FitToScreen, &ComputedNode, &Node, &ChildOf, Option<&Children>, &mut UiTransform, &mut Visibility)>,
    sizes: Query<&ComputedNode>,
) {
    for (kind, node, style, child_of, kids, mut tf, mut vis) in &mut q {
        if *kind != FitToScreen::Panel {
            continue;
        }
        let Some(ratio) = sizes.get(child_of.parent()).ok().and_then(|p| fit_ratio(node, style, p, kids, &sizes)) else { continue };
        let s = ratio.min(1.0);
        // a small dead band: text reflow nudges sizes by a pixel from frame to frame
        if (tf.scale.x - s).abs() > 0.004 {
            tf.scale = Vec2::splat(s);
        }
        if *vis == Visibility::Hidden {
            *vis = Visibility::Inherited;
        }
    }
}

/// Ease `MenuFit` until every menu screen on show fits, then reveal it. Runs before
/// `apply_ui_scale`, so the layout it measures is the one done at the current `UiScale`.
#[allow(clippy::type_complexity)]
pub fn fit_menus(
    mut q: Query<(Entity, &FitToScreen, &ComputedNode, &Node, &ChildOf, Option<&Children>, &mut Visibility)>,
    sizes: Query<&ComputedNode>,
    ui_scale: Res<UiScale>,
    save: Res<crate::save::MetaSave>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut fit: ResMut<MenuFit>,
    // (the screen being fitted, adjustments in a row without settling): wrapping text can
    // make a screen's height jump as the scale moves, so a fit that keeps hunting stops
    // where it is until the screen changes
    mut tries: Local<(Option<Entity>, u32)>,
) {
    let base = settings::effective_ui_scale(save.accessibility.ui_scale, windows.iter().next());
    let mut target: Option<f32> = None;
    let mut first = None;
    for (e, kind, node, style, child_of, kids, _) in &q {
        if *kind != FitToScreen::Menu {
            continue;
        }
        // the screen underneath (the oldest), not an overlay on it like the join panel
        first = Some(first.map_or(e, |f: Entity| f.min(e)));
        let Some(ratio) = sizes.get(child_of.parent()).ok().and_then(|p| fit_ratio(node, style, p, kids, &sizes)) else { return };
        // grow back only with real slack (a closed list), so a screen sitting right at the
        // edge does not hunt
        let r = if ratio > 1.0 && ratio < 1.03 { 1.0 } else { ratio };
        let t = (ui_scale.0 * r).min(base);
        target = Some(target.map_or(t, |x: f32| x.min(t)));
    }
    let Some(target) = target else {
        fit.0 = 1.0;
        *tries = (None, 0);
        return;
    };
    // A new screen starts again from the full scale and eases down only as far as it needs.
    if tries.0 != first {
        *tries = (first, 0);
        if fit.0 != 1.0 {
            fit.0 = 1.0;
            return;
        }
    }
    let want = (target / base.max(0.01)).clamp(0.25, 1.0);
    if (want - fit.0).abs() > 0.004 && tries.1 < 8 {
        fit.0 = want;
        tries.1 += 1;
        return;
    }
    if tries.1 < 8 {
        tries.1 = 0;
    }
    for (_, kind, _, _, _, _, mut vis) in &mut q {
        if *kind == FitToScreen::Menu && *vis == Visibility::Hidden {
            *vis = Visibility::Inherited;
        }
    }
}

/// A scroll list the mouse wheel drives while the pointer is over it (Bevy clips and offsets
/// an `Overflow::scroll_y` node but leaves input to the game). Needs `RelativeCursorPosition`
/// — `wheel_scroll_list()` adds both.
#[derive(Component)]
pub struct WheelScroll;

pub fn wheel_scroll_list() -> impl Bundle {
    (WheelScroll, bevy::ui::RelativeCursorPosition::default())
}

/// Shown only while its list has more than fits ("scroll for more").
#[derive(Component)]
pub struct ScrollHint(pub Entity);

/// UI units per wheel notch.
const WHEEL_LINE: f32 = 48.0;

pub fn wheel_scroll(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut q: Query<(&mut ScrollPosition, &ComputedNode, &bevy::ui::RelativeCursorPosition), With<WheelScroll>>,
) {
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy -= match ev.unit {
            bevy::input::mouse::MouseScrollUnit::Line => ev.y * WHEEL_LINE,
            bevy::input::mouse::MouseScrollUnit::Pixel => ev.y,
        };
    }
    if dy == 0.0 {
        return;
    }
    for (mut pos, node, cursor) in &mut q {
        if !cursor.cursor_over() {
            continue;
        }
        let max = ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0);
        pos.y = (pos.y + dy).clamp(0.0, max);
    }
}

pub fn scroll_hints(lists: Query<&ComputedNode, With<WheelScroll>>, mut hints: Query<(&ScrollHint, &mut Visibility)>) {
    for (hint, mut vis) in &mut hints {
        let more = lists.get(hint.0).is_ok_and(|n| n.content_size.y > n.size.y + 1.0);
        let want = if more { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}
