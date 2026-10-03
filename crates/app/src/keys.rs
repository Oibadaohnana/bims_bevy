//! The keys: which key does what, and the player's say over it.
//!
//! Every hotkey a screen reads is an [`Action`] looked up in [`Keys`]
//! rather than a key written into the screen, so the Controls page of the
//! Esc sheet can rebind any of them: click the key beside an action, press
//! the one you want, Esc to think again. Two actions may share a key —
//! Turn shares R with the reload by default (task 123; the fourth ability
//! slot had R until October 2026), and is read only in the yard and the
//! armoury, where no gun is reloaded —
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
    /// The engineer's remote trigger (task 154): every satchel charge of
    /// his set off. On Space, which paused the world until then — the
    /// pause is the Esc sheet's now — and on G since October 2026, when
    /// the ultimate took Space.
    Detonate,
    Speed1,
    /// Walk the Bim you steer up the screen (task 144). The four walks
    /// share W, A, S and D with the pan, which the ship view no longer
    /// reads: there the camera follows the Bim, and the keys walk it.
    WalkUp,
    WalkDown,
    WalkLeft,
    WalkRight,
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
    /// only there, so it shares R with the reload (the fourth ability
    /// slot until October 2026; task 123).
    Turn,
    /// Open and close the inventory of the crew member you steer.
    Inventory,
    /// The steered crew member's first ability slot, on Q (task 123;
    /// the class's first action since features 74 to 77): an engineer
    /// lays a mine on the deck tile under the pointer, a soldier
    /// throws a grenade at it, a medic drops a Heal Drone, a tank
    /// raises or puts down his Riot Shield. Nothing with a classless crew member steered.
    Ability1,
    /// The second slot, on C: empty for every class so far.
    Ability2,
    /// The third, on E — the class's second action as it was: an
    /// engineer throws a satchel charge at the pointer, a soldier
    /// charges a Stun Shot at it, a medic beams the crew member under the
    /// pointer, a tank raises his Reflect Barrier.
    Ability3,
    /// The fourth, the class's ultimate: on R from task 123, on G since
    /// October 2026, when R went to the reload.
    Ability4,
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
    /// **Revive**: held, the Bim you steer gets the downed crewmate
    /// nearest it back up — it must be standing close — and lets go when
    /// the key comes up before they are. On G until October 2026, when
    /// the ultimate took G and the revive went to T, which the reload
    /// had left.
    Revive,
    /// **The medkit** (task 138; one key since October 2026): the medkit
    /// into the hands of the Bim you steer — it holds its fire, and a
    /// right-click on a downed crewmate revives them — or, with it in hand
    /// already, the weapon back. On H, the items having taken 1 to 4.
    Medkit,
    /// **The four item slots** (October 2026): the item in that slot of
    /// the Bim you steer used at the pointer — a Blink Drive blinks there.
    /// On 1, 2, 3 and 4, where the quickselect was.
    Item1,
    Item2,
    Item3,
    Item4,
    /// **The character sheet** (feature 107): the player's own Bim's
    /// class, level, body, gear and skills, on the left of the canvas.
    /// Pressed again, it shuts.
    CharacterSheet,
    /// **Reload** (October 2026): the Bim you steer reloads the magazine
    /// in its hand now, shots left in it or not. On R since October 2026
    /// (on T until then, while R was the ultimate's).
    Reload,
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

    /// The four item slots, in the order the hero panel lays them out.
    pub const ITEMS: [Action; 4] = [Action::Item1, Action::Item2, Action::Item3, Action::Item4];

    pub const ALL: [Action; 33] = [
        Action::Map,
        Action::NorthUp,
        Action::Follow,
        Action::Detonate,
        Action::Speed1,
        Action::WalkUp,
        Action::WalkDown,
        Action::WalkLeft,
        Action::WalkRight,
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
        Action::AttackMove,
        Action::Attack,
        Action::Retreat,
        Action::Carry,
        Action::Revive,
        Action::Medkit,
        Action::Item1,
        Action::Item2,
        Action::Item3,
        Action::Item4,
        Action::CharacterSheet,
        Action::Reload,
    ];

    /// The key it starts on.
    pub fn default_key(self) -> egui::Key {
        use egui::Key;
        match self {
            Action::Map => Key::M,
            Action::NorthUp => Key::N,
            // F is the attack-move and X the bots' banner, which had F
            // from feature 84 until then — so following the camera is V.
            Action::Follow => Key::V,
            // G since October 2026: Space is the ultimate's.
            Action::Detonate => Key::G,
            // 1 and 2 are the quickselect (task 138), so the 1× speed
            // went to the key left of them.
            Action::Speed1 => Key::Backtick,
            Action::WalkUp => Key::W,
            Action::WalkDown => Key::S,
            Action::WalkLeft => Key::A,
            Action::WalkRight => Key::D,
            Action::PanLeft => Key::A,
            Action::PanRight => Key::D,
            Action::PanUp => Key::W,
            Action::PanDown => Key::S,
            // C and R are the second and fourth ability slots (task
            // 123), so Select went to F1 and Recruit to L. Turn keeps
            // R: it is read only in the yard and the armoury, where no
            // gun is reloaded (R is the reload since October 2026).
            Action::Select => Key::F1,
            Action::Recruit => Key::L,
            Action::Turn => Key::R,
            Action::Inventory => Key::Tab,
            Action::Ability1 => Key::Q,
            Action::Ability2 => Key::C,
            Action::Ability3 => Key::E,
            // The ultimate is Space and the reload R (October 2026): the
            // ultimate went from R to G, the held revive to T, which the
            // reload had, and then the ultimate to Space and the remote
            // trigger to G.
            Action::Ability4 => Key::Space,
            Action::AttackMove => Key::F,
            Action::Attack => Key::X,
            Action::Retreat => Key::Y,
            // G is the held revive and H the medkit, so the medic's carry
            // went to B.
            Action::Carry => Key::B,
            Action::Revive => Key::T,
            Action::Medkit => Key::H,
            Action::Item1 => Key::Num1,
            Action::Item2 => Key::Num2,
            Action::Item3 => Key::Num3,
            Action::Item4 => Key::Num4,
            Action::CharacterSheet => Key::K,
            Action::Reload => Key::R,
        }
    }

    /// The name it is kept under in the file, and shown by on the page.
    pub fn name(self) -> &'static str {
        match self {
            Action::Map => "map",
            Action::NorthUp => "north-up",
            Action::Follow => "follow",
            Action::Detonate => "detonate",
            Action::Speed1 => "speed-1",
            Action::WalkUp => "walk-up",
            Action::WalkDown => "walk-down",
            Action::WalkLeft => "walk-left",
            Action::WalkRight => "walk-right",
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
            Action::AttackMove => "attack-move",
            Action::Attack => "attack",
            Action::Retreat => "retreat",
            Action::Carry => "carry",
            Action::Revive => "revive",
            Action::Medkit => "medkit",
            Action::Item1 => "item-1",
            Action::Item2 => "item-2",
            Action::Item3 => "item-3",
            Action::Item4 => "item-4",
            Action::CharacterSheet => "character-sheet",
            Action::Reload => "reload",
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
            Action::Detonate => {
                "An engineer's remote trigger: every satchel charge he has thrown goes off at once. The pause is on the Esc sheet."
            }
            Action::Speed1 => "Run the world at 1×, out of a pause.",
            Action::WalkUp => {
                "Walk the Bim you steer up the screen. The mouse aims it, and the left button fires."
            }
            Action::WalkDown => "Walk the Bim you steer down the screen.",
            Action::WalkLeft => "Walk the Bim you steer left.",
            Action::WalkRight => "Walk the Bim you steer right.",
            Action::PanLeft => {
                "Pan the view left: the yard and the map. Middle-drag does the same."
            }
            Action::PanRight => "Pan the view right.",
            Action::PanUp => "Pan the view up.",
            Action::PanDown => "Pan the view down.",
            Action::Select => {
                "Select the crew member you steer, and put them in the middle of the view."
            }
            Action::Recruit => "Recruit the crew member you steer, or let them go.",
            Action::Turn => {
                "Turn the part in hand in the yard, or a thing in the armoury. Read only there, so it shares R with the reload."
            }
            Action::Inventory => "Open and close the inventory of the crew member you steer.",
            Action::Ability1 => {
                "The first ability slot, by the crew member you steer: an engineer lays a mine on the deck tile under the pointer, which goes off when an enemy comes within a tile of it; a soldier throws a grenade at it; a medic drops a Heal Drone; a tank raises or puts down his Riot Shield; a commander calls a Battle Cry. With Ctrl held, it is ranked up instead."
            }
            Action::Ability2 => {
                "The second ability slot: empty for every class for now. With Ctrl held, it is ranked up instead."
            }
            Action::Ability3 => {
                "The third ability slot: an engineer throws a satchel charge at the pointer, held to aim and let go to throw — several may lie on one tile, and G sets them all off; a soldier charges a Stun Shot at the pointer, two seconds planted before it fires; a medic beams the crew member under the pointer, and unlinks when pressed on the one it holds or on nobody; a tank raises his Reflect Barrier; a commander rallies. With Ctrl held, it is ranked up instead."
            }
            Action::Ability4 => {
                "The fourth ability slot, the ultimate: a soldier goes on a Rampage; an engineer lays its sentry on the deck tile under the pointer; a medic switches his Healing Circle on or off; a tank throws his Bastion over the crew round him. With Ctrl held, it is ranked up instead."
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
                "A medic picks the downed crewmate under the pointer up and carries them out of the fire, holding its fire and walking slowly while it does. Press it again to set them down, and revive them where it is quiet. Nothing for anybody but a medic or a hired field medic. A left click on a downed crewmate offers the same."
            }
            Action::Revive => {
                "Hold it standing close to a downed crewmate and the Bim you steer gets them back up — the nearest of them. Let go before they are up and it stops. A bar over them shows how far it has got. A right-click on a downed crewmate walks over and revives them too."
            }
            Action::Medkit => {
                "Take the medkit in hand: the Bim you steer holds its fire, and a right-click on a downed crewmate walks over and revives them. Their countdown stands while the hands are on them. Pressed again, the weapon is back in hand."
            }
            Action::Item1 => {
                "Use the item in the first item slot of the Bim you steer at the pointer: a Blink Drive puts it there, as far as the drive reaches."
            }
            Action::Item2 => "Use the item in the second item slot.",
            Action::Item3 => "Use the item in the third item slot.",
            Action::Item4 => "Use the item in the fourth item slot.",
            Action::CharacterSheet => {
                "Open and close your Bim's character sheet: its class and level, its health, what it wears and holds, and the four abilities a level's skill point is spent on."
            }
            Action::Reload => {
                "Reload the gun of the Bim you steer now, shots left in the magazine or not. An empty magazine reloads by itself, and there is no end to the magazines: only the seconds the reload takes."
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

/// The most the guest's playout buffer may hold (task 148,
/// `crate::playout`) for a new player, in hundredths of a second: 0.4 s,
/// which it reaches only on a line that needs it.
pub const NET_BUFFER_DEFAULT: u8 = 40;
/// The most the Esc sheet's slider goes to, in hundredths: one second.
pub const NET_BUFFER_MAX: u8 = 100;
/// The name the buffer's cap is kept under in the keys file.
const NET_BUFFER_NAME: &str = "network-buffer";

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
    /// The most the playout buffer may hold back the host's steps on a
    /// guest (task 148), in hundredths of a second: nought is off. Kept
    /// beside the keys like the edge scroll; this player's own.
    pub net_buffer: u8,
}

impl Default for Keys {
    fn default() -> Keys {
        Keys {
            keys: Action::ALL.map(|a| a.default_key()),
            listening: None,
            edge_scroll: EDGE_SCROLL_DEFAULT,
            net_buffer: NET_BUFFER_DEFAULT,
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

    /// Whether the action's key went down among `events`, Shift held or
    /// not: under Shift a digit comes over as its symbol (`!` for 1 on
    /// most layouts), so with Shift down a key whose symbol is no letter
    /// counts by where it sits on the board — how an item's key is read
    /// mid-sprint. A letter never does: on QWERTZ the key where QWERTY
    /// has Z is Y.
    pub fn pressed_through_shift(&self, events: &[egui::Event], action: Action) -> bool {
        let key = self.key(action);
        events.iter().any(|e| {
            matches!(e, egui::Event::Key { key: k, physical_key, pressed: true, modifiers, .. }
                if *k == key
                    || (modifiers.shift && *physical_key == Some(key) && !is_letter(*k)))
        })
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

    /// The playout buffer's cap, in seconds; nought is off.
    pub fn net_buffer_seconds(&self) -> f64 {
        f64::from(self.net_buffer) / 100.0
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
    /// speed and the network buffer last.
    pub fn to_text(self) -> String {
        let mut text: String = Action::ALL
            .iter()
            .map(|&a| format!("{}={}\n", a.name(), self.key(a).name()))
            .collect();
        text.push_str(&format!(
            "{EDGE_SCROLL_NAME}={:.1}\n",
            self.edge_scroll_speed()
        ));
        text.push_str(&format!(
            "{NET_BUFFER_NAME}={:.2}\n",
            self.net_buffer_seconds()
        ));
        text
    }

    /// The bindings a file's text says, over the defaults; a line that
    /// names no action or no key is skipped.
    pub fn from_text(text: &str) -> Keys {
        let mut keys = Keys::default();
        // A file from before the held revive took G has the carry on G
        // and no revive line: the carry goes to its own key, B, rather
        // than sharing G with the revive.
        let old_carry = !text.lines().any(|l| l.trim_start().starts_with("revive="));
        // And one from before the quickselect took 1 (task 138) has the
        // 1× speed there: it goes to its new key rather than sharing 1
        // with the weapon.
        let old_speed = !text.lines().any(|l| {
            let l = l.trim_start();
            l.starts_with("hand-weapon=") || l.starts_with("item-1=")
        });
        // And one from before the items took 1 to 4 and the medkit H
        // (October 2026) has the carry on H: it goes to its new key, B,
        // rather than sharing H with the medkit.
        let old_medkit = !text.lines().any(|l| l.trim_start().starts_with("medkit="));
        // And one from before the ultimate took G and the reload R
        // (October 2026) — its reload on T, or no reload line at all —
        // has the ultimate on R and the revive on G: those three go to
        // their new keys rather than the ultimate sharing R with the
        // reload and G with the revive. A key moved off them keeps its
        // word.
        let old_ultimate = text.lines().all(|l| {
            let l = l.trim_start();
            !l.starts_with("reload=") || l.trim_end() == "reload=T"
        });
        // And one from before the ultimate took Space and the remote
        // trigger G (October 2026) has the trigger on Space: it and an
        // ultimate on G go to their new keys rather than the two
        // sharing. A key moved off them keeps its word.
        let old_space = text.lines().any(|l| l.trim() == "detonate=Space");
        for line in text.lines() {
            let Some((name, key)) = line.split_once('=') else {
                continue;
            };
            if name.trim() == NET_BUFFER_NAME {
                if let Ok(seconds) = key.trim().parse::<f64>()
                    && seconds.is_finite()
                {
                    keys.net_buffer = (seconds * 100.0)
                        .round()
                        .clamp(0.0, f64::from(NET_BUFFER_MAX))
                        as u8;
                }
                continue;
            }
            if name.trim() == EDGE_SCROLL_NAME {
                // A number the slider could have set, or the default.
                if let Ok(speed) = key.trim().parse::<f32>()
                    && speed.is_finite()
                {
                    keys.edge_scroll = (speed * 10.0)
                        .round()
                        .clamp(0.0, f32::from(EDGE_SCROLL_MAX))
                        as u8;
                }
                continue;
            }
            if let (Some(action), Some(key)) = (
                Action::from_name(name.trim()),
                egui::Key::from_name(key.trim()),
            ) {
                if old_carry && action == Action::Carry && key == egui::Key::G {
                    continue;
                }
                if old_speed && action == Action::Speed1 && key == Action::Item1.default_key() {
                    continue;
                }
                if old_medkit && action == Action::Carry && key == Action::Medkit.default_key() {
                    continue;
                }
                if old_ultimate
                    && matches!(
                        (action, key),
                        (Action::Ability4, egui::Key::R)
                            | (Action::Revive, egui::Key::G)
                            | (Action::Reload, egui::Key::T)
                    )
                {
                    continue;
                }
                if old_space
                    && matches!(
                        (action, key),
                        (Action::Detonate, egui::Key::Space) | (Action::Ability4, egui::Key::G)
                    )
                {
                    continue;
                }
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

/// Whether `key` went down among `events`, repeats and all — what
/// `egui::InputState::key_pressed` asks, over a list a test can make.
fn key_went_down(events: &[egui::Event], key: egui::Key) -> bool {
    events
        .iter()
        .any(|e| matches!(e, egui::Event::Key { key: k, pressed: true, .. } if *k == key))
}

/// Whether `key` is a letter, A to Z.
fn is_letter(key: egui::Key) -> bool {
    matches!(key.name().as_bytes(), [c] if c.is_ascii_alphabetic())
}

/// Whether the game's keys are read under `modifiers`: with none held,
/// or Shift alone — the sprint (task 150), under which an ability, an
/// item or any other key does what it does without Shift. Ctrl and an
/// ability's key is its rank-up, and Alt is the dodge roll.
pub fn plain_or_sprinting(modifiers: egui::Modifiers) -> bool {
    !(modifiers.ctrl || modifiers.alt || modifiers.command || modifiers.mac_cmd)
}

/// Where the bindings are kept: `bims/keys` under the config directory.
fn path() -> Option<std::path::PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => std::path::PathBuf::from(dir),
        _ => std::path::PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    Some(base.join("bims").join("keys"))
}

/// Shift held (task 150): the Bim you steer sprints while the keys walk
/// it. Shift, Alt and Ctrl are modifiers, which egui never hands over as
/// an [`egui::Key`], so none of the three is an [`Action`] to rebind:
/// they are read here and nowhere else, and the Controls page lists them
/// with the pointer.
pub fn sprint_held(input: &egui::InputState) -> bool {
    input.modifiers.shift
}

/// Alt held: the dodge roll goes on its way down (task 150).
pub fn dodge_held(input: &egui::InputState) -> bool {
    input.modifiers.alt
}

/// Ctrl held round a left click: a ping (Alt until task 150 took it for
/// the roll).
pub fn ping_held(input: &egui::InputState) -> bool {
    input.modifiers.ctrl
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
        // G is the engineer's remote trigger (task 154; Space, where the
        // pause was, until October 2026), and nothing else's.
        assert_eq!(keys.key(Action::Detonate), egui::Key::G);
        assert!(keys.shared_with(Action::Detonate).is_empty());
        // The four ability slots are Q, C, E and Space (task 123; the
        // ultimate left R for G and G for Space in October 2026), none
        // bound to anything else. R is the reload, shared with Turn,
        // which only the yard and the armoury read.
        assert_eq!(keys.key(Action::Ability1), egui::Key::Q);
        assert_eq!(keys.key(Action::Ability2), egui::Key::C);
        assert_eq!(keys.key(Action::Ability3), egui::Key::E);
        assert_eq!(keys.key(Action::Ability4), egui::Key::Space);
        assert!(keys.shared_with(Action::Ability1).is_empty());
        assert!(keys.shared_with(Action::Ability2).is_empty());
        assert!(keys.shared_with(Action::Ability3).is_empty());
        assert!(keys.shared_with(Action::Ability4).is_empty());
        assert_eq!(keys.key(Action::Reload), egui::Key::R);
        assert_eq!(keys.shared_with(Action::Reload), vec![Action::Turn]);
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
        // And the carry (feature 86): B since the held revive took G and
        // the medkit H, each bound to nothing else. The revive is on T
        // since the ultimate took G (October 2026).
        assert_eq!(keys.key(Action::Carry), egui::Key::B);
        assert!(keys.shared_with(Action::Carry).is_empty());
        assert_eq!(keys.key(Action::Revive), egui::Key::T);
        assert!(keys.shared_with(Action::Revive).is_empty());
        let mut changed = keys;
        changed.set(Action::Inventory, egui::Key::I);
        changed.set(Action::PanUp, egui::Key::ArrowUp);
        let text = changed.to_text();
        // A line an action, the edge-scroll speed's and the network
        // buffer's.
        assert_eq!(text.lines().count(), Action::ALL.len() + 2);
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
        assert_eq!(keys.key(Action::Ability4), egui::Key::Space);
        let text = keys.to_text();
        assert!(text.contains("ability-1=G\n") && text.contains("ability-3=H\n"));
        assert!(!text.contains("class-"));
        assert_eq!(Keys::from_text(&text), keys);
    }

    /// The held revive is T (G until the ultimate took it), the medkit H
    /// and the carry B; a file from before, with the carry on G and no
    /// revive line, or on H and no medkit line, is read with the carry
    /// on its own key rather than sharing, and one that says both keeps
    /// its word.
    #[test]
    fn the_revive_is_t_the_medkit_h_and_an_old_carry_moves_to_b() {
        let keys = Keys::default();
        assert_eq!(keys.key(Action::Revive), egui::Key::T);
        assert_eq!(keys.key(Action::Medkit), egui::Key::H);
        assert_eq!(keys.key(Action::Carry), egui::Key::B);
        assert!(keys.shared_with(Action::Revive).is_empty());
        assert!(keys.shared_with(Action::Carry).is_empty());
        assert!(keys.shared_with(Action::Medkit).is_empty());
        let old = Keys::from_text("carry=G\n");
        assert_eq!(old.key(Action::Carry), egui::Key::B);
        assert_eq!(old.key(Action::Revive), egui::Key::T);
        let old = Keys::from_text("revive=G\ncarry=H\n");
        assert_eq!(old.key(Action::Carry), egui::Key::B);
        assert_eq!(old.key(Action::Medkit), egui::Key::H);
        let both = Keys::from_text("carry=G\nrevive=J\n");
        assert_eq!(both.key(Action::Carry), egui::Key::G);
        assert_eq!(both.key(Action::Revive), egui::Key::J);
        assert_eq!(Keys::from_text(&keys.to_text()), keys);
    }

    /// The reload is R and the revive T (October 2026, when the ultimate
    /// left R); a file written before — the ultimate on R, the revive on
    /// G, the reload on T or not named, the trigger on Space — is read
    /// with them all on their new keys, a key the player had moved off
    /// them kept, and a file written since keeps its word, even an
    /// ultimate put back on R.
    #[test]
    fn the_reload_is_r_and_an_old_file_follows() {
        let keys = Keys::default();
        assert_eq!(keys.key(Action::Reload), egui::Key::R);
        assert_eq!(keys.key(Action::Revive), egui::Key::T);
        assert!(keys.shared_with(Action::Ability4).is_empty());
        assert!(keys.shared_with(Action::Revive).is_empty());
        let mut before = keys;
        before.set(Action::Ability4, egui::Key::R);
        before.set(Action::Revive, egui::Key::G);
        before.set(Action::Reload, egui::Key::T);
        before.set(Action::Detonate, egui::Key::Space);
        before.set(Action::Inventory, egui::Key::I);
        let old = Keys::from_text(&before.to_text());
        let mut want = keys;
        want.set(Action::Inventory, egui::Key::I);
        assert_eq!(old, want);
        let older: String = before
            .to_text()
            .lines()
            .filter(|l| !l.starts_with("reload="))
            .map(|l| format!("{l}\n"))
            .collect();
        assert_eq!(Keys::from_text(&older), want);
        let moved = Keys::from_text("ability-4=Z\nrevive=J\nreload=T\n");
        assert_eq!(moved.key(Action::Ability4), egui::Key::Z);
        assert_eq!(moved.key(Action::Revive), egui::Key::J);
        assert_eq!(moved.key(Action::Reload), egui::Key::R);
        let mut since = keys;
        since.set(Action::Ability4, egui::Key::R);
        since.set(Action::Reload, egui::Key::J);
        since.set(Action::Revive, egui::Key::G);
        assert_eq!(Keys::from_text(&since.to_text()), since);
    }

    /// The ultimate is Space and the remote trigger G (October 2026); a
    /// file written while the ultimate was G and the trigger Space is
    /// read with the two swapped, a key the player had moved off them
    /// kept, and a file written since keeps its word.
    #[test]
    fn the_ultimate_is_space_the_trigger_g_and_an_old_file_follows() {
        let keys = Keys::default();
        assert_eq!(keys.key(Action::Ability4), egui::Key::Space);
        assert_eq!(keys.key(Action::Detonate), egui::Key::G);
        let mut before = keys;
        before.set(Action::Ability4, egui::Key::G);
        before.set(Action::Detonate, egui::Key::Space);
        before.set(Action::Inventory, egui::Key::I);
        let mut want = keys;
        want.set(Action::Inventory, egui::Key::I);
        assert_eq!(Keys::from_text(&before.to_text()), want);
        let moved = Keys::from_text("ability-4=Z\ndetonate=Space\nreload=R\n");
        assert_eq!(moved.key(Action::Ability4), egui::Key::Z);
        assert_eq!(moved.key(Action::Detonate), egui::Key::G);
        let mut since = keys;
        since.set(Action::Ability4, egui::Key::G);
        since.set(Action::Detonate, egui::Key::J);
        assert_eq!(Keys::from_text(&since.to_text()), since);
    }

    /// The items are 1 to 4 (October 2026, where the quickselect was) and
    /// the 1× speed left 1; a file from before, with the speed on 1 and
    /// no item line, is read with the speed on its new key, and one that
    /// says both keeps its word.
    #[test]
    fn the_items_are_one_to_four_and_an_old_speed_on_one_moves() {
        let keys = Keys::default();
        for (action, key) in Action::ITEMS.into_iter().zip([
            egui::Key::Num1,
            egui::Key::Num2,
            egui::Key::Num3,
            egui::Key::Num4,
        ]) {
            assert_eq!(keys.key(action), key);
            assert!(keys.shared_with(action).is_empty());
        }
        assert!(keys.shared_with(Action::Speed1).is_empty());
        let old = Keys::from_text("speed-1=1\n");
        assert_eq!(old.key(Action::Speed1), Action::Speed1.default_key());
        assert_eq!(old.key(Action::Item1), egui::Key::Num1);
        let both = Keys::from_text("speed-1=1\nitem-1=5\n");
        assert_eq!(both.key(Action::Speed1), egui::Key::Num1);
        assert_eq!(both.key(Action::Item1), egui::Key::Num5);
        assert_eq!(Keys::from_text(&keys.to_text()), keys);
    }

    /// Sprinting (Shift held, task 150) an ability's key is still the
    /// ability and an item's still the item — Shift+1 comes over as `!`
    /// from the key where 1 is — while Ctrl and Alt still keep the rows
    /// out. A letter is never read by where it sits: QWERTZ's Y, on
    /// QWERTY's Z, is no Z.
    #[test]
    fn sprinting_an_ability_or_an_item_is_still_used() {
        let keys = Keys::default();
        let shift = egui::Modifiers::SHIFT;
        assert!(plain_or_sprinting(egui::Modifiers::NONE));
        assert!(plain_or_sprinting(shift));
        assert!(!plain_or_sprinting(egui::Modifiers::CTRL));
        assert!(!plain_or_sprinting(egui::Modifiers::ALT));
        let q = [down(egui::Key::Q, shift)];
        assert!(keys.used(&q, shift, Action::Ability1), "Shift+Q is Q");
        assert_eq!(keys.rank_up_asked(&q, shift), None);
        let bang = |modifiers| egui::Event::Key {
            key: egui::Key::Exclamationmark,
            physical_key: Some(egui::Key::Num1),
            pressed: true,
            repeat: false,
            modifiers,
        };
        assert!(keys.pressed_through_shift(&[bang(shift)], Action::Item1));
        assert!(!keys.pressed_through_shift(&[bang(shift)], Action::Item2));
        assert!(!keys.pressed_through_shift(&[bang(egui::Modifiers::NONE)], Action::Item1));
        assert!(keys.pressed_through_shift(&[down(egui::Key::Num1, shift)], Action::Item1));
        let mut zed = Keys::default();
        zed.set(Action::Item1, egui::Key::Z);
        let y = egui::Event::Key {
            key: egui::Key::Y,
            physical_key: Some(egui::Key::Z),
            pressed: true,
            repeat: false,
            modifiers: shift,
        };
        assert!(!zed.pressed_through_shift(&[y], Action::Item1));
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
        let q = [down(egui::Key::Q, none)];
        assert!(keys.used(&q, none, Action::Ability1));
        assert_eq!(keys.rank_up_asked(&q, none), None);
        let e = [down(egui::Key::E, ctrl)];
        assert_eq!(keys.rank_up_asked(&e, ctrl), Some(Action::Ability3));
        let space = [down(egui::Key::Space, ctrl)];
        assert_eq!(keys.rank_up_asked(&space, ctrl), Some(Action::Ability4));
        // R is the reload now and G the remote trigger, no slot's:
        // Ctrl and either ranks nothing up.
        let r = [down(egui::Key::R, ctrl)];
        assert_eq!(keys.rank_up_asked(&r, ctrl), None);
        let g = [down(egui::Key::G, ctrl)];
        assert_eq!(keys.rank_up_asked(&g, ctrl), None);
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

    /// And the network buffer's cap (task 148), in hundredths of a
    /// second, the same way.
    #[test]
    fn the_network_buffer_is_kept_beside_the_keys() {
        let keys = Keys::default();
        assert!(keys.to_text().contains("network-buffer=0.40\n"));
        let mut off = keys;
        off.net_buffer = 0;
        let text = off.to_text();
        assert!(text.contains("network-buffer=0.00\n"), "{text}");
        assert_eq!(Keys::from_text(&text), off);
        assert_eq!(
            Keys::from_text("network-buffer=7\n").net_buffer,
            NET_BUFFER_MAX
        );
        assert_eq!(
            Keys::from_text("network-buffer=lots\n").net_buffer,
            NET_BUFFER_DEFAULT
        );
        assert_eq!(Keys::from_text("map=M\n").net_buffer, NET_BUFFER_DEFAULT);
    }
}
