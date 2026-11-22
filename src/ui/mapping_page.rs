//! The mapping editor: up to eight surfaces on up to four outputs, placed
//! with professional tools (docs/superpowers/plans/2026-11-21-mapping-editor-v2.md).
//!
//! The canvas shows one output (its tab) at its shape, with its surfaces
//! live. Two tools: Transform (move, rotate, scale) and Corner pin (corners
//! and edges). A drag grabs on press and the shape follows the mouse
//! exactly, worked out from where the drag began; a move that would fold
//! the shape stops at the last good one.

use egui::{
    Color32, CornerRadius, CursorIcon, Pos2, Rect, RichText, Sense, Stroke, Ui, pos2, vec2,
};
use glam::Vec2;

use super::icons::{self, Icon};
use super::style;
use super::{Live, UiAction};
use crate::mapping::{self, Corners};
use crate::settings::{
    ComplementStyle, Content, MAX_MAINS, MAX_OUTPUTS, MAX_SURFACES, OutputConf, OutputPattern,
    Settings, Surface,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    /// Move, rotate, scale.
    #[default]
    Transform,
    /// Corners and edges, one at a time.
    CornerPin,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelTab {
    #[default]
    Surfaces,
    Transform,
    Look,
    Output,
}

/// What part of a surface the pointer is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Corner(usize),
    Edge(usize),
    Rotate,
    Inside,
}

/// A drag in progress.
#[derive(Clone, Debug, PartialEq)]
pub struct Drag {
    pub surface: usize,
    pub handle: Handle,
    /// Where the pointer and the corners were when the drag (or its fine
    /// part) began, in output coordinates.
    pub start_pointer: Vec2,
    pub start_corners: Corners,
    pub fine: bool,
}

/// What the page remembers between frames.
#[derive(Clone, Debug, Default)]
pub struct MappingState {
    /// The output on the canvas (its tab).
    pub output: usize,
    pub selected: usize,
    /// The corner the keys move (corner pin).
    pub corner: usize,
    pub tool: Tool,
    pub tab: PanelTab,
    pub drag: Option<Drag>,
    /// Why the last choice in Look was not taken.
    pub note: Option<&'static str>,
}

/// Handles are this far round, in points.
const HANDLE_RADIUS: f32 = 9.0;
/// They are grabbed this close.
const GRAB_RADIUS: f32 = 14.0;
/// The rotate handle sits this far above the top edge.
const ROTATE_STEM: f32 = 36.0;
const SNAP_ANGLE: f32 = std::f32::consts::PI / 12.0; // 15°
const TOOLBAR_W: f32 = 56.0;
const STATUS_H: f32 = 34.0;
const HEADER_H: f32 = 44.0;

fn out_to_canvas(c: Vec2, canvas: Rect) -> Pos2 {
    pos2(
        canvas.left() + c.x * canvas.width(),
        canvas.top() + c.y * canvas.height(),
    )
}

/// A point on the canvas as a position on the output (0..1 each way).
/// Points, not pixels: the same on a Retina screen and any other.
pub fn canvas_to_output(p: Pos2, canvas: Rect) -> Vec2 {
    Vec2::new(
        (p.x - canvas.left()) / canvas.width(),
        (p.y - canvas.top()) / canvas.height(),
    )
}

/// The canvas inside `card`: as big as fits, in the output's shape.
pub fn canvas_rect(card: Rect, aspect: f32) -> Rect {
    let room = card.shrink2(vec2(20.0, 38.0));
    let size = if room.width() / room.height() > aspect {
        vec2(room.height() * aspect, room.height())
    } else {
        vec2(room.width(), room.width() / aspect)
    };
    Rect::from_center_size(room.center(), size)
}

/// The page's parts inside `area` (below the header): toolbar, canvas card,
/// side panel, status line.
pub fn layout(area: Rect) -> (Rect, Rect, Rect, Rect) {
    let panel_w = (area.width() * 0.3).clamp(300.0, 420.0);
    let toolbar = Rect::from_min_size(area.min, vec2(TOOLBAR_W, area.height() - STATUS_H));
    let card = Rect::from_min_max(
        pos2(toolbar.right() + 10.0, area.top()),
        pos2(
            area.right() - panel_w - style::GAP,
            area.bottom() - STATUS_H,
        ),
    );
    let panel = Rect::from_min_max(pos2(card.right() + style::GAP, area.top()), area.max);
    let status = Rect::from_min_max(
        pos2(area.left(), card.bottom() + 8.0),
        pos2(card.right(), area.bottom()),
    );
    (toolbar, card, panel, status)
}

/// Where the handles of a surface are on the canvas, for `tool`.
pub fn handles(corners: &Corners, tool: Tool, canvas: Rect) -> Vec<(Handle, Pos2)> {
    let mut out: Vec<(Handle, Pos2)> = (0..4)
        .map(|i| {
            (
                Handle::Corner(i),
                out_to_canvas(Vec2::from(corners[i]), canvas),
            )
        })
        .collect();
    match tool {
        Tool::CornerPin => {
            for e in 0..4 {
                out.push((
                    Handle::Edge(e),
                    out_to_canvas(mapping::edge_middle(corners, e), canvas),
                ));
            }
        }
        Tool::Transform => out.push((Handle::Rotate, rotate_handle(corners, canvas))),
    }
    out
}

/// The rotate handle: above the middle of the top edge, away from the centre.
fn rotate_handle(corners: &Corners, canvas: Rect) -> Pos2 {
    let top = out_to_canvas(mapping::edge_middle(corners, 0), canvas);
    let centre = out_to_canvas(mapping::centroid(corners), canvas);
    let away = (top - centre).normalized();
    let away = if away.length() > 0.5 {
        away
    } else {
        vec2(0.0, -1.0)
    };
    top + away * ROTATE_STEM
}

/// What is under `p`: the selected surface's handles first, then the
/// topmost surface it is inside. Locked surfaces can be selected, not grabbed.
pub fn hit_test(
    surfaces: &[(usize, Corners, bool)],
    selected: usize,
    tool: Tool,
    p: Pos2,
    canvas: Rect,
) -> Option<(usize, Handle)> {
    if let Some((i, corners, locked)) = surfaces.iter().find(|(i, ..)| *i == selected)
        && !locked
    {
        let nearest = handles(corners, tool, canvas)
            .into_iter()
            .map(|(h, at)| (h, at.distance(p)))
            .filter(|(_, d)| *d <= GRAB_RADIUS)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((h, _)) = nearest {
            return Some((*i, h));
        }
    }
    let at = canvas_to_output(p, canvas);
    surfaces
        .iter()
        .rev()
        .find(|(_, c, _)| mapping::contains(c, at))
        .map(|(i, ..)| (*i, Handle::Inside))
}

