//! The crew's panels: everything on the game screen that is about the Bims
//! rather than about the deck they are standing on — the room aboard,
//! stepped by the world.
//!
//! The selected crew member's health and sheet, the tray with its Stash
//! and Squad (feature 107), the character sheet, the fixture menus, and
//! the tooltips everything hangs off. What is *not* here is the canvas and the
//! pointer, because those are the screen's own — the ship turns the deck
//! with the hull — and the screen hands the room its coordinates.
//!
//! # Tooltips are asked for, never stumbled into
//!
//! Every tooltip hangs off an affordance: a word underlined with
//! [`theme::asks`], or a `?` from [`theme::question_mark`]. Never a row, a
//! bar or a panel — a player crossing the health panel on the way to the
//! deck should get nothing.
//!
//! # A highlight is not a tooltip
//!
//! Resting on a work row rings the fixture it names on the deck.
//! Nothing pops up, nothing is said, and the ring is gone the moment the
//! pointer moves — [`CrewPanels::points`] is the mechanism, and it hangs off
//! a whole row on purpose, because every row is about exactly one place.
//! The ring is worked out afresh every frame from what is hovered, so a
//! panel that folds away under the pointer cannot leave a fixture lit.
//!
//! # Gear moves by order, and the hold is handed in
//!
//! The pack, the worn slots and the container windows are grids
//! (`crate::grid`), and what a click on a cell asks for — put this on, put
//! it away, take that out — is a [`GearOrder`] left on
//! [`CrewPanels::orders`] for the screen to send through the seam as a
//! `world::Command`, since the hold is the world's and every player's ship
//! has to agree about what is in it. (A `Hold` snapshot came the other
//! way, until the last thing it said — a relic cache on a desk — went in
//! October 2026.)
//!
//! A body is handed in the same way. The Loot window is a grid over what
//! a dead or unconscious crewmate has on it, and what it shows, whether the
//! Bim is still down and whether the looter is within reach come as a
//! [`Body`] snapshot the screen sets every frame *after* the fixture menu
//! has run, since the menu is what opens the window. Taking is an order
//! like the rest, `GearOrder::Loot`, and the walk over to the body is the
//! screen's too, since where a body lies is the world's to say.

use bevy_egui::egui;
use bims::combat::{Item as PackItem, Piece};
use bims::game::Game;
use bims::order::CrewOrder;
use bims::room::*;
use bims::{door, health};
use physics::ResourceId;

use crate::ability_icons;
use crate::format::date_text;
use crate::icons;
use crate::keys::{Action, Keys};
use crate::names::*;
use crate::theme;

/// Below this the pointer moved so little that it counts as a click, not a
/// sweep, in points.
pub const CLICK_SLOP: f32 = 4.0;

/// What a press on the Armory panel or a row asked for, about somebody's
/// gear. The screen sends it as the matching `world::Command`.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum GearOrder {
    /// Put a thing — out of the armory, or off another Bim's slot — on
    /// `who`, what was there into the armory (task 113).
    Equip { who: u32, from: world::GearSource },
    /// [`GearOrder::Equip`] onto one slot named — an item dragged onto one
    /// of the six item boxes (October 2026).
    EquipAt {
        who: u32,
        from: world::GearSource,
        at: world::GearSlot,
    },
    /// Take what `who` has on `part` off into the armory.
    Unequip { who: u32, part: world::GearSlot },
    /// Offer what the player's own Bim has on `part` to player `to`.
    Offer { part: world::GearSlot, to: u32 },
    /// Answer an offer player `from` made of its `part`: take it or not;
    /// the offerer's own no takes it back.
    AnswerOffer {
        from: u32,
        part: world::GearSlot,
        yes: bool,
    },
}

/// Something within reach of the Bim shown, for the nearby strip: the
/// window it opens and what to call it.
#[derive(Clone, PartialEq, Debug)]
pub struct Near {
    pub open: Open,
    pub label: String,
}

/// What a row or the nearby strip put up: one of the acts below, which
/// are no window of the panels' own.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Open {
    /// Not a window either: the engineer.s deployable within reach packed
    /// up into its pack (`Command::PackUp`) — the row on the nearby strip
    /// beside one (feature 74). By the deployable.s id. There is no
    /// refill any more: a sentry never runs out of shots (feature 88).
    PackUp(u32),
    /// Not a window either: the menu's Carry row on a downed crewmate —
    /// the screen walks the player's own Bim over and picks them up once
    /// within reach (`CrewPanels::carry_requested`). By the patient's
    /// crew index.
    Carry(u32),
}

/// What the class section and the deployable rows asked for this frame
/// (feature 74), for the screen to send through the seam.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DeployOrder {
    PackUp(u32),
    SetClass(world::Class),
    /// A rank of the ranked kit bought off the Skills tab (task 124).
    RankUp {
        ability_slot: u32,
    },
}

/// A player's class as the panel shows it, a snapshot the screen hands
/// over every frame off the world: what it is, how far along, the ranks
/// bought, and whether the class may still be changed (before the first
/// undock).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ClassView {
    pub class: world::Class,
    pub level: u8,
    pub to_next: u32,
    /// The experience itself, whole (feature 107): what the character
    /// sheet and the hero panel read the level's progress off.
    pub xp: u32,
    pub can_change: bool,
    /// The ranks bought of the class's four abilities, Q C E R (task
    /// 124; every class since task 139): what the Skills tab draws.
    pub ranks: [u8; 4],
    /// Skill points not spent on those ranks.
    pub points: u8,
    /// The soldier's rows (feature 75): grenade charges in the pack,
    /// seconds of the clock until the next comes back (feature 90), and
    /// its Stun Shot — `None` for anybody but a soldier.
    pub soldier: Option<SoldierView>,
    /// The medic's rows (feature 76) — `None` for anybody but a medic.
    pub medic: Option<MedicView>,
    /// The tank's rows (feature 77) — `None` for anybody but a tank.
    pub tank: Option<TankView>,
    /// The commander's rows (feature 78) — `None` for anybody else.
    pub commander: Option<CommanderView>,
    /// The crew's relics (feature 106), in the order they took them: what
    /// the sheet lists under the gear.
    pub relics: Vec<world::Relic>,
    /// The classes the host's profile opened for the run, for the sheet's
    /// chooser; empty is every class.
    pub open_classes: Vec<world::Class>,
}

/// What the panel says of a soldier (feature 75).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct SoldierView {
    pub grenades: u32,
    pub cooldown: f64,
    /// A Stun Shot charging (October 2026).
    pub charging: bool,
    /// Seconds until the Stun Shot may be charged again.
    pub shot_cooldown: f64,
}

/// What the panel says of a medic (feature 76; task 153): who the beam
/// holds, by name; whether the Heal Drone is learnt, the seconds until
/// the next and the seconds the one up has left; and whether the Healing
/// Circle is learnt and on.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct MedicView {
    pub patients: Vec<String>,
    pub drone_learnt: bool,
    pub drone_cooldown: f64,
    pub drone_left: f64,
    pub circle_learnt: bool,
    pub circling: bool,
}

/// What the panel says of a tank (task 155): the Riot Shield — up, its
/// hit points left and whole (nought before a rank), the seconds a broken
/// one waits — the Reflect Barrier's seconds left and to the next, and
/// the Bastion's seconds to the next, each with whether a rank is bought.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct TankView {
    pub shield_up: bool,
    pub shield_left: f32,
    pub shield_whole: f32,
    pub shield_cooldown: f64,
    pub reflect_left: f64,
    pub reflect_cooldown: f64,
    pub reflect_learnt: bool,
    pub bastion_cooldown: f64,
    pub bastion_learnt: bool,
}

/// What the panel says of a commander (feature 78): minutes of the rally
/// left, seconds until he may rally again, and whether he has a rank of
/// it.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct CommanderView {
    pub rally_left: f64,
    pub cooldown: f64,
    pub can_rally: bool,
    /// His Battle Cry (task 129): seconds left, seconds to wait, and
    /// whether he has a rank of it.
    pub cry_left: f64,
    pub cry_cooldown: f64,
    pub can_cry: bool,
}

/// The tray's panel, when it is up (feature 107). The tray is its row of
/// buttons and nothing else until this is pressed; its Armory, Map and
/// Trade buttons are windows, not panels here (task 113 put the Armory
/// panel where the Stash was).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrayTab {
    /// The bots, and the orders that move them.
    Squad,
}

/// What the tray is handed each frame that the room alone does not know.
pub struct TrayView {
    /// The player's own Bim is out: the tray is its Map button alone.
    pub out: bool,
    /// Every bot, as the portraits show them, for the Squad panel.
    pub bots: Vec<crate::screens::hud::Portrait>,
    /// The player's standing order to the crew — `world::Standing::code`.
    pub standing: u32,
}

/// What a press in the tray asked the screen for: a window of its own, or
/// an order it sends the way the key would have.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrayAsk {
    /// The world map, as M.
    Map,
    /// An attack banner, as X: the pointer armed, or a banner taken up.
    Attack,
    /// The crew called back to the ship, as T.
    Retreat,
    /// Whatever the crew are under let go, so they follow again.
    Follow,
}

/// How wide [`CrewPanels::side`] draws itself once somebody is picked:
/// the health bar, its number and the armour's, in one row. The width is the panel's own rather than each
/// screen's, so the two agree and so a screen can leave the strip it
/// anchors in the right size — the game's is [`SIDE_W`] plus its
/// margins. With nobody picked there is no panel at all (feature 107).
pub const SIDE_W: f32 = 340.0;

/// The side panel's health bar. Wide, because the health block round it
/// is, and the one bar a player reads in a fight.
const BAR_W: f32 = 140.0;
/// The health points beside the bar: the one number on the panel a
/// player reads in a fight, so it is the one number bigger than the
/// type round it.
const HEALTH_NUMBER: f32 = 16.0;
/// The downed block's headline and the dead's line: bigger than the
/// panel's small type, since it is the one thing on it read in a fight.
const PERIL_NAME: f32 = 15.0;
/// The countdown under it — the line the whole block exists for.
const PERIL_LEFT: f32 = 14.0;

/// The frame round the downed block: the panel's own, filled and edged
/// in the countdown ring's red. Nothing else on this panel is framed,
/// which is the point.
fn downed_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(56, 14, 12, 220))
        .stroke(egui::Stroke::new(1.0, theme::DYING))
        .corner_radius(4.0)
        .inner_margin(6.0)
}

/// Whether `w`'s health is drawn in the red rather than the green: dead,
/// down, or under [`health::BLEEDS_UNDER`] and bleeding on the deck —
/// the side panel's own rule, for the portraits and the hero panel to
/// read the same way (feature 107, task 120).
pub fn is_hurt(game: &Game, w: usize) -> bool {
    !game.is_alive(w) || game.is_downed(w) || game.health(w) < health::BLEEDS_UNDER
}

