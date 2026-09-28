//! The keys: which key does what, and the player's say over it.
//!
//! Every hotkey a screen reads is an [`Action`] looked up in [`Keys`]
//! rather than a key written into the screen, so the Controls page of the
//! Esc sheet can rebind any of them: click the key beside an action, press
//! the one you want, Esc to think again. Two actions may share a key —
//! Turn shares R with the fourth ability slot by default (task 123), and
//! is read only in the yard and the armoury, where no ability is used —
//! so nothing refuses a binding; the page says where a key is used
//! twice. Esc itself is not an action: it is what closes the
//! sheet and cancels a rebind, and a key that could be bound away from
//! that is a sheet that cannot be closed.
//!
//! The bindings are kept in a plain text file, `keys` under the app's
//! own directory in the user's config directory (`$XDG_CONFIG_HOME` or
//! `~/.config`, then `bims/`), one `action=Key` a line by egui's key
//! names, written whenever a binding changes and read at start; a line
//! that names no action or no key is skipped, and a missing file is the
//! defaults. The one other thing kept there is the **edge-scroll speed**
//! (task 123), `edge-scroll-speed=1.0`, the pointer's say over the camera
//! as the keys are the keyboard's; the volumes and the UI scale still
//! start afresh each run, and when they are kept, this is the file they
//! go in. A file written before task 123 names the class's two keys
//! `class-primary` and `class-secondary`: they are read as `ability-1`
//! and `ability-3`, the slots that do what they did.
//!
//! **Ctrl and a slot's key ranks the ability up** (task 123) rather than
//! using it: not an action of its own and not bindable, it follows
//! whatever key the slot is on ([`Keys::rank_up_asked`]), and a key read
//! as the ability itself is read with Ctrl up ([`Keys::used`]).

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
    PanLeft,
    PanRight,
    PanUp,
    PanDown,
    /// The crew member you steer, selected and in the middle. On F1
    /// since the ability slots took C (task 123).
    Select,
    /// Recruit them, or let them go. On L since the fourth ability slot
    /// took R (task 123).
    Recruit,
    /// Turn the part in hand in the yard, or a thing in the armoury. Read
    /// only there, so it shares R with the fourth ability slot (task 123).
    Turn,
    /// Open and close the inventory of the crew member you steer.
    Inventory,
    /// The steered crew member's first ability slot, on Q (task 123;
    /// the class's first action since features 74 to 77): an engineer
    /// sets a sentry up on the deck tile under the pointer, a soldier
    /// throws a grenade at it, a medic triggers its surge, a tank
    /// taunts. Nothing with a classless crew member steered.
    Ability1,
    /// The second slot, on C: empty for every class so far.
    Ability2,
    /// The third, on E — the class's second action as it was: an
    /// engineer lays sandbags on the tile under the pointer, a soldier
    /// braces or stands easy, a medic beams the crew member under the
    /// pointer, a tank puts its wall up or down.
    Ability3,
    /// The fourth, on R: empty for every class so far.
    Ability4,
    /// A commander calls the squad back to the deck tile under the
    /// pointer, or to himself with the pointer on nothing (feature 78).
    /// Nothing for any other class.
    SquadFallBack,
    /// A commander has the squad hold exactly where it stands.
    SquadStandGround,
    /// **Attack-move**: arms the pointer, and the next click on the deck
    /// sends the Bim you steer there with its weapon out, stopping to
    /// shoot whatever comes into its sights on the way — Dota's
    /// attack-move. On F, which the bots' banner had until then.
    AttackMove,
    /// **Attack** (feature 84): arms the pointer, and the next click on
    /// the deck puts an attack banner down there for the bots that
    /// follow you. Pressed again with the pointer armed, or with the
    /// banner already where you click, it is called off. On X since the
    /// attack-move took F.
    Attack,
    /// **Retreat**: the bots that follow you fall back to the ship and
    /// hold there. Pressed again, they go back to following.
    Retreat,
    /// **Carry** (feature 86): a medic takes the crewmate under the
    /// pointer up into its arms — a downed one — to
    /// walk them out of the fire, and sets down whoever it is carrying
    /// when pressed again. Nothing for anybody but a medic of the class
    /// or a hired field medic.
    Carry,
    /// **The character sheet** (feature 107): the player's own Bim's
    /// class, level, body, gear and talents, on the left of the canvas.
    /// Pressed again, it shuts.
    CharacterSheet,
    /// **Revive**: held, the Bim you steer gets the downed crewmate
    /// nearest it back up — it must be standing close — and lets go when
    /// the key comes up before they are. On G; the carry moved to H.
    Revive,
}