/// The corners a drag gives with the pointer at `pointer` (output
/// coordinates), or `None` if that would fold the shape (keep the last).
/// Fine: a tenth of the movement, or 15° steps when rotating.
pub fn apply_drag(
    drag: &Drag,
    pointer: Vec2,
    tool: Tool,
    snap: bool,
    aspect: f32,
) -> Option<Corners> {
    let start = &drag.start_corners;
    let raw = pointer - drag.start_pointer;
    let delta = if drag.fine && drag.handle != Handle::Rotate {
        raw * 0.1
    } else {
        raw
    };
    let sq = |v: Vec2| Vec2::new(v.x * aspect, v.y);
    match (drag.handle, tool) {
        (Handle::Corner(i), Tool::CornerPin) => {
            mapping::place_corner(start, i, Vec2::from(start[i]) + delta, snap)
        }
        (Handle::Corner(i), Tool::Transform) => {
            // Scale about the middle, by how far the corner is pulled out.
            let c = sq(mapping::centroid(start));
            let from = sq(Vec2::from(start[i])) - c;
            let to = sq(Vec2::from(start[i]) + delta) - c;
            let factor = to.dot(from) / from.length_squared().max(1e-9);
            mapping::scale(start, factor.max(0.05), aspect)
        }
        (Handle::Edge(e), _) => mapping::move_edge(start, e, delta),
        (Handle::Inside, _) => {
            let moved = mapping::drag_surface(start, delta);
            if snap {
                let c = mapping::centroid(&moved);
                let to = Vec2::new((c.x * 100.0).round() / 100.0, (c.y * 100.0).round() / 100.0);
                Some(mapping::drag_surface(&moved, to - c))
            } else {
                Some(moved)
            }
        }
        (Handle::Rotate, _) => {
            let c = sq(mapping::centroid(start));
            let a0 = (sq(drag.start_pointer) - c).to_angle();
            let a1 = (sq(pointer) - c).to_angle();
            let mut angle = a1 - a0;
            if drag.fine {
                let base = mapping::angle(start, aspect);
                angle = ((base + angle) / SNAP_ANGLE).round() * SNAP_ANGLE - base;
            }
            mapping::rotate(start, angle, aspect)
        }
    }
}

pub fn draw(
    ui: &mut Ui,
    settings: &mut Settings,
    live: &Live,
    preview: Option<egui::TextureId>,
    state: &mut MappingState,
    actions: &mut Vec<UiAction>,
) {
    state.output = state.output.min(settings.output.outputs.len() - 1);
    state.selected = state.selected.min(settings.output.surfaces.len() - 1);
    if settings.output.surfaces[state.selected].output != state.output
        && let Some((i, _)) = settings.output.on_output(state.output).next()
    {
        // Keep a surface of this output selected when there is one.
        state.selected = i;
    }
    let selected_before = state.selected;
    let top = ui.available_rect_before_wrap();
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(Rect::from_min_size(top.min, vec2(top.width(), HEADER_H))),
        |ui| header(ui, settings, live, state, actions),
    );
    let area = Rect::from_min_max(top.min + vec2(0.0, HEADER_H + 10.0), top.max);
    let (toolbar, card, panel, status) = layout(area);

    toolbar_ui(ui, toolbar, settings, state);
    canvas_ui(ui, card, settings, live, preview, state);
    ui.scope_builder(egui::UiBuilder::new().max_rect(panel), |ui| {
        panel_ui(ui, settings, live, state, actions)
    });
    // The hint for the tool, shorter when the window is narrow.
    let font = egui::FontId::proportional(13.0);
    let hints = match state.tool {
        Tool::Transform => [
            "Transform (V): drag inside to move · round handle rotates (Shift: 15°) · corners scale · arrows move 1 px (Shift 10) · ⌘D duplicate · ⌫ delete",
            "Drag to move · round handle rotates (Shift 15°) · corners scale · arrows 1 px",
            "Drag · rotate · scale",
        ],
        Tool::CornerPin => [
            "Corner pin (C): drag a corner or an edge · Shift for fine · arrows move the corner 1 px (Shift 10) · Tab next corner · ⌘D duplicate · ⌫ delete",
            "Drag a corner or edge · Shift fine · arrows 1 px · Tab next corner",
            "Corners and edges",
        ],
    };
    let fits = |t: &str| {
        ui.fonts_mut(|f| {
            f.layout_no_wrap(t.to_string(), font.clone(), style::MUTED)
                .size()
                .x
        }) <= status.width()
    };
    let hint = hints.iter().find(|t| fits(t)).unwrap_or(&hints[2]);
    ui.painter().with_clip_rect(status).text(
        status.left_center(),
        egui::Align2::LEFT_CENTER,
        *hint,
        font,
        style::MUTED,
    );
    keys(ui, settings, live, state);
    if state.selected != selected_before {
        state.note = None;
    }
    ui.advance_cursor_after_rect(top);
}

fn output_px(live: &Live, k: usize) -> (u32, u32) {
    live.output_sizes
        .get(k)
        .copied()
        .flatten()
        .unwrap_or((1920, 1080))
}

fn header(
    ui: &mut Ui,
    settings: &mut Settings,
    live: &Live,
    state: &mut MappingState,
    actions: &mut Vec<UiAction>,
) {
    let _ = actions;
    ui.horizontal(|ui| {
        ui.label(style::title("Screens"));
        ui.add_space(10.0);
        // One tab per output, as a segmented control; a dot when connected.
        egui::Frame::new().fill(style::WELL).corner_radius(CornerRadius::same(style::WIDGET_RADIUS + 1)).inner_margin(egui::Margin::same(2)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (k, o) in settings.output.outputs.iter().enumerate() {
                    let connected = live.output_sizes.get(k).is_some_and(|s| s.is_some());
                    let on = k == state.output;
                    let label = format!("{}{}", o.name, o.display.as_ref().map(|d| format!(" · {}", style::visual(d))).unwrap_or_default());
                    let text = RichText::new(label).size(12.5).color(if on { style::TEXT } else { style::MUTED });
                    let b = egui::Button::new(text)
                        .fill(if on { style::LINE } else { Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::same(style::WIDGET_RADIUS - 1))
                        .min_size(vec2(0.0, 24.0));
                    let r = ui.add(b);
                    if connected {
                        ui.painter().circle_filled(r.rect.right_top() + vec2(-5.0, 5.0), 3.0, style::GREEN);
                    }
                    if r.clicked() {
                        state.output = k;
                        if let Some((i, _)) = settings.output.on_output(k).next() {
                            state.selected = i;
                        }
                    }
                }
            });
        });
        if settings.output.outputs.len() < MAX_OUTPUTS && style::button(ui, "+ Output").on_hover_text("Another projector, display or network screen").clicked() {
            let k = settings.output.outputs.len();
            settings.output.outputs.push(OutputConf { name: format!("Output {}", k + 1), ..OutputConf::default() });
            state.output = k;
            state.tab = PanelTab::Output;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let o = &mut settings.output.outputs[state.output];
            let grid = o.showing == OutputPattern::Grid;
            if icons::button(
                ui,
                Icon::Grid,
                grid,
                "Calibration pattern on this output: grid, circle and number, on black",
            )
            .clicked()
            {
                o.showing = if grid {
                    OutputPattern::Show
                } else {
                    OutputPattern::Grid
                };
            }
        });
    });
}

