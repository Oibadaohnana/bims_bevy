//! The keys: which key does what, and the player's say over it.
//!
//! Every hotkey a screen reads is an [`Action`] looked up in [`Keys`]
//! rather than a key written into the screen, so the Controls page of the
//! Esc sheet can rebind any of them: click the key beside an action, press
//! the one you want, Esc to think again. Two actions may share a key —
//! Recruit and Turn do by default, and the screens keep them apart by
//! what is in hand — so nothing refuses a binding; the page says where a
//! key is used twice. Esc itself is not an action: it is what closes the
//! sheet and cancels a rebind, and a key that could be bound away from
//! that is a sheet that cannot be closed.
//!
//! The bindings are kept in a plain text file, `keys` under the app's
//! own directory in the user's config directory (`$XDG_CONFIG_HOME` or
//! `~/.config`, then `bims/`), one `action=Key` a line by egui's key
//! names, written whenever a binding changes and read at start; a line
//! that names no action or no key is skipped, and a missing file is the
//! defaults. Nothing else the sheet sets is kept yet — the volumes and
//! the UI scale start afresh each run — and when they are, this is the
//! file they go in.

use bevy::prelude::Resource;
use bevy_egui::egui;

/// Something a key does. In the order the Controls page lists them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Switch between the ship and the map.
    Map,
    /// Head up or north up.
    NorthUp,
    /// Follow the crew member you steer, or a free camera.
    Follow,
    /// Pause, or set going again.
    Pause,
    Speed1,
    Speed3,
    Speed10,
    Speed24,
    SpeedTop,
    PanLeft,
    PanRight,
    PanUp,
    PanDown,
    /// The crew member you steer, selected and in the middle.
    Select,
    /// Recruit them, or let them go.
    Recruit,
    /// Turn the part in hand — on the deck, in the yard — or a thing in
    /// the armoury.
    Turn,
    /// Open and close the inventory of the crew member you steer.
    Inventory,
    /// The steered crew member's class's first action (features 74 to
    /// 77): an engineer sets a sentry up on the deck tile under the
    /// pointer, a soldier throws a grenade at it, a medic triggers its
    /// surge, a tank taunts. Nothing with a classless crew member
    /// steered.
    ClassPrimary,
    /// Its second: an engineer lays sandbags on the tile under the
    /// pointer, a soldier braces or stands easy, a medic beams the crew
    /// member under the pointer, a tank puts its wall up or down.
    ClassSecondary,
    /// A commander calls the squad back to the deck tile under the
    /// pointer, or to himself with the pointer on nothing (feature 78).
    /// Nothing for any other class.
    SquadFallBack,
    /// A commander has the squad hold exactly where it stands.
    SquadStandGround,
    /// **Attack** (feature 84): arms the pointer, and the next click on
    /// the deck puts an attack banner down there for the bots that
    /// follow you. Pressed again with the pointer armed, or with the
    /// banner already where you click, it is called off.
    Attack,
    /// **Retreat**: the bots that follow you fall back to the ship and
    /// hold there. Pressed again, they go back to following.
    Retreat,
}

impl Action {
    pub const ALL: [Action; 23] = [
        Action::Map,
        Action::NorthUp,
        Action::Follow,
        Action::Pause,
        Action::Speed1,
        Action::Speed3,
        Action::Speed10,
        Action::Speed24,
        Action::SpeedTop,
        Action::PanLeft,
        Action::PanRight,
        Action::PanUp,
        Action::PanDown,
        Action::Select,
        Action::Recruit,
        Action::Turn,
        Action::Inventory,
        Action::ClassPrimary,
        Action::ClassSecondary,
        Action::SquadFallBack,
        Action::SquadStandGround,
        Action::Attack,
        Action::Retreat,
    ];

    /// The key it starts on.
    pub fn default_key(self) -> egui::Key {
        use egui::Key;
        match self {
            Action::Map => Key::M,
            Action::NorthUp => Key::N,
            // F is Attack since feature 84 — the user asked for it by
            // name — so following the camera moved to V.
            Action::Follow => Key::V,
            Action::Pause => Key::Space,
            Action::Speed1 => Key::Num1,
            Action::Speed3 => Key::Num2,
            Action::Speed10 => Key::Num3,
            Action::Speed24 => Key::Num4,
            Action::SpeedTop => Key::Num5,
            Action::PanLeft => Key::A,
            Action::PanRight => Key::D,
            Action::PanUp => Key::W,
            Action::PanDown => Key::S,
            Action::Select => Key::C,
            Action::Recruit => Key::R,
            Action::Turn => Key::R,
            Action::Inventory => Key::Tab,
            Action::ClassPrimary => Key::Q,
            Action::ClassSecondary => Key::E,
            Action::SquadFallBack => Key::X,
            Action::SquadStandGround => Key::Z,
            Action::Attack => Key::F,
            Action::Retreat => Key::T,
        }
    }