impl Action {
    /// The four ability slots, in the order the hero panel lays them
    /// out (task 123).
    pub const ABILITIES: [Action; 4] = [
        Action::Ability1,
        Action::Ability2,
        Action::Ability3,
        Action::Ability4,
    ];

    pub const ALL: [Action; 25] = [
        Action::Map,
        Action::NorthUp,
        Action::Follow,
        Action::Pause,
        Action::Speed1,
        Action::PanLeft,
        Action::PanRight,
        Action::PanUp,
        Action::PanDown,
        Action::Select,
        Action::Recruit,
        Action::Turn,
        Action::Inventory,
        Action::Ability1,
        Action::Ability2,
        Action::Ability3,
        Action::Ability4,
        Action::SquadFallBack,
        Action::SquadStandGround,
        Action::AttackMove,
        Action::Attack,
        Action::Retreat,
        Action::Carry,
        Action::CharacterSheet,
    ];

    /// The key it starts on.
    pub fn default_key(self) -> egui::Key {
        Action::Revive,
        use egui::Key;
        match self {
            Action::Map => Key::M,
            Action::NorthUp => Key::N,
            // F is the attack-move and X the bots' banner, which had F
            // from feature 84 until then — so following the camera is V.
            Action::Follow => Key::V,
            Action::Pause => Key::Space,
            Action::Speed1 => Key::Num1,
            Action::PanLeft => Key::A,
            Action::PanRight => Key::D,
            Action::PanUp => Key::W,
            Action::PanDown => Key::S,
            // C and R are the second and fourth ability slots (task
            // 123), so Select went to F1 and Recruit to L. Turn keeps
            // R: it is read only in the yard and the armoury, where no
            // ability is used.
            Action::Select => Key::F1,
            Action::Recruit => Key::L,
            Action::Turn => Key::R,
            Action::Inventory => Key::Tab,
            Action::Ability1 => Key::Q,
            Action::Ability2 => Key::C,
            Action::Ability3 => Key::E,
            Action::Ability4 => Key::R,
            Action::SquadFallBack => Key::T,
            Action::SquadStandGround => Key::Z,
            Action::AttackMove => Key::F,
            Action::Attack => Key::X,
            Action::Retreat => Key::Y,
            // G is the held revive, so the medic's carry went to H.
            Action::Carry => Key::H,
            Action::Revive => Key::G,
            Action::CharacterSheet => Key::K,
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
            Action::PanLeft => "pan-left",
            Action::PanRight => "pan-right",
            Action::PanUp => "pan-up",
            Action::PanDown => "pan-down",
            Action::Select => "select",
            Action::Recruit => "recruit",
            Action::Turn => "turn",
            Action::Inventory => "inventory",
            Action::Ability1 => "ability-1",
            Action::Ability2 => "ability-2",
            Action::Ability3 => "ability-3",
            Action::Ability4 => "ability-4",
            Action::SquadFallBack => "squad-fall-back",
            Action::SquadStandGround => "squad-stand-ground",
            Action::AttackMove => "attack-move",
            Action::Attack => "attack",
            Action::Retreat => "retreat",
            Action::Carry => "carry",
            Action::CharacterSheet => "character-sheet",
        }
    }