fn toolbar_ui(ui: &mut Ui, rect: Rect, settings: &mut Settings, state: &mut MappingState) {
    ui.painter().rect(
        rect,
        CornerRadius::same(16),
        style::CARD,
        Stroke::new(1.0, style::LINE),
        egui::StrokeKind::Inside,
    );
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(rect.shrink2(vec2(9.0, 10.0))),
        |ui| {
            // A short window: the tools scroll.
            egui::ScrollArea::vertical()
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    if icons::button(
                        ui,
                        Icon::Transform,
                        state.tool == Tool::Transform,
                        "Transform (V): move, rotate, scale",
                    )
                    .clicked()
                    {
                        state.tool = Tool::Transform;
                    }
                    if icons::button(
                        ui,
                        Icon::CornerPin,
                        state.tool == Tool::CornerPin,
                        "Corner pin (C): corners and edges",
                    )
                    .clicked()
                    {
                        state.tool = Tool::CornerPin;
                    }
                    separator(ui);
                    let room = settings.output.surfaces.len() < MAX_SURFACES;
                    if ui
                        .add_enabled_ui(room, |ui| {
                            icons::button(ui, Icon::Add, false, "Add a surface on this output")
                        })
                        .inner
                        .clicked()
                    {
                        add_surface(settings, state);
                    }
                    if ui
                        .add_enabled_ui(room, |ui| {
                            icons::button(ui, Icon::Duplicate, false, "Duplicate (⌘D)")
                        })
                        .inner
                        .clicked()
                    {
                        duplicate(settings, state);
                    }
                    let deletable = can_delete(settings, state.selected);
                    if ui
                        .add_enabled_ui(deletable, |ui| {
                            icons::button(
                                ui,
                                Icon::Delete,
                                false,
                                "Delete (⌫); one Main surface always stays",
                            )
                        })
                        .inner
                        .clicked()
                    {
                        delete(settings, state);
                    }
                    separator(ui);
                    let locked = settings.output.surfaces[state.selected].locked;
                    ui.add_enabled_ui(!locked, |ui| {
                        if icons::button(ui, Icon::RotateLeft, false, "Rotate 90° left").clicked()
                        {
                            transform(settings, state, |c, a| {
                                mapping::rotate(c, -std::f32::consts::FRAC_PI_2, a)
                            });
                        }
                        if icons::button(ui, Icon::RotateRight, false, "Rotate 90° right").clicked()
                        {
                            transform(settings, state, |c, a| {
                                mapping::rotate(c, std::f32::consts::FRAC_PI_2, a)
                            });
                        }
                        let flipped = settings.output.surfaces[state.selected].flip;
                        if icons::button(ui, Icon::Flip, flipped, "Flip the picture").clicked() {
                            settings.output.surfaces[state.selected].flip = !flipped;
                        }
                        if icons::button(ui, Icon::Fit, false, "Fit to the whole output").clicked()
                        {
                            settings.output.surfaces[state.selected].corners = mapping::FULL_FRAME;
                        }
                        if icons::button(
                            ui,
                            Icon::Reset,
                            false,
                            "Reset: back to its place side by side",
                        )
                        .clicked()
                        {
                            let (k, count) = position_among(settings, state.selected);
                            settings.output.surfaces[state.selected].corners =
                                mapping::default_corners(k, count);
                        }
                    });
                    separator(ui);
                    let snap = settings.output.snap;
                    if icons::button(ui, Icon::Snap, snap, "Snap to 1% of the output").clicked() {
                        settings.output.snap = !snap;
                    }
                });
        },
    );
}

fn separator(ui: &mut Ui) {
    ui.add_space(3.0);
    let (r, _) = ui.allocate_exact_size(vec2(38.0, 1.0), Sense::hover());
    ui.painter().line_segment(
        [r.left_center(), r.right_center()],
        Stroke::new(1.0, style::LINE),
    );
    ui.add_space(3.0);
}

/// Where surface `i` is among the surfaces of its output, and how many there are.
fn position_among(settings: &Settings, i: usize) -> (usize, usize) {
    let k = settings.output.surfaces[i].output;
    let here: Vec<usize> = settings.output.on_output(k).map(|(j, _)| j).collect();
    (
        here.iter().position(|j| *j == i).unwrap_or(0),
        here.len().max(1),
    )
}

fn transform(
    settings: &mut Settings,
    state: &MappingState,
    f: impl Fn(&Corners, f32) -> Option<Corners>,
) {
    let s = &mut settings.output.surfaces[state.selected];
    if let Some(c) = f(&s.corners, 16.0 / 9.0) {
        s.corners = c;
    }
}

fn add_surface(settings: &mut Settings, state: &mut MappingState) {
    let k = state.output;
    let count = settings.output.on_output(k).count();
    let names = [
        "Left",
        "Centre",
        "Right",
        "Far left",
        "Far right",
        "Top",
        "Bottom",
        "Extra",
    ];
    let n = settings.output.surfaces.len();
    // Make room: the ones still in their side-by-side places move over.
    let here: Vec<usize> = settings.output.on_output(k).map(|(j, _)| j).collect();
    for (pos, j) in here.iter().enumerate() {
        let s = &mut settings.output.surfaces[*j];
        if s.corners == mapping::default_corners(pos, count) || s.corners == mapping::FULL_FRAME {
            s.corners = mapping::default_corners(pos, count + 1);
        }
    }
    settings.output.surfaces.push(Surface {
        name: names.get(n).unwrap_or(&"Surface").to_string(),
        output: k,
        corners: mapping::default_corners(count, count + 1),
        content: Content::Complement,
        ..Surface::default()
    });
    state.selected = n;
}

fn mains(settings: &Settings) -> usize {
    settings
        .output
        .surfaces
        .iter()
        .filter(|s| s.content == Content::Main)
        .count()
}

/// Any but the last surface, and any but the last Main.
fn can_delete(settings: &Settings, i: usize) -> bool {
    settings.output.surfaces.len() > 1
        && (settings.output.surfaces[i].content != Content::Main || mains(settings) > 1)
}

fn duplicate(settings: &mut Settings, state: &mut MappingState) {
    if settings.output.surfaces.len() >= MAX_SURFACES {
        return;
    }
    let mut copy = settings.output.surfaces[state.selected].clone();
    copy.name = format!("{} copy", copy.name);
    if copy.content == Content::Main && mains(settings) >= MAX_MAINS {
        copy.content = Content::Same;
    }
    copy.locked = false;
    copy.corners = mapping::drag_surface(&copy.corners, Vec2::new(0.03, 0.03));
    settings.output.surfaces.push(copy);
    state.selected = settings.output.surfaces.len() - 1;
}

fn delete(settings: &mut Settings, state: &mut MappingState) {
    if can_delete(settings, state.selected) {
        settings.output.surfaces.remove(state.selected);
        state.selected = state
            .selected
            .saturating_sub(1)
            .min(settings.output.surfaces.len() - 1);
    }
}

