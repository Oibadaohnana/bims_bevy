//! A grid of cells: how an inventory is looked at.
//!
//! The pack on a Bim's back is ten by five, a storage twenty by
//! twenty, the cold store ten by ten, and they are all one widget,
//! [`grid`]: square cells row by row, an icon in each that has something
//! in it, a count in the corner of a stack and a sliver of health under a
//! piece of armour. The grid does not know what a cell *means* — it
//! answers which cell the pointer is over, clicked, ctrl-clicked or
//! right-clicked, and the caller, which knows whether the grid is a pack
//! or a shelf, does the moving.
//!
//! The armoury is the other widget, [`lockers`]: the ship's locker grid,
//! ten across, with every thing laid over the cells its footprint covers
//! — a rifle along seven, a vest four by four — the way a survival game
//! lays an inventory out. The pointer moves them: a press and a pull
//! carries a thing, `R` on the way turns it, and letting go where the
//! ghost is green drops it there. Where a thing may lie is the caller's
//! rule, asked every frame; the widget only draws the answer.
//!
//! The whole grid is one egui response rather than one a cell: a storage
//! is four hundred cells, and four hundred widgets a frame is a cost with
//! nothing to show for it when one hit-test on the pointer answers the
//! same question.

use bevy_egui::egui::{self, Pos2, Rect, pos2, vec2};
use bims::combat::Item;

use crate::icons;
use crate::theme;

/// The space between cells, in points.
pub const GAP: f32 = 2.0;
/// How much of a cell the icon leaves clear round itself.
const INSET: f32 = 0.14;
/// The health bar under a piece: its height, and the room left for it.
const BAR: f32 = 3.0;

/// One thing in one cell.
pub struct Cell {
    pub item: Item,
    /// How many, for a stack; one for a piece or a weapon.
    pub count: u32,
    /// The tooltip: the name, and the numbers that matter.
    pub tip: String,
    /// How many rows down the grid it reaches: one for everything but a
    /// research key, which is two, kept in the upper cell. The cells it
    /// reaches over are left `None` by the caller, drawn as part of this
    /// one, and a click on them is a click on this one.
    pub rows: usize,
}

impl Cell {
    /// A cell for an item, as tall as the item is.
    pub fn new(item: Item, count: u32, tip: String) -> Cell {
        Cell {
            item,
            count,
            tip,
            rows: item.rows(),
        }
    }
}

/// What the pointer did to the grid this frame. A cell index is row by
/// row from the top left, the way the caller's list runs.
#[derive(Default)]
pub struct Picked {
    /// A plain left click.
    pub clicked: Option<usize>,
    /// A left click with Ctrl held — the quick move — and where.
    pub ctrl_clicked: Option<(usize, Pos2)>,
    /// A right click, and where on the window, for the pop-up.
    pub right_clicked: Option<(usize, Pos2)>,
}