    /// What it does, for the Controls page.
    pub fn what(self) -> &'static str {
        match self {
            Action::Map => "Switch between the ship and the map.",
            Action::Revive => "revive",
            Action::NorthUp => "Turn the view head up or north up.",
            Action::Follow => {
                "Follow the crew member you steer, and the ship on the map, or let the camera go free."
            }
            Action::Pause => "Pause the world, or set it going again.",
            Action::Speed1 => "Run the world at 1×.",
            Action::PanLeft => "Pan the view left. Middle-drag does the same.",
            Action::PanRight => "Pan the view right.",
            Action::PanUp => "Pan the view up.",
            Action::PanDown => "Pan the view down.",
            Action::Select => {
                "Select the crew member you steer, and put them in the middle of the view."
            }
            Action::Recruit => "Recruit the crew member you steer, or let them go.",
            Action::Turn => {
                "Turn the part in hand in the yard, or a thing in the armoury. Read only there, so it shares R with the fourth ability slot."
            }
            Action::Inventory => "Open and close the inventory of the crew member you steer.",
            Action::Ability1 => {
                "The first ability slot, by the crew member you steer: an engineer sets a sentry up on the deck tile under the pointer, out of a kit in its pack; a soldier throws a grenade at it; a medic triggers its surge; a tank taunts; a commander rallies. With Ctrl held, it is ranked up instead."
            }
            Action::Ability2 => {
                "The second ability slot: empty for every class for now. With Ctrl held, it is ranked up instead."
            }
            Action::Ability3 => {
                "The third ability slot: an engineer lays sandbags on the deck tile under the pointer, out of a kit in its pack; a soldier braces where it stands, or stands easy again; a medic beams the crew member under the pointer, and unlinks when pressed on the one it holds or on nobody; a tank puts its wall up, or takes it down; a commander sends the squad at the enemy under the pointer. With Ctrl held, it is ranked up instead."
            }
            Action::Ability4 => {
                "The fourth ability slot: empty for every class for now. With Ctrl held, it is ranked up instead."
            }
            Action::SquadFallBack => {
                "A commander calls the squad back to the deck tile under the pointer, or to himself with the pointer on nothing. Nothing for any other class."
            }
            Action::SquadStandGround => {
                "A commander has the squad hold exactly where it stands. Nothing for any other class."
            }
            Action::AttackMove => {
                "Arm the pointer — it turns red — and the next click on the deck sends the Bim you steer there with its weapon out. It stops to shoot whatever comes into its sights on the way, and walks on once nothing is left."
            }
            Action::Attack => {
                "Arm the pointer — it turns red — and the next click on the deck puts an attack banner down there for the bots. The crew that follow you fight their way to it, taking the cover on the way and pushing on when nothing is in range. Press it again to think better of it, or click the banner where it already stands to call it off."
            }
            Action::Retreat => {
                "The crew that follow you fall back to the ship and hold there. Press it again and they go back to keeping to your side. Nobody leaves a fight aboard the ship: cornered in your own hull they stand and shoot whatever they were told."
            }
            Action::Carry => {
                "A medic picks the downed crewmate under the pointer up and carries them out of the fire, holding its fire and walking slowly while it does. Press it again to set them down, and revive them where it is quiet. Nothing for anybody but a medic or a hired field medic. A right-click on a downed crewmate offers the same."
            }
            Action::Revive => {
                "Hold it standing close to a downed crewmate and the Bim you steer gets them back up — the nearest of them. Let go before they are up and it stops. A bar over them shows how far it has got."
            }
            Action::CharacterSheet => {
                "Open and close your Bim's character sheet: its class and level, its health, what it wears and holds, and the talent tree a level's pick is spent on."
            }
        }
    }

    /// The action a file's name is for — and the two names the class's
    /// keys had before the four slots (task 123), read as the slots that
    /// do what they did.
    fn from_name(name: &str) -> Option<Action> {
        match name {
            "class-primary" => Some(Action::Ability1),
            "class-secondary" => Some(Action::Ability3),
            _ => Action::ALL.iter().copied().find(|a| a.name() == name),
        }
    }

    /// Which of the four ability slots it is, nought to three.
    pub fn ability_slot(self) -> Option<usize> {
        Action::ABILITIES.iter().position(|&a| a == self)
    }
}