fn canvas_ui(
    ui: &mut Ui,
    card: Rect,
    settings: &mut Settings,
    live: &Live,
    preview: Option<egui::TextureId>,
    state: &mut MappingState,
) {
    let painter = ui.painter_at(card);
    painter.rect(
        card,
        CornerRadius::same(style::CARD_RADIUS),
        style::CARD,
        Stroke::new(1.0, style::LINE),
        egui::StrokeKind::Inside,
    );
    let (ow, oh) = output_px(live, state.output);
    let aspect = ow as f32 / oh.max(1) as f32;
    let canvas = canvas_rect(card, aspect);
    painter.rect_filled(
        canvas,
        CornerRadius::same(3),
        Color32::from_rgb(0x05, 0x05, 0x07),
    );
    if let Some(texture) = preview {
        egui::Image::new((texture, canvas.size())).paint_at(ui, canvas);
    }
    painter.rect_stroke(
        canvas,
        CornerRadius::same(3),
        Stroke::new(1.0, Color32::from_rgb(0x3a, 0x3a, 0x48)),
        egui::StrokeKind::Outside,
    );
    let o = &settings.output.outputs[state.output];
    painter.text(
        card.left_top() + vec2(18.0, 12.0),
        egui::Align2::LEFT_TOP,
        format!(
            "{} · {} · {ow} × {oh}",
            o.name.to_uppercase(),
            style::visual(&o.display.clone().unwrap_or_else(|| "NO DISPLAY YET".into()))
        ),
        egui::FontId::new(12.0, egui::FontFamily::Name("semibold".into())),
        style::MUTED,
    );

    let here: Vec<(usize, Corners, bool)> = settings
        .output
        .on_output(state.output)
        .filter(|(_, s)| s.visible)
        .map(|(i, s)| (i, s.corners, s.locked))
        .collect();
    let response = ui.interact(
        card,
        ui.scope_id().with("mapping canvas"),
        Sense::click_and_drag(),
    );
    let (pressed, down, pointer, shift) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.pointer.latest_pos(),
            i.modifiers.shift,
        )
    });

    // Grab on press: no threshold, the shape is in hand at once.
    if pressed
        && response.hovered()
        && let Some(p) = pointer
        && let Some((i, handle)) = hit_test(&here, state.selected, state.tool, p, canvas)
    {
        state.selected = i;
        if let Handle::Corner(c) = handle {
            state.corner = c;
        }
        if !settings.output.surfaces[i].locked {
            state.drag = Some(Drag {
                surface: i,
                handle,
                start_pointer: canvas_to_output(p, canvas),
                start_corners: settings.output.surfaces[i].corners,
                fine: shift,
            });
        }
    }
    if let (Some(drag), Some(p)) = (&mut state.drag, pointer) {
        let at = canvas_to_output(p, canvas);
        let s = &mut settings.output.surfaces[drag.surface];
        if drag.fine != shift {
            // Shift pressed or let go mid-drag: carry on from here, no jump.
            drag.start_pointer = at;
            drag.start_corners = s.corners;
            drag.fine = shift;
        }
        if let Some(c) = apply_drag(drag, at, state.tool, settings.output.snap, aspect) {
            s.corners = c;
        }
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        if !down {
            state.drag = None;
        }
    } else if let Some(p) = pointer.filter(|_| response.hovered()) {
        ui.ctx().set_cursor_icon(
            match hit_test(&here, state.selected, state.tool, p, canvas).map(|(_, h)| h) {
                Some(Handle::Inside) => CursorIcon::Move,
                Some(Handle::Corner(_)) if state.tool == Tool::CornerPin => CursorIcon::Crosshair,
                Some(Handle::Corner(_)) => CursorIcon::ResizeNeSw,
                Some(Handle::Edge(e)) => {
                    if e % 2 == 0 {
                        CursorIcon::ResizeVertical
                    } else {
                        CursorIcon::ResizeHorizontal
                    }
                }
                Some(Handle::Rotate) => CursorIcon::Grab,
                None => CursorIcon::Default,
            },
        );
    }

    // Outlines: the selected one with its handles, the others thin.
    let hover = pointer
        .filter(|_| response.hovered())
        .and_then(|p| hit_test(&here, state.selected, state.tool, p, canvas));
    for (i, s) in settings.output.on_output(state.output) {
        let colour = style::SURFACE_COLOURS[i % 8];
        let pts: Vec<Pos2> = s
            .corners
            .iter()
            .map(|c| out_to_canvas(Vec2::from(*c), canvas))
            .collect();
        let selected = i == state.selected;
        let width = if selected {
            2.5
        } else if hover.is_some_and(|(j, _)| j == i) {
            2.0
        } else {
            1.2
        };
        let colour = if s.visible {
            colour
        } else {
            colour.gamma_multiply(0.35)
        };
        painter.add(egui::Shape::closed_line(
            pts.clone(),
            Stroke::new(width, colour),
        ));
        let centre = pts.iter().fold(egui::Vec2::ZERO, |a, p| a + p.to_vec2()) / 4.0;
        painter.text(
            centre.to_pos2(),
            egui::Align2::CENTER_CENTER,
            (i + 1).to_string(),
            egui::FontId::new(26.0, egui::FontFamily::Name("heavy".into())),
            colour.gamma_multiply(0.9),
        );
        if s.locked {
            icons::paint(
                ui,
                Rect::from_center_size(centre.to_pos2() + vec2(0.0, 26.0), egui::Vec2::splat(16.0)),
                Icon::Lock,
                colour,
            );
        }
        if selected && !s.locked {
            let hs = handles(&s.corners, state.tool, canvas);
            if state.tool == Tool::Transform {
                let top = out_to_canvas(mapping::edge_middle(&s.corners, 0), canvas);
                painter.line_segment(
                    [top, rotate_handle(&s.corners, canvas)],
                    Stroke::new(1.5, colour),
                );
            }
            for (h, at) in hs {
                let hot =
                    hover == Some((i, h)) || state.drag.as_ref().is_some_and(|d| d.handle == h);
                let fill = if hot { colour } else { style::BG };
                match h {
                    Handle::Corner(c) if state.tool == Tool::CornerPin => {
                        let fill = if c == state.corner { colour } else { fill };
                        painter.circle(at, HANDLE_RADIUS, fill, Stroke::new(2.5, Color32::WHITE));
                    }
                    Handle::Corner(_) => {
                        painter.rect(
                            Rect::from_center_size(at, egui::Vec2::splat(HANDLE_RADIUS * 1.6)),
                            2.0,
                            fill,
                            Stroke::new(2.0, Color32::WHITE),
                            egui::StrokeKind::Middle,
                        );
                    }
                    Handle::Edge(_) => {
                        painter.rect(
                            Rect::from_center_size(at, egui::Vec2::splat(HANDLE_RADIUS * 1.2)),
                            2.0,
                            fill,
                            Stroke::new(2.0, colour),
                            egui::StrokeKind::Middle,
                        );
                    }
                    Handle::Rotate => {
                        painter.circle(
                            at,
                            HANDLE_RADIUS + 2.0,
                            fill,
                            Stroke::new(2.0, Color32::WHITE),
                        );
                        icons::paint(
                            ui,
                            Rect::from_center_size(at, egui::Vec2::splat(12.0)),
                            Icon::RotateRight,
                            if hot { style::BG } else { Color32::WHITE },
                        );
                    }
                    Handle::Inside => {}
                }
            }
        }
    }
    // While dragging, what is changing, in numbers.
    if let (Some(drag), Some(p)) = (&state.drag, pointer) {
        let s = &settings.output.surfaces[drag.surface];
        let text = match drag.handle {
            Handle::Corner(c) if state.tool == Tool::CornerPin => format!(
                "{:.0}, {:.0} px",
                s.corners[c][0] * ow as f32,
                s.corners[c][1] * oh as f32
            ),
            Handle::Corner(_) => format!("size {:.0}%", mapping::size(&s.corners, aspect) * 100.0),
            Handle::Rotate => format!("{:.1}°", mapping::angle(&s.corners, aspect).to_degrees()),
            Handle::Edge(_) | Handle::Inside => {
                let c = mapping::centroid(&s.corners);
                format!("{:.0}, {:.0} px", c.x * ow as f32, c.y * oh as f32)
            }
        };
        let r = Rect::from_min_size(
            p + vec2(18.0, 14.0),
            vec2(8.0 * text.len() as f32 + 20.0, 26.0),
        );
        painter.rect_filled(r, CornerRadius::same(7), Color32::from_black_alpha(220));
        painter.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(13.0),
            Color32::WHITE,
        );
    }
    if here.is_empty() {
        painter.text(
            canvas.center(),
            egui::Align2::CENTER_CENTER,
            "No surfaces on this output: add one with +",
            egui::FontId::proportional(15.0),
            style::MUTED,
        );
    }
}