/// Lay out and draw a grid of `cols` by `rows` cells of `side` points,
/// `cells[i]` in the *i*th. Cells past the end of the list are empty, and
/// an empty cell is drawn but never picked.
pub fn grid(
    ui: &mut egui::Ui,
    cols: usize,
    rows: usize,
    side: f32,
    cells: &[Option<Cell>],
) -> Picked {
    let step = side + GAP;
    let size = vec2(cols as f32 * step - GAP, rows as f32 * step - GAP);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter_at(rect);

    // The cell a tall item in the row above reaches down into, if one
    // does: the cell it is kept in.
    let head_of = |i: usize| -> usize {
        if i >= cols
            && let Some(Some(above)) = cells.get(i - cols)
            && above.rows > 1
        {
            return i - cols;
        }
        i
    };
    // Which cell the pointer is in, if it is in one at all and the cell
    // has something in it — or is reached into by a tall one above it,
    // which is the same thing.
    let cell_at = |p: Pos2| -> Option<usize> {
        if !rect.contains(p) {
            return None;
        }
        let c = ((p.x - rect.min.x) / step).floor() as usize;
        let r = ((p.y - rect.min.y) / step).floor() as usize;
        if c >= cols || r >= rows {
            return None;
        }
        let i = head_of(r * cols + c);
        // The gap between cells is nobody's — bar the gap inside a tall
        // item, which is its own.
        let tall = cells
            .get(i)
            .is_some_and(|cell| cell.as_ref().is_some_and(|c| c.rows > 1));
        let inside = (p.x - rect.min.x) - c as f32 * step <= side
            && ((p.y - rect.min.y) - r as f32 * step <= side || (tall && i != r * cols + c));
        (inside && cells.get(i).is_some_and(|cell| cell.is_some())).then_some(i)
    };
    let hovered = response.hover_pos().and_then(cell_at);

    for r in 0..rows {
        for c in 0..cols {
            let i = r * cols + c;
            // A cell a tall item reaches into is drawn as part of it.
            if head_of(i) != i {
                continue;
            }
            let tall = cells
                .get(i)
                .and_then(|cell| cell.as_ref().map(|c| c.rows))
                .unwrap_or(1)
                .clamp(1, rows - r);
            let cell_rect = Rect::from_min_size(
                pos2(rect.min.x + c as f32 * step, rect.min.y + r as f32 * step),
                vec2(side, tall as f32 * step - GAP),
            );
            let lit = hovered == Some(i);
            painter.rect(
                cell_rect,
                3.0,
                if lit {
                    theme::RAISED
                } else {
                    theme::PANEL_DEEP
                },
                egui::Stroke::new(1.0, if lit { theme::ACCENT } else { theme::LINE }),
                egui::StrokeKind::Inside,
            );
            let Some(Some(cell)) = cells.get(i) else {
                continue;
            };
            // A tiered thing's cell is washed and edged in its tier's
            // colour — blue at two, gold at three — under the icon.
            if let Some(tint) = theme::item_tint(cell.item) {
                theme::tint_cell(&painter, cell_rect, 3.0, tint);
            }
            let mut icon_rect = cell_rect.shrink(side * INSET);
            if tall > 1 {
                // A tall item's icon is drawn as tall as the cell, not
                // squared off: a key is a tall thing.
                icon_rect = Rect::from_center_size(
                    cell_rect.center(),
                    vec2(
                        side * (1.0 - 2.0 * INSET),
                        cell_rect.height() - 2.0 * side * INSET,
                    ),
                );
            }
            if let Item::Armour(piece) = cell.item {
                // The health left, as a sliver along the bottom: blue while
                // the piece is whole, red once it is broken.
                icon_rect.max.y -= BAR + 1.0;
                let bar = Rect::from_min_max(
                    pos2(cell_rect.min.x + 3.0, cell_rect.max.y - BAR - 2.0),
                    pos2(cell_rect.max.x - 3.0, cell_rect.max.y - 2.0),
                );
                let max = piece.stats().health.max(1.0);
                theme::bar_in(
                    &painter,
                    bar,
                    piece.health / max,
                    if piece.broken() {
                        theme::BAD
                    } else {
                        theme::ARMOUR
                    },
                );
            }
            icons::icon(&painter, icon_rect, cell.item);
            if cell.count > 1 {
                // The count, bottom right, on a dark pill so it reads over
                // the icon.
                let font = egui::FontId::proportional((side * 0.34).clamp(9.0, 12.0));
                let text = cell.count.to_string();
                let galley = painter.layout_no_wrap(text, font, theme::INK);
                let pad = 2.0;
                let pill = Rect::from_min_max(
                    pos2(
                        cell_rect.max.x - galley.size().x - pad * 2.0 - 1.0,
                        cell_rect.max.y - galley.size().y - pad - 1.0,
                    ),
                    pos2(cell_rect.max.x - 1.0, cell_rect.max.y - 1.0),
                );
                painter.rect_filled(pill, 2.0, egui::Color32::from_black_alpha(190));
                painter.galley(pill.min + vec2(pad, pad * 0.5), galley, theme::INK);
            }
        }
    }

    let mut picked = Picked::default();
    if let Some(i) = hovered
        && let Some(Some(cell)) = cells.get(i)
        && !cell.tip.is_empty()
    {
        response.clone().on_hover_text(cell.tip.clone());
    }
    let at = response.interact_pointer_pos();
    if response.clicked()
        && let Some(i) = at.and_then(cell_at)
    {
        let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        if ctrl {
            picked.ctrl_clicked = at.map(|p| (i, p));
        } else {
            picked.clicked = Some(i);
        }
    }
    if response.secondary_clicked()
        && let Some(p) = at
        && let Some(i) = cell_at(p)
    {
        picked.right_clicked = Some((i, p));
    }
    picked
}

