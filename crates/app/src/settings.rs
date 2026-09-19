//! The Esc sheet: settings and the keys.
//!
//! One window in the middle of the screen, three pages. The first is the
//! menu — the UI scale, and a button each for the audio and the controls —
//! and the other two are those, with a way back. Esc opens it from any
//! screen that has one and closes it again from any page; the screens
//! own whether it is up and which page, as a [`Sheet`], and lay it out
//! with [`settings_sheet`] after their panels so it sits over them.

use bevy_egui::egui;

use crate::sound::Mix;
use crate::theme;

/// Which page of the sheet is up.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Sheet {
    #[default]
    Menu,
    Audio,
    Controls,
}

/// The sheet, on `page`. `None` afterwards means it was closed.
pub fn settings_sheet(ctx: &egui::Context, sheet: &mut Option<Sheet>, mix: &mut Mix) {
    let Some(page) = *sheet else {
        return;
    };
    let title = match page {
        Sheet::Menu => "Settings",
        Sheet::Audio => "Audio",
        Sheet::Controls => "Controls",
    };
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| match page {
            Sheet::Menu => menu(ui, sheet),
            Sheet::Audio => audio(ui, sheet, mix),
            Sheet::Controls => controls(ui, sheet),
        });
}

fn menu(ui: &mut egui::Ui, sheet: &mut Option<Sheet>) {
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

fn controls(ui: &mut egui::Ui, sheet: &mut Option<Sheet>) {
    let table = |ui: &mut egui::Ui, title: &str, rows: &[(&str, &str)]| {
        theme::heading(ui, title);
        egui::Grid::new(title)
            .num_columns(2)
            .spacing([12.0, 2.0])
            .show(ui, |ui| {
                for (key, what) in rows {
                    ui.label(egui::RichText::new(*key).strong());
                    ui.label(*what);
                    ui.end_row();
                }
            });
    };
    table(
        ui,
        "At the helm",
        &[
            ("M", "Switch between the ship and the map."),
            ("N", "Turn the view head up or north up."),
            (
                "F",
                "Follow the crew member you steer, or let the camera go free.",
            ),
            ("Space", "Pause the world, or set it going again."),
            ("1 2 3 4", "Run the world at 1×, 3×, 10× or the top speed."),
            (
                "W A S D",
                "Pan the view. Middle-drag does the same. A free camera goes anywhere; one following the crew stops at the edge.",
            ),
            ("Wheel", "Zoom, about the pointer."),
            (
                "Take the helm",
                "Send the crew member you steer to the helm. The ship is flown from there: nothing can be aimed at or confirmed until they are standing at it.",
            ),
            (
                "Click the map",
                "Aim at a planet or a station; the helm quotes the trip, and Confirm sends the ship. At a station, the crew all come back aboard first and the ship casts off once they have.",
            ),
        ],
    );
    table(
        ui,
        "On the deck",
        &[
            (
                "C",
                "Select the crew member you steer, and put them in the middle of the view.",
            ),
            ("Drag", "Select whoever is inside the box."),
            (
                "Right-click the deck",
                "Send the selected crew member there.",
            ),
            ("Right-click a fixture", "Open its menu."),
            ("R", "Recruit the crew member you steer, or let them go."),
        ],
    );
    table(
        ui,
        "Building",
        &[
            (
                "Build tab",
                "Pick a part by category, or search for one. The crew carry what it is made of from the shelves and build it; a site beyond the hull is built in a suit.",
            ),
            (
                "Click",
                "Lay the part in hand out where the pointer is. Green goes; red says why not at the top left.",
            ),
            ("R", "Turn the part in hand."),
            ("Right-click, Esc", "Put the part down."),
        ],
    );
    table(
        ui,
        "In the yard",
        &[
            ("Drag", "Lay the chosen part over every tile of the box."),
            ("Right-drag", "Take the top part off every tile of the box."),
            ("R", "Turn the part you are about to place."),
        ],
    );
    table(
        ui,
        "Anywhere",
        &[(
            "Esc",
            "Close a menu, stop aiming, or open and close the settings.",
        )],
    );
    ui.add_space(8.0);
    if ui.button("< Back").clicked() {
        *sheet = Some(Sheet::Menu);
    }
}
