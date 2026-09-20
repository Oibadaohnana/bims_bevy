//! The station builder: a grid to sketch a station's rough shape on.
//!
//! `bims stationbuilder [name]` opens a square of tiles and a brush. Left
//! drag paints — a rectangle from corner to corner, or tile by tile with
//! the pen — and right drag rubs out; the wheel zooms about the pointer
//! and a middle drag pans. Four things can be painted: **deck** (the hull
//! — every deck tile that touches void is drawn as skin, so the outline
//! is the outside wall without being drawn twice), **wall** (a partition
//! inside), **door** and **airlock**. Nothing here is a part: the sketch
//! is the rough edges, saved as text to `stations/<name>.txt` at the
//! repository root (`Sketch::to_text`, one character a tile) for a plan
//! in `world::station` to be written from by hand, and read back the next
//! time the same name is opened. The centre lines are drawn faintly
//! because the port belongs in the west skin on the two rows either side
//! of the middle, which is where every plan's is.
//!
//! A tool, not a screen of the game: it has no `Game`, no sound and no
//! sheet, and its two shortcuts — Ctrl+S saves, Ctrl+Z undoes — are its
//! own rather than `keys::Action`s, since neither belongs on the Controls
//! page beside Map and Follow.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use crate::canvas::{Pointer, canvas_painter, rect_of, root_ui, zoom_factor};
use crate::{Screen, theme};

/// How many tiles a sketch is across, at least, at most, and to begin with
/// — the smallest plan is 34 and the arena 72.
pub const MIN_SIDE: u32 = 16;
pub const MAX_SIDE: u32 = 96;
pub const DEFAULT_SIDE: u32 = 64;

/// The name a sketch is opened under when the command names none.
pub const DEFAULT_NAME: &str = "sketch";

/// How long a line on the status stays up, in seconds.
const STATUS_SECONDS: f64 = 4.0;

/// The colours of the picture: the deck, the skin round it, a wall, a
/// door, an airlock, and the grid over the lot.
const DECK: egui::Color32 = egui::Color32::from_rgb(0x2c, 0x3a, 0x33);
const SKIN: egui::Color32 = egui::Color32::from_rgb(0x9a, 0xa8, 0xa0);
const WALL: egui::Color32 = egui::Color32::from_rgb(0x62, 0x74, 0x6a);
const DOOR: egui::Color32 = theme::ACCENT;
const AIRLOCK: egui::Color32 = theme::TIER_THREE;
const GRID: egui::Color32 = egui::Color32::from_rgba_premultiplied(0x2a, 0x3b, 0x33, 0xa0);
const GRID_EIGHTH: egui::Color32 = egui::Color32::from_rgba_premultiplied(0x3f, 0x56, 0x4a, 0xc0);
const CENTRE: egui::Color32 = egui::Color32::from_rgba_premultiplied(0x5f, 0x9c, 0x7e, 0x80);

pub struct StationBuilderPlugin;

impl Plugin for StationBuilderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::StationBuilder), open)
            .add_systems(
                EguiPrimaryContextPass,
                frame.run_if(in_state(Screen::StationBuilder)),
            );
    }
}

/// Which sketch the command asked for: the `[name]` after `stationbuilder`.
#[derive(Resource, Clone, Debug)]
pub struct SketchName(pub String);

/// What a tile of the sketch is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Cell {
    #[default]
    Void,
    Deck,
    Wall,
    Door,
    Airlock,
}

impl Cell {
    pub const BRUSHES: [Cell; 4] = [Cell::Deck, Cell::Wall, Cell::Door, Cell::Airlock];

    /// The character a tile is saved as.
    pub fn glyph(self) -> char {
        match self {
            Cell::Void => ' ',
            Cell::Deck => '.',
            Cell::Wall => '#',
            Cell::Door => 'D',
            Cell::Airlock => 'A',
        }
    }

    pub fn of_glyph(c: char) -> Option<Cell> {
        Some(match c {
            ' ' => Cell::Void,
            '.' => Cell::Deck,
            '#' => Cell::Wall,
            'D' | 'd' => Cell::Door,
            'A' | 'a' => Cell::Airlock,
            _ => return None,
        })
    }

