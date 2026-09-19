//! A grid of cells: how an inventory is looked at.
//!
//! The pack on a Bim's back is three by three, the armoury fifteen by
//! fifteen, a storage twenty by twenty, the cold store ten by ten, and
//! they are all this one widget: square cells row by row, an icon in each
//! that has something in it, a count in the corner of a stack and a sliver
//! of health under a piece of armour. The grid does not know what a cell
//! *means* — it answers which cell the pointer is over, clicked,
//! ctrl-clicked or right-clicked, and the caller, which knows whether the
//! grid is a pack or a shelf, does the moving.
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