/// Keys work while no text field has the keyboard.
fn keys(ui: &mut Ui, settings: &mut Settings, live: &Live, state: &mut MappingState) {
    if ui.memory(|m| m.focused().is_some()) {
        return;
    }
    let px = output_px(live, state.output);
    let (dx, dy, tab, del, dup, v, c) = ui.input_mut(|i| {
        let step = if i.modifiers.shift { 10 } else { 1 };
        let mut d = (0, 0);
        for (key, (x, y)) in [
            (egui::Key::ArrowLeft, (-1, 0)),
            (egui::Key::ArrowRight, (1, 0)),
            (egui::Key::ArrowUp, (0, -1)),
            (egui::Key::ArrowDown, (0, 1)),
        ] {
            if i.key_pressed(key) {
                d = (d.0 + x * step, d.1 + y * step);
            }
        }
        (
            d.0,
            d.1,
            i.consume_key(egui::Modifiers::NONE, egui::Key::Tab),
            i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::Delete),
            i.consume_key(egui::Modifiers::COMMAND, egui::Key::D),
            i.key_pressed(egui::Key::V) && !i.modifiers.any(),
            i.key_pressed(egui::Key::C) && !i.modifiers.any(),
        )
    });
    if v {
        state.tool = Tool::Transform;
    }
    if c {
        state.tool = Tool::CornerPin;
    }
    if tab {
        state.corner = (state.corner + 1) % 4;
    }
    if del {
        delete(settings, state);
    }
    if dup {
        duplicate(settings, state);
    }
    if (dx, dy) != (0, 0)
        && let Some(s) = settings.output.surfaces.get_mut(state.selected)
        && !s.locked
    {
        s.corners = match state.tool {
            Tool::CornerPin => mapping::nudge(&s.corners, state.corner, dx, dy, px),
            Tool::Transform => mapping::drag_surface(
                &s.corners,
                Vec2::new(dx as f32 / px.0 as f32, dy as f32 / px.1 as f32),
            ),
        };
    }
}

fn panel_ui(ui: &mut Ui, settings: &mut Settings, live: &Live, state: &mut MappingState, actions: &mut Vec<UiAction>) {
    // Tabs, icon and name.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let w = ((ui.available_width() - 12.0) / 4.0).max(60.0);
        for (tab, icon, name) in [
            (PanelTab::Surfaces, Icon::Surfaces, "Surfaces"),
            (PanelTab::Transform, Icon::Move, "Transform"),
            (PanelTab::Look, Icon::Look, "Look"),
            (PanelTab::Output, Icon::Output, "Output"),
        ] {
            let on = state.tab == tab;
            let (rect, r) = ui.allocate_exact_size(vec2(w, 54.0), Sense::click());
            ui.painter().rect(
                rect,
                12.0,
                if on {
                    style::CARD_LIGHT
                } else {
                    Color32::TRANSPARENT
                },
                Stroke::new(
                    1.0,
                    if on {
                        style::PINK.gamma_multiply(0.7)
                    } else {
                        style::LINE
                    },
                ),
                egui::StrokeKind::Inside,
            );
            let colour = if on {
                style::PINK
            } else if r.hovered() {
                style::TEXT
            } else {
                style::MUTED
            };
            icons::paint(
                ui,
                Rect::from_center_size(rect.center() - vec2(0.0, 9.0), egui::Vec2::splat(18.0)),
                icon,
                colour,
            );
            ui.painter().text(
                rect.center() + vec2(0.0, 14.0),
                egui::Align2::CENTER_CENTER,
                name,
                egui::FontId::proportional(12.0),
                colour,
            );
            if r.clicked() {
                state.tab = tab;
            }
        }
    });
    ui.add_space(10.0);
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect(
        rect,
        CornerRadius::same(style::CARD_RADIUS),
        style::CARD,
        Stroke::new(1.0, style::LINE),
        egui::StrokeKind::Inside,
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(16.0)), |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match state.tab {
                PanelTab::Surfaces => surfaces_tab(ui, settings, state),
                PanelTab::Transform => transform_tab(ui, settings, live, state),
                PanelTab::Look => look_tab(ui, settings, state),
                PanelTab::Output => output_tab(ui, settings, live, state, actions),
            });
    });
}

fn surfaces_tab(ui: &mut Ui, settings: &mut Settings, state: &mut MappingState) {
    for (k, o) in settings.output.outputs.clone().iter().enumerate() {
        style::label(
            ui,
            &format!(
                "{} · {}",
                o.name,
                style::visual(&o.display.clone().unwrap_or_else(|| "no display".into()))
            ),
        );
        let rows: Vec<usize> = settings.output.on_output(k).map(|(i, _)| i).collect();
        if rows.is_empty() {
            ui.label(style::muted("No surfaces"));
        }
        for i in rows {
            let colour = style::SURFACE_COLOURS[i % 8];
            let selected = i == state.selected;
            let frame = egui::Frame::new()
                .fill(if selected {
                    colour.gamma_multiply(0.1)
                } else {
                    style::CARD_LIGHT
                })
                .stroke(Stroke::new(
                    1.0,
                    if selected {
                        colour.gamma_multiply(0.7)
                    } else {
                        style::LINE
                    },
                ))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(10, 6));
            let r = frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    let s = &mut settings.output.surfaces[i];
                    ui.label(style::heavy((i + 1).to_string(), 15.0).color(colour));
                    ui.label(&s.name);
                    ui.label(style::muted(match s.content {
                        Content::Main => "Main",
                        Content::Same => "Same",
                        Content::Complement => "Complement",
                    }));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let lock = if s.locked { Icon::Lock } else { Icon::Unlock };
                        let tip = if s.locked {
                            "Locked: click to unlock"
                        } else {
                            "Lock its corners"
                        };
                        if icons::small(
                            ui,
                            lock,
                            tip,
                            if s.locked { style::AMBER } else { style::MUTED },
                        )
                        .clicked()
                        {
                            s.locked = !s.locked;
                        }
                        let eye = if s.visible { Icon::Eye } else { Icon::EyeOff };
                        if icons::small(
                            ui,
                            eye,
                            if s.visible { "Hide" } else { "Show" },
                            style::MUTED,
                        )
                        .clicked()
                        {
                            s.visible = !s.visible;
                        }
                    });
                });
            });
            if r.response.interact(Sense::click()).clicked() {
                state.selected = i;
                state.output = k;
            }
        }
        ui.add_space(10.0);
    }
    ui.horizontal(|ui| {
        let room = settings.output.surfaces.len() < MAX_SURFACES;
        if ui
            .add_enabled(
                room,
                egui::Button::new(style::semibold("+ Add surface", 14.0)),
            )
            .clicked()
        {
            add_surface(settings, state);
        }
        if ui
            .add_enabled(room, egui::Button::new(style::semibold("Duplicate", 14.0)))
            .clicked()
        {
            duplicate(settings, state);
        }
    });
    ui.label(style::muted(format!(
        "{} of {MAX_SURFACES} surfaces",
        settings.output.surfaces.len()
    )));
}