/// What is wrong with `w` in one line, for the hero panel (feature 107):
/// the countdown while it is down, in the ring's red. `None` while it is
/// up (a revive leaves no slow since October 2026), and for the dead. There is nothing else to say
/// since task 120: one bar, and the bar says the rest.
pub fn peril_summary(game: &Game, w: usize) -> Option<(String, egui::Color32)> {
    if !game.is_alive(w) {
        return None;
    }
    game.down_left(w)
        .map(|left| (downed_short(left), theme::DYING))
}

/// How wide the character sheet is (feature 107): the Skills tab across
/// it, and everything else in a column the same width, on the left of
/// the canvas where it has to leave the hero panel room at 1280 wide.
pub const SHEET_W: f32 = 330.0;
/// What the sheet's head takes before its scrolling body: the title, the
/// experience bar and its line, and the frame round them.
const SHEET_HEAD: f32 = 74.0;

/// The tray's panels (feature 107): as wide as the tray may grow before
/// the hero panel beside it would be pushed into the bottom right at
/// 1280 wide, and how tall its list may grow before it scrolls.
const TRAY_W: f32 = 330.0;
const TRAY_PANEL_H: f32 = 230.0;
/// A tray button.
const TRAY_BUTTON_W: f32 = 62.0;
const TRAY_BUTTON_H: f32 = 32.0;
/// A bot's health bar on the Squad panel.
const SQUAD_BAR_W: f32 = 90.0;
/// A thing's cell on the Stash panel.
const STASH_CELL: f32 = 28.0;

/// The Skills tab's type: a few points over the panels' small text, since
/// four abilities and every rank's numbers are read rather than glanced
/// at. (The talent tree it once was went with the talents, task 139.)
const SKILL_TEXT: f32 = 13.5;
/// The type of an ability's words and numbers.
const SKILL_BOX_TEXT: f32 = 12.5;
/// An ability's name, and the sheet's title.
const SKILL_NAME_TEXT: f32 = 17.0;
/// The side of an ability's picture beside its name: the hero panel's
/// box, smaller.
const SKILL_ICON: f32 = 30.0;

/// An open fixture menu: which fixture, and where on the window it was
/// asked for.
pub struct Menu {
    fixture: u32,
    at: egui::Pos2,
    /// Opened this frame: the press that opened it must not shut it.
    fresh: bool,
}

/// What a menu row does when it is clicked: an order to the room, sent
/// through the seam the way the screen sends one, that walks the player's
/// Bim over to do it.
type Errand = CrewOrder;

/// One row of a fixture's menu: an errand for the Bim, or a window to
/// open — which is the panels' to do rather than the room's.
struct Item {
    label: String,
    hint: String,
    disabled: bool,
    run: Option<Errand>,
    opens: Option<Open>,
}

impl Item {
    fn note(label: &str, hint: String) -> Item {
        Item {
            label: label.into(),
            hint,
            disabled: true,
            run: None,
            opens: None,
        }
    }

    fn run(
        label: impl Into<String>,
        hint: impl Into<String>,
        disabled: bool,
        order: CrewOrder,
    ) -> Item {
        Item {
            label: label.into(),
            hint: hint.into(),
            disabled,
            run: Some(order),
            opens: None,
        }
    }
}

pub struct CrewPanels {
    /// The one the mouse steers.
    pub player: usize,
    pub crew_count: u32,
    /// The tray's panel that is up, if one is (feature 107): collapsed to
    /// its buttons by default.
    tray: Option<TrayTab>,
    /// Whether the character sheet is up (feature 107): K, the `+1` on the
    /// hero panel, or the player's own portrait twice.
    pub sheet_open: bool,
    /// The spot the pointer's row rang last frame, and the one the rows
    /// hovered this frame want.
    ringed: u32,
    wanted: u32,
    menu: Option<Menu>,
    /// The Armory panel (task 113): up from the Inventory key (Tab) or the
    /// tray's Armory button, on every screen of a run, until it is shut.
    pub armory_open: bool,
    /// What the nearby strip has up, if anything.
    open: Option<Open>,
    /// The Carry row was picked on this downed crewmate: the screen walks
    /// the player's own Bim over and sends the carry once it is within
    /// reach, and takes this.
    pub carry_requested: Option<u32>,
    /// Whether the player's own Bim may carry a body at all — a medic or
    /// a field medic (`World::can_lift`) — as the screen last
    /// handed it over. What greys the menu's Carry row.
    pub may_lift: bool,
    /// What is within reach of the Bim shown, nearest first — an
    /// engineer's deployable to pack up — with a name for the strip, as
    /// the screen last handed it over (`Near`). Fresh every frame: the Bim is walking.
    pub nearby: Vec<Near>,
    /// The player's own class, as the screen last handed it over
    /// (feature 74): drawn under the health of their own crew member.
    pub class_view: Option<ClassView>,
    /// The crew's relics (feature 106), as the screen last handed them
    /// over (`World::relics`): what the side panel shows of a crewmate
    /// picked (task 136) — every relic is the whole crew's.
    pub crew_relics: Vec<world::Relic>,
    /// How long the player's own Bim takes to revive a crewmate, in
    /// seconds, as the screen last handed it over off the world
    /// (`World::revive_seconds`, task 120): the class and a field medic's
    /// trade move it, and the menu's row says it.
    pub revive_seconds: f32,
    /// What the class section and the deployable rows asked for this
    /// frame, drained by the screen.
    pub deploy_orders: Vec<DeployOrder>,
    /// The player's keys, as the screen last handed them over.
    pub keys: Keys,
    /// What the rows and the ctrl-clicks asked for this frame, for the
    /// screen to send. Drained by it.
    pub orders: Vec<GearOrder>,
    /// What the menus and the rows asked of the room this frame
    /// — `bims::order::CrewOrder`s — for the screen to send through the
    /// seam. Nothing in here reaches into the room itself, since the
    /// crew's positions are every player's to agree on (feature 59).
    pub crew_orders: Vec<CrewOrder>,
    /// The same, given with Shift held: to wait their turn behind what
    /// the crew member is on (feature 69) — `Game::order_later`,
    /// `Order::CrewLater` on the ship. Drained beside `crew_orders`.
    pub later_orders: Vec<CrewOrder>,
}

impl CrewPanels {
    pub fn new(player: usize, crew_count: u32) -> CrewPanels {
        CrewPanels {
            player,
            crew_count,
            tray: crate::dev::tray(),
            sheet_open: crate::dev::sheet(),
            ringed: SPOT_NOTHING,
            wanted: SPOT_NOTHING,
            menu: None,
            armory_open: crate::dev::armory(),
            open: None,
            carry_requested: None,
            may_lift: false,
            nearby: Vec::new(),
            class_view: None,
            crew_relics: Vec::new(),
            revive_seconds: bims::health::REVIVE_SECONDS,
            deploy_orders: Vec::new(),
            keys: Keys::default(),
            orders: Vec::new(),
            crew_orders: Vec::new(),
            later_orders: Vec::new(),
        }
    }

    /// The room's crew has changed size — a ship docking brings a station's
    /// residents into its room, and leaving takes them out again.
    pub fn rebuild_crew(&mut self, count: u32) {
        if count == self.crew_count {
            return;
        }
        self.crew_count = count;
        self.menu = None;
        // The room was rebuilt with it, and the station's people with it.
        self.open = None;
    }

    // --- pointing at the thing itself ---------------------------------------

    /// Call once a frame, before any panel: nothing wants a ring yet.
    pub fn begin_frame(&mut self) {
        self.wanted = SPOT_NOTHING;
    }

    /// Call once a frame, after every panel: the ring follows the pointer.
    pub fn end_frame(&mut self, game: &mut Game) {
        if self.wanted != self.ringed {
            self.ringed = self.wanted;
            game.set_highlight(self.ringed);
        }
    }

    // --- fixture menus ------------------------------------------------------

    /// A click landed on a fixture: its menu opens at the pointer. Nothing
    /// is a container since task 113: the armoury, the lockers and the
    /// shelves stand on the deck as pictures, and what the crew own is the
    /// Armory panel.
    pub fn open_menu(&mut self, fixture: u32, at: egui::Pos2, _game: &mut Game) {
        self.menu = Some(Menu {
            fixture,
            at,
            fresh: true,
        });
    }

    /// Shut a fixture's menu, the way a click away does.
    pub fn close_menu(&mut self) {
        self.menu = None;
    }

    /// Escape: the innermost thing up goes first — a fixture's menu, then
    /// the nearby strip's, then the Armory panel, then the character sheet.
    /// `true` when something was shut, so the screen knows the key is
    /// spent.
    pub fn escape(&mut self) -> bool {
        if self.menu.is_some() {
            self.menu = None;
        } else if self.open.is_some() {
            self.open = None;
        } else if self.armory_open {
            self.armory_open = false;
        } else if self.sheet_open {
            self.sheet_open = false;
        } else {
            return false;
        }
        true
    }