// --- the lockers: a grid things are laid on ------------------------------------

/// One thing laid on the lockers' grid, as the widget draws it: where its
/// top-left cell is, how many cells it covers as laid, which way round it
/// is — the picture is turned with it — and what it is.
pub struct Laid {
    pub x: usize,
    pub y: usize,
    pub cols: usize,
    pub rows: usize,
    pub turned: bool,
    pub cell: Cell,
}

/// A thing being dragged across the lockers: which of the caller's list,
/// which of its cells the pointer took hold of — so the thing stays under
/// the hand rather than jumping to it — and which way round it is being
/// carried, which `R` changes on the way.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Drag {
    pub index: usize,
    pub grab: (usize, usize),
    pub turned: bool,
}

/// What the pointer did to the lockers this frame.
#[derive(Default)]
pub struct Moved {
    /// A left click with Ctrl held on a thing — the quick take — and where.
    pub ctrl_clicked: Option<(usize, Pos2)>,
    /// A right click on a thing, and where, for the pop-up.
    pub right_clicked: Option<(usize, Pos2)>,
    /// A thing let go of somewhere: which, the cell its top-left corner
    /// would take, and which way round. Only where it would lie — the
    /// caller's rule said so while it was carried.
    pub dropped: Option<(usize, usize, usize, bool)>,
    /// `R` over a thing that is not being carried: turn it where it lies.
    pub turn: Option<usize>,
}