fn transform_tab(ui: &mut Ui, settings: &mut Settings, live: &Live, state: &mut MappingState) {
    let i = state.selected;
    let k = settings.output.surfaces[i].output;
    let (ow, oh) = output_px(live, k);
    let aspect = ow as f32 / oh as f32;
    style::label(
        ui,
        &format!("Surface {} · {}", i + 1, settings.output.surfaces[i].name),
    );
    let s = &mut settings.output.surfaces[i];
    ui.add_enabled_ui(!s.locked, |ui| {
        egui::Grid::new("transform")
            .num_columns(2)
            .spacing(vec2(12.0, 10.0))
            .show(ui, |ui| {
                let c = mapping::centroid(&s.corners);
                let (mut x, mut y) = (c.x * ow as f32, c.y * oh as f32);
                ui.label("Position");
                ui.horizontal(|ui| {
                    let a = ui
                        .add(
                            egui::DragValue::new(&mut x)
                                .speed(1.0)
                                .prefix("x ").fixed_decimals(0)
                                .suffix(" px"),
                        )
                        .changed();
                    let b = ui
                        .add(
                            egui::DragValue::new(&mut y)
                                .speed(1.0)
                                .prefix("y ").fixed_decimals(0)
                                .suffix(" px"),
                        )
                        .changed();
                    if a || b {
                        s.corners = mapping::drag_surface(
                            &s.corners,
                            Vec2::new(x / ow as f32 - c.x, y / oh as f32 - c.y),
                        );
                    }
                });
                ui.end_row();
                let before = mapping::angle(&s.corners, aspect).to_degrees();
                let mut deg = before;
                ui.label("Rotation");
                if ui
                    .add(
                        egui::DragValue::new(&mut deg)
                            .speed(0.25)
                            .suffix("°")
                            .fixed_decimals(1),
                    )
                    .changed()
                    && let Some(c) =
                        mapping::rotate(&s.corners, (deg - before).to_radians(), aspect)
                {
                    s.corners = c;
                }
                ui.end_row();
                let before = mapping::size(&s.corners, aspect) * 100.0;
                let mut size = before;
                ui.label("Size");
                if ui
                    .add(
                        egui::DragValue::new(&mut size)
                            .speed(0.2)
                            .suffix("%")
                            .fixed_decimals(1)
                            .range(2.0..=300.0),
                    )
                    .changed()
                    && let Some(c) = mapping::scale(&s.corners, size / before.max(0.01), aspect)
                {
                    s.corners = c;
                }
                ui.end_row();
            });
        ui.add_space(8.0);
        style::label(ui, "Corners");
        egui::Grid::new("corners")
            .num_columns(3)
            .spacing(vec2(10.0, 8.0))
            .show(ui, |ui| {
                for (n, name) in ["Top left", "Top right", "Bottom right", "Bottom left"]
                    .iter()
                    .enumerate()
                {
                    ui.label(*name);
                    let mut x = s.corners[n][0] * ow as f32;
                    let mut y = s.corners[n][1] * oh as f32;
                    let a = ui
                        .add(egui::DragValue::new(&mut x).speed(0.5).prefix("x ").fixed_decimals(0).fixed_decimals(0))
                        .changed();
                    let b = ui
                        .add(egui::DragValue::new(&mut y).speed(0.5).prefix("y ").fixed_decimals(0).fixed_decimals(0))
                        .changed();
                    if (a || b)
                        && let Some(c) = mapping::place_corner(
                            &s.corners,
                            n,
                            Vec2::new(x / ow as f32, y / oh as f32),
                            false,
                        )
                    {
                        s.corners = c;
                    }
                    ui.end_row();
                }
            });
    });
    if s.locked {
        ui.label(
            RichText::new("Locked: unlock it in Surfaces to move it.")
                .color(style::AMBER)
                .size(13.0),
        );
    }
}

fn look_tab(ui: &mut Ui, settings: &mut Settings, state: &mut MappingState) {
    let i = state.selected;
    let current = settings.output.surfaces[i].clone();
    style::label(ui, &format!("Surface {}", i + 1));
    let mut s = current.clone();
    ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(ui.available_width()));
    ui.add_space(6.0);
    ui.label(style::muted("On output"));
    let names: Vec<String> = settings
        .output
        .outputs
        .iter()
        .map(|o| o.name.clone())
        .collect();
    egui::ComboBox::from_id_salt("surface output")
        .selected_text(names.get(s.output).cloned().unwrap_or_default())
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (k, n) in names.iter().enumerate() {
                ui.selectable_value(&mut s.output, k, n);
            }
        });
    ui.add_space(6.0);
    ui.label(style::muted("Content"));
    let mut content = s.content;
    style::segmented(
        ui,
        &mut content,
        &[
            (Content::Main, "Main"),
            (Content::Same, "Same"),
            (Content::Complement, "Complement"),
        ],
    );
    ui.add_space(6.0);
    ui.label(style::muted("When it is a complement"));
    ui.add_enabled_ui(content == Content::Complement, |ui| {
        style::segmented(
            ui,
            &mut s.style,
            &[
                (ComplementStyle::MirrorMotion, "Mirror"),
                (ComplementStyle::Chase, "Chase"),
                (ComplementStyle::Echo, "Echo"),
            ],
        );
    });
    ui.add_space(8.0);
    ui.spacing_mut().slider_width = (ui.available_width() - 175.0).max(80.0);
    egui::Grid::new("look")
        .num_columns(2)
        .spacing(vec2(12.0, 10.0))
        .show(ui, |ui| {
            ui.label("Edge softness");
            ui.add(
                egui::Slider::new(&mut s.feather_px, 0.0..=120.0)
                    .suffix(" px")
                    .fixed_decimals(0),
            );
            ui.end_row();
            ui.label("Brightness");
            ui.add(egui::Slider::new(&mut s.brightness, 0.0..=1.5).fixed_decimals(2));
            ui.end_row();
            ui.label("Mirror");
            style::toggle(ui, &mut s.flip);
            ui.end_row();
        });
    ui.add_space(10.0);
    // For walls from the side and ceilings: the picture's real shape, and
    // which way is up, without moving the corners on the wall.
    style::label(ui, "Picture on the surface");
    ui.horizontal(|ui| {
        if ui.button("Turn 90°").on_hover_text("Turns the picture inside the same corners: for a ceiling, choose which edge is up").clicked() {
            s.corners = mapping::turn_picture(&s.corners);
        }
        if ui.button("Turn 180°").on_hover_text("Upside down, corners stay: for a projector hung from the ceiling, or a ceiling seen from the other side").clicked() {
            s.corners = mapping::turn_picture(&mapping::turn_picture(&s.corners));
        }
    });
    ui.add_space(6.0);
    ui.label(style::muted("Real shape where it lands"));
    let mut shape = shape_of(s.aspect);
    let names = [(Shape::Auto, "Auto, from the corners"), (Shape::Wide, "16:9"), (Shape::Classic, "4:3"), (Shape::Square, "1:1 square"), (Shape::Tall, "9:16 tall"), (Shape::Custom, "Other…")];
    let shown = names.iter().find(|(k, _)| *k == shape).map_or("", |(_, n)| *n);
    egui::ComboBox::from_id_salt("surface shape").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
        for (k, n) in names {
            ui.selectable_value(&mut shape, k, n);
        }
    });
    if shape != shape_of(s.aspect) {
        s.aspect = match shape {
            Shape::Auto => None,
            Shape::Wide => Some(16.0 / 9.0),
            Shape::Classic => Some(4.0 / 3.0),
            Shape::Square => Some(1.0),
            Shape::Tall => Some(9.0 / 16.0),
            Shape::Custom => Some(s.aspect.unwrap_or(2.0)),
        };
    }
    if let Some(a) = &mut s.aspect {
        ui.horizontal(|ui| {
            ui.label("Width ÷ height");
            ui.add(
                egui::DragValue::new(a)
                    .speed(0.01)
                    .range(0.1..=10.0)
                    .fixed_decimals(2),
            );
        });
    }
    ui.label(style::muted("Auto works it out from the corners. On a wall seen from the side or a ceiling the corners are slanted: set the real shape (measure it) and the picture and its 3D keep their true proportions; the grid's squares come out square on the wall."));
    // One or two Main surfaces, never none and never three.
    let others = mains(settings) - usize::from(current.content == Content::Main);
    if content == Content::Main && current.content != Content::Main && others >= MAX_MAINS {
        state.note =
            Some("Two surfaces are Main already: make one of them Same or Complement first.");
    } else if current.content == Content::Main && content != Content::Main && others == 0 {
        state.note = Some("One surface must stay Main: choose Main on another first.");
    } else {
        if content != current.content {
            state.note = None;
        }
        s.content = content;
    }
    if let Some(note) = state.note {
        ui.label(RichText::new(note).color(style::AMBER).size(13.0));
    }
    if s.output != current.output {
        state.output = s.output;
    }
    settings.output.surfaces[i] = s;
}