    /// Build the rows for the fixture that was clicked. Nothing here is
    /// remote: every item walks the Bim over to do it by hand. Nor does
    /// anything wait for the Bim to be free — a new errand takes over, and
    /// what it displaced goes on the agenda to be finished afterwards.
    /// Every menu acts on the player's Bim; the other crew take no orders.
    fn items(&self, fixture: u32, game: &Game, name: &dyn Fn(u32) -> String) -> Vec<Item> {
        let who = self.player;
        let busy = game.is_busy(who);
        let takes_over: Option<&str> = busy.then_some("takes over — the rest waits its turn");
        let mut items = Vec::new();
        match fixture {
            HIT_SHIP_DOOR => {
                // A powered door in a bulkhead: "Open" holds it open,
                // "Close" hands it back to itself, and a locked door is a
                // wall until it is unlocked.
                let door = game.hit_door();
                let held = game.ship_door_is_held(door);
                let locked = game.ship_door_is_locked(door);
                items.push(Item::run(
                    if held { "Close door" } else { "Hold door open" },
                    if locked {
                        "unlock it first".to_string()
                    } else {
                        takes_over
                            .unwrap_or(if held {
                                "and let it shut behind people again"
                            } else {
                                "so it stops shutting itself"
                            })
                            .into()
                    },
                    locked,
                    CrewOrder::Door {
                        who: who as u32,
                        door: door as u32,
                        order: if held {
                            door::Order::Close
                        } else {
                            door::Order::Open
                        },
                    },
                ));
                // Locking is no longer anybody's (task 138): the row is
                // there for a body whose skill may lock, and Unlock for
                // everybody, so no door stays shut against the crew.
                if locked || game.may_lock_doors(who) {
                    items.push(Item::run(
                        if locked { "Unlock" } else { "Lock" },
                        takes_over
                            .unwrap_or(if locked {
                                "at the panel"
                            } else {
                                "shuts it, and nobody gets through"
                            })
                            .to_string(),
                        false,
                        CrewOrder::Door {
                            who: who as u32,
                            door: door as u32,
                            order: if locked {
                                door::Order::Unlock
                            } else {
                                door::Order::Lock
                            },
                        },
                    ));
                }
            }
            HIT_BIM => {
                // A body on the deck — the player's own, or a crewmate
                // (task 120): two rows, *Get up* and *Carry*. The hands are always
                // the player's, and the row is there whether it can be
                // done or not, greyed with the reason when it cannot — a
                // body on its feet, the player's own, one somebody else is
                // already bringing round — so a click on a body always
                // says what it would take.
                let patient = game.hit_bim();
                let why = revive_refused(game, who, patient, name);
                let hint = match &why {
                    Some(why) => why.clone(),
                    None => takes_over
                        .map(str::to_string)
                        .unwrap_or_else(|| revive_hint(self.revive_seconds)),
                };
                items.push(Item::run(
                    REVIVE_ROW,
                    hint,
                    why.is_some(),
                    CrewOrder::Revive {
                        who: who as u32,
                        patient: patient as u32,
                    },
                ));
                // And the carry: the walk over and the body taken up out
                // of the fire, for a medic of either kind — greyed with
                // the reason for anybody else, so the choice is always
                // the two rows.
                let why = carry_refused_row(game, who, patient, self.may_lift);
                items.push(Item {
                    label: CARRY_ROW.into(),
                    hint: why.clone().unwrap_or_else(|| CARRY_ROW_HINT.into()),
                    disabled: why.is_some(),
                    run: None,
                    opens: Some(Open::Carry(patient as u32)),
                });
            }
            _ => {}
        }
        // While there is something to wait behind, a word about Shift: a
        // row given with it waits its turn (feature 69). Only under rows
        // that are errands.
        let something_on = busy || game.agenda_len(who) > 0 || game.is_walking(who);
        if something_on && items.iter().any(|item| item.run.is_some()) {
            items.push(Item::note(SHIFT_LATER, SHIFT_LATER_HINT.into()));
        }
        items
    }

    /// Draw the open menu, if there is one, at the cursor it was opened
    /// at, and shut it on a click away.
    pub fn menu(&mut self, ctx: &egui::Context, game: &mut Game, name: &dyn Fn(u32) -> String) {
        let Some(menu) = &self.menu else { return };
        if !game.is_alive(self.player) {
            self.menu = None;
            return;
        }
        let items = self.items(menu.fixture, game, name);
        if items.is_empty() {
            self.menu = None;
            return;
        }
        let at = menu.at;
        let fresh = menu.fresh;
        let rows: Vec<theme::Row> = items
            .iter()
            .map(|item| theme::Row::new(item.label.clone(), item.hint.clone(), item.disabled))
            .collect();
        let (chosen, rect) = theme::popup(ctx, "fixture-menu", at, &rows);
        // Shift, as the row was clicked: the errand waits its turn rather
        // than taking over (feature 69). Asked of the context outside any
        // input closure of its own.
        let later = ctx.input(|i| i.modifiers.shift);
        if let Some(i) = chosen {
            let item = items.into_iter().nth(i).unwrap();
            self.menu = None;
            match item.opens {
                Some(open @ Open::PackUp(_)) => self.show(open),
                Some(Open::Carry(patient)) => self.carry_requested = Some(patient),
                None => {
                    if let Some(order) = item.run {
                        if later {
                            self.later_orders.push(order);
                        } else {
                            self.crew_orders.push(order);
                        }
                    }
                }
            }
            return;
        }
        // Clicking away dismisses it — but not the press that opened it.
        let pressed = ctx.input(|i| i.pointer.any_pressed());
        let pos = ctx.input(|i| i.pointer.interact_pos());
        if let Some(menu) = &mut self.menu {
            if fresh {
                menu.fresh = false;
            } else if pressed && !pos.is_some_and(|p| rect.contains(p)) {
                self.menu = None;
            }
        }
    }

    // --- what the crew want -------------------------------------------------

    /// The header that says whose panel this is.
    fn who_header(&self, ui: &mut egui::Ui, who: u32, name: &dyn Fn(u32) -> String) {
        ui.horizontal(|ui| {
            let yours = who as usize == self.player;
            ui.label(egui::RichText::new(name(who)).strong().color(if yours {
                theme::YOURS
            } else {
                theme::INK
            }));
            ui.label(
                egui::RichText::new(if yours { "yours" } else { "her own" })
                    .small()
                    .color(theme::MUTED),
            );
        });
    }

