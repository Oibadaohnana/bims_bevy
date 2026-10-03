//! The Esc sheet: settings and the keys.
//!
//! One window in the middle of the screen, six pages. The first is the
//! menu — the UI scale, the edge-scroll speed (task 123, kept in the keys
//! file beside the bindings) and the network buffer's cap (task 148, the
//! same), a button each for the audio and the controls, and
//! one each for saving, loading and starting the run again, and one back to
//! the start menu — and the other
//! five are those, with a way back. The controls page is where every key is
//! rebound (`crate::keys`); the save, load and restart pages are
//! `crate::save`'s, and what
//! they ask for comes back out of [`settings_sheet`] as a `Request` for
//! the screen to carry out. Esc opens it from any
//! screen that has one and closes it again from any page; the screens
//! own whether it is up and which page, as a [`Sheet`], and lay it out
//! with [`settings_sheet`] after their panels so it sits over them. The
//! menu, the setup and the lobby have it too, as the settings alone
//! ([`Allowed::SETTINGS_ONLY`]): there is no game yet to save or load.

use bevy_egui::egui;

use crate::keys::{Action, EDGE_SCROLL_MAX, Keys, NET_BUFFER_MAX, Profile};
use crate::names::{
    LOAD_GUEST, MENU_BUTTON, NET_BUFFER_HINT, PAUSE_BUTTON, RESTART_BUTTON, RESTART_GUEST,
    RESTART_NONE, RESUME_BUTTON,
};
use crate::save::{self, Request, Saves};
use crate::sound::Mix;
use crate::theme;

/// What the sheet may do with the game, which is the screen's to say:
/// `save` is whether there is a game to write — the design phase has
/// none — and `load` whether this end may read one in. A guest may not
/// (feature 67): the host's world is the world, and a guest that loaded
/// would be a second clock. Greyed rather than hidden, with the reason.
/// `restart` is both at once (feature 79): there has to be a run to go
/// back to the beginning of, and a restart is a world replaced like any
/// other, so it is the host's to do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Allowed {
    pub save: bool,
    pub load: bool,
    pub restart: bool,
    /// Whether there is a game to speak of at all: none on the menu,
    /// the setup and in the lobby, where the sheet is the settings alone
    /// and the save, load and restart row is not shown.
    pub game: bool,
    /// Whether the world can be paused from the sheet, and whether this
    /// player has it paused (task 154): `None` with no world running —
    /// the design phase, the menu, the lobby — and no Pause button.
    pub paused: Option<bool>,
}

impl Allowed {
    /// The sheet before a game: the settings and nothing else.
    pub const SETTINGS_ONLY: Allowed = Allowed {
        save: false,
        load: false,
        restart: false,
        game: false,
        paused: None,
    };

    /// The screen's phase and company, in one: `playing` is whether the
    /// world is open, `guest` whether this end is somebody's guest.
    pub fn of(playing: bool, guest: bool) -> Allowed {
        Allowed {
            save: playing,
            load: !guest,
            restart: playing && !guest,
            game: true,
            paused: None,
        }
    }