/// A network screen's link: how it is doing, its delay, the TV's modes.
fn network_link(ui: &mut Ui, o: &mut crate::settings::OutputConf, link: Option<crate::ui::NetLink>, searching: bool, actions: &mut Vec<UiAction>) {
    ui.add_space(6.0);
    if !link.as_ref().is_some_and(|l| l.connected) {
        ui.horizontal(|ui| {
            if searching {
                ui.add(egui::Spinner::new());
                ui.label(style::muted("Searching the network…"));
            } else {
                if super::icons::small(ui, super::icons::Icon::Refresh, "Search for the box again", style::MUTED).clicked() {
                    actions.push(UiAction::NetSearch);
                }
                ui.label(style::muted("Search again"));
            }
        });
    }
    match &link {
        Some(l) if l.connected => {
            let late = if l.shown > 0 { 100.0 * l.late as f32 / (l.shown + l.late) as f32 } else { 0.0 };
            ui.label(RichText::new(format!("Streaming · {:.0} ms behind the Mac, drawn ahead to match", l.latency_ms)).color(style::GREEN).size(13.0));
            ui.label(style::muted(format!("Network {:.0} ms round trip · {:.1}% frames late", l.round_trip_ms, late)));
            ui.label(style::muted(if l.hdr.is_empty() {
                "The TV takes SDR from this box: the picture is sent in SDR, colours tagged BT.709".to_string()
            } else {
                format!("The TV takes {}: the picture is sent in SDR, tagged BT.709", l.hdr.join(", "))
            }));
        }
        Some(l) => {
            ui.label(RichText::new(l.error.clone().unwrap_or_else(|| "Connecting…".into())).color(style::AMBER).size(13.0));
        }
        None => {
            ui.label(RichText::new("Waiting for the screen app to be open on the box").color(style::AMBER).size(13.0));
        }
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Fine tune");
        ui.add(
            egui::Slider::new(&mut o.delay_ms, crate::settings::NETWORK_DELAY_RANGE.0..=crate::settings::NETWORK_DELAY_RANGE.1)
                .step_by(5.0)
                .suffix(" ms"),
        )
        .on_hover_text("If the TV is a little late (or early) against the room, move it here: + shows it later.");
    });
    ui.horizontal(|ui| {
        ui.label("TV game mode").on_hover_text("Asks the TV for its game mode (HDMI ALLM): less delay of its own and steadier timing");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| style::toggle(ui, &mut o.tv_game_mode));
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Auto,
    Wide,
    Classic,
    Square,
    Tall,
    Custom,
}

fn shape_of(aspect: Option<f32>) -> Shape {
    let near = |x: f32| aspect.is_some_and(|a| (a - x).abs() < 1e-3);
    match aspect {
        None => Shape::Auto,
        _ if near(16.0 / 9.0) => Shape::Wide,
        _ if near(4.0 / 3.0) => Shape::Classic,
        _ if near(1.0) => Shape::Square,
        _ if near(9.0 / 16.0) => Shape::Tall,
        _ => Shape::Custom,
    }
}