    /// The class section under the health: the class and the level, the
    /// class's own rows, and — until the first undock — the class picker.
    fn class_section(&mut self, ui: &mut egui::Ui, view: &ClassView) {
        ui.add_space(4.0);
        if view.can_change {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(BIM_CLASS).color(theme::MUTED));
                for class in world::Class::ALL
                    .into_iter()
                    .filter(|c| view.open_classes.is_empty() || view.open_classes.contains(c))
                {
                    if theme::toggle(ui, class == view.class, class_name(class)).clicked()
                        && class != view.class
                    {
                        self.deploy_orders.push(DeployOrder::SetClass(class));
                    }
                }
                theme::question_mark(ui, BIM_CLASS_NOTE);
            });
        }
        if view.class == world::Class::None {
            if !view.can_change {
                ui.label(
                    egui::RichText::new(class_name(view.class))
                        .small()
                        .color(theme::MUTED),
                );
            }
            return;
        }
        ui.label(
            egui::RichText::new(class_line(view.class, view.level, view.to_next))
                .small()
                .color(theme::INK),
        );
        if let Some(soldier) = view.soldier {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(grenades_line(soldier.grenades, soldier.cooldown))
                        .small()
                        .color(theme::INK),
                );
                let (word, color) = if soldier.charging {
                    (CHARGING.to_string(), theme::CAUTION)
                } else {
                    (stun_shot_ready(soldier.shot_cooldown), theme::MUTED)
                };
                ui.label(egui::RichText::new(word).small().color(color));
                theme::question_mark(ui, CHARGING_TIP);
            });
        }
        if let Some(medic) = &view.medic {
            ui.horizontal(|ui| {
                let (words, color) = if medic.patients.is_empty() {
                    (BEAM_OFF.to_string(), theme::MUTED)
                } else {
                    (beam_line(&medic.patients), theme::YOURS)
                };
                ui.label(egui::RichText::new(words).small().color(color));
                theme::question_mark(ui, BEAM_TIP);
            });
            // The Heal Drone and the Healing Circle (task 153).
            let drone_ready = medic.drone_learnt && medic.drone_cooldown <= 0.0;
            ui.label(
                egui::RichText::new(crate::names::heal_drone_line(
                    medic.drone_left,
                    medic.drone_cooldown,
                    medic.drone_learnt,
                ))
                .small()
                .color(if medic.drone_left > 0.0 {
                    theme::YOURS
                } else if drone_ready {
                    theme::CAUTION
                } else {
                    theme::MUTED
                }),
            );
            ui.label(
                egui::RichText::new(crate::names::healing_circle_line(
                    medic.circling,
                    medic.circle_learnt,
                ))
                .small()
                .color(if medic.circling {
                    theme::YOURS
                } else if medic.circle_learnt {
                    theme::CAUTION
                } else {
                    theme::MUTED
                }),
            );
        }
        if let Some(tank) = view.tank {
            ui.horizontal(|ui| {
                let word = crate::names::riot_shield_line(
                    tank.shield_up,
                    tank.shield_left,
                    tank.shield_whole,
                    tank.shield_cooldown,
                );
                let color = if tank.shield_cooldown > 0.0 {
                    theme::WARN
                } else if tank.shield_up {
                    theme::CAUTION
                } else {
                    theme::MUTED
                };
                ui.label(egui::RichText::new(word).small().color(color));
                theme::question_mark(ui, crate::names::RIOT_SHIELD_TIP);
            });
            let reflecting = tank.reflect_left > 0.0;
            ui.label(
                egui::RichText::new(crate::names::reflect_line(
                    tank.reflect_left,
                    tank.reflect_cooldown,
                    tank.reflect_learnt,
                ))
                .small()
                .color(if reflecting {
                    theme::WARN
                } else {
                    theme::MUTED
                }),
            );
            ui.label(
                egui::RichText::new(crate::names::bastion_line(
                    tank.bastion_cooldown,
                    tank.bastion_learnt,
                ))
                .small()
                .color(theme::MUTED),
            );
        }
        if let Some(commander) = view.commander {
            let rallying = commander.rally_left > 0.0;
            ui.label(
                egui::RichText::new(rally_line(
                    commander.rally_left,
                    commander.cooldown,
                    commander.can_rally,
                ))
                .small()
                .color(if rallying { theme::WARN } else { theme::MUTED }),
            );
            let crying = commander.cry_left > 0.0;
            ui.label(
                egui::RichText::new(crate::names::battle_cry_line(
                    commander.cry_left,
                    commander.cry_cooldown,
                    commander.can_cry,
                ))
                .small()
                .color(if crying { theme::WARN } else { theme::MUTED }),
            );
        }
    }

    /// The crew member the side panel is about, when it is up at all
    /// (feature 107): somebody the player has picked **other than their
    /// own Bim**, which the hero panel and the character sheet already
    /// say everything of — and which is picked from the start, so a panel
    /// for it would stand over the deck the whole run.
    pub fn inspected(&self, game: &Game) -> Option<u32> {
        (0..self.crew_count).find(|&w| {
            w as usize != self.player && game.is_selected(w as usize, self.player as u32)
        })
    }

    /// The selected crew member's panels: the health bars, the health lines
    /// and the character sheet. One at a time, and only when somebody is
    /// picked: the right-hand side answers "who am I looking at", not
    /// "what is everybody up to". `true` when something was drawn.
    pub fn side(
        &mut self,
        ui: &mut egui::Ui,
        game: &mut Game,
        name: &dyn Fn(u32) -> String,
    ) -> bool {
        let Some(who) = self.inspected(game) else {
            return false;
        };
        let w = who as usize;
        let alive = game.is_alive(w);
        ui.set_min_width(SIDE_W);
        self.who_header(ui, who, name);

        // How it is bearing up. The armour worn adds its health to the
        // body's: the blue on the end of the green is what the pieces
        // still have, and the number reads the two apart.
        //
        // This whole block is drawn big on purpose: in a fight it is the
        // only part of the panel anybody looks at, and the question it has
        // to answer at a glance is not only "how much health" but, for a
        // body down, "how long has it got" — which is the downed block
        // under the bar.
        ui.add_space(6.0);
        let points = game.health(w);
        let armour = if alive { game.armour_health(w) } else { 0.0 };
        let shield = if alive { game.shield_hp(w) } else { 0.0 };
        let hurt = is_hurt(game, w);
        ui.horizontal(|ui| {
            if who == 0 {
                theme::asks(ui, "Health", HEALTH_TIP);
            } else {
                ui.label("Health");
            }
            let total = game.max_health(w) + armour + shield;
            theme::health_bar(
                ui,
                BAR_W,
                &[
                    (
                        points / total,
                        if hurt { theme::BAD } else { theme::HEALTH },
                    ),
                    (armour / total, theme::ARMOUR),
                    (shield / total, theme::SHIELD),
                ],
            );
            ui.label(
                egui::RichText::new(format!("{}", points.round()))
                    .size(HEALTH_NUMBER)
                    .strong()
                    .color(if hurt { theme::BAD } else { theme::INK }),
            );
            if armour > 0.0 {
                ui.label(
                    egui::RichText::new(format!("+{}", armour.round()))
                        .size(HEALTH_NUMBER)
                        .color(theme::ARMOUR),
                );
            }
            if shield > 0.0 {
                ui.label(
                    egui::RichText::new(format!("+{}", shield.round()))
                        .size(HEALTH_NUMBER)
                        .color(theme::SHIELD),
                );
            }
        });

        // Down, and how long it has (task 120). The one thing on this
        // panel that is framed: it is a warning, and a warning that looks
        // like the rows around it is not one. The countdown is the ring's
        // over the body on the deck, in words, and under it who is at the
        // body, or what it would take.
        if alive && let Some(left) = game.down_left(w) {
            ui.add_space(3.0);
            downed_frame().show(ui, |ui| {
                ui.set_min_width(BAR_W + 60.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(DOWNED_HEAD)
                            .size(PERIL_NAME)
                            .strong()
                            .color(theme::DYING),
                    );
                    theme::question_mark(ui, DOWNED_TIP);
                });
                ui.label(
                    egui::RichText::new(downed_left(left))
                        .size(PERIL_LEFT)
                        .strong()
                        .color(theme::WARN),
                );
                let line = match reviver_of(game, w, w) {
                    Some(helper) => downed_reviver(&name(helper as u32)),
                    None => downed_remedy(),
                };
                ui.label(egui::RichText::new(line).small().color(theme::MUTED));
            });
            ui.add_space(3.0);
        } else if alive && hurt {
            ui.label(egui::RichText::new(BADLY_HURT).small().color(theme::BAD));
        }
        if !alive {
            ui.label(
                egui::RichText::new(died_line(&name(who)))
                    .size(PERIL_NAME)
                    .strong()
                    .color(theme::DYING),
            );
        }
        // The class (feature 74): the player's own crew member's, with its
        // level, what the next wants and the class's own rows. Only
        // their own: a class is a slot's.
        if who == self.player as u32
            && let Some(view) = self.class_view.clone()
        {
            self.class_section(ui, &view);
        }

        // The character sheet under the bars.
        ui.add_space(6.0);
        egui::Grid::new(("about", who))
            .num_columns(2)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Name").color(theme::MUTED));
                ui.label(name(who));
                ui.end_row();
                ui.label(egui::RichText::new("Age").color(theme::MUTED));
                ui.label(format!("{}", game.age(w)));
                ui.end_row();
                ui.label(egui::RichText::new("Born").color(theme::MUTED));
                ui.label(date_text(
                    game.born_date(w),
                    game.born_month(w),
                    game.born_year(w),
                ));
                ui.end_row();
            });
        // The crew's relics, as the character sheet shows them (task 136):
        // what the crewmate picked is lifted and burdened by.
        sheet_relics(ui, &self.crew_relics);
        true
    }

    // --- the tray ---------------------------------------------------------------

    /// Bottom left (feature 107): a row of buttons — Armory, Squad, Map,
    /// and Trade while the ship is at a desk — and over it the Squad
    /// panel while it is up. The Armory button is the Armory panel (task
    /// 113, where the Stash was), the Inventory key's. A player whose Bim
    /// is out has the Armory and Map buttons alone. What was asked of the
    /// screen comes back, for it to do.
    pub fn tray(
        &mut self,
        ui: &mut egui::Ui,
        _game: &Game,
        view: &TrayView,
        _name: &dyn Fn(u32) -> String,
    ) -> Vec<TrayAsk> {
        let mut asks = Vec::new();
        if view.out {
            self.tray = None;
        }
        // The panel first, over the buttons: the tray is anchored at its
        // foot and grows upwards.
        if self.tray == Some(TrayTab::Squad) {
            self.squad(ui, view, &mut asks);
            ui.separator();
        }
        let keys = self.keys;
        ui.horizontal(|ui| {
            let button = |ui: &mut egui::Ui, on: bool, word: &str, key: Option<Action>| {
                let tip = match key {
                    Some(action) => keyed(word, keys.key(action)),
                    None => word.to_string(),
                };
                ui.add(
                    theme::toggle_button(on, egui::RichText::new(word).strong())
                        .min_size(egui::vec2(TRAY_BUTTON_W, TRAY_BUTTON_H)),
                )
                .on_hover_text(tip)
            };
            if button(ui, self.armory_open, TRAY_ARMORY, Some(Action::Inventory)).clicked() {
                self.toggle_armory();
            }
            if !view.out
                && button(ui, self.tray == Some(TrayTab::Squad), TRAY_SQUAD, None).clicked()
            {
                self.tray = if self.tray == Some(TrayTab::Squad) {
                    None
                } else {
                    Some(TrayTab::Squad)
                };
            }
            if button(ui, false, TRAY_MAP, Some(Action::Map)).clicked() {
                asks.push(TrayAsk::Map);
            }
        });
        asks
    }

    /// The Squad (feature 107): the player's standing order to the crew
    /// and the buttons that give the others — the same orders F and T
    /// give — then every bot
    /// with its class, its level and its health.
    fn squad(&mut self, ui: &mut egui::Ui, view: &TrayView, asks: &mut Vec<TrayAsk>) {
        ui.set_max_width(TRAY_W);
        let keys = self.keys;
        let standing = orders_line(view.standing);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(standing.unwrap_or(SQUAD_FOLLOWING))
                    .strong()
                    .color(if standing.is_some() {
                        theme::ATTACK
                    } else {
                        theme::MUTED
                    }),
            );
            theme::question_mark(ui, ORDERS_TIP);
        });
        ui.horizontal_wrapped(|ui| {
            let order = |ui: &mut egui::Ui, word: &str, action: Option<Action>| {
                let words = match action {
                    Some(action) => keyed(word, keys.key(action)),
                    None => word.to_string(),
                };
                ui.button(words).clicked()
            };
            if order(ui, SQUAD_ATTACK, Some(Action::Attack)) {
                asks.push(TrayAsk::Attack);
            }
            if order(ui, SQUAD_RETREAT, Some(Action::Retreat)) {
                asks.push(TrayAsk::Retreat);
            }
            if standing.is_some() && order(ui, SQUAD_FOLLOW, None) {
                asks.push(TrayAsk::Follow);
            }
        });
        ui.separator();
        if view.bots.is_empty() {
            ui.label(
                egui::RichText::new(SQUAD_NO_BOTS)
                    .small()
                    .color(theme::MUTED),
            );
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("squad")
            .max_height(TRAY_PANEL_H)
            .min_scrolled_height(TRAY_PANEL_H)
            .show(ui, |ui| {
                egui::Grid::new("squad-bots")
                    .num_columns(4)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        for bot in &view.bots {
                            let ink = if bot.out { theme::MUTED } else { theme::INK };
                            ui.label(egui::RichText::new(&bot.name).color(ink));
                            ui.label(
                                egui::RichText::new(bot_class_line(bot.class, bot.level))
                                    .small()
                                    .color(theme::MUTED),
                            );
                            theme::two_tone_bar(
                                ui,
                                SQUAD_BAR_W,
                                bot.body,
                                bot.armour,
                                if bot.hurt { theme::BAD } else { theme::HEALTH },
                                theme::ARMOUR,
                            );
                            let state = if bot.out {
                                Some((PORTRAIT_OUT, theme::MUTED))
                            } else if bot.downed {
                                Some((DOWNED_WORD, theme::BAD))
                            } else {
                                None
                            };
                            match state {
                                Some((word, colour)) => {
                                    ui.label(egui::RichText::new(word).small().color(colour));
                                }
                                None => {
                                    ui.label("");
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
    }

    // --- the character sheet -------------------------------------------------------

    /// The character sheet up or shut — K, the hero panel's `+1`, the
    /// player's own portrait twice.
    pub fn toggle_sheet(&mut self) {
        self.sheet_open = !self.sheet_open;
    }

    /// The character sheet (feature 107): the player's own Bim, on the
    /// left of the canvas from `at` and no lower than `bottom` — the class
    /// and the level with the experience, the class picker where the
    /// class may still change, the health of each part of the body, what
    /// is worn and what is in hand, and the Skills tab — its four
    /// abilities a rank at a time. Nothing on it is worked out here that the side panel or the
    /// inventory does not already read. The rectangle it took, `None`
    /// while it is shut.
    pub fn character_sheet(
        &mut self,
        ctx: &egui::Context,
        game: &Game,
        at: egui::Pos2,
        bottom: f32,
    ) -> Option<egui::Rect> {
        if !self.sheet_open {
            return None;
        }
        let view = self.class_view.clone().unwrap_or_default();
        let w = self.player;
        let keys = self.keys;
        let mut shut = false;
        let tall = (bottom - at.y).max(120.0);
        let area = egui::Area::new(egui::Id::new("hud-sheet"))
            .fixed_pos(at)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                theme::tray_frame().show(ui, |ui| {
                    ui.set_width(SHEET_W);
                    ui.horizontal(|ui| {
                        let title = if view.class == world::Class::None {
                            class_name(view.class).to_string()
                        } else {
                            sheet_title(class_name(view.class), view.level)
                        };
                        ui.label(
                            egui::RichText::new(title)
                                .size(SKILL_NAME_TEXT)
                                .strong()
                                .color(theme::INK),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button(keys.key(Action::CharacterSheet).name())
                                .on_hover_text(SHEET_CLOSE)
                                .clicked()
                            {
                                shut = true;
                            }
                        });
                    });
                    if view.class != world::Class::None {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(SHEET_W, 5.0), egui::Sense::hover());
                        theme::bar_in(
                            ui.painter(),
                            rect,
                            crate::screens::hud::xp_fill(view.xp),
                            theme::HYPER,
                        );
                    }
                    ui.label(
                        egui::RichText::new(crate::screens::hud::xp_text(view.class, view.xp))
                            .small()
                            .color(theme::MUTED),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("sheet")
                        .max_height(tall - SHEET_HEAD)
                        .min_scrolled_height(tall - SHEET_HEAD)
                        .show(ui, |ui| {
                            ui.set_width(SHEET_W);
                            if view.can_change {
                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(egui::RichText::new(BIM_CLASS).color(theme::MUTED));
                                    for class in world::Class::ALL.into_iter().filter(|c| {
                                        view.open_classes.is_empty()
                                            || view.open_classes.contains(c)
                                    }) {
                                        if theme::toggle(ui, class == view.class, class_name(class))
                                            .clicked()
                                            && class != view.class
                                        {
                                            self.deploy_orders.push(DeployOrder::SetClass(class));
                                        }
                                    }
                                    theme::question_mark(ui, BIM_CLASS_NOTE);
                                });
                            }
                            sheet_body(ui, game, w);
                            sheet_gear(ui, game, w);
                            sheet_relics(ui, &view.relics);
                            theme::heading(ui, SHEET_SKILLS);
                            self.skills(ui, &view);
                        });
                });
            });
        if shut {
            self.sheet_open = false;
        }
        Some(area.response.rect)
    }

    // --- the armory (task 113) ------------------------------------------------

    /// Whose gear the hero panel, the class keys and the walks are about:
    /// the selected crew member, or the one the player steers when nobody
    /// is picked.
    pub fn inventory_who(&self, game: &Game) -> usize {
        (0..self.crew_count)
            .map(|w| w as usize)
            .find(|&w| game.is_selected(w, self.player as u32))
            .unwrap_or(self.player)
    }

    /// The Armory panel up, or shut — the Inventory key (Tab) and the
    /// tray's Armory button, on every screen of a run.
    pub fn toggle_armory(&mut self) {
        self.armory_open = !self.armory_open;
    }

    /// Act on a row of the nearby strip: the engineer's pack-up, or a
    /// window of the panels' own.
    fn show(&mut self, open: Open) {
        // Not a window: the engineer's pack-up row acts and leaves what is
        // up as it is.
        if let Open::PackUp(id) = open {
            self.deploy_orders.push(DeployOrder::PackUp(id));
            return;
        }
        self.open = Some(open);
        self.menu = None;
    }

    /// Whatever the nearby strip was clicked on this frame, put up.
    fn follow_strip(&mut self, pick: Option<Open>) {
        if let Some(open) = pick {
            self.show(open);
        }
    }

    /// **The Armory panel** (task 113): nothing is stored, so what the crew
    /// own is one window — a column a crew member (its portrait, its class,
    /// its weapon and each armour part with the tier and the health left)
    /// and under them the ship's armory, the money and the keys. A thing is
    /// dragged between the armory and a slot: onto a column it goes on the
    /// slot it is made for, what was there into the armory; off a slot onto
    /// the armory it is taken off. The player's own thing dropped on another
    /// player's column is **offered**, and the offer shows on that column
    /// with Accept and Decline. A right-click on a thing says the same in
    /// rows. In a mission only a Bim inside the ship may be changed — the
    /// crew kit out from the armory on arriving at a site — and no offer
    /// is made; the panel says so. Every press is a [`GearOrder`] through the seam, and the world's
    /// refusal, if it refuses, is said in the log.
    pub fn armory_window(&mut self, ctx: &egui::Context, view: &ArmoryView) {
        if !self.armory_open {
            return;
        }
        let mut open = true;
        let mut asked: Vec<GearOrder> = Vec::new();
        let mut strip = None;
        egui::Window::new(ARMORY_TITLE)
            .id(egui::Id::new("armory-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, ARMORY_TOP))
            .frame(theme::panel_frame())
            .show(ctx, |ui| {
                ui.set_max_width(ARMORY_W);
                if view.locked {
                    ui.label(
                        egui::RichText::new(ARMORY_LOCKED)
                            .small()
                            .color(theme::MUTED),
                    );
                    strip = nearby_strip(ui, &self.nearby.clone(), self.open);
                } else {
                    ui.label(egui::RichText::new(ARMORY_HOW).small().color(theme::MUTED));
                }
                ui.add_space(4.0);
                // Two columns: on the left the players' own Bims and the
                // armory under them, on the right the bots; each wraps its
                // crew into rows. A side with nobody in it is left out.
                // Scrolled up and down past the screen's height, so the
                // armory is never cut off below a long crew.
                let tall = (ui.ctx().content_rect().height() - ARMORY_TOP - 90.0).max(200.0);
                let crew = |players: bool| -> Vec<&ArmoryColumn> {
                    view.columns
                        .iter()
                        .filter(|c| c.portrait.player == players)
                        .collect()
                };
                let (players, bots) = (crew(true), crew(false));
                egui::ScrollArea::vertical()
                    .id_salt("armory-rows")
                    .max_height(tall)
                    .min_scrolled_height(tall)
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(ARMORY_HALF);
                                if !players.is_empty() {
                                    theme::heading(ui, ARMORY_PLAYERS);
                                    armory_crew(ui, view, &players, &mut asked);
                                    ui.separator();
                                }
                                armory_stock(ui, view, &mut asked);
                            });
                            if !bots.is_empty() {
                                ui.add_space(12.0);
                                ui.vertical(|ui| {
                                    ui.set_width(ARMORY_HALF);
                                    theme::heading(ui, ARMORY_BOTS);
                                    armory_crew(ui, view, &bots, &mut asked);
                                });
                            }
                        });
                    });
            });
        self.armory_open = open;
        self.orders.extend(asked);
        self.follow_strip(strip);
    }

    /// The Skills tab on the character sheet (features 83 and 107; every
    /// class a ranked kit since task 139): the four abilities of the
    /// class, Q C E R, a rank at a time ([`CrewPanels::ranked_skills`]),
    /// or a line saying a crew member with no class has nothing to learn.
    fn skills(&mut self, ui: &mut egui::Ui, view: &ClassView) {
        if view.class == world::Class::None {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(SKILLS_NO_CLASS)
                        .size(SKILL_TEXT)
                        .color(theme::MUTED),
                )
                .wrap(),
            );
            return;
        }
        self.ranked_skills(ui, view, view.ranks);
    }

    /// The Skills tab of a ranked kit (task 124): the skill points waiting,
    /// and each of the four abilities, Q E F Space, with its rank as pips,
    /// what it does, and every rank's numbers and the level it wants —
    /// the ranks bought lit, the next one with a button that spends a
    /// point on it while one could. Nothing here is asked of the world but
    /// what `ClassView` hands over and `world::class`'s own tables.
    fn ranked_skills(&mut self, ui: &mut egui::Ui, view: &ClassView, ranks: [u8; 4]) {
        let class = view.class;
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(ranked_points(view.points))
                        .size(SKILL_TEXT)
                        .color(if view.points > 0 {
                            theme::CAUTION
                        } else {
                            theme::MUTED
                        }),
                )
                .wrap(),
            );
            theme::question_mark(ui, RANKED_SKILLS_TIP);
        });
        ui.add_space(4.0);
        // In the hero panel's order, by their keys (Q E F R).
        for slot in Action::LAID_OUT
            .into_iter()
            .filter_map(Action::ability_slot)
        {
            let rank = ranks[slot];
            let slot = slot as u8;
            let top = world::class::MAX_RANK;
            let next = rank + 1;
            let open = view.points > 0
                && rank < top
                && world::class::rank_level(class, slot, next).is_some_and(|l| l <= view.level);
            egui::Frame::new()
                .fill(theme::RAISED)
                .stroke(egui::Stroke::new(
                    1.0,
                    if open { theme::ACCENT } else { theme::LINE },
                ))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        // Its picture, the one on its box on the hero panel.
                        if let Some(glyph) = ability_icons::Glyph::of(class, slot) {
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(SKILL_ICON, SKILL_ICON),
                                egui::Sense::hover(),
                            );
                            ability_icons::paint(ui.painter(), rect, glyph, false, 3.0);
                        }
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {}",
                                self.keys.key(Action::ABILITIES[slot as usize]).name(),
                                ranked_ability(class, slot)
                            ))
                            .size(SKILL_NAME_TEXT)
                            .strong()
                            .color(theme::INK),
                        );
                        // The ranks as pips, drawn: the default font has no
                        // round glyphs to trust.
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(f32::from(top) * 12.0, 12.0),
                            egui::Sense::hover(),
                        );
                        for k in 0..top {
                            let at =
                                egui::pos2(rect.min.x + 6.0 + 12.0 * f32::from(k), rect.center().y);
                            if k < rank {
                                ui.painter().circle_filled(at, 4.0, theme::CAUTION);
                            } else {
                                ui.painter().circle_stroke(
                                    at,
                                    4.0,
                                    egui::Stroke::new(1.0, theme::MUTED),
                                );
                            }
                        }
                    });
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(ranked_what(class, slot))
                                .size(SKILL_BOX_TEXT)
                                .color(theme::MUTED),
                        )
                        .wrap(),
                    );
                    // Every rank's numbers at once, the one bought lit
                    // (Dota 2's tooltip), and the level each rank wants:
                    // bought in ink, within reach in caution, not yet muted.
                    theme::stat_rows(ui, &ranked_stats(class, slot), Some(rank), SKILL_BOX_TEXT);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.label(
                            egui::RichText::new(format!("{RANK_LEVELS}: "))
                                .size(SKILL_BOX_TEXT)
                                .color(theme::MUTED),
                        );
                        for r in 1..=top {
                            let level = world::class::rank_level(class, slot, r).unwrap_or(1);
                            let colour = if r <= rank {
                                theme::INK
                            } else if level <= view.level {
                                theme::CAUTION
                            } else {
                                theme::MUTED
                            };
                            if r > 1 {
                                ui.label(
                                    egui::RichText::new(" / ")
                                        .size(SKILL_BOX_TEXT)
                                        .color(theme::MUTED),
                                );
                            }
                            ui.label(
                                egui::RichText::new(level.to_string())
                                    .size(SKILL_BOX_TEXT)
                                    .color(colour),
                            );
                        }
                    });
                    if open
                        && ui
                            .button(egui::RichText::new(rank_learn(next)).size(SKILL_TEXT))
                            .clicked()
                    {
                        self.deploy_orders.push(DeployOrder::RankUp {
                            ability_slot: u32::from(slot),
                        });
                    }
                });
            ui.add_space(4.0);
        }
    }

    // --- what the pointer is over -------------------------------------------

    /// The state worth naming beside a fixture, or "" for the things that
    /// have none.
    fn spot_state(game: &Game, spot: u32, x: f32, y: f32) -> String {
        match spot {
            SPOT_SHIP_DOOR => match game.door_at(x, y) {
                None => String::new(),
                Some(door) => {
                    if game.ship_door_is_locked(door) {
                        "locked".into()
                    } else if game.ship_door_is_held(door) {
                        "held open".into()
                    } else if game.ship_door_is_open(door) {
                        "open".into()
                    } else {
                        "shut".into()
                    }
                }
            },
            _ => String::new(),
        }
    }

    /// What the room makes of a point on the deck, in room coordinates: the
    /// spot code, and the thing there with its state — "Door · locked".
    pub fn spot_readout(game: &Game, x: f32, y: f32) -> (u32, String) {
        let spot = game.spot_at(x, y);
        let mut thing = SPOT_NAMES
            .get(spot as usize)
            .copied()
            .unwrap_or("Something")
            .to_string();
        let state = Self::spot_state(game, spot, x, y);
        if !state.is_empty() {
            thing = format!("{thing} · {state}");
        }
        (spot, thing)
    }
}