    fn label(self) -> &'static str {
        match self {
            Cell::Void => "Void",
            Cell::Deck => "Deck",
            Cell::Wall => "Wall",
            Cell::Door => "Door",
            Cell::Airlock => "Airlock",
        }
    }
}

/// The sketch: a square of cells, `side` across.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Sketch {
    pub side: u32,
    cells: Vec<Cell>,
}

impl Sketch {
    pub fn new(side: u32) -> Sketch {
        let side = side.clamp(MIN_SIDE, MAX_SIDE);
        Sketch {
            side,
            cells: vec![Cell::Void; (side * side) as usize],
        }
    }

    pub fn get(&self, x: i32, y: i32) -> Cell {
        if x < 0 || y < 0 || x >= self.side as i32 || y >= self.side as i32 {
            return Cell::Void;
        }
        self.cells[(y as u32 * self.side + x as u32) as usize]
    }

    pub fn set(&mut self, x: i32, y: i32, cell: Cell) {
        if x < 0 || y < 0 || x >= self.side as i32 || y >= self.side as i32 {
            return;
        }
        self.cells[(y as u32 * self.side + x as u32) as usize] = cell;
    }

    /// Every tile of the rectangle with `a` and `b` for corners, either
    /// way round.
    pub fn fill(&mut self, a: (i32, i32), b: (i32, i32), cell: Cell) {
        for y in a.1.min(b.1)..=a.1.max(b.1) {
            for x in a.0.min(b.0)..=a.0.max(b.0) {
                self.set(x, y, cell);
            }
        }
    }

    /// The same drawing on a grid of another size, anchored at the top
    /// left; what falls off is lost.
    pub fn resized(&self, side: u32) -> Sketch {
        let mut out = Sketch::new(side);
        for y in 0..self.side.min(out.side) as i32 {
            for x in 0..self.side.min(out.side) as i32 {
                out.set(x, y, self.get(x, y));
            }
        }
        out
    }

    /// Whether a hull tile is skin: not void, with void (or the grid's
    /// edge) among its eight neighbours. What `world::station`'s layout
    /// calls `skin`, so the picture here is the picture the plan will be.
    pub fn is_skin(&self, x: i32, y: i32) -> bool {
        self.get(x, y) != Cell::Void
            && (-1..=1).any(|dx| {
                (-1..=1).any(|dy| (dx != 0 || dy != 0) && self.get(x + dx, y + dy) == Cell::Void)
            })
    }

    /// How many tiles there are of each kind, in [`Cell::BRUSHES`] order,
    /// and how many of the hull are skin.
    pub fn counts(&self) -> ([u32; 4], u32) {
        let mut counts = [0u32; 4];
        let mut skin = 0;
        for y in 0..self.side as i32 {
            for x in 0..self.side as i32 {
                let cell = self.get(x, y);
                if let Some(i) = Cell::BRUSHES.iter().position(|&b| b == cell) {
                    counts[i] += 1;
                }
                if self.is_skin(x, y) {
                    skin += 1;
                }
            }
        }
        (counts, skin)
    }

    /// The file: a few `# ` lines saying what it is, `name = ` and
    /// `side = `, then `side` rows of `side` characters, one a tile.
    pub fn to_text(&self, name: &str) -> String {
        let mut out = String::new();
        out.push_str("# Bims station sketch, from `bims stationbuilder`.\n");
        out.push_str(
            "# One character a tile: ' ' void, '.' deck, '#' wall, 'D' door, 'A' airlock.\n",
        );
        out.push_str(
            "# The skin is every hull tile touching void; the port is the west airlock.\n",
        );
        out.push_str(&format!("name = {name}\n"));
        out.push_str(&format!("side = {}\n", self.side));
        for y in 0..self.side as i32 {
            let row: String = (0..self.side as i32)
                .map(|x| self.get(x, y).glyph())
                .collect();
            // A row's trailing void is not written, since an editor would
            // strip it anyway; `from_text` pads it back.
            out.push_str(row.trim_end());
            out.push('\n');
        }
        out
    }