    /// The name it is kept under in the file, and shown by on the page.
    pub fn name(self) -> &'static str {
        match self {
            Action::Map => "map",
            Action::NorthUp => "north-up",
            Action::Follow => "follow",
            Action::Pause => "pause",
            Action::Speed1 => "speed-1",
            Action::Speed3 => "speed-3",
            Action::Speed10 => "speed-10",
            Action::Speed24 => "speed-24",
            Action::SpeedTop => "speed-top",
            Action::PanLeft => "pan-left",
            Action::PanRight => "pan-right",
            Action::PanUp => "pan-up",
            Action::PanDown => "pan-down",
            Action::Select => "select",
            Action::Recruit => "recruit",
            Action::Turn => "turn",
            Action::Inventory => "inventory",
            Action::ClassPrimary => "class-primary",
            Action::ClassSecondary => "class-secondary",
            Action::SquadFallBack => "squad-fall-back",
            Action::SquadStandGround => "squad-stand-ground",
            Action::Attack => "attack",
            Action::Retreat => "retreat",
        }
    }

    /// What it does, for the Controls page.
    pub fn what(self) -> &'static str {
        match self {
            Action::Map => "Switch between the ship and the map.",
            Action::NorthUp => "Turn the view head up or north up.",
            Action::Follow => {
                "Follow the crew member you steer, and the ship on the map, or let the camera go free."
            }
            Action::Pause => "Pause the world, or set it going again.",
            Action::Speed1 => "Run the world at 1×.",
            Action::Speed3 => "Run the world at 3×.",
            Action::Speed10 => "Run the world at 10×.",
            Action::Speed24 => "Run the world at 24×, a day a minute.",
            Action::SpeedTop => "Run the world at the top speed, 48×.",
            Action::PanLeft => "Pan the view left. Middle-drag does the same.",
            Action::PanRight => "Pan the view right.",
            Action::PanUp => "Pan the view up.",
            Action::PanDown => "Pan the view down.",
            Action::Select => {
                "Select the crew member you steer, and put them in the middle of the view."
            }
            Action::Recruit => {
                "Recruit the crew member you steer, or let them go — with nothing in hand."
            }
            Action::Turn => {
                "Turn the part in hand on the deck or in the yard, or a thing in the armoury."
            }
            Action::Inventory => "Open and close the inventory of the crew member you steer.",
            Action::ClassPrimary => {
                "The class's first action, by the crew member you steer: an engineer sets a sentry up on the deck tile under the pointer, out of a kit in its pack; a soldier throws a grenade at it; a medic triggers its surge; a tank taunts."
            }
            Action::ClassSecondary => {
                "The class's second action: an engineer lays sandbags on the deck tile under the pointer, out of a kit in its pack; a soldier braces where it stands, or stands easy again; a medic beams the crew member under the pointer, and unlinks when pressed on the one it holds or on nobody; a tank puts its wall up, or takes it down; a commander sends the squad at the enemy under the pointer."
            }
            Action::SquadFallBack => {
                "A commander calls the squad back to the deck tile under the pointer, or to himself with the pointer on nothing. Nothing for any other class."
            }
            Action::SquadStandGround => {
                "A commander has the squad hold exactly where it stands. Nothing for any other class."
            }
            Action::Attack => {
                "Arm the pointer — it turns red — and the next click on the deck puts an attack banner down there. The crew that follow you fight their way to it, taking the cover on the way and pushing on when nothing is in range. Press it again to think better of it, or click the banner where it already stands to call it off."
            }
            Action::Retreat => {
                "The crew that follow you fall back to the ship and hold there. Press it again and they go back to keeping to your side. Nobody leaves a fight aboard the ship: cornered in your own hull they stand and shoot whatever they were told."
            }
        }
    }

    fn from_name(name: &str) -> Option<Action> {
        Action::ALL.iter().copied().find(|a| a.name() == name)
    }
}

/// The bindings, one key an action, and which action the Controls page
/// is waiting on a key for.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Keys {
    keys: [egui::Key; Action::ALL.len()],
    /// The action whose key is being chosen: the next key pressed is it.
    pub listening: Option<Action>,
}

impl Default for Keys {
    fn default() -> Keys {
        Keys {
            keys: Action::ALL.map(|a| a.default_key()),
            listening: None,
        }
    }
}

impl Keys {
    pub fn key(&self, action: Action) -> egui::Key {
        self.keys[Action::ALL.iter().position(|&a| a == action).unwrap_or(0)]
    }

    pub fn set(&mut self, action: Action, key: egui::Key) {
        if let Some(i) = Action::ALL.iter().position(|&a| a == action) {
            self.keys[i] = key;
        }
    }

    /// Whether the action's key went down this frame.
    pub fn pressed(&self, input: &egui::InputState, action: Action) -> bool {
        input.key_pressed(self.key(action))
    }

    /// Whether the action's key is held.
    pub fn down(&self, input: &egui::InputState, action: Action) -> bool {
        input.key_down(self.key(action))
    }

