//! The Esc sheet: settings and the keys.
//!
//! One window in the middle of the screen, five pages. The first is the
//! menu — the UI scale, a button each for the audio and the controls, and
//! one each for saving and loading — and the other four are those, with a
//! way back. The controls page is where every key is rebound
//! (`crate::keys`); the save and load pages are `crate::save`'s, and what
//! they ask for comes back out of [`settings_sheet`] as a `Request` for
//! the screen to carry out. Esc opens it from any
//! screen that has one and closes it again from any page; the screens
//! own whether it is up and which page, as a [`Sheet`], and lay it out
//! with [`settings_sheet`] after their panels so it sits over them.

use bevy_egui::egui;

use crate::keys::{Action, Keys};
use crate::save::{self, Request, Saves};
use crate::sound::Mix;
use crate::theme;

/// Which page of the sheet is up.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Sheet {
    #[default]
    Menu,
    Audio,
    Controls,
    /// The game written out, and read back: `crate::save`'s pages.
    Save,
    Load,
}

/// The sheet, on `page`. `None` afterwards means it was closed. While the
/// controls page is waiting on a key (`keys.listening`) Esc is its to
/// cancel with, and the screens leave the sheet up.
pub fn settings_sheet(
    ctx: &egui::Context,
    sheet: &mut Option<Sheet>,
    mix: &mut Mix,
    keys: &mut Keys,
    saves: &mut Saves,
    can_save: bool,
) -> Option<Request> {
    let page = (*sheet)?;
    let title = match page {
        Sheet::Menu => "Settings",
        Sheet::Audio => "Audio",
        Sheet::Controls => "Controls",
        Sheet::Save => "Save",
        Sheet::Load => "Load",
    };
    let mut request = None;
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| match page {
            Sheet::Menu => menu(ui, sheet, saves, can_save),
            Sheet::Audio => audio(ui, sheet, mix),
            Sheet::Controls => controls(ui, sheet, keys),
            Sheet::Save => {
                request = save::save_page(ui, saves, can_save);
                back(ui, sheet);
            }
            Sheet::Load => {
                request = save::load_page(ui, saves);
                back(ui, sheet);
            }
        });
    request
}

/// The way back to the menu, under a page.
fn back(ui: &mut egui::Ui, sheet: &mut Option<Sheet>) {
    ui.add_space(8.0);
    if ui.button("< Back").clicked() {
        *sheet = Some(Sheet::Menu);
    }
}

fn menu(ui: &mut egui::Ui, sheet: &mut Option<Sheet>, saves: &mut Saves, can_save: bool) {
    theme::heading(ui, "UI scale");
    theme::ui_scale_row(ui);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button("Audio").clicked() {
            *sheet = Some(Sheet::Audio);
        }
        if ui.button("Controls").clicked() {
            *sheet = Some(Sheet::Controls);
        }
    });
    ui.add_space(8.0);
    // The game, written out and read back. The directory is read again as
    // a page opens, so a file from another run is there.
    ui.horizontal(|ui| {
        let save = ui
            .add_enabled(can_save, egui::Button::new("Save"))
            .on_disabled_hover_text(
                "Nothing to save yet: the game starts when the ship is accepted.",
            );
        if save.clicked() {
            saves.note = None;
            saves.refresh();
            *sheet = Some(Sheet::Save);
        }
        if ui.button("Load").clicked() {
            saves.note = None;
            saves.refresh();
            *sheet = Some(Sheet::Load);
        }
    });
    ui.add_space(8.0);
    if ui.button("Close").clicked() {
        *sheet = None;
    }
}

/// The player's volumes: one over everything, one over the sounds that
/// happen and one over the hum underneath, and a mute. See `sound::Mix`.
fn audio(ui: &mut egui::Ui, sheet: &mut Option<Sheet>, mix: &mut Mix) {
    let slider = |ui: &mut egui::Ui, label: &str, value: &mut f32| {
        ui.label(label);
        ui.add(
            egui::Slider::new(value, 0.0..=1.0)
                .show_value(false)
                .trailing_fill(true),
        );
        ui.label(format!("{}%", (*value * 100.0).round() as u32));
        ui.end_row();
    };
    egui::Grid::new("audio")
        .num_columns(3)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            slider(ui, "Master", &mut mix.master);
            slider(ui, "Effects", &mut mix.effects);
            slider(ui, "Ambience", &mut mix.ambience);
        });
    ui.add_space(4.0);
    ui.checkbox(&mut mix.muted, "Mute");
    ui.label(
        egui::RichText::new(
            "Effects are the doors, the galley and the fight; ambience is the ship's hum, a station's, and the engines.",
        )
        .small()
        .color(theme::MUTED),
    );
    ui.add_space(8.0);
    if ui.button("< Back").clicked() {
        *sheet = Some(Sheet::Menu);
    }
}