    /// A sketch read back from [`Sketch::to_text`]'s form: rows shorter
    /// than `side` are void to the right, rows beyond `side` are ignored,
    /// and a character that is not a tile is an error naming its row.
    pub fn from_text(text: &str) -> Result<(String, Sketch), String> {
        let mut name = DEFAULT_NAME.to_string();
        let mut side = None;
        let mut rows: Vec<&str> = Vec::new();
        for line in text.lines() {
            if side.is_none() {
                if line.starts_with('#') || line.trim().is_empty() {
                    continue;
                }
                if let Some(rest) = line.strip_prefix("name =") {
                    name = rest.trim().to_string();
                    continue;
                }
                if let Some(rest) = line.strip_prefix("side =") {
                    let n: u32 = rest
                        .trim()
                        .parse()
                        .map_err(|_| format!("side is not a number: {line:?}"))?;
                    if !(MIN_SIDE..=MAX_SIDE).contains(&n) {
                        return Err(format!("side {n} is outside {MIN_SIDE}..={MAX_SIDE}"));
                    }
                    side = Some(n);
                    continue;
                }
                return Err(format!("unexpected line before the grid: {line:?}"));
            } else {
                rows.push(line);
            }
        }
        let side = side.ok_or("no `side = ` line")?;
        let mut sketch = Sketch::new(side);
        for (y, row) in rows.iter().take(side as usize).enumerate() {
            for (x, c) in row.chars().take(side as usize).enumerate() {
                let cell = Cell::of_glyph(c)
                    .ok_or_else(|| format!("row {}: {c:?} is not a tile", y + 1))?;
                sketch.set(x as i32, y as i32, cell);
            }
        }
        Ok((name, sketch))
    }
}

/// Where the sketches live: `stations/` at the repository root — beside
/// the crates, since a sketch becomes a plan in `crates/world` — or
/// wherever `BIMS_STATIONS_DIR` says.
pub fn stations_dir() -> std::path::PathBuf {
    match std::env::var_os("BIMS_STATIONS_DIR") {
        Some(dir) => dir.into(),
        None => std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../stations"),
    }
}

/// A sketch's file, by name. A name is letters, digits, `-` and `_`, so
/// it cannot name a path.
pub fn sketch_path(name: &str) -> std::path::PathBuf {
    stations_dir().join(format!("{name}.txt"))
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// How a drag paints: a rectangle from where it began, or every tile it
/// passes over.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tool {
    Rect,
    Pen,
}

#[derive(Resource)]
pub struct StationScreen {
    name: String,
    sketch: Sketch,
    /// Every state before an edit, oldest first; Ctrl+Z pops one.
    undo: Vec<Sketch>,
    /// What the sketch was when it was last saved or loaded, to say
    /// whether there is anything to save.
    saved: Sketch,
    brush: Cell,
    tool: Tool,
    /// A drag under way: the tile it began on and whether it rubs out.
    drag: Option<((i32, i32), bool)>,
    /// The tile under the pointer, on the grid or not.
    hover: Option<(i32, i32)>,
    /// A tile's size in points and where tile (0, 0)'s corner is, in
    /// points from the canvas's corner. Nought until the first frame fits
    /// the grid to the canvas, and fitted again whenever the canvas
    /// changes size — the window opens small and grows on the first
    /// frames — which loses a zoom, and nothing else.
    tile: f32,
    origin: Vec2,
    fitted_to: Vec2,
    /// The side the slider is at; `Resize` applies it.
    side_edit: u32,
    /// A line for the status, and when to take it down.
    status: Option<(String, f64)>,
}

fn open(mut commands: Commands, name: Option<Res<SketchName>>) {
    let name = name
        .map(|n| n.0.clone())
        .unwrap_or_else(|| DEFAULT_NAME.to_string());
    let mut status = None;
    let sketch = match std::fs::read_to_string(sketch_path(&name)) {
        Ok(text) => match Sketch::from_text(&text) {
            Ok((_, sketch)) => {
                status = Some((format!("Opened {}", sketch_path(&name).display()), f64::MAX));
                sketch
            }
            Err(why) => {
                status = Some((format!("Could not read the sketch: {why}"), f64::MAX));
                Sketch::new(DEFAULT_SIDE)
            }
        },
        Err(_) => Sketch::new(DEFAULT_SIDE),
    };
    let side_edit = sketch.side;
    commands.insert_resource(StationScreen {
        name,
        saved: sketch.clone(),
        sketch,
        undo: Vec::new(),
        brush: Cell::Deck,
        tool: Tool::Rect,
        drag: None,
        hover: None,
        tile: 0.0,
        origin: Vec2::ZERO,
        fitted_to: Vec2::ZERO,
        side_edit,
        status,
    });
}