/// One slot of the inventory: a box with what is in it — its icon, its
/// name, a line of its numbers and, for a piece of armour, its health
/// along the bottom — and the slot's name under the box. An empty slot is
/// drawn hollow, a filled one lit. Painted on a rect of a fixed size
/// rather than laid out, so it is a slot and not whatever space the
/// window happens to have. The response is the box's, for a right-click.
fn slot_box(
    ui: &mut egui::Ui,
    label: &str,
    item: Option<PackItem>,
    name: &str,
    line: &str,
) -> egui::Response {
    ui.vertical(|ui| {
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(SLOT_WIDTH, SLOT_HEIGHT), egui::Sense::click());
        let painter = ui.painter();
        painter.rect(
            rect,
            4.0,
            theme::PANEL_DEEP,
            egui::Stroke::new(
                1.0,
                if item.is_some() {
                    theme::ACCENT
                } else {
                    theme::LINE
                },
            ),
            egui::StrokeKind::Inside,
        );
        match item {
            None => {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    name,
                    egui::FontId::proportional(13.0),
                    theme::MUTED,
                );
            }
            Some(item) => {
                // A tiered thing's slot is washed and edged in its tier's
                // colour, like its cell in a grid.
                if let Some(tint) = theme::item_tint(item) {
                    theme::tint_cell(painter, rect, 4.0, tint);
                }
                let pad = 6.0;
                let icon = egui::Rect::from_min_size(
                    rect.min + egui::vec2(pad, pad),
                    egui::vec2(SLOT_HEIGHT - pad * 2.0, SLOT_HEIGHT - pad * 2.0),
                );
                icons::icon(painter, icon, item);
                let x = icon.max.x + pad;
                painter.text(
                    egui::pos2(x, rect.min.y + pad),
                    egui::Align2::LEFT_TOP,
                    name,
                    egui::FontId::proportional(12.5),
                    theme::INK,
                );
                if !line.is_empty() {
                    painter.text(
                        egui::pos2(x, rect.max.y - pad),
                        egui::Align2::LEFT_BOTTOM,
                        line,
                        egui::FontId::proportional(10.0),
                        theme::MUTED,
                    );
                }
                if let PackItem::Armour(piece) = item {
                    let bar = egui::Rect::from_min_max(
                        egui::pos2(icon.min.x, rect.max.y - 5.0),
                        egui::pos2(icon.max.x, rect.max.y - 3.0),
                    );
                    theme::bar_in(
                        painter,
                        bar,
                        piece.health / piece.stats().health.max(1.0),
                        if piece.broken() {
                            theme::BAD
                        } else {
                            theme::ARMOUR
                        },
                    );
                }
            }
        }
        ui.label(egui::RichText::new(label).small().color(theme::MUTED));
        ui.add_space(4.0);
        response
    })
    .inner
}