    /// The other actions on the same key as this one, for the page to say.
    pub fn shared_with(&self, action: Action) -> Vec<Action> {
        let key = self.key(action);
        Action::ALL
            .iter()
            .copied()
            .filter(|&a| a != action && self.key(a) == key)
            .collect()
    }

    /// The file's text: one `action=Key` a line.
    pub fn to_text(self) -> String {
        Action::ALL
            .iter()
            .map(|&a| format!("{}={}\n", a.name(), self.key(a).name()))
            .collect()
    }

    /// The bindings a file's text says, over the defaults; a line that
    /// names no action or no key is skipped.
    pub fn from_text(text: &str) -> Keys {
        let mut keys = Keys::default();
        for line in text.lines() {
            let Some((name, key)) = line.split_once('=') else {
                continue;
            };
            if let (Some(action), Some(key)) = (
                Action::from_name(name.trim()),
                egui::Key::from_name(key.trim()),
            ) {
                keys.set(action, key);
            }
        }
        keys
    }

    /// The bindings as last saved, or the defaults.
    pub fn load() -> Keys {
        match path().and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(text) => Keys::from_text(&text),
            None => Keys::default(),
        }
    }

    /// Keep the bindings for next time. Quiet about a directory that
    /// cannot be written: the bindings still hold for this run.
    pub fn save(&self) {
        if let Some(path) = path() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, self.to_text());
        }
    }
}

/// Where the bindings are kept: `bims/keys` under the config directory.
fn path() -> Option<std::path::PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => std::path::PathBuf::from(dir),
        _ => std::path::PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    Some(base.join("bims").join("keys"))
}

/// Tab is egui's key for moving keyboard focus to the next widget, and a
/// widget with focus is egui wanting the keyboard — so the frame after
/// Tab opened the inventory, every key would have been egui's and none the
/// screen's. Called at the top of a frame with the flag the last frame
/// left: the focus Tab gave is surrendered, and the flag is set again
/// when Tab goes down this frame with the keys the screen's, so the next
/// frame does the same. A text field that has focus keeps it: Tab is its
/// own then, and the screen is not reading keys at all.
pub fn release_tab_focus(ctx: &egui::Context, took: &mut bool, keys_are_ours: bool) {
    if std::mem::take(took)
        && let Some(id) = ctx.memory(|m| m.focused())
    {
        ctx.memory_mut(|m| m.surrender_focus(id));
    }
    *took = keys_are_ours && ctx.input(|i| i.key_pressed(egui::Key::Tab));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults are the keys the screens always had, the text round
    /// trips, and a line that names nothing is left alone.
    #[test]
    fn the_bindings_round_trip_through_their_text() {
        let keys = Keys::default();
        assert_eq!(keys.key(Action::Inventory), egui::Key::Tab);
        assert_eq!(keys.key(Action::Recruit), egui::Key::R);
        assert_eq!(keys.shared_with(Action::Recruit), vec![Action::Turn]);
        assert!(keys.shared_with(Action::Map).is_empty());
        // The class's two (features 74 and 75): Q and E, one each
        // whatever the class, and bound to nothing else.
        assert_eq!(keys.key(Action::ClassPrimary), egui::Key::Q);
        assert_eq!(keys.key(Action::ClassSecondary), egui::Key::E);
        assert!(keys.shared_with(Action::ClassPrimary).is_empty());
        assert!(keys.shared_with(Action::ClassSecondary).is_empty());
        // And the commander's two squad keys (feature 78): X and Z,
        // bound to nothing else.
        assert_eq!(keys.key(Action::SquadFallBack), egui::Key::X);
        assert_eq!(keys.key(Action::SquadStandGround), egui::Key::Z);
        assert!(keys.shared_with(Action::SquadFallBack).is_empty());
        assert!(keys.shared_with(Action::SquadStandGround).is_empty());
        // And every player's own two (feature 84): F attacks and T
        // retreats, bound to nothing else — which is what moved the
        // camera's Follow off F onto V.
        assert_eq!(keys.key(Action::Attack), egui::Key::F);
        assert_eq!(keys.key(Action::Retreat), egui::Key::T);
        assert_eq!(keys.key(Action::Follow), egui::Key::V);
        assert!(keys.shared_with(Action::Attack).is_empty());
        assert!(keys.shared_with(Action::Retreat).is_empty());
        let mut changed = keys;
        changed.set(Action::Inventory, egui::Key::I);
        changed.set(Action::PanUp, egui::Key::ArrowUp);
        let text = changed.to_text();
        assert_eq!(text.lines().count(), Action::ALL.len());
        assert!(text.contains("inventory=I\n"));
        assert_eq!(Keys::from_text(&text), changed);
        let back = Keys::from_text("inventory=I\nnonsense=Q\nmap=NoSuchKey\n\n");
        assert_eq!(back.key(Action::Inventory), egui::Key::I);
        assert_eq!(back.key(Action::Map), egui::Key::M);
        // Every action has a name of its own, or the file could not tell
        // two apart.
        let mut names: Vec<&str> = Action::ALL.iter().map(|a| a.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Action::ALL.len());
    }
}