impl StationScreen {
    fn say(&mut self, now: f64, what: impl Into<String>) {
        self.status = Some((what.into(), now + STATUS_SECONDS));
    }

    /// Remember the sketch as it is, for Ctrl+Z.
    fn remember(&mut self) {
        self.undo.push(self.sketch.clone());
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
    }

    fn undo(&mut self) {
        if let Some(before) = self.undo.pop() {
            self.sketch = before;
            self.side_edit = self.sketch.side;
        }
    }

    fn save(&mut self, now: f64) {
        let path = sketch_path(&self.name);
        let written = path
            .parent()
            .map(std::fs::create_dir_all)
            .unwrap_or(Ok(()))
            .and_then(|_| std::fs::write(&path, self.sketch.to_text(&self.name)));
        match written {
            Ok(()) => {
                self.saved = self.sketch.clone();
                self.say(now, format!("Saved {}", path.display()));
            }
            Err(why) => self.say(now, format!("Could not save {}: {why}", path.display())),
        }
    }

    /// Fit the whole grid into `canvas`, centred.
    fn fit(&mut self, canvas: crate::shapes::Rect) {
        let size = canvas.size();
        self.tile = ((size.x.min(size.y) - 24.0) / self.sketch.side as f32).max(1.0);
        let span = self.tile * self.sketch.side as f32;
        self.origin = (size - Vec2::splat(span)) / 2.0;
        self.fitted_to = size;
    }

    /// The tile under a point of the canvas, on the grid or off it.
    fn tile_at(&self, p: Vec2) -> (i32, i32) {
        let t = (p - self.origin) / self.tile;
        (t.x.floor() as i32, t.y.floor() as i32)
    }

    fn tile_rect(&self, canvas_min: Vec2, x: i32, y: i32) -> egui::Rect {
        let min = canvas_min + self.origin + Vec2::new(x as f32, y as f32) * self.tile;
        egui::Rect::from_min_size(egui::pos2(min.x, min.y), egui::vec2(self.tile, self.tile))
    }
}