/// A slot's box.
const SLOT_WIDTH: f32 = 120.0;
const SLOT_HEIGHT: f32 = 44.0;

/// The line under a worn piece's name: what it is adding and taking off,
/// or that it is past it.
fn worn_line(piece: Piece) -> String {
    if piece.broken() {
        "down — regenerates".into()
    } else {
        format!(
            "+{} hp · {} prot",
            piece.health.round(),
            tidy_hundredths(piece.stats().protection)
        )
    }
}

/// An item's tier table under its tooltip (October 2026): a row a tier it
/// is made at, what that tier gives, the row of `lit` (the item's own
/// tier, or the one a combine makes) on a plate in its tier's colour and
/// the others muted, so the player sees what an upgrade adds. Nothing for
/// a kind made at one tier alone.
pub(crate) fn item_tiers(ui: &mut egui::Ui, kind: bims::module::ModuleKind, lit: u32) {
    let rows: Vec<_> = bims::combat::Tier::ALL
        .into_iter()
        .filter_map(|tier| crate::names::item_tier_line(kind, tier.code()).map(|line| (tier, line)))
        .collect();
    if rows.is_empty() {
        return;
    }
    ui.add_space(4.0);
    for (tier, line) in rows {
        let on = tier.code() == lit;
        let colour = theme::tier_tint(tier).unwrap_or(theme::INK);
        let label = egui::RichText::new(crate::names::item_tier_label(tier.code()));
        let row = |ui: &mut egui::Ui| {
            ui.horizontal(|ui| {
                if on {
                    ui.label(label.strong().color(colour));
                    ui.label(egui::RichText::new(line).strong().color(theme::INK));
                } else {
                    ui.label(label.color(theme::MUTED));
                    ui.label(egui::RichText::new(line).color(theme::MUTED));
                }
            });
        };
        let frame = egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(4, 1))
            .corner_radius(3.0);
        if on {
            frame
                .fill(colour.gamma_multiply(0.18))
                .stroke(egui::Stroke::new(1.0, colour.gamma_multiply(0.7)))
                .show(ui, row);
        } else {
            frame.show(ui, row);
        }
    }
}

/// A cell's tooltip, laid out: [`tip_of`]'s words and, for an item, its
/// tier table with its own tier lit.
pub(crate) fn tip_ui(ui: &mut egui::Ui, item: PackItem, count: u32) {
    ui.label(tip_of(item, count));
    if let PackItem::Module(module) = item {
        item_tiers(ui, module.kind, module.tier.code());
    }
    gear_tiers(ui, item, None);
}

/// A weapon's or a piece of armour's table, every tier's numbers side by
/// side and one lit — the abilities' way (task 124): `lit`, or the
/// thing's own tier. Nothing for anything else.
pub(crate) fn gear_tiers(ui: &mut egui::Ui, item: PackItem, lit: Option<u32>) {
    let (rows, own) = match item {
        PackItem::Weapon(weapon) => crate::names::weapon_stats(weapon),
        PackItem::Armour(piece) => crate::names::armour_stats(piece.kind, piece.tier),
        _ => return,
    };
    let lit = match (item, lit.and_then(bims::combat::Tier::from_code)) {
        (PackItem::Weapon(weapon), Some(tier)) if weapon.kind.made_at(tier) => {
            crate::names::weapon_stats(bims::combat::Weapon { tier, ..weapon }).1
        }
        (PackItem::Armour(piece), Some(tier)) => crate::names::armour_stats(piece.kind, tier).1,
        _ => own,
    };
    ui.add_space(2.0);
    theme::stat_rows(ui, &rows, Some(lit), 13.0);
}

/// A cell's tooltip: the name and — a piece of armour — what it has
/// left; a weapon's and a piece's numbers are [`gear_tiers`]' table, and
/// a resource's its line.
pub(crate) fn tip_of(item: PackItem, count: u32) -> String {
    // A tier above one is said after the name: "Armour — tier 2".
    let tiered = |name: &str, tier: bims::combat::Tier| match tier_word(tier) {
        Some(word) => format!("{name} — {word}"),
        None => name.to_string(),
    };
    match item {
        // The name and what is left of it; the numbers are the tier
        // table under it (`gear_tiers`).
        PackItem::Armour(piece) => format!(
            "{}\n{} / {} hp",
            tiered(armour_name(Some(piece.kind)), piece.tier),
            piece.health.max(0.0).round(),
            piece.stats().health.round()
        ),
        PackItem::Weapon(weapon) => {
            let name = tiered(weapon_name(Some(weapon.kind)), weapon.tier);
            if count > 1 {
                format!("{name} × {count}")
            } else {
                name
            }
        }
        PackItem::Stack(code) => match ResourceId::from_code(code) {
            Some(id) if count > 1 => format!("{} × {count}\n{}", resource_name(id), item_tip(id)),
            Some(id) => format!("{}\n{}", resource_name(id), item_tip(id)),
            None => "Something the hold does not know".into(),
        },
        PackItem::Module(item) => {
            let name = if item.kind.tiered() {
                tiered(crate::names::item_name(item.kind), item.tier)
            } else {
                crate::names::item_name(item.kind).to_string()
            };
            format!("{name}\n{}", crate::names::item_line(item))
        }
    }
}

/// The strip of what is within reach of the Bim shown — every container,
/// every body down — a button each, the open one lit, so the rest are a
/// click away. Over the container and Loot windows, and over the
/// inventory when nothing else is up. What was clicked, for
/// `CrewPanels::follow_strip` once the window is laid out.
fn nearby_strip(ui: &mut egui::Ui, nearby: &[Near], open: Option<Open>) -> Option<Open> {
    if nearby.is_empty() {
        return None;
    }
    let mut pick = None;
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Nearby").small().color(theme::MUTED));
        for near in nearby {
            let lit = open == Some(near.open);
            if ui.selectable_label(lit, &near.label).clicked() && !lit {
                pick = Some(near.open);
            }
        }
    });
    pick
}

/// A word with the key that does the same beside it: `Attack (X)`.
fn keyed(word: &str, key: egui::Key) -> String {
    with_key(word, key.name())
}

/// One thing on the Stash panel: its picture in a cell, and how many in
/// the corner where there is more than one.
fn stash_cell(
    ui: &mut egui::Ui,
    count: u32,
    paint: impl FnOnce(&egui::Painter, egui::Rect),
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(STASH_CELL, STASH_CELL), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, theme::RAISED);
    paint(painter, rect.shrink(4.0));
    if count > 1 {
        painter.text(
            rect.max - egui::vec2(2.0, 1.0),
            egui::Align2::RIGHT_BOTTOM,
            count.to_string(),
            egui::FontId::proportional(10.5),
            theme::INK,
        );
    }
    response
}

/// The character sheet's body (feature 107): the one bar in words —
/// the hit points against the whole, with what worn armour adds — and,
/// under it, the countdown while it is down or the slower walk once it
/// was (task 120). The side panel's own readings.
fn sheet_body(ui: &mut egui::Ui, game: &Game, w: usize) {
    theme::heading(ui, SHEET_BODY);
    let alive = game.is_alive(w);
    let points = game.health(w);
    let armour = if alive { game.armour_health(w) } else { 0.0 };
    egui::Grid::new("sheet-body")
        .num_columns(2)
        .spacing([12.0, 2.0])
        .min_col_width(SHEET_W / 2.0 - 12.0)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(SHEET_HEALTH).color(theme::MUTED));
            ui.label(
                egui::RichText::new(health_line(points, game.max_health(w), armour)).color(
                    if is_hurt(game, w) {
                        theme::BAD
                    } else if points < game.max_health(w) {
                        theme::CAUTION
                    } else {
                        theme::INK
                    },
                ),
            );
            ui.end_row();
        });
    if let Some((line, colour)) = peril_summary(game, w) {
        ui.label(egui::RichText::new(line).small().strong().color(colour));
    }
}