fn output_tab(ui: &mut Ui, settings: &mut Settings, live: &Live, state: &mut MappingState, actions: &mut Vec<UiAction>) {
    let k = state.output;
    let others: Vec<String> = settings
        .output
        .outputs
        .iter()
        .enumerate()
        .filter(|(j, _)| *j != k)
        .filter_map(|(_, o)| o.display.clone())
        .collect();
    let o = &mut settings.output.outputs[k];
    style::label(ui, &format!("Output {}", k + 1));
    ui.add(egui::TextEdit::singleline(&mut o.name).desired_width(ui.available_width()));
    ui.add_space(6.0);
    ui.label(style::muted("Display"));
    egui::ComboBox::from_id_salt("output display")
        .selected_text(style::visual(&o.display.clone().unwrap_or_else(|| "None".into())))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut o.display, None, "None");
            for d in live.displays.iter().filter(|d| !others.contains(d)) {
                ui.selectable_value(&mut o.display, Some(d.clone()), style::visual(d));
            }
        });
    let (w, h) = output_px(live, k);
    let connected = live.output_sizes.get(k).is_some_and(|s| s.is_some());
    ui.label(style::muted(if connected {
        format!("Connected · {w} × {h}")
    } else {
        "Not connected".into()
    }));
    let network = o.display.as_deref().and_then(crate::settings::network_name).is_some();
    if network {
        network_link(ui, o, live.net_links.get(k).cloned().flatten(), live.net_searching, actions);
    }
    ui.add_space(8.0);
    ui.label(style::muted("On it now"));
    style::segmented(
        ui,
        &mut o.showing,
        &[
            (OutputPattern::Show, "Show"),
            (OutputPattern::Grid, "Grid"),
            (OutputPattern::White, "White"),
            (OutputPattern::Black, "Black"),
        ],
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("Black outside the surfaces");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            style::toggle(ui, &mut o.black_outside)
        });
    });
    ui.horizontal(|ui| {
        ui.label("Snap to 1%");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            style::toggle(ui, &mut settings.output.snap)
        });
    });
    if settings.output.outputs.len() > 1 {
        ui.add_space(12.0);
        if style::button(ui, "Remove this output")
            .on_hover_text("Its surfaces move to output 1")
            .clicked()
        {
            settings.output.outputs.remove(k);
            for s in &mut settings.output.surfaces {
                if s.output == k {
                    s.output = 0;
                } else if s.output > k {
                    s.output -= 1;
                }
            }
            state.output = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASPECT: f32 = 16.0 / 9.0;
    const Q: Corners = [[0.3, 0.2], [0.7, 0.2], [0.7, 0.8], [0.3, 0.8]];

    fn drag(handle: Handle, from: Vec2, fine: bool) -> Drag {
        Drag {
            surface: 0,
            handle,
            start_pointer: from,
            start_corners: Q,
            fine,
        }
    }

    #[test]
    fn a_pinned_corner_follows_the_pointer_exactly() {
        let d = drag(Handle::Corner(1), Vec2::new(0.71, 0.21), false);
        let c = apply_drag(&d, Vec2::new(0.81, 0.11), Tool::CornerPin, false, ASPECT).unwrap();
        assert!((Vec2::from(c[1]) - Vec2::new(0.8, 0.1)).length() < 1e-6);
        assert_eq!(c[0], Q[0]);
        // Shift: a tenth.
        let d = drag(Handle::Corner(1), Vec2::new(0.7, 0.2), true);
        let c = apply_drag(&d, Vec2::new(0.8, 0.2), Tool::CornerPin, false, ASPECT).unwrap();
        assert!((c[1][0] - 0.71).abs() < 1e-6);
        // A fold is refused (the caller keeps the last good corners).
        let d = drag(Handle::Corner(1), Vec2::new(0.7, 0.2), false);
        assert!(apply_drag(&d, Vec2::new(0.1, 0.95), Tool::CornerPin, false, ASPECT).is_none());
    }

    #[test]
    fn transform_corners_scale_about_the_middle() {
        let d = drag(Handle::Corner(2), Vec2::new(0.7, 0.8), false);
        // Pull the corner out to twice its distance from the middle.
        let c = apply_drag(&d, Vec2::new(0.9, 1.1), Tool::Transform, false, ASPECT).unwrap();
        assert!((mapping::size(&c, ASPECT) / mapping::size(&Q, ASPECT) - 2.0).abs() < 1e-3);
        assert!(mapping::centroid(&c).distance(mapping::centroid(&Q)) < 1e-6);
    }

    #[test]
    fn the_rotate_handle_turns_by_the_pointers_angle_and_shift_snaps() {
        // Centre (0.5, 0.5): from straight above to straight right is +90°.
        let d = drag(Handle::Rotate, Vec2::new(0.5, 0.0), false);
        let right = Vec2::new(0.5 + 0.5 / ASPECT, 0.5);
        let c = apply_drag(&d, right, Tool::Transform, false, ASPECT).unwrap();
        assert!((mapping::angle(&c, ASPECT).to_degrees() - 90.0).abs() < 0.01);
        // Shift: whole 15° steps.
        let d = drag(Handle::Rotate, Vec2::new(0.5, 0.0), true);
        let p = Vec2::new(
            0.5 + 0.2 * (0.35f32).sin() / ASPECT,
            0.5 - 0.2 * (0.35f32).cos(),
        );
        let c = apply_drag(&d, p, Tool::Transform, false, ASPECT).unwrap();
        let deg = mapping::angle(&c, ASPECT).to_degrees();
        assert!((deg / 15.0 - (deg / 15.0).round()).abs() < 1e-3, "{deg}°");
    }

    #[test]
    fn inside_moves_and_edges_move_two_corners() {
        let d = drag(Handle::Inside, Vec2::new(0.5, 0.5), false);
        let c = apply_drag(&d, Vec2::new(0.6, 0.45), Tool::Transform, false, ASPECT).unwrap();
        assert!((mapping::centroid(&c) - Vec2::new(0.6, 0.45)).length() < 1e-6);
        let d = drag(Handle::Edge(1), Vec2::new(0.7, 0.5), false);
        let c = apply_drag(&d, Vec2::new(0.75, 0.6), Tool::CornerPin, false, ASPECT).unwrap();
        assert!((c[1][0] - 0.75).abs() < 1e-6 && (c[2][0] - 0.75).abs() < 1e-6 && c[0] == Q[0]);
    }

    #[test]
    fn handles_come_before_insides_and_locked_ones_are_not_grabbed() {
        let canvas = Rect::from_min_size(pos2(100.0, 100.0), vec2(1600.0, 900.0));
        let corner = out_to_canvas(Vec2::from(Q[1]), canvas);
        let surfaces = [(0, Q, false), (1, mapping::FULL_FRAME, false)];
        assert_eq!(
            hit_test(
                &surfaces,
                0,
                Tool::CornerPin,
                corner + vec2(5.0, 5.0),
                canvas
            ),
            Some((0, Handle::Corner(1)))
        );
        // Not selected: inside the topmost surface.
        assert_eq!(
            hit_test(&surfaces, 1, Tool::CornerPin, pos2(900.0, 550.0), canvas),
            Some((1, Handle::Inside))
        );
        let locked = [(0, Q, true)];
        assert_eq!(
            hit_test(
                &locked,
                0,
                Tool::CornerPin,
                corner + vec2(-3.0, 3.0),
                canvas
            ),
            Some((0, Handle::Inside))
        );
        // The rotate handle, above the top edge.
        let r = rotate_handle(&Q, canvas);
        assert_eq!(
            hit_test(&surfaces, 0, Tool::Transform, r, canvas),
            Some((0, Handle::Rotate))
        );
    }

    #[test]
    fn drag_is_in_output_pixels_at_any_scale() {
        let canvas = Rect::from_min_size(pos2(40.0, 60.0), vec2(900.0, 506.25));
        let a = canvas_to_output(pos2(130.0, 60.0), canvas);
        assert!((a.x - 0.1).abs() < 1e-6 && a.y.abs() < 1e-6);
        let big = Rect::from_min_size(pos2(80.0, 120.0), vec2(1800.0, 1012.5));
        let b = canvas_to_output(pos2(80.0 + 180.0, 120.0), big);
        assert!((a - b).length() < 1e-6);
    }

    #[test]
    fn one_or_two_mains_never_none() {
        let mut settings = Settings::default();
        let mut state = MappingState::default();
        // The only surface, Main: not deleted.
        assert!(!can_delete(&settings, 0));
        // A duplicate of the only Main is a second Main; a third copy is Same.
        duplicate(&mut settings, &mut state);
        assert_eq!(mains(&settings), 2);
        duplicate(&mut settings, &mut state);
        assert_eq!(mains(&settings), 2);
        assert_eq!(settings.output.surfaces[2].content, Content::Same);
        // With two Mains either can go, then the last one stays.
        assert!(can_delete(&settings, 0));
        state.selected = 0;
        delete(&mut settings, &mut state);
        assert_eq!(mains(&settings), 1);
        let last_main = settings
            .output
            .surfaces
            .iter()
            .position(|s| s.content == Content::Main)
            .unwrap();
        assert!(!can_delete(&settings, last_main));
        assert_eq!(
            settings.clone().validated(),
            settings.clone().validated().validated()
        );
    }

    #[test]
    fn rotating_a_surface_keeps_it_editable_at_every_angle() {
        // Round and round in 15° steps (a ceiling at any angle): each step a
        // quad the tools still take, and back where it began after 360°.
        let mut c = Q;
        for _ in 0..24 {
            c = mapping::rotate(&c, SNAP_ANGLE, ASPECT).unwrap();
            assert!(mapping::is_valid(&c));
            let d = drag(Handle::Corner(0), Vec2::from(c[0]), false);
            let d = Drag {
                start_corners: c,
                ..d
            };
            assert!(
                apply_drag(
                    &d,
                    Vec2::from(c[0]) + Vec2::new(0.01, 0.01),
                    Tool::CornerPin,
                    false,
                    ASPECT
                )
                .is_some()
            );
        }
        for (a, b) in c.iter().zip(Q.iter()) {
            assert!((Vec2::from(*a) - Vec2::from(*b)).length() < 1e-4);
        }
    }

    /// The whole editor, headless: press on a corner, move, let go.
    #[test]
    fn dragging_a_corner_in_the_editor_moves_it_with_the_mouse() {
        let ctx = egui::Context::default();
        style::apply(&ctx);
        let mut settings = Settings::default();
        settings.output.surfaces[0].corners = Q;
        let live = Live {
            version: "test",
            output_sizes: vec![Some((1920, 1080))],
            ..Live::default()
        };
        let mut state = MappingState {
            tool: Tool::CornerPin,
            ..MappingState::default()
        };
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0));
        let run =
            |events: Vec<egui::Event>, settings: &mut Settings, state: &mut MappingState| {
                let input = egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                };
                let mut actions = Vec::new();
                let _ = ctx.run_ui(input, |ui| {
                    draw(ui, settings, &live, None, state, &mut actions)
                });
            };
        run(vec![], &mut settings, &mut state);
        // The canvas, worked out as draw() does from the whole screen.
        let area = Rect::from_min_max(pos2(0.0, HEADER_H + 10.0), screen.max);
        let (_, card, _, _) = layout(area);
        let cv = canvas_rect(card, 16.0 / 9.0);
        let handle = out_to_canvas(Vec2::from(Q[1]), cv);
        let press = handle + vec2(4.0, -3.0);
        let target = press + vec2(80.0, 40.0);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        run(
            vec![egui::Event::PointerMoved(press), button(press, true)],
            &mut settings,
            &mut state,
        );
        assert!(
            state.drag.is_some(),
            "the press did not grab the corner (handle at {handle:?})"
        );
        run(
            vec![egui::Event::PointerMoved(press + vec2(30.0, 10.0))],
            &mut settings,
            &mut state,
        );
        run(
            vec![egui::Event::PointerMoved(target)],
            &mut settings,
            &mut state,
        );
        run(vec![button(target, false)], &mut settings, &mut state);
        run(vec![], &mut settings, &mut state);
        assert!(state.drag.is_none(), "still dragging after the release");
        let moved = out_to_canvas(Vec2::from(settings.output.surfaces[0].corners[1]), cv);
        let expected = handle + vec2(80.0, 40.0);
        assert!(
            moved.distance(expected) < 1.0,
            "corner at {moved:?}, expected {expected:?}"
        );
    }
}