/// The keys, and the player's say over them: every [`Action`] with its
/// key on a button — click it, press the key you want, Esc to think
/// again — a line where a key is used twice, and a way back to the
/// defaults. The rest of the page is what the pointer does, which is
/// not rebound. A change is kept for next time (`Keys::save`).
/// A wrapped line in a grid's column: given its width, since a grid
/// gives a wrapping label none and it comes out a word a line.
fn wide(ui: &mut egui::Ui, text: &str) {
    ui.allocate_ui_with_layout(
        egui::vec2(400.0, 0.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_min_width(400.0);
            ui.add(egui::Label::new(text).wrap());
        },
    );
}

fn controls(ui: &mut egui::Ui, sheet: &mut Option<Sheet>, keys: &mut Keys) {
    // The key being chosen: the next key down is it, bar Esc, which
    // cancels — asked before the rows, so the row it lands on is drawn
    // already bound.
    if let Some(action) = keys.listening {
        let pressed = ui.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key {
                    key, pressed: true, ..
                } => Some(*key),
                _ => None,
            })
        });
        match pressed {
            Some(egui::Key::Escape) => keys.listening = None,
            Some(key) => {
                keys.set(action, key);
                keys.listening = None;
                keys.save();
            }
            None => {}
        }
    }
    ui.set_max_width(620.0);
    // Sixteen rows and the pointer's table: taller than a short window, so
    // the page scrolls and the way back stays under it.
    egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
    theme::heading(ui, "Keys");
    ui.label(
        egui::RichText::new(
            "Click a key to change it, then press the one you want; Esc keeps the old one. Two actions on one key both happen — Recruit and Turn share R and are told apart by what is in hand.",
        )
        .small()
        .color(theme::MUTED),
    );
    ui.add_space(4.0);
    // A row an action rather than a grid: a grid gives a wrapped line no
    // height of its own, and the rows ran into each other.
    for action in Action::ALL {
        ui.horizontal_top(|ui| {
                let listening = keys.listening == Some(action);
                let label = if listening {
                    "press a key…".to_string()
                } else {
                    keys.key(action).symbol_or_name().to_string()
                };
                let button = egui::Button::new(egui::RichText::new(label).strong())
                    .min_size(egui::vec2(96.0, 0.0))
                    .selected(listening);
                if ui.add(button).clicked() {
                    keys.listening = if listening { None } else { Some(action) };
                }
                wide(ui, action.what());
                let shared = keys.shared_with(action);
                if !shared.is_empty() {
                    let names: Vec<&str> = shared.iter().map(|a| a.name()).collect();
                    ui.label(
                        egui::RichText::new(format!("also {}", names.join(", ")))
                            .small()
                            .color(theme::WARN),
                    );
                }
        });
        ui.add_space(2.0);
    }
    ui.add_space(4.0);
    if ui.button("Reset to defaults").clicked() {
        *keys = Keys::default();
        keys.save();
    }
    ui.add_space(8.0);
    let table = |ui: &mut egui::Ui, title: &str, rows: &[(&str, &str)]| {
        theme::heading(ui, title);
        egui::Grid::new(title)
            .num_columns(2)
            .spacing([12.0, 2.0])
            .show(ui, |ui| {
                for (key, what) in rows {
                    ui.label(egui::RichText::new(*key).strong());
                    wide(ui, what);
                    ui.end_row();
                }
            });
    };
    table(
        ui,
        "The pointer",
        &[
            ("Wheel", "Zoom, about the pointer."),
            ("Middle-drag", "Pan the view."),
            (
                "Take the helm",
                "Send the crew member you steer to the helm. The ship is flown from there: nothing can be aimed at or confirmed until they are standing at it.",
            ),
            (
                "Click the map",
                "Aim at a planet or a station; the helm quotes the trip, and Confirm sends the ship. At a station, the crew all come back aboard first and the ship casts off once they have.",
            ),
            ("Drag on the deck", "Select whoever is inside the box."),
            (
                "Right-click the deck",
                "Send the selected crew member there.",
            ),
            ("Right-click a fixture", "Open its menu."),
            (
                "Build tab",
                "Pick a part by category, or search for one. The crew carry what it is made of from the shelves and build it; a site beyond the hull is built in a suit.",
            ),
            (
                "Click with a part in hand",
                "Lay it out where the pointer is. Green goes; red says why not at the top left. Right-click or Esc puts it down.",
            ),
            (
                "In the armoury",
                "Drag a thing to move it, Turn while carrying it to stand it on end, and let go where the ghost is green. Ctrl-click takes a thing into the pack.",
            ),
            ("Drag in the yard", "Lay the chosen part over every tile of the box."),
            ("Right-drag in the yard", "Take the top part off every tile of the box."),
            (
                "Esc",
                "Close a menu, stop aiming, put a part down, or open and close the settings. Not rebound: it is what closes this sheet.",
            ),
        ],
    );
    });
    ui.add_space(8.0);
    if ui.button("< Back").clicked() {
        keys.listening = None;
        *sheet = Some(Sheet::Menu);
    }
}