/// The character sheet's gear (feature 107): each piece worn with its
/// tier and how much of it is left, and the weapon in hand with its
/// tier — what the inventory's slots say, in a line each.
/// The crew's relics (feature 106): each its picture (task 136) beside
/// its name, its boons in green and its price in red. On the character
/// sheet and on the side panel for a crewmate picked; the deck and the
/// map show them as a row of plates under the top frame
/// (`hud::relic_bar`).
fn sheet_relics(ui: &mut egui::Ui, relics: &[world::Relic]) {
    theme::heading(ui, RELICS_HEADING);
    if relics.is_empty() {
        ui.label(egui::RichText::new(NO_RELICS).small().color(theme::MUTED));
        return;
    }
    for &relic in relics {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            icons::relic_cell(ui, relic);
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(relic_name(relic))
                        .strong()
                        .color(theme::INK),
                );
                crate::screens::worldmap::relic_lines_ui(ui, relic);
            });
        });
        ui.add_space(2.0);
    }
}

fn sheet_gear(ui: &mut egui::Ui, game: &Game, w: usize) {
    theme::heading(ui, SHEET_GEAR);
    let gear = game.gear(w);
    egui::Grid::new("sheet-gear")
        .num_columns(2)
        .spacing([12.0, 2.0])
        .min_col_width(SHEET_W / 2.0 - 12.0)
        .show(ui, |ui| {
            let slot = |s: world::GearSlot| SLOT_NAMES[s.code() as usize];
            ui.label(egui::RichText::new(slot(world::GearSlot::Weapon)).color(theme::MUTED));
            match gear.weapon {
                Some(weapon) => ui.label(held_weapon_line(
                    weapon_name(Some(weapon.kind)),
                    weapon.tier.code(),
                )),
                None => ui.label(egui::RichText::new(NOTHING_IN_HAND).color(theme::MUTED)),
            };
            ui.end_row();
            ui.label(egui::RichText::new(slot(world::GearSlot::Armour)).color(theme::MUTED));
            match gear.worn() {
                Some(piece) => ui.label(worn_piece_line(
                    armour_name(Some(piece.kind)),
                    piece.tier.code(),
                    piece.health,
                    piece.stats().health,
                )),
                None => ui.label(egui::RichText::new(NOTHING_WORN).color(theme::MUTED)),
            };
            ui.end_row();
        });
}

/// Why `who` cannot revive `patient` now, in the words the greyed row
/// says, or `None` when it can (task 120): the room's own refusals
/// (`Game::revive_crewmate`) read off what the room shows — a patient
/// that is not down, the helper itself, a helper dead, down or outside, a
/// patient outside or in somebody's arms, and a patient another crewmate
/// is already bringing round, since one reviver counts.
pub fn revive_refused(
    game: &Game,
    who: usize,
    patient: usize,
    name: &dyn Fn(u32) -> String,
) -> Option<String> {
    if patient == who {
        return Some(REVIVE_YOURSELF.to_string());
    }
    // A townsperson down on the joined deck (`bims::game::GUEST`): the
    // world has asked of its own room whether it may be picked up.
    let guest = patient >= bims::game::GUEST;
    let down = if guest {
        game.is_revivable_guest(patient)
    } else {
        game.is_downed(patient)
    };
    if !down {
        return Some(REVIVE_NOT_DOWN.to_string());
    }
    if !game.is_alive(who) || game.is_downed(who) || game.is_outside(who) {
        return Some(HELPER_OUT.to_string());
    }
    if !guest && game.is_outside(patient) {
        return Some(PATIENT_OUT.to_string());
    }
    if !guest && game.is_carried(patient) {
        return Some(REVIVE_CARRIED.to_string());
    }
    // A bot on its way gives way to a player's own Bim (the room drops
    // its revive), so only another player's hands refuse it.
    reviver_of(game, patient, who)
        .filter(|&other| game.is_player(other))
        .map(|other| revive_taken(&name(other as u32)))
}

/// Why the Carry row on a body is greyed, or `None` when the player's
/// own Bim `who` could walk over and take `patient` up: `may_lift` is the
/// world's word that it is a medic of either kind. The room's own
/// conditions otherwise, as `World::can_carry` asks them bar the reach,
/// which the walk over closes.
pub fn carry_refused_row(
    game: &Game,
    who: usize,
    patient: usize,
    may_lift: bool,
) -> Option<String> {
    if patient == who {
        return Some(CARRY_YOURSELF.to_string());
    }
    if !game.is_downed(patient) {
        return Some(CARRY_NOT_DOWN.to_string());
    }
    if !may_lift {
        return Some(CARRY_MEDICS_ONLY.to_string());
    }
    if !game.is_alive(who) || game.is_downed(who) || game.is_outside(who) {
        return Some(HELPER_OUT.to_string());
    }
    if game.carrying(who).is_some() {
        return Some(CARRY_ARMS_FULL.to_string());
    }
    if game.is_outside(patient) {
        return Some(PATIENT_OUT.to_string());
    }
    if game.is_carried(patient) {
        return Some(CARRY_TAKEN.to_string());
    }
    None
}

/// Which crew member other than `except` is bringing `patient` round, or
/// on its way to — the patient's one reviver.
pub fn reviver_of(game: &Game, patient: usize, except: usize) -> Option<usize> {
    (0..game.crew_count() as usize)
        .find(|&other| other != except && other != patient && game.reviving(other) == Some(patient))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the panels say of a body is the room's own reading (task
    /// 120): nothing on a whole one, the countdown while it is down —
    /// the seconds the ring over it counts — and the revive row greyed
    /// with the reason until it can be done, then open.
    #[test]
    fn a_downed_bim_says_how_long_it_has_and_who_may_revive_it() {
        let mut game = Game::bare(7, bims::room::ROOM_W, bims::room::ROOM_H);
        let name = |who: u32| format!("crew {who}");
        assert!(game.crew_count() >= 2, "a bare room has two to stand in");
        assert!(!is_hurt(&game, 1));
        assert_eq!(peril_summary(&game, 1), None, "a whole body says nothing");
        assert_eq!(
            revive_refused(&game, 0, 1, &name).as_deref(),
            Some(REVIVE_NOT_DOWN)
        );

        let hit = game.wound(1, 1_000.0);
        assert!(hit.downed, "a hit to nothing downs it");
        // The body lies down at the top of its next tick.
        game.simulate(1.0 / 60.0);
        assert!(game.is_downed(1) && is_hurt(&game, 1));
        let left = game.down_left(1).expect("a countdown on a downed body");
        assert!(left > 0.0 && left <= health::DOWNED_SECONDS);
        assert_eq!(
            peril_summary(&game, 1),
            Some((downed_short(left), theme::DYING))
        );
        // Nobody revives themselves, and the player's Bim may revive the
        // crewmate.
        assert_eq!(
            revive_refused(&game, 1, 1, &name).as_deref(),
            Some(REVIVE_YOURSELF)
        );
        assert_eq!(revive_refused(&game, 0, 1, &name), None);
        assert_eq!(reviver_of(&game, 1, 0), None);
    }
}

/// Where the Armory panel hangs from the top of the screen, and how wide
/// it may grow before its columns scroll.
const ARMORY_TOP: f32 = 110.0;
const ARMORY_W: f32 = 980.0;
/// Each of the panel's two columns — the players and the armory, the
/// bots — is a little under half of it, room for three crew a row.
const ARMORY_HALF: f32 = 470.0;
/// A column's width: a portrait, a name and four slots under them.
const ARMORY_COLUMN_W: f32 = SLOT_WIDTH + 12.0;

/// One crew member's column on the Armory panel, as the screen hands it
/// over every frame off the world (task 113).
#[derive(Clone)]
pub struct ArmoryColumn {
    pub who: u32,
    /// Its portrait, as the HUD draws it: the name's initials, the health
    /// bar, the level.
    pub portrait: crate::screens::hud::Portrait,
    pub gear: bims::combat::Gear,
    /// Whether the player looking may change it now — its own Bim or a
    /// bot, and in a mission only inside the ship
    /// (`World::may_change_now`).
    pub may_change: bool,
    /// Offers made **to** this column's player: who from, which slot, the
    /// thing, and the giver's name.
    pub offers_in: Vec<(u32, world::GearSlot, PackItem, String)>,
    /// Offers this column's player has made: which slot, to whom by name.
    pub offers_out: Vec<(world::GearSlot, u32, String)>,
}

/// The Armory panel's whole reading (task 113): a column a crew member,
/// the armory, the money and the keys, and whether a mission is running —
/// when a column's `may_change` holds only for a Bim inside the ship and
/// no offer is made or answered.
#[derive(Clone, Default)]
pub struct ArmoryView {
    pub local: u32,
    pub columns: Vec<ArmoryColumn>,
    pub armory: Vec<world::Stored>,
    pub money: economy::Money,
    pub locked: bool,
}

/// What is being dragged across the Armory panel: a thing out of the
/// armory, or one off a slot.
#[derive(Clone, Copy, PartialEq, Debug)]
struct ArmoryDrag(world::GearSource);

/// The slot a thing on a loadout is in, named: the weapon, or the armour.
fn slot_label(slot: world::GearSlot) -> &'static str {
    SLOT_NAMES[slot.code() as usize]
}

/// What dropping `drag` on column `onto` asks for, if anything: onto a
/// Bim the player may change, a thing out of the armory or off a slot it
/// may change; onto another player's Bim, the player's own thing
/// **offered**.
fn drop_on_column(view: &ArmoryView, drag: ArmoryDrag, onto: &ArmoryColumn) -> Option<GearOrder> {
    let ArmoryDrag(from) = drag;
    if let world::GearSource::Worn { who, slot } = from {
        if who == onto.who {
            return None;
        }
        if !onto.may_change && who == view.local && onto.portrait.player {
            return Some(GearOrder::Offer {
                part: slot,
                to: onto.who,
            });
        }
    }
    Some(GearOrder::Equip {
        who: onto.who,
        from,
    })
}

/// Crew members' columns, as many a row as fit across the width left.
fn armory_crew(
    ui: &mut egui::Ui,
    view: &ArmoryView,
    crew: &[&ArmoryColumn],
    asked: &mut Vec<GearOrder>,
) {
    let each = ARMORY_COLUMN_W + 8.0 + ui.spacing().item_spacing.x;
    let across = ((ui.available_width() + ui.spacing().item_spacing.x) / each).max(1.0) as usize;
    for row in crew.chunks(across) {
        ui.horizontal_top(|ui| {
            for column in row {
                armory_column(ui, view, column, asked);
            }
        });
    }
}