/// Lay out and draw the lockers' grid — `cols` by `rows` cells of `side`
/// points, the last `blocked` cells of the bottom row not part of any
/// locker — with `things` laid on it, and let the pointer move them: a
/// press on a thing and a pull carries it, `turn` on the way turns it, and
/// letting go where it would lie drops it there — `fits(index, x, y,
/// turned)` is the caller's rule, asked every frame the thing is over a
/// place, which colours the ghost. `drag` is the widget's memory between
/// frames and the caller's to keep; with `movable` off nothing is carried
/// or turned — a body's pack is looked at, not tidied. A plain click does
/// nothing; ctrl and right clicks are the stack grid's.
#[allow(clippy::too_many_arguments)]
pub fn lockers(
    ui: &mut egui::Ui,
    cols: usize,
    rows: usize,
    blocked: usize,
    side: f32,
    things: &[Laid],
    drag: &mut Option<Drag>,
    fits: &dyn Fn(usize, usize, usize, bool) -> bool,
    turn: egui::Key,
    movable: bool,
) -> Moved {
    let step = side + GAP;
    let size = vec2(cols as f32 * step - GAP, rows as f32 * step - GAP);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let cell_rect = |x: usize, y: usize, w: usize, h: usize| {
        Rect::from_min_size(
            pos2(rect.min.x + x as f32 * step, rect.min.y + y as f32 * step),
            vec2(w as f32 * step - GAP, h as f32 * step - GAP),
        )
    };
    // Which cell a point is in, ignoring the gaps: the gap belongs to the
    // cell before it, which is what a drag wants.
    let cell_at = |p: Pos2| -> Option<(usize, usize)> {
        if !rect.contains(p) {
            return None;
        }
        let c = ((p.x - rect.min.x) / step).floor() as usize;
        let r = ((p.y - rect.min.y) / step).floor() as usize;
        (c < cols && r < rows).then_some((c, r))
    };
    let thing_at = |(c, r): (usize, usize)| -> Option<usize> {
        things
            .iter()
            .position(|t| c >= t.x && c < t.x + t.cols && r >= t.y && r < t.y + t.rows)
    };
    let hovered = response.hover_pos().and_then(cell_at).and_then(thing_at);

    // The cells, empty: a blocked one darker and without an edge.
    for r in 0..rows {
        for c in 0..cols {
            let blocked = r * cols + c >= rows * cols - blocked;
            painter.rect(
                cell_rect(c, r, 1, 1),
                3.0,
                theme::PANEL_DEEP,
                if blocked {
                    egui::Stroke::NONE
                } else {
                    egui::Stroke::new(1.0, theme::LINE)
                },
                egui::StrokeKind::Inside,
            );
        }
    }

    // What the drag is doing this frame, before the things are drawn, so
    // the ghost goes under the carried thing and the carried thing over
    // the rest.
    let pointer = response.interact_pointer_pos().or(response.hover_pos());
    let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
    let turn_key = ui.input(|i| i.key_pressed(turn));
    if movable
        && response.drag_started()
        && !ctrl
        && let Some(p) = pointer
        && let Some(at) = cell_at(p)
        && let Some(i) = thing_at(at)
    {
        *drag = Some(Drag {
            index: i,
            grab: (at.0 - things[i].x, at.1 - things[i].y),
            turned: things[i].turned,
        });
    }
    // A square thing has no other way round to turn to.
    let square = |i: usize| things.get(i).is_some_and(|t| t.cols == t.rows);
    if let Some(d) = drag.as_mut()
        && turn_key
        && !square(d.index)
    {
        d.turned = !d.turned;
        // Turned about the cell in hand, as far as the thing allows.
        let t = &things[d.index];
        let (w, h) = if d.turned == t.turned {
            (t.cols, t.rows)
        } else {
            (t.rows, t.cols)
        };
        d.grab = (d.grab.0.min(w - 1), d.grab.1.min(h - 1));
    }
    // Where the carried thing would land: the cell under the pointer less
    // the cell it was taken by. Off the grid to the top or left is nowhere.
    let target = drag.and_then(|d| {
        let p = pointer?;
        let (c, r) = cell_at(p)?;
        let x = c.checked_sub(d.grab.0)?;
        let y = r.checked_sub(d.grab.1)?;
        Some((x, y))
    });
    let mut moved = Moved::default();

    for (i, thing) in things.iter().enumerate() {
        let carried = drag.is_some_and(|d| d.index == i);
        let r = cell_rect(thing.x, thing.y, thing.cols, thing.rows);
        let lit = hovered == Some(i) && drag.is_none();
        if carried {
            // Its place, hollow, while it is in the hand.
            painter.rect_stroke(
                r,
                3.0,
                egui::Stroke::new(1.0, theme::MUTED),
                egui::StrokeKind::Inside,
            );
            continue;
        }
        draw_laid(&painter, r, thing, lit, 1.0);
    }

    // The ghost where it would land, green or red by the rule, and the
    // thing itself under the pointer.
    if let Some(d) = *drag
        && let Some(thing) = things.get(d.index)
    {
        let (w, h) = if d.turned == thing.turned {
            (thing.cols, thing.rows)
        } else {
            (thing.rows, thing.cols)
        };
        if let Some((x, y)) = target {
            let ok = fits(d.index, x, y, d.turned);
            let ghost = Rect::from_min_size(
                pos2(rect.min.x + x as f32 * step, rect.min.y + y as f32 * step),
                vec2(w as f32 * step - GAP, h as f32 * step - GAP),
            );
            let tint = if ok { theme::ACCENT } else { theme::BAD };
            painter.rect(
                ghost.intersect(rect),
                3.0,
                tint.gamma_multiply(0.25),
                egui::Stroke::new(1.5, tint),
                egui::StrokeKind::Inside,
            );
        }
        if let Some(p) = pointer {
            let top = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("lockers-drag"),
            ));
            let at = pos2(
                p.x - (d.grab.0 as f32 + 0.5) * step,
                p.y - (d.grab.1 as f32 + 0.5) * step,
            );
            let held = Rect::from_min_size(at, vec2(w as f32 * step - GAP, h as f32 * step - GAP));
            let ghost = Laid {
                x: 0,
                y: 0,
                cols: w,
                rows: h,
                turned: d.turned,
                cell: Cell {
                    item: thing.cell.item,
                    count: thing.cell.count,
                    tip: String::new(),
                    rows: 1,
                },
            };
            draw_laid(&top, held, &ghost, true, 0.85);
        }
    }

    if response.drag_stopped()
        && let Some(d) = drag.take()
        && let Some((x, y)) = target
        && let Some(thing) = things.get(d.index)
        && (x != thing.x || y != thing.y || d.turned != thing.turned)
        && fits(d.index, x, y, d.turned)
    {
        moved.dropped = Some((d.index, x, y, d.turned));
    }
    if drag.is_some() && !response.dragged() && !response.drag_stopped() {
        // The pointer got away — a window closed under it, the button
        // released elsewhere; nothing moves.
        *drag = None;
    }

    if let Some(i) = hovered
        && drag.is_none()
    {
        if movable && turn_key && !square(i) {
            moved.turn = Some(i);
        }
        if !things[i].cell.tip.is_empty() {
            response.clone().on_hover_text(things[i].cell.tip.clone());
        }
    }
    let at = response.interact_pointer_pos();
    if response.clicked()
        && ctrl
        && let Some(p) = at
        && let Some(i) = cell_at(p).and_then(thing_at)
    {
        moved.ctrl_clicked = Some((i, p));
    }
    if response.secondary_clicked()
        && let Some(p) = at
        && let Some(i) = cell_at(p).and_then(thing_at)
    {
        moved.right_clicked = Some((i, p));
    }
    moved
}