/// The edge-scroll speed a new player starts at, in tenths: 1.0×.
pub const EDGE_SCROLL_DEFAULT: u8 = 10;
/// The most the Esc sheet's slider goes to, in tenths: 3.0×.
pub const EDGE_SCROLL_MAX: u8 = 30;
/// The name the edge-scroll speed is kept under in the keys file.
const EDGE_SCROLL_NAME: &str = "edge-scroll-speed";

/// The bindings, one key an action, which action the Controls page is
/// waiting on a key for, and the edge-scroll speed.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Keys {
    keys: [egui::Key; Action::ALL.len()],
    /// The action whose key is being chosen: the next key pressed is it.
    pub listening: Option<Action>,
    /// How fast the camera pans with the pointer against the window's
    /// edge (task 123), in tenths of `designer::PAN_SPEED`: nought is
    /// off. Tenths, since the slider steps by them and so the bindings
    /// stay `Eq`. Local to this player, never sent anywhere.
    pub edge_scroll: u8,
}

impl Default for Keys {
    fn default() -> Keys {
        Keys {
            keys: Action::ALL.map(|a| a.default_key()),
            listening: None,
            edge_scroll: EDGE_SCROLL_DEFAULT,
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

    /// Whether the action's key went down among `events` **with Ctrl
    /// up** — how an ability slot's key is read as the ability itself
    /// (task 123), since with Ctrl held it is a rank-up instead.
    pub fn used(&self, events: &[egui::Event], modifiers: egui::Modifiers, action: Action) -> bool {
        !modifiers.ctrl && key_went_down(events, self.key(action))
    }

    /// The ability slot whose rank-up Ctrl and its key asked for among
    /// `events` (task 123), or `None`. Not an action of its own: it is
    /// whatever key the slot is bound to, with Ctrl held. Ctrl+C is read
    /// whichever way it comes — bevy_egui 0.42 sends the key *and*
    /// `Event::Copy`, and a build that sent the copy alone would still
    /// be heard — and so are Ctrl+X (`Event::Cut`) and Ctrl+V
    /// (`Event::Paste`) for a slot rebound onto them.
    pub fn rank_up_asked(
        &self,
        events: &[egui::Event],
        modifiers: egui::Modifiers,
    ) -> Option<Action> {
        if !modifiers.ctrl {
            return None;
        }
        Action::ABILITIES.into_iter().find(|&action| {
            let key = self.key(action);
            key_went_down(events, key)
                || events.iter().any(|e| match e {
                    egui::Event::Copy => key == egui::Key::C,
                    egui::Event::Cut => key == egui::Key::X,
                    egui::Event::Paste(_) => key == egui::Key::V,
                    _ => false,
                })
        })
    }

    /// The edge-scroll speed as a factor of `designer::PAN_SPEED`.
    pub fn edge_scroll_speed(&self) -> f32 {
        f32::from(self.edge_scroll) / 10.0
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

    /// The file's text: one `action=Key` a line, and the edge-scroll
    /// speed last.
    pub fn to_text(self) -> String {
        let mut text: String = Action::ALL
            .iter()
            .map(|&a| format!("{}={}\n", a.name(), self.key(a).name()))
            .collect();
        text.push_str(&format!(
            "{EDGE_SCROLL_NAME}={:.1}\n",
            self.edge_scroll_speed()
        ));
        text
    }

    /// The bindings a file's text says, over the defaults; a line that
    /// names no action or no key is skipped.
    pub fn from_text(text: &str) -> Keys {
        let mut keys = Keys::default();
        for line in text.lines() {
            let Some((name, key)) = line.split_once('=') else {
                continue;
            };
            if name.trim() == EDGE_SCROLL_NAME {
                // A number the slider could have set, or the default.
                if let Ok(speed) = key.trim().parse::<f32>()
                    && speed.is_finite()
                {
                    keys.edge_scroll = (speed * 10.0)
                        .round()
        // A file from before the held revive took G has the carry on G
        // and no revive line: the carry goes to its new key, H, rather
        // than sharing G with the revive.
        let old_carry = !text.lines().any(|l| l.trim_start().starts_with("revive="));
                        .clamp(0.0, f32::from(EDGE_SCROLL_MAX))
                        as u8;
                }
                continue;
            }
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
                if old_carry && action == Action::Carry && key == Action::Revive.default_key() {
                    continue;
                }
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

/// Whether `key` went down among `events`, repeats and all — what
/// `egui::InputState::key_pressed` asks, over a list a test can make.
fn key_went_down(events: &[egui::Event], key: egui::Key) -> bool {
    events
        .iter()
        .any(|e| matches!(e, egui::Event::Key { key: k, pressed: true, .. } if *k == key))
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
        assert!(keys.shared_with(Action::Map).is_empty());
        // The four ability slots are Q, C, E and R (task 123), the first
        // three bound to nothing else and the fourth sharing R with
        // Turn, which only the yard and the armoury read.
        assert_eq!(keys.key(Action::Ability1), egui::Key::Q);
        assert_eq!(keys.key(Action::Ability2), egui::Key::C);
        assert_eq!(keys.key(Action::Ability3), egui::Key::E);
        assert_eq!(keys.key(Action::Ability4), egui::Key::R);
        assert!(keys.shared_with(Action::Ability1).is_empty());
        assert!(keys.shared_with(Action::Ability2).is_empty());
        assert!(keys.shared_with(Action::Ability3).is_empty());
        assert_eq!(keys.shared_with(Action::Ability4), vec![Action::Turn]);
        // So Select left C for F1 and Recruit left R for L; Turn stays.
        assert_eq!(keys.key(Action::Select), egui::Key::F1);
        assert_eq!(keys.key(Action::Recruit), egui::Key::L);
        assert_eq!(keys.key(Action::Turn), egui::Key::R);
        assert!(keys.shared_with(Action::Select).is_empty());
        assert!(keys.shared_with(Action::Recruit).is_empty());
        // And the pan is WASD, as it always was.
        assert_eq!(keys.key(Action::PanLeft), egui::Key::A);
        assert_eq!(keys.key(Action::PanRight), egui::Key::D);
        assert_eq!(keys.key(Action::PanUp), egui::Key::W);
        assert_eq!(keys.key(Action::PanDown), egui::Key::S);
        // And the commander's two squad keys (feature 78): T and Z,
        // bound to nothing else — the fall back on T since the bots'
        // banner took X.
        assert_eq!(keys.key(Action::SquadFallBack), egui::Key::T);
        assert_eq!(keys.key(Action::SquadStandGround), egui::Key::Z);
        assert!(keys.shared_with(Action::SquadFallBack).is_empty());
        assert!(keys.shared_with(Action::SquadStandGround).is_empty());
        // The attack-move is F, for the Bim you steer; every player's
        // two orders for the bots (feature 84) are X to attack and Y to
        // fall back to the ship; none shares its key. F was the bots'
        // banner once, which is what moved the camera's Follow onto V.
        assert_eq!(keys.key(Action::AttackMove), egui::Key::F);
        assert_eq!(keys.key(Action::Attack), egui::Key::X);
        assert_eq!(keys.key(Action::Retreat), egui::Key::Y);
        assert_eq!(keys.key(Action::Follow), egui::Key::V);
        assert!(keys.shared_with(Action::AttackMove).is_empty());
        assert!(keys.shared_with(Action::Attack).is_empty());
        assert!(keys.shared_with(Action::Retreat).is_empty());
        // And the medic's carry (feature 86): G, bound to nothing else.
        assert_eq!(keys.key(Action::Carry), egui::Key::G);
        assert!(keys.shared_with(Action::Carry).is_empty());
        let mut changed = keys;
        changed.set(Action::Inventory, egui::Key::I);
        changed.set(Action::PanUp, egui::Key::ArrowUp);
        let text = changed.to_text();
        // A line an action, and the edge-scroll speed's.
        assert_eq!(text.lines().count(), Action::ALL.len() + 1);
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

    /// The character sheet's key (feature 107) starts on K, shares it with
    /// nothing, and comes back through the settings file as it went in —
    /// the default when the file does not name it, a rebinding when it
    /// does.
    #[test]
    fn the_character_sheet_has_k_and_survives_the_settings_file() {
        let keys = Keys::default();
        assert_eq!(keys.key(Action::CharacterSheet), egui::Key::K);
        assert!(keys.shared_with(Action::CharacterSheet).is_empty());
        let text = keys.to_text();
        assert!(text.contains("character-sheet=K\n"));
        assert_eq!(Keys::from_text(&text), keys);
        // A file written before the action existed leaves it on K.
        let old: String = text
            .lines()
            .filter(|l| !l.starts_with("character-sheet="))
            .map(|l| format!("{l}\n"))
            .collect();
        assert_eq!(
            Keys::from_text(&old).key(Action::CharacterSheet),
            egui::Key::K
        );
        let mut moved = keys;
        moved.set(Action::CharacterSheet, egui::Key::J);
        let back = Keys::from_text(&moved.to_text());
        assert_eq!(back.key(Action::CharacterSheet), egui::Key::J);
        assert_eq!(back, moved);
    }

    /// A key going down, as bevy_egui hands it over.
    fn down(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    /// A keys file from before the four slots (task 123) names the
    /// class's two keys `class-primary` and `class-secondary`: they come
    /// back as the first and third slots, which do what they did, and
    /// the file written again uses the new names.
    #[test]
    fn the_old_class_keys_are_read_as_the_first_and_third_slots() {
        let keys = Keys::from_text("class-primary=G\nclass-secondary=H\n");
        assert_eq!(keys.key(Action::Ability1), egui::Key::G);
        assert_eq!(keys.key(Action::Ability3), egui::Key::H);
        assert_eq!(keys.key(Action::Ability2), egui::Key::C);
        assert_eq!(keys.key(Action::Ability4), egui::Key::R);
        let text = keys.to_text();
        assert!(text.contains("ability-1=G\n") && text.contains("ability-3=H\n"));
        assert!(!text.contains("class-"));
        assert_eq!(Keys::from_text(&text), keys);
    }

    /// Ctrl and a slot's key is that slot's rank-up (task 123) and never
    /// the ability: Ctrl+Q is not the first slot used, and a plain Q is
    /// no rank-up. It follows the binding rather than being one.
    #[test]
    fn ctrl_and_a_slot_s_key_ranks_it_up_and_does_not_use_it() {
        let keys = Keys::default();
        let ctrl = egui::Modifiers::CTRL;
        let none = egui::Modifiers::NONE;
        let q = [down(egui::Key::Q, ctrl)];
        assert!(!keys.used(&q, ctrl, Action::Ability1), "Ctrl+Q is no Q");
        assert_eq!(keys.rank_up_asked(&q, ctrl), Some(Action::Ability1));
    /// The held revive is G and the carry moved to H; a file from before,
    /// with the carry on G and no revive line, is read with the carry on
    /// H rather than sharing G, and one that says both keeps its word.
    #[test]
    fn the_revive_is_g_and_an_old_carry_on_g_moves_to_h() {
        let keys = Keys::default();
        assert_eq!(keys.key(Action::Revive), egui::Key::G);
        assert_eq!(keys.key(Action::Carry), egui::Key::H);
        assert!(keys.shared_with(Action::Revive).is_empty());
        assert!(keys.shared_with(Action::Carry).is_empty());
        let old = Keys::from_text("carry=G\n");
        assert_eq!(old.key(Action::Carry), egui::Key::H);
        assert_eq!(old.key(Action::Revive), egui::Key::G);
        let both = Keys::from_text("carry=G\nrevive=J\n");
        assert_eq!(both.key(Action::Carry), egui::Key::G);
        assert_eq!(both.key(Action::Revive), egui::Key::J);
        assert_eq!(Keys::from_text(&keys.to_text()), keys);
    }

        let q = [down(egui::Key::Q, none)];
        assert!(keys.used(&q, none, Action::Ability1));
        assert_eq!(keys.rank_up_asked(&q, none), None);
        let e = [down(egui::Key::E, ctrl)];
        assert_eq!(keys.rank_up_asked(&e, ctrl), Some(Action::Ability3));
        let r = [down(egui::Key::R, ctrl)];
        assert_eq!(keys.rank_up_asked(&r, ctrl), Some(Action::Ability4));
        // A key no slot is on asks nothing.
        let m = [down(egui::Key::M, ctrl)];
        assert_eq!(keys.rank_up_asked(&m, ctrl), None);
        // The slot moved, the rank-up moves with it.
        let mut moved = keys;
        moved.set(Action::Ability1, egui::Key::B);
        assert_eq!(moved.rank_up_asked(&q, ctrl), None);
        let b = [down(egui::Key::B, ctrl)];
        assert_eq!(moved.rank_up_asked(&b, ctrl), Some(Action::Ability1));
    }

    /// Ctrl+C is the second slot's rank-up and not Select, however
    /// bevy_egui says it: the key with Ctrl held (0.42 does), the key and
    /// `Event::Copy` together, or the copy alone.
    #[test]
    fn ctrl_c_ranks_the_second_slot_up_whichever_way_it_comes() {
        let keys = Keys::default();
        let ctrl = egui::Modifiers::CTRL;
        let key = down(egui::Key::C, ctrl);
        for events in [
            vec![key.clone()],
            vec![key, egui::Event::Copy],
            vec![egui::Event::Copy],
        ] {
            assert_eq!(
                keys.rank_up_asked(&events, ctrl),
                Some(Action::Ability2),
                "{events:?}"
            );
            assert!(!keys.used(&events, ctrl, Action::Ability2));
            assert!(!keys.used(&events, ctrl, Action::Select), "{events:?}");
        }
        // Select is F1 whatever else is held.
        assert!(!keys.used(
            &[down(egui::Key::C, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
            Action::Select
        ));
    }

    /// The edge-scroll speed (task 123) is kept in the keys file in
    /// tenths: the default 1.0, a value the slider could set read back
    /// exactly, one past the slider's end held to it, and a line that
    /// is not a number, or no line at all, the default.
    #[test]
    fn the_edge_scroll_speed_is_kept_beside_the_keys() {
        let keys = Keys::default();
        assert_eq!(keys.edge_scroll_speed(), 1.0);
        assert!(keys.to_text().contains("edge-scroll-speed=1.0\n"));
        let mut slow = keys;
        slow.edge_scroll = 3;
        let text = slow.to_text();
        assert!(text.contains("edge-scroll-speed=0.3\n"), "{text}");
        assert_eq!(Keys::from_text(&text), slow);
        assert_eq!(Keys::from_text("edge-scroll-speed=0\n").edge_scroll, 0);
        assert_eq!(
            Keys::from_text("edge-scroll-speed=9.5\n").edge_scroll,
            EDGE_SCROLL_MAX
        );
        assert_eq!(
            Keys::from_text("edge-scroll-speed=fast\n").edge_scroll,
            EDGE_SCROLL_DEFAULT
        );
        assert_eq!(Keys::from_text("map=M\n").edge_scroll, EDGE_SCROLL_DEFAULT);
    }
}