/// One crew member's column: its portrait, its class and its four slots,
/// each a drag source when the player may change it, the column a drop
/// zone; and the offers standing to it and from it.
fn armory_column(
    ui: &mut egui::Ui,
    view: &ArmoryView,
    column: &ArmoryColumn,
    asked: &mut Vec<GearOrder>,
) {
    let frame = egui::Frame::new()
        .inner_margin(egui::Margin::same(4))
        .corner_radius(4.0);
    // A drop on one of the item cells is theirs, not the column's.
    let mut inner_drop = false;
    let (_, dropped) = ui.dnd_drop_zone::<ArmoryDrag, ()>(frame, |ui| {
        ui.set_width(ARMORY_COLUMN_W);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let _ = crate::screens::hud::portrait(ui, &column.portrait);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&column.portrait.name).strong().color(
                        if column.portrait.yours {
                            theme::YOURS
                        } else {
                            theme::INK
                        },
                    ));
                    ui.label(
                        egui::RichText::new(class_name(column.portrait.class))
                            .small()
                            .color(theme::MUTED),
                    );
                    if !column.portrait.player {
                        ui.label(egui::RichText::new(ARMORY_BOT).small().color(theme::MUTED));
                    }
                });
            });
            for slot in [world::GearSlot::Weapon, world::GearSlot::Armour] {
                let item = slot.read(&column.gear);
                let (name, line) = match item {
                    Some(PackItem::Weapon(w)) => (
                        weapon_name(Some(w.kind)).to_string(),
                        format!("tier {}", roman_tier(w.tier.code())),
                    ),
                    Some(PackItem::Armour(p)) => (
                        armour_name(Some(p.kind)).to_string(),
                        format!("tier {} · {}", roman_tier(p.tier.code()), worn_line(p)),
                    ),
                    _ => (ARMORY_EMPTY_SLOT.to_string(), String::new()),
                };
                let movable = item.is_some()
                    && (column.may_change || (column.who == view.local && !view.locked));
                let id = egui::Id::new(("armory-slot", column.who, slot.code()));
                let from = world::GearSource::Worn {
                    who: column.who,
                    slot,
                };
                let response = if movable {
                    ui.dnd_drag_source(id, ArmoryDrag(from), |ui| {
                        slot_box(ui, slot_label(slot), item, &name, &line)
                    })
                    .response
                } else {
                    slot_box(ui, slot_label(slot), item, &name, &line)
                };
                let response = match item {
                    Some(item) => response.on_hover_ui(|ui| tip_ui(ui, item, 1)),
                    None => response,
                };
                if movable {
                    response.context_menu(|ui| {
                        if column.may_change && ui.button(ARMORY_TAKE_OFF).clicked() {
                            asked.push(GearOrder::Unequip {
                                who: column.who,
                                part: slot,
                            });
                            ui.close();
                        }
                        if column.who == view.local && !view.locked {
                            for other in view
                                .columns
                                .iter()
                                .filter(|c| c.portrait.player && c.who != view.local)
                            {
                                if ui
                                    .button(format!("{ARMORY_OFFER_TO} {}", other.portrait.name))
                                    .clicked()
                                {
                                    asked.push(GearOrder::Offer {
                                        part: slot,
                                        to: other.who,
                                    });
                                    ui.close();
                                }
                            }
                        }
                    });
                }
            }
            // The six items (October 2026), a player's Bim's alone: a
            // cell each, a thing dragged onto one going on that slot.
            if column.portrait.player {
                inner_drop |= item_cells(ui, view, column, asked);
            }
            // Offers made to this Bim's player, the thing and the giver,
            // with the two answers — the recipient's alone to press.
            for (from, slot, item, giver) in &column.offers_in {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(format!("{giver} {ARMORY_OFFERS} {}", item_name(*item)))
                        .small()
                        .color(theme::ACCENT),
                );
                if column.who == view.local && !view.locked {
                    ui.horizontal(|ui| {
                        if ui.small_button(ARMORY_ACCEPT).clicked() {
                            asked.push(GearOrder::AnswerOffer {
                                from: *from,
                                part: *slot,
                                yes: true,
                            });
                        }
                        if ui.small_button(ARMORY_DECLINE).clicked() {
                            asked.push(GearOrder::AnswerOffer {
                                from: *from,
                                part: *slot,
                                yes: false,
                            });
                        }
                    });
                }
            }
            for (slot, to, whom) in &column.offers_out {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {ARMORY_OFFERED_TO} {whom}",
                            slot_label(*slot)
                        ))
                        .small()
                        .color(theme::MUTED),
                    );
                    if column.who == view.local
                        && !view.locked
                        && ui.small_button(ARMORY_TAKE_BACK).clicked()
                    {
                        asked.push(GearOrder::AnswerOffer {
                            from: column.who,
                            part: *slot,
                            yes: false,
                        });
                    }
                    let _ = to;
                });
            }
        });
    });
    if let Some(drag) = dropped
        && !inner_drop
        && (column.may_change || !view.locked)
        && let Some(order) = drop_on_column(view, *drag, column)
    {
        asked.push(order);
    }
}

/// A player's column's six item cells, in a row (October 2026): each
/// its picture or empty, a drag source when the player may change it, a
/// drop zone that puts what is dropped on that slot, and a right-click to
/// take it off or offer it. Whether something was dropped on one.
fn item_cells(
    ui: &mut egui::Ui,
    view: &ArmoryView,
    column: &ArmoryColumn,
    asked: &mut Vec<GearOrder>,
) -> bool {
    let mut dropped_here = false;
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for slot in world::GearSlot::ITEMS {
            let item = slot.read(&column.gear);
            let movable =
                item.is_some() && (column.may_change || (column.who == view.local && !view.locked));
            let frame = egui::Frame::new();
            let (_, dropped) = ui.dnd_drop_zone::<ArmoryDrag, ()>(frame, |ui| {
                let cell = |ui: &mut egui::Ui| match item {
                    Some(thing) => stash_cell(ui, 1, |p, r| icons::icon(p, r.expand(3.0), thing)),
                    None => stash_cell(ui, 1, |p, r| {
                        p.rect_stroke(
                            r.expand(4.0),
                            3.0,
                            egui::Stroke::new(1.0, theme::LINE),
                            egui::StrokeKind::Inside,
                        );
                    }),
                };
                let response = if movable {
                    ui.dnd_drag_source(
                        egui::Id::new(("armory-item", column.who, slot.code())),
                        ArmoryDrag(world::GearSource::Worn {
                            who: column.who,
                            slot,
                        }),
                        cell,
                    )
                    .response
                } else {
                    cell(ui)
                };
                let response = match item {
                    Some(thing) => response.on_hover_ui(|ui| tip_ui(ui, thing, 1)),
                    None => response.on_hover_text(slot_label(slot)),
                };
                if movable {
                    response.context_menu(|ui| {
                        if column.may_change && ui.button(ARMORY_TAKE_OFF).clicked() {
                            asked.push(GearOrder::Unequip {
                                who: column.who,
                                part: slot,
                            });
                            ui.close();
                        }
                        if column.who == view.local && !view.locked {
                            for other in view
                                .columns
                                .iter()
                                .filter(|c| c.portrait.player && c.who != view.local)
                            {
                                if ui
                                    .button(format!("{ARMORY_OFFER_TO} {}", other.portrait.name))
                                    .clicked()
                                {
                                    asked.push(GearOrder::Offer {
                                        part: slot,
                                        to: other.who,
                                    });
                                    ui.close();
                                }
                            }
                        }
                    });
                }
            });
            if let Some(drag) = dropped {
                dropped_here = true;
                let ArmoryDrag(from) = *drag;
                if column.may_change {
                    asked.push(GearOrder::EquipAt {
                        who: column.who,
                        from,
                        at: slot,
                    });
                }
            }
        }
    });
    dropped_here
}

/// The armory and the money: every thing nobody wears, a
/// picture a thing, each a drag source onto a column; the whole a drop
/// zone that takes a thing off a slot.
fn armory_stock(ui: &mut egui::Ui, view: &ArmoryView, asked: &mut Vec<GearOrder>) {
    // Whether anything may be put on or taken off at all: on the map,
    // and in a mission while a Bim that may be changed is inside the ship.
    let open = view.columns.iter().any(|c| c.may_change);
    ui.horizontal(|ui| {
        theme::heading(ui, ARMORY_STOCK);
        ui.label(egui::RichText::new(crate::format::euros(view.money)).color(theme::MONEY));
    });
    let frame = egui::Frame::new()
        .inner_margin(egui::Margin::same(6))
        .corner_radius(4.0)
        .fill(theme::PANEL_DEEP);
    let (_, dropped) = ui.dnd_drop_zone::<ArmoryDrag, ()>(frame, |ui| {
        ui.set_width(ARMORY_HALF - 12.0);
        ui.set_min_height(STASH_CELL + 8.0);
        if view.armory.is_empty() {
            ui.label(
                egui::RichText::new(ARMORY_NOTHING)
                    .small()
                    .color(theme::MUTED),
            );
            return;
        }
        // Laid out in rows by hand: a drag source is a scope, and a scope
        // never wraps in `horizontal_wrapped` — the row ran on and widened
        // the panel.
        ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
        let across = ((ui.available_width() + 3.0) / (STASH_CELL + 3.0)).max(1.0) as usize;
        for row in view.armory.chunks(across) {
            ui.horizontal(|ui| {
                for stored in row {
                    let item = stored.item;
                    let from = world::GearSource::Armory { id: stored.id };
                    let response = if !open {
                        stash_cell(ui, 1, |p, r| icons::icon(p, r, item))
                    } else {
                        ui.dnd_drag_source(
                            egui::Id::new(("armory-stock", stored.id)),
                            ArmoryDrag(from),
                            |ui| stash_cell(ui, 1, |p, r| icons::icon(p, r, item)),
                        )
                        .response
                    };
                    let response = response.on_hover_ui(|ui| tip_ui(ui, item, 1));
                    if open {
                        response.context_menu(|ui| {
                            for column in view.columns.iter().filter(|c| c.may_change) {
                                if ui
                                    .button(format!("{ARMORY_PUT_ON} {}", column.portrait.name))
                                    .clicked()
                                {
                                    asked.push(GearOrder::Equip {
                                        who: column.who,
                                        from,
                                    });
                                    ui.close();
                                }
                            }
                        });
                    }
                }
            });
        }
    });
    if let Some(drag) = dropped
        && open
        && let ArmoryDrag(world::GearSource::Worn { who, slot }) = *drag
    {
        asked.push(GearOrder::Unequip { who, part: slot });
    }
}

/// A thing's name alone.
fn item_name(item: PackItem) -> &'static str {
    match item {
        PackItem::Armour(p) => armour_name(Some(p.kind)),
        PackItem::Weapon(w) => weapon_name(Some(w.kind)),
        PackItem::Module(m) => crate::names::item_name(m.kind),
        PackItem::Stack(_) => "",
    }
}

/// A tier's numeral.
fn roman_tier(tier: u32) -> &'static str {
    match tier {
        1 => "I",
        2 => "II",
        3 => "III",
        _ => "?",
    }
}