    /// Why a restart is greyed, where it is: no run yet, or somebody
    /// else's world.
    fn no_restart(self) -> &'static str {
        if self.load {
            RESTART_NONE
        } else {
            RESTART_GUEST
        }
    }
}

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
    /// The run from its beginning again (feature 79): what that means,
    /// and the button that asks for it.
    Restart,
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
    allowed: Allowed,
) -> Option<Request> {
    let page = (*sheet)?;
    let title = match page {
        Sheet::Menu => "Settings",
        Sheet::Audio => "Audio",
        Sheet::Controls => "Controls",
        Sheet::Save => "Save",
        Sheet::Load => "Load",
        Sheet::Restart => RESTART_BUTTON,
    };
    let mut request = None;
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| match page {
            Sheet::Menu => request = menu(ui, sheet, saves, allowed, keys),
            Sheet::Audio => audio(ui, sheet, mix),
            Sheet::Controls => controls(ui, sheet, keys),
            Sheet::Save => {
                request = save::save_page(ui, saves, allowed.save);
                back(ui, sheet);
            }
            Sheet::Load => {
                if allowed.load {
                    request = save::load_page(ui, saves);
                } else {
                    ui.set_min_width(360.0);
                    ui.label(egui::RichText::new(LOAD_GUEST).color(theme::MUTED));
                }
                back(ui, sheet);
            }
            Sheet::Restart => {
                request = save::restart_page(ui, saves, allowed.restart, allowed.no_restart());
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

fn menu(
    ui: &mut egui::Ui,
    sheet: &mut Option<Sheet>,
    saves: &mut Saves,
    allowed: Allowed,
    keys: &mut Keys,
) -> Option<Request> {
    let mut request = None;
    // The pause (task 154): Space paused the world until it became the
    // engineer's remote trigger, so it is here, first. A pause by anybody
    // is a pause for everybody; Resume takes this player's back.
    if let Some(paused) = allowed.paused {
        let label = if paused { RESUME_BUTTON } else { PAUSE_BUTTON };
        if ui
            .add(egui::Button::new(label).min_size(egui::vec2(120.0, 0.0)))
            .clicked()
        {
            request = Some(Request::Pause(!paused));
            if paused {
                *sheet = None;
            }
        }
        ui.add_space(8.0);
    }
    theme::heading(ui, "UI scale");
    theme::ui_scale_row(ui);
    ui.add_space(8.0);
    // How fast the pointer against the window's edge pans the view (task
    // 123): nought to three times its own pan, nought being off. This
    // player's own, kept with the keys and sent nowhere.
    theme::heading(ui, "Edge scroll speed");
    let mut speed = keys.edge_scroll_speed();
    let slider = ui.add(
        egui::Slider::new(&mut speed, 0.0..=f32::from(EDGE_SCROLL_MAX) / 10.0)
            .step_by(0.1)
            .trailing_fill(true)
            .custom_formatter(|v, _| {
                if v <= 0.0 {
                    "Off".to_string()
                } else {
                    format!("{v:.1}×")
                }
            }),
    );
    if slider.changed() {
        keys.edge_scroll = (speed * 10.0)
            .round()
            .clamp(0.0, f32::from(EDGE_SCROLL_MAX)) as u8;
    }
    // Kept when the value settles, not every frame of a drag.
    if slider.drag_stopped() || (slider.changed() && !slider.dragged()) {
        keys.save();
    }
    ui.add_space(8.0);
    // The most a guest's world may run behind the host's to play smoothly
    // over a shaky line (task 148, `crate::playout`): it holds only what
    // the line needs, up to this. Nought is off. This player's own, kept
    // with the keys; a host and a game of one never use it.
    theme::heading(ui, "Network buffer");
    let mut cap = keys.net_buffer_seconds();
    let slider = ui
        .add(
            egui::Slider::new(&mut cap, 0.0..=f64::from(NET_BUFFER_MAX) / 100.0)
                .step_by(0.05)
                .trailing_fill(true)
                .custom_formatter(|v, _| {
                    if v <= 0.0 {
                        "Off".to_string()
                    } else {
                        format!("up to {:.0} ms", v * 1000.0)
                    }
                }),
        )
        .on_hover_text(NET_BUFFER_HINT);
    if slider.changed() {
        keys.net_buffer = (cap * 100.0).round().clamp(0.0, f64::from(NET_BUFFER_MAX)) as u8;
    }
    if slider.drag_stopped() || (slider.changed() && !slider.dragged()) {
        keys.save();
    }
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
    if allowed.game {
        game_row(ui, sheet, saves, allowed);
        ui.add_space(8.0);
        // Out of the game and back to the start menu: the world is left
        // unsaved, and with company this end leaves the room.
        if ui.button(MENU_BUTTON).clicked() {
            *sheet = None;
            request = Some(Request::ToMenu);
        }
        ui.add_space(8.0);
    }
    if ui.button("Close").clicked() {
        *sheet = None;
    }
    request
}

/// The menu's Save, Load and Restart, where there is a game.
fn game_row(ui: &mut egui::Ui, sheet: &mut Option<Sheet>, saves: &mut Saves, allowed: Allowed) {
    ui.horizontal(|ui| {
        let save = ui
            .add_enabled(allowed.save, egui::Button::new("Save"))
            .on_disabled_hover_text(
                "Nothing to save yet: the game starts when the ship is accepted.",
            );
        if save.clicked() {
            saves.note = None;
            saves.refresh();
            *sheet = Some(Sheet::Save);
        }
        let load = ui
            .add_enabled(allowed.load, egui::Button::new("Load"))
            .on_disabled_hover_text(LOAD_GUEST);
        if load.clicked() {
            saves.note = None;
            saves.refresh();
            *sheet = Some(Sheet::Load);
        }
        // The run again from where it opened (feature 79) — a page of
        // its own, since it asks before it throws a run away.
        let restart = ui
            .add_enabled(allowed.restart, egui::Button::new(RESTART_BUTTON))
            .on_disabled_hover_text(allowed.no_restart());
        if restart.clicked() {
            saves.note = None;
            *sheet = Some(Sheet::Restart);
        }
    });
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

/// Why a key cannot be clicked on the defaults.
const DEFAULTS_FIXED: &str = "These are the game's defaults. Switch to My profile to change a key.";

/// Whose keys are in play (October 2026): the player's own profile, kept
/// on this computer, or the game's defaults, from `keys.ron` where the
/// game was started. A choice is kept for next time.
fn profile_row(ui: &mut egui::Ui, keys: &mut Keys) {
    ui.horizontal(|ui| {
        let now = keys.profile();
        for (profile, label, tip) in [
            (
                Profile::Mine,
                "My profile",
                "Your own keys, kept on this computer and saved as you change them. The first time, they start as the defaults.",
            ),
            (
                Profile::Defaults,
                "Defaults",
                "The game's keys, from keys.ron where the game was started. Shown, not changed here.",
            ),
        ] {
            if ui
                .selectable_label(now == profile, label)
                .on_hover_text(tip)
                .clicked()
                && now != profile
            {
                keys.use_profile(profile, crate::keys::defaults());
                keys.save();
            }
        }
    });
}

/// Under the profiles, over the keys: the player's own profile put back on the defaults,
/// and the keys shown written as the game's defaults — `keys.ron`, which
/// a player with no profile of their own plays — with a line under them
/// a few seconds saying how that went.
fn profile_buttons(ui: &mut egui::Ui, keys: &mut Keys) {
    let note_id = egui::Id::new("keys-defaults-note");
    ui.horizontal(|ui| {
        let reset = ui
            .add_enabled(
                keys.profile() == Profile::Mine,
                egui::Button::new("Reset to defaults"),
            )
            .on_hover_text("Every key of your profile back to the game's defaults.")
            .on_disabled_hover_text(DEFAULTS_FIXED);
        if reset.clicked() {
            // The keys, not the edge-scroll speed or the network buffer:
            // those are the menu's sliders.
            keys.reset_to(crate::keys::defaults());
            keys.save();
        }
        let path = crate::keys::defaults_path();
        let save = ui.button("Save as defaults").on_hover_text(format!(
            "Write the keys shown into {} as the game's defaults: what a player plays with no profile of their own.",
            path.display()
        ));
        if save.clicked() {
            let note = match crate::keys::save_defaults(keys.bindings()) {
                Ok(()) => format!("Saved as the defaults in {}.", path.display()),
                Err(why) => format!("Not saved: {why}"),
            };
            let until = ui.input(|i| i.time) + 5.0;
            ui.data_mut(|d| d.insert_temp(note_id, (note, until)));
        }
    });
    let note: Option<(String, f64)> = ui.data(|d| d.get_temp(note_id));
    if let Some((note, until)) = note
        && ui.input(|i| i.time) < until
    {
        ui.label(egui::RichText::new(note).small().color(theme::MUTED));
    }
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
    profile_row(ui, keys);
    profile_buttons(ui, keys);
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(
            "Click a key to change it, then press the one you want; Esc keeps the old one. Two actions on one key both happen where both are read — Turn shares R with the reload, and is read only in the yard and the armoury. Ctrl and an ability slot's key ranks that ability up rather than using it, whatever key the slot is on.",
        )
        .small()
        .color(theme::MUTED),
    );
    ui.add_space(4.0);
    let mine = keys.profile() == Profile::Mine;
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
                let button = ui
                    .add_enabled(mine, button)
                    .on_disabled_hover_text(DEFAULTS_FIXED);
                if button.clicked() {
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
            (
                "Wheel",
                "Zoom: about the Bim you steer while the camera follows it, else about the pointer.",
            ),
            (
                "The pointer over the deck",
                "Aim: the Bim you steer turns to face it, a whole turn in a second.",
            ),
            ("Middle-drag", "Pan the view."),
            (
                "Edge of the window",
                "Pan the view: rest the pointer against an edge, or a corner for both ways. Its speed is on the settings' first page.",
            ),
            (
                "Ctrl + ability key, or Ctrl-click its box",
                "Rank the ability up, rather than using it.",
            ),
            (
                "Click the map",
                "Pick a station, or a planet with a settlement, on the world map's list: the trip is quoted there and put to the crew.",
            ),
            (
                "Hold the left button on the deck",
                "Fire where the Bim you steer faces, as fast as the weapon goes. A click is one shot.",
            ),
            (
                "Shift, walking",
                "Sprint: the Bim you steer runs at 1.8 times its walking pace from the first step, facing the way it runs, its weapon across its chest — it fires nothing until you let go.",
            ),
            (
                "Alt",
                "Dodge roll: the Bim you steer rolls two and a half tiles the way you are walking it, or last walked it — not where the pointer is — and every bolt and beam misses it while it rolls. Once a second or so.",
            ),
            (
                "Ctrl + left click",
                "Ping: a mark in your colour on everybody's screen, on the deck or the galaxy chart.",
            ),
            (
                "Right-click on the deck",
                "Open the menu of what is there — a door, a downed crewmate, a mercenary, a cache — or pick the crew member under it. With the medkit in hand, a right-click on a downed crewmate revives them.",
            ),
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