/// One laid thing: its footprint filled and edged — lit when the pointer
/// rests on it — washed in its tier's colour, the picture turned with it,
/// a health sliver along the bottom of a piece of armour, and the count
/// in the corner of a stack. `alpha` fades the one in the hand a little.
fn draw_laid(painter: &egui::Painter, r: Rect, thing: &Laid, lit: bool, alpha: f32) {
    let fade = |c: egui::Color32| c.gamma_multiply(alpha);
    painter.rect(
        r,
        3.0,
        fade(if lit {
            theme::RAISED
        } else {
            theme::PANEL_DEEP
        }),
        egui::Stroke::new(1.0, fade(if lit { theme::ACCENT } else { theme::MUTED })),
        egui::StrokeKind::Inside,
    );
    if let Some(tint) = theme::item_tint(thing.cell.item) {
        theme::tint_cell(painter, r, 3.0, tint);
    }
    let short = r.width().min(r.height());
    let mut icon_rect = r.shrink(short * INSET);
    if let Item::Armour(piece) = thing.cell.item {
        icon_rect.max.y -= BAR + 1.0;
        let bar = Rect::from_min_max(
            pos2(r.min.x + 3.0, r.max.y - BAR - 2.0),
            pos2(r.max.x - 3.0, r.max.y - 2.0),
        );
        let max = piece.stats().health.max(1.0);
        theme::bar_in(
            painter,
            bar,
            piece.health / max,
            if piece.broken() {
                theme::BAD
            } else {
                theme::ARMOUR
            },
        );
    }
    icons::laid(painter, icon_rect, thing.cell.item, thing.turned);
    if thing.cell.count > 1 {
        let font = egui::FontId::proportional((short * 0.34).clamp(9.0, 12.0));
        let text = thing.cell.count.to_string();
        let galley = painter.layout_no_wrap(text, font, theme::INK);
        let pad = 2.0;
        let pill = Rect::from_min_max(
            pos2(
                r.max.x - galley.size().x - pad * 2.0 - 1.0,
                r.max.y - galley.size().y - pad - 1.0,
            ),
            pos2(r.max.x - 1.0, r.max.y - 1.0),
        );
        painter.rect_filled(pill, 2.0, egui::Color32::from_black_alpha(190));
        painter.galley(pill.min + vec2(pad, pad * 0.5), galley, theme::INK);
    }
}