fn frame(mut contexts: EguiContexts, mut screen: ResMut<StationScreen>) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let screen = &mut *screen;
    let now = ctx.input(|i| i.time);

    // --- the header ----------------------------------------------------------
    let mut root = root_ui(&ctx);
    let mut resize_to = None;
    let mut fit = false;
    egui::Panel::top("station-header").show(&mut root, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.heading("Station builder");
            let dirty = screen.sketch != screen.saved;
            ui.label(
                egui::RichText::new(format!("{}{}", screen.name, if dirty { " *" } else { "" }))
                    .color(if dirty { theme::CAUTION } else { theme::MUTED }),
            );
            ui.separator();
            if ui.button("Save").on_hover_text("Ctrl+S").clicked() {
                screen.save(now);
            }
            if ui
                .add_enabled(!screen.undo.is_empty(), egui::Button::new("Undo"))
                .on_hover_text("Ctrl+Z")
                .clicked()
            {
                screen.undo();
            }
            ui.separator();
            for (tool, label, tip) in [
                (Tool::Rect, "Rect", "Drag a rectangle from corner to corner"),
                (Tool::Pen, "Pen", "Paint every tile the pointer passes over"),
            ] {
                if ui
                    .add(theme::toggle_button(screen.tool == tool, label))
                    .on_hover_text(tip)
                    .clicked()
                {
                    screen.tool = tool;
                }
            }
            ui.separator();
            for brush in Cell::BRUSHES {
                if ui
                    .add(theme::toggle_button(screen.brush == brush, brush.label()))
                    .clicked()
                {
                    screen.brush = brush;
                }
            }
            ui.label(egui::RichText::new("right button rubs out").color(theme::MUTED));
            ui.separator();
            ui.label("Side");
            ui.add(egui::DragValue::new(&mut screen.side_edit).range(MIN_SIDE..=MAX_SIDE));
            if ui
                .add_enabled(
                    screen.side_edit != screen.sketch.side,
                    egui::Button::new("Resize"),
                )
                .on_hover_text("Keeps the top-left corner; what falls off the grid is lost")
                .clicked()
            {
                resize_to = Some(screen.side_edit);
            }
            if ui.button("Fit").clicked() {
                fit = true;
            }
        });
        ui.horizontal(|ui| {
            let (counts, skin) = screen.sketch.counts();
            ui.label(
                egui::RichText::new(format!(
                    "{} deck ({skin} skin) · {} wall · {} door · {} airlock",
                    counts[0], counts[1], counts[2], counts[3]
                ))
                .color(theme::MUTED),
            );
            if let Some((x, y)) = screen.hover {
                ui.label(egui::RichText::new(format!("({x}, {y})")).color(theme::MUTED));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(
                        "wheel zooms · middle drag pans · the port goes in the west skin on the centre rows",
                    )
                    .color(theme::MUTED),
                );
                if let Some((line, until)) = &screen.status
                    && now < *until
                {
                    ui.label(egui::RichText::new(line.as_str()).color(theme::ACCENT));
                }
            });
        });
    });
    if let Some(side) = resize_to {
        screen.remember();
        screen.sketch = screen.sketch.resized(side);
        fit = true;
    }

    // --- the grid ------------------------------------------------------------
    let canvas = rect_of(root.available_rect_before_wrap());
    if (screen.tile <= 0.0 || fit || canvas.size() != screen.fitted_to)
        && canvas.size().x > 0.0
        && canvas.size().y > 0.0
    {
        screen.fit(canvas);
    }
    let pointer = Pointer::read(&ctx);
    let (primary_down, secondary_down, delta, ctrl_s, ctrl_z) = ctx.input(|i| {
        (
            i.pointer.primary_down(),
            i.pointer.secondary_down(),
            i.pointer.delta(),
            i.modifiers.command && i.key_pressed(egui::Key::S),
            i.modifiers.command && i.key_pressed(egui::Key::Z),
        )
    });
    if !ctx.egui_wants_keyboard_input() {
        if ctrl_s {
            screen.save(now);
        }
        if ctrl_z {
            screen.undo();
        }
    }

    let on_canvas = pointer.on(canvas);
    screen.hover = on_canvas.map(|p| screen.tile_at(p));
    if let Some(p) = on_canvas {
        // The wheel zooms about the pointer: the tile under it stays put.
        if pointer.scroll != 0.0 {
            let factor = zoom_factor(pointer.scroll);
            let tile = (screen.tile * factor).clamp(2.0, 80.0);
            let factor = tile / screen.tile;
            screen.origin = p - (p - screen.origin) * factor;
            screen.tile = tile;
        }
        if pointer.middle_down {
            screen.origin += Vec2::new(delta.x, delta.y);
        }
        // A press begins a stroke; the pen paints as it goes, a rectangle
        // when the button comes up.
        if pointer.primary_pressed || pointer.secondary_pressed {
            let erase = pointer.secondary_pressed;
            screen.remember();
            let at = screen.tile_at(p);
            screen.drag = Some((at, erase));
            if screen.tool == Tool::Pen {
                let cell = if erase { Cell::Void } else { screen.brush };
                screen.sketch.set(at.0, at.1, cell);
            }
        }
    }
    if let Some((from, erase)) = screen.drag {
        let cell = if erase { Cell::Void } else { screen.brush };
        let down = if erase { secondary_down } else { primary_down };
        let at = pointer
            .pos
            .map(|p| screen.tile_at(p - canvas.min))
            .unwrap_or(from);
        match screen.tool {
            Tool::Pen => {
                screen.sketch.set(at.0, at.1, cell);
                if !down {
                    screen.drag = None;
                }
            }
            Tool::Rect => {
                if !down {
                    screen.sketch.fill(from, at, cell);
                    screen.drag = None;
                }
            }
        }
    }

    // --- the picture ---------------------------------------------------------
    let painter = canvas_painter(&ctx, canvas);
    let side = screen.sketch.side as i32;
    let tile = screen.tile;
    let hull =
        screen
            .tile_rect(canvas.min, 0, 0)
            .union(screen.tile_rect(canvas.min, side - 1, side - 1));
    painter.rect_filled(hull, 0.0, theme::PANEL_DEEP);
    for y in 0..side {
        for x in 0..side {
            let cell = screen.sketch.get(x, y);
            let fill = match cell {
                Cell::Void => continue,
                Cell::Deck if screen.sketch.is_skin(x, y) => SKIN,
                Cell::Deck => DECK,
                Cell::Wall => WALL,
                Cell::Door => DOOR,
                Cell::Airlock => AIRLOCK,
            };
            painter.rect_filled(screen.tile_rect(canvas.min, x, y), 0.0, fill);
        }
    }
    // The grid, every eighth line stronger, and the centre lines in the
    // accent: where the port's rows are.
    if tile >= 4.0 {
        for i in 0..=side {
            let along = canvas.min + screen.origin + Vec2::splat(i as f32 * tile);
            let colour = if i % 8 == 0 { GRID_EIGHTH } else { GRID };
            let stroke = egui::Stroke::new(1.0, colour);
            painter.line_segment(
                [
                    egui::pos2(along.x, hull.min.y),
                    egui::pos2(along.x, hull.max.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(hull.min.x, along.y),
                    egui::pos2(hull.max.x, along.y),
                ],
                stroke,
            );
        }
    }
    let mid = canvas.min + screen.origin + Vec2::splat((side / 2) as f32 * tile);
    let centre = egui::Stroke::new(1.5, CENTRE);
    painter.line_segment(
        [egui::pos2(mid.x, hull.min.y), egui::pos2(mid.x, hull.max.y)],
        centre,
    );
    painter.line_segment(
        [egui::pos2(hull.min.x, mid.y), egui::pos2(hull.max.x, mid.y)],
        centre,
    );
    // The stroke under way, and the tile under the pointer.
    if let Some((from, erase)) = screen.drag
        && screen.tool == Tool::Rect
        && let Some((x, y)) = screen.hover
    {
        let a = screen.tile_rect(
            canvas.min,
            from.0.clamp(0, side - 1),
            from.1.clamp(0, side - 1),
        );
        let b = screen.tile_rect(canvas.min, x.clamp(0, side - 1), y.clamp(0, side - 1));
        let colour = if erase { theme::WARN } else { theme::INK };
        painter.rect_stroke(
            a.union(b),
            0.0,
            egui::Stroke::new(2.0, colour),
            egui::StrokeKind::Outside,
        );
    } else if let Some((x, y)) = screen.hover
        && x >= 0
        && y >= 0
        && x < side
        && y < side
    {
        painter.rect_stroke(
            screen.tile_rect(canvas.min, x, y),
            0.0,
            egui::Stroke::new(1.5, theme::INK),
            egui::StrokeKind::Inside,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sketch goes to text and comes back the same, void to the right
    /// of a row and all, and a bad character says which row.
    #[test]
    fn a_sketch_round_trips_through_its_text() {
        let mut sketch = Sketch::new(MIN_SIDE);
        sketch.fill((2, 2), (9, 6), Cell::Deck);
        sketch.set(5, 4, Cell::Wall);
        sketch.set(5, 5, Cell::Door);
        sketch.set(2, 4, Cell::Airlock);
        let text = sketch.to_text("round-trip");
        let (name, back) = Sketch::from_text(&text).unwrap();
        assert_eq!(name, "round-trip");
        assert_eq!(back, sketch);
        assert!(text.lines().any(|l| l == "side = 16"));
        // The skin is the outline of the deck, the inside is not.
        assert!(sketch.is_skin(2, 2) && sketch.is_skin(9, 6) && sketch.is_skin(2, 4));
        assert!(!sketch.is_skin(4, 4));
        assert!(!sketch.is_skin(0, 0));
        let (counts, skin) = sketch.counts();
        assert_eq!(counts, [8 * 5 - 3, 1, 1, 1]);
        assert_eq!(skin, 2 * 8 + 2 * 3);
        assert!(
            Sketch::from_text("side = 16\n..x\n")
                .unwrap_err()
                .contains("row 1")
        );
        assert!(Sketch::from_text("side = 4\n").is_err());
        assert!(valid_name("hub-2_b") && !valid_name("../x") && !valid_name(""));
    }
}
