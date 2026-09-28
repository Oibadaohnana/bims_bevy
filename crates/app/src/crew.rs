//! The crew's panels: everything on the game screen that is about the Bims
//! rather than about the deck they are standing on — the room aboard,
//! stepped by the world.
//!
//! The selected crew member's health and diary, the tray with its Stash
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
//! has to agree about what is in it. The hold itself comes the other way
//! as a [`Hold`] snapshot the screen sets every frame. The station's
//! shelves on a joined deck are told from the ship's by
//! `Hold::station_shelves`: they are not the hold's, and a click on one
//! opens nothing.
//!
//! A body is handed in the same way. The Loot window is a grid over what
//! a dead or unconscious crewmate has on it, and what it shows, whether the
//! Bim is still down and whether the looter is within reach come as a
//! [`Body`] snapshot the screen sets every frame *after* the fixture menu
//! has run, since the menu is what opens the window. Taking is an order
//! like the rest, `GearOrder::Loot`, and the walk over to the body is the
//! screen's too (`walk`), since where a body lies is the world's to say —
//! the same walk takes the Bim to a mercenary the Hire row is open on.

use bevy_egui::egui;
use bims::combat::{Item as PackItem, Piece};
use bims::game::Game;
use bims::order::CrewOrder;
use bims::room::*;
use bims::{door, health};
use physics::ResourceId;
use world::LootSource;

use crate::format::{clock_text, date_text};
use crate::icons;
use crate::keys::{Action, Keys};
use crate::names::*;
use crate::theme;

/// Below this the pointer moved so little that it counts as a click, not a
/// sweep, in points.
pub const CLICK_SLOP: f32 = 4.0;

/// What of the ship's the panels are handed every frame that the room
/// alone does not know: whether a relic cache lies on the station's
/// research desk (feature 106, `World::cache_here`), which the desk's row
/// reads.
/// Nothing is stored anywhere since task 113: the ship's holdings are the
/// Armory panel's ([`ArmoryView`]).
#[derive(Clone, Default)]
pub struct Hold {
    pub station_cache: bool,
}

/// Where the Hire window sits: right of the character sheet and under
/// the portraits and the top frame (feature 107).
const HIRE_AT: egui::Vec2 = egui::vec2(370.0, 150.0);

/// A mercenary's terms, as the panels see them: a snapshot the screen
/// hands over every frame for the resident the Hire window is open on
/// (`World::hire_offer`), and `None` once it is not for hire — hired, or
/// the rooms parted — which shuts the window.
#[derive(Clone, Copy)]
pub struct Terms {
    pub fee: economy::Money,
    pub gear: bims::combat::Gear,
    pub in_reach: bool,
    pub affordable: bool,
    /// Whether this one is a **field medic** (feature 86): hired to
    /// fetch the fallen out of the fire and treat them, not to shoot.
    pub medic: bool,
}

/// What a press on the Armory panel or a row asked for, about somebody's
/// gear. The screen sends it as the matching `world::Command`.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum GearOrder {
    /// Put a thing — out of the armory, or off another Bim's slot — on
    /// `who`, what was there into the armory (task 113).
    Equip { who: u32, from: world::GearSource },
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
    /// Hire the mercenary that is that resident of the station, `who`
    /// doing the hiring — `Command::Hire`.
    Hire { who: u32, resident: u32 },
    /// Open the relic cache on the station's research desk, `who` doing
    /// it — `Command::OpenCache` (feature 106). Sent by the screen once
    /// `who` is within reach of the desk, after the desk's row walked them
    /// there.
    OpenCache { who: u32 },
}

/// Something within reach of the Bim shown, for the nearby strip: the
/// window it opens and what to call it.
#[derive(Clone, PartialEq, Debug)]
pub struct Near {
    pub open: Open,
    pub label: String,
}

/// What a row or the nearby strip put up: a mercenary's terms, or one
/// of the acts below that are no window of the panels' own.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Open {
    Hire(u32),
    /// Not a window either: the station's research desk's row walks
    /// the Bim shown to it and asks the screen to open the relic cache
    /// on it (feature 106, `CrewPanels::cache_requested`).
    Cache(usize),
    /// Not a window either: the engineer.s deployable within reach packed
    /// up into its pack (`Command::PackUp`) — the row on the nearby strip
    /// beside one (feature 74). By the deployable.s id. There is no
    /// refill any more: a sentry never runs out of shots (feature 88).
    PackUp(u32),
}

/// What the class section and the deployable rows asked for this frame
/// (feature 74), for the screen to send through the seam.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DeployOrder {
    PackUp(u32),
    Pick { level: u32, side: world::Side },
    SetClass(world::Class),
    /// A rank of the ranked kit bought off the Skills tab (task 124).
    RankUp { ability_slot: u32 },
}

/// A player's class as the panel shows it, a snapshot the screen hands
/// over every frame off the world: what it is, how far along, the pick
/// waiting if one is, the talents learnt, and whether the class may
/// still be changed (before the first undock).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ClassView {
    pub class: world::Class,
    pub level: u8,
    pub to_next: u32,
    /// The experience itself, whole (feature 107): what the character
    /// sheet and the hero panel read the level's progress off.
    pub xp: u32,
    pub pending: Option<(u8, world::Talent, world::Talent)>,
    /// Every pick made, level and side, in level order — what the Skills
    /// tree draws as taken and as given up (feature 83). The talents
    /// themselves are `talents`, which is the same list read through the
    /// class.
    pub picks: Vec<(u8, world::Side)>,
    pub talents: Vec<world::Talent>,
    pub can_change: bool,
    /// The ranks bought of a ranked kit's four abilities, Q C E R (task
    /// 124) — `None` for a class of talents. What the Skills tab draws in
    /// place of the tree.
    pub ranks: Option<[u8; 4]>,
    /// Skill points not spent on those ranks.
    pub points: u8,
    /// The soldier's rows (feature 75): grenade charges in the pack,
    /// seconds of the clock until the next comes back (feature 90), and
    /// whether it is braced — `None` for anybody but a soldier.
    pub soldier: Option<SoldierView>,
    /// The medic's rows (feature 76) — `None` for anybody but a medic.
    pub medic: Option<MedicView>,
    /// The tank's rows (feature 77) — `None` for anybody but a tank.
    pub tank: Option<TankView>,
    /// The commander's rows (feature 78) — `None` for anybody else.
    pub commander: Option<CommanderView>,
    /// The relics its Bim holds (feature 106), in the order it was given
    /// them: what the sheet lists under the gear.
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
    pub braced: bool,
}

/// What the panel says of a medic (feature 76): who the beam holds, by
/// name; how charged the surge is, nought to one; whether the level for
/// one has been reached; and whether one is running on the medic now.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct MedicView {
    pub patients: Vec<String>,
    pub charge: f32,
    pub can_surge: bool,
    pub surging: bool,
}

/// What the panel says of a tank (feature 77): whether the wall is up,
/// minutes of the taunt left, seconds until it may taunt again, and
/// whether the level for one has been reached.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct TankView {
    pub bulwark: bool,
    pub taunt_left: f64,
    pub cooldown: f64,
    pub can_taunt: bool,
}

/// What the panel says of a commander (feature 78): what his squad is
/// under — `world::SquadKind`'s code, `None` with no order — how many
/// are in it, minutes of the rally left, seconds until he may rally
/// again, and whether the level for one has been reached.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct CommanderView {
    pub squad: Option<u32>,
    pub members: usize,
    pub rally_left: f64,
    pub cooldown: f64,
    pub can_rally: bool,
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
    /// The player steers a commander, whose squad has two orders that
    /// want no target: fall back and stand ground.
    pub commander: bool,
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
    /// The commander's squad called back to him, as X with the pointer
    /// on nothing.
    FallBack,
    /// The commander's squad held where it stands, as Z.
    StandGround,
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
/// the countdown while it is down, in the ring's red, and the slower walk
/// for the rest of the mission once it was, in the caution colour. `None`
/// while neither holds, and for the dead. There is nothing else to say
/// since task 120: one bar, and the bar says the rest.
pub fn peril_summary(game: &Game, w: usize) -> Option<(String, egui::Color32)> {
    if !game.is_alive(w) {
        return None;
    }
    if let Some(left) = game.down_left(w) {
        return Some((downed_short(left), theme::DYING));
    }
    game.was_downed(w).then(|| (slowed_short(), theme::CAUTION))
}

/// How wide the character sheet is (feature 107): the talent tree across
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

/// The talent tree's geometry: the numbered gutter down the left, a
/// slot's box, and the gaps between them. A fixed level's box is two of
/// [`SKILL_BOX_W`] and the gap wide; a pick level's two are one each —
/// the whole [`SHEET_W`] across.
const SKILL_GUTTER: f32 = 32.0;
const SKILL_BOX_W: f32 = (SHEET_W - SKILL_GUTTER - SKILL_GAP_X) / 2.0;
const SKILL_BOX_H: f32 = 28.0;
const SKILL_GAP_X: f32 = 10.0;
const SKILL_GAP_Y: f32 = 6.0;

/// The tree's type: a few points over the panels' small text, since a
/// tree of seventy talents and what each is worth is read rather than
/// glanced at.
const SKILL_TEXT: f32 = 13.5;
/// The type in a slot's box.
const SKILL_BOX_TEXT: f32 = 12.5;
/// The picked slot's name under the tree, and the sheet's title.
const SKILL_NAME_TEXT: f32 = 17.0;

/// How big the tree comes out: ten levels down, two slots across.
fn skill_tree_size() -> egui::Vec2 {
    let levels = world::class::LEVELS as f32;
    egui::vec2(
        SKILL_GUTTER + 2.0 * SKILL_BOX_W + SKILL_GAP_X,
        levels * SKILL_BOX_H + (levels - 1.0) * SKILL_GAP_Y,
    )
}

/// One slot of the Skills tree (feature 83): a fixed level's own, which
/// comes with the level, or one of the two a pick level offers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    Fixed(u8),
    Pick(u8, world::Side),
}

impl Slot {
    /// The level it sits at.
    fn level(self) -> u8 {
        match self {
            Slot::Fixed(level) | Slot::Pick(level, _) => level,
        }
    }
}

/// What a slot of the Skills tree is, for the crew member looking at it:
/// what colours its box and what the line under the tree says.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SkillState {
    /// Learnt — picked, or a fixed level reached.
    Learnt,
    /// The other side of a level already chosen at: gone for good.
    GivenUp,
    /// Reached, unchosen: a point away.
    Open,
    /// The level is not reached yet.
    Locked,
}

impl SkillState {
    /// What a slot is for the crew member the view is of: a fixed level
    /// is learnt once reached, and a pick level's side is learnt if it
    /// was the one chosen, given up if the other was, open if the level
    /// is reached and nothing has been chosen at it, and locked until
    /// the level is.
    fn of(view: &ClassView, slot: Slot) -> SkillState {
        let level = slot.level();
        match slot {
            Slot::Fixed(_) if level <= view.level => SkillState::Learnt,
            Slot::Fixed(_) => SkillState::Locked,
            Slot::Pick(_, side) => match taken_at(view, level) {
                Some(chosen) if chosen == side => SkillState::Learnt,
                Some(_) => SkillState::GivenUp,
                None if level <= view.level => SkillState::Open,
                None => SkillState::Locked,
            },
        }
    }
}

/// Which side was chosen at a level, if the level was chosen at.
fn taken_at(view: &ClassView, level: u8) -> Option<world::Side> {
    view.picks
        .iter()
        .find(|&&(l, _)| l == level)
        .map(|&(_, side)| side)
}

/// How many skill points are waiting: one for every pick level reached
/// and not chosen at (feature 83). Nought for a crew member with no
/// class, which has no levels to choose at.
fn points_left(view: &ClassView) -> usize {
    if view.class == world::Class::None {
        return 0;
    }
    (2..=view.level)
        .filter(|&l| world::class::is_pick_level(view.class, l) && taken_at(view, l).is_none())
        .count()
}

/// **What a slot is worth in numbers** — the line the column beside the
/// tree puts under a slot's name, so a choice between two talents is a
/// choice between two figures. The words and the arithmetic are
/// `names.rs`'s, off the rules crates' own constants: a fixed level's
/// [`level_numbers`], a pick level's [`talent_numbers`] for its side.
fn skill_numbers(class: world::Class, slot: Slot) -> String {
    match slot {
        Slot::Fixed(level) => level_numbers(class, level).unwrap_or_default(),
        Slot::Pick(level, side) => world::class::pick_at(class, level)
            .map(|(left, right)| {
                talent_numbers(match side {
                    world::Side::Left => left,
                    world::Side::Right => right,
                })
            })
            .unwrap_or_default(),
    }
}

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
/// open — a body's "Loot", a mercenary's "Hire" — which is the panels' to
/// do rather than the room's.
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

    fn opens(label: impl Into<String>, hint: impl Into<String>, what: Open) -> Item {
        Item {
            label: label.into(),
            hint: hint.into(),
            disabled: false,
            run: None,
            opens: Some(what),
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
    /// The side panel's page, per crew member: `true` for the diary.
    diary_open: Vec<bool>,
    /// The spot the pointer's row rang last frame, and the one the rows
    /// hovered this frame want.
    ringed: u32,
    wanted: u32,
    menu: Option<Menu>,
    /// The talent tree: the slot picked, whose details are under the tree
    /// (feature 83). Cleared when the class changes under it.
    skill_pick: Option<Slot>,
    /// The Armory panel (task 113): up from the Inventory key (Tab) or the
    /// tray's Armory button, on every screen of a run, until it is shut.
    pub armory_open: bool,
    /// What the ship has that the panels read, as the screen last handed
    /// it over. See [`Hold`].
    pub hold: Option<Hold>,
    /// The Hire window, if one is up.
    open: Option<Open>,
    /// The mercenary the Hire row opened the window on: the screen walks
    /// the Bim shown over to it and takes this.
    pub walk: Option<LootSource>,
    /// The mercenary's terms the Hire window is over, as the screen last
    /// handed them; `None` while none is open, or once the body is no
    /// longer for hire — hired, or the rooms parted — which shuts it.
    pub terms: Option<Terms>,
    /// The research desk's Open row was picked for this Bim (feature
    /// 106): the screen
    /// opens the relic cache once they are within reach, and takes this.
    pub cache_requested: Option<u32>,
    /// What is within reach of the Bim shown, nearest first — an
    /// engineer's deployable to pack up — with a name for the strip, as
    /// the screen last handed it over (`Near`). Fresh every frame: the Bim is walking.
    pub nearby: Vec<Near>,
    /// The player's own class, as the screen last handed it over
    /// (feature 74): drawn under the health of their own crew member.
    pub class_view: Option<ClassView>,
    /// How long the player's own Bim takes to revive a crewmate, in
    /// seconds, as the screen last handed it over off the world
    /// (`World::revive_seconds`, task 120): the class, a hired medic's
    /// trade and *Trauma Kit* all move it, and the menu's row says it.
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
            diary_open: vec![false; crew_count as usize],
            ringed: SPOT_NOTHING,
            wanted: SPOT_NOTHING,
            menu: None,
            skill_pick: None,
            armory_open: crate::dev::armory(),
            hold: None,
            open: None,
            walk: None,
            terms: None,
            cache_requested: None,
            nearby: Vec::new(),
            class_view: None,
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
        self.diary_open = vec![false; count as usize];
        self.menu = None;
        // The room was rebuilt with it, and the station's people with it:
        // a mercenary the Hire window was over has gone with its room.
        self.open = None;
        self.walk = None;
        self.terms = None;
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

    /// Open the Hire window on one of the station's people: the walk over
    /// is the screen's, and the terms are handed in every frame
    /// ([`Terms`]).
    fn open_hire(&mut self, resident: u32) {
        self.open = Some(Open::Hire(resident));
        self.walk = Some(LootSource::Resident(resident));
        self.terms = None;
        self.menu = None;
    }

    /// The resident the Hire window is up on, if it is: what the screen
    /// hands [`Terms`] for.
    pub fn hire_source(&self) -> Option<u32> {
        match self.open {
            Some(Open::Hire(resident)) => Some(resident),
            _ => None,
        }
    }

    /// Shut a fixture's menu, the way a click away does.
    pub fn close_menu(&mut self) {
        self.menu = None;
    }

    /// Escape: the innermost thing up goes first — a fixture's menu, then
    /// the Hire window, then the Armory panel, then the character sheet.
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
            HIT_BIM => {
                // A body on the deck — the player's own, or a crewmate
                // (task 120): one row, the revive. The hands are always
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
            }
            // One of the station's people, on its feet and hailable: a
            // mercenary for hire. What it asks is the world's to say — the
            // window reads it. A resident down has no row: what it had on
            // it is its own.
            HIT_VISITOR => {
                let body = game.hit_visitor() as u32;
                if !game.visitor_down(body as usize) {
                    items.push(Item::opens(
                        HIRE_ROW,
                        format!(
                            "{} — a mercenary, paid by the month",
                            name(self.crew_count + body)
                        ),
                        Open::Hire(body),
                    ));
                }
            }
            HIT_RESEARCH => {
                // A station's research desk: the one row walks the Bim
                // shown over and opens the relic cache on it, if there is
                // one (feature 106). Nothing else is done at one.
                let desk = game.hit_research();
                if self.hold.as_ref().is_some_and(|h| h.station_cache) {
                    items.push(Item::opens(CACHE_ROW, CACHE_ROW_HINT, Open::Cache(desk)));
                }
            }
            _ => {}
        }
        // While there is something to wait behind, a word about Shift: a
        // row given with it waits its turn (feature 69). Only under rows
        // that are errands — the Hire and Open rows are windows.
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
                Some(Open::Hire(resident)) => self.open_hire(resident),
                Some(Open::Cache(desk)) => {
                    let who = self.inventory_who(game);
                    if let Some(spot) = game.research_spot(desk) {
                        self.crew_orders.push(CrewOrder::SendTo {
                            who: who as u32,
                            x: spot.x,
                            y: spot.y,
                        });
                    }
                    self.cache_requested = Some(who as u32);
                }
                Some(open @ Open::PackUp(_)) => self.show(open),
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
    /// pick waiting as two buttons with what each does behind a `?`, the
    /// talents learnt, and — until the first undock — the class picker.
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
                let (word, color) = if soldier.braced {
                    (BRACED, theme::CAUTION)
                } else {
                    (STAND_EASY, theme::MUTED)
                };
                ui.label(egui::RichText::new(word).small().color(color));
                theme::question_mark(ui, BRACED_TIP);
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
            ui.horizontal(|ui| {
                let charged = medic.can_surge && medic.charge >= 1.0;
                ui.label(
                    egui::RichText::new(surge_line(medic.charge, medic.can_surge))
                        .small()
                        .color(if charged {
                            theme::CAUTION
                        } else {
                            theme::MUTED
                        }),
                );
                if medic.surging {
                    ui.label(egui::RichText::new(SURGING).small().color(theme::YOURS));
                }
            });
        }
        if let Some(tank) = view.tank {
            ui.horizontal(|ui| {
                let (word, color) = if tank.bulwark {
                    (WALL_UP, theme::CAUTION)
                } else {
                    (WALL_DOWN, theme::MUTED)
                };
                ui.label(egui::RichText::new(word).small().color(color));
                theme::question_mark(ui, WALL_TIP);
            });
            let taunting = tank.taunt_left > 0.0;
            ui.label(
                egui::RichText::new(taunt_line(tank.taunt_left, tank.cooldown, tank.can_taunt))
                    .small()
                    .color(if taunting { theme::WARN } else { theme::MUTED }),
            );
        }
        if let Some(commander) = view.commander {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(squad_line(commander.squad, commander.members))
                        .small()
                        .color(if commander.squad.is_some() {
                            theme::CAUTION
                        } else {
                            theme::MUTED
                        }),
                );
                theme::question_mark(ui, SQUAD_TIP);
            });
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
        }
        if let Some((level, left, right)) = view.pending {
            ui.label(
                egui::RichText::new(format!("{PICK_PENDING} level {level}"))
                    .small()
                    .color(theme::CAUTION),
            );
            ui.horizontal(|ui| {
                for (talent, side) in [(left, world::Side::Left), (right, world::Side::Right)] {
                    if ui.button(talent_name(talent)).clicked() {
                        self.deploy_orders.push(DeployOrder::Pick {
                            level: level as u32,
                            side,
                        });
                    }
                    theme::question_mark(ui, talent_tip(talent));
                }
            });
        }
        if !view.talents.is_empty() {
            let learnt: Vec<&str> = view.talents.iter().map(|t| talent_name(*t)).collect();
            ui.label(
                egui::RichText::new(learnt.join(" · "))
                    .small()
                    .color(theme::MUTED),
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
        let hurt = is_hurt(game, w);
        ui.horizontal(|ui| {
            if who == 0 {
                theme::asks(ui, "Health", HEALTH_TIP);
            } else {
                ui.label("Health");
            }
            let total = health::MAX_HEALTH + armour;
            theme::health_bar(
                ui,
                BAR_W,
                points / total,
                armour / total,
                if hurt { theme::BAD } else { theme::ACCENT },
                theme::ARMOUR,
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
        } else if alive && game.was_downed(w) {
            // Up again, and slower for it until the mission ends: the one
            // thing a revive leaves behind.
            ui.label(
                egui::RichText::new(slowed_note())
                    .small()
                    .color(theme::CAUTION),
            );
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
        // level, what the next wants, the pick waiting and the talents
        // learnt. Only their own: a class is a slot's.
        if who == self.player as u32
            && let Some(view) = self.class_view.clone()
        {
            self.class_section(ui, &view);
        }

        // The character sheet: two pages under the bars.
        ui.add_space(6.0);
        let diary = self.diary_open.get(w).copied().unwrap_or(false);
        ui.horizontal(|ui| {
            if theme::toggle(ui, !diary, "About").clicked() {
                self.diary_open[w] = false;
            }
            if theme::toggle(ui, diary, "Memory").clicked() {
                self.diary_open[w] = true;
            }
        });
        if !diary {
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
        } else {
            self.diary(ui, game, w);
        }
        true
    }

    /// The Bim's own account of its days: newest day first, and within a
    /// day in the order it happened. An empty page is the *good* outcome:
    /// the diary keeps only what went wrong.
    fn diary(&self, ui: &mut egui::Ui, game: &Game, who: usize) {
        let count = game.memory_len(who);
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                if count == 0 {
                    ui.label(egui::RichText::new("Nothing has gone wrong.").color(theme::MUTED));
                    return;
                }
                let mut days: Vec<(u32, Vec<(f32, String)>)> = Vec::new();
                for i in 0..count {
                    let day = game.memory_day(who, i);
                    let Some(words) =
                        memory_line(game.memory_what(who, i), game.memory_detail(who, i))
                    else {
                        continue;
                    };
                    let at = game.memory_at(who, i);
                    match days.iter_mut().find(|(d, _)| *d == day) {
                        Some((_, lines)) => lines.push((at, words)),
                        None => days.push((day, vec![(at, words)])),
                    }
                }
                days.sort_by_key(|a| std::cmp::Reverse(a.0));
                for (day, lines) in days {
                    ui.label(
                        egui::RichText::new(format!("Day {day}"))
                            .strong()
                            .color(theme::MUTED),
                    );
                    for (at, words) in lines {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(clock_text(at))
                                    .small()
                                    .color(theme::MUTED),
                            );
                            ui.label(words);
                        });
                    }
                }
            });
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
    /// give, and a commander's two that want no target — then every bot
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
        if view.commander {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(keyed(FALL_BACK, keys.key(Action::SquadFallBack)))
                    .on_hover_text(FALL_BACK_TIP)
                    .clicked()
                {
                    asks.push(TrayAsk::FallBack);
                }
                if ui
                    .button(keyed(STAND_GROUND, keys.key(Action::SquadStandGround)))
                    .on_hover_text(STAND_GROUND_TIP)
                    .clicked()
                {
                    asks.push(TrayAsk::StandGround);
                }
                theme::question_mark(ui, SQUAD_TIP);
            });
        }
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
                                if bot.hurt { theme::BAD } else { theme::ACCENT },
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
    /// is worn and what is in hand, and the talent tree the Skills tab
    /// was. Nothing on it is worked out here that the side panel or the
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
                            crate::screens::hud::xp_fill(view.class, view.xp),
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
                            theme::heading(ui, SHEET_TALENTS);
                            self.talents(ui, &view);
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
                egui::ScrollArea::horizontal()
                    .id_salt("armory-columns")
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            for column in &view.columns {
                                armory_column(ui, view, column, &mut asked);
                            }
                        });
                    });
                ui.separator();
                armory_stock(ui, view, &mut asked);
            });
        self.armory_open = open;
        self.orders.extend(asked);
        self.follow_strip(strip);
    }

    /// The Hire window, if a mercenary is open: whose, what it carries —
    /// the weapon and every piece worn, which is what the fee is — and a
    /// month's fee, with the button that sends the hire through the seam
    /// (`GearOrder::Hire`). Greyed, with the reason, while the Bim shown
    /// is out of reach (the row walked it over) or the money is short.
    /// Shut by its cross, by Escape, by the hire
    /// going through, or by the body no longer being for hire — the rooms
    /// parted. Call once a frame after the screen has set
    /// [`CrewPanels::terms`], in the Loot window's place.
    pub fn hire_window(&mut self, ctx: &egui::Context, game: &Game, name: &dyn Fn(u32) -> String) {
        let Some(resident) = self.hire_source() else {
            return;
        };
        let Some(terms) = self.terms else {
            self.open = None;
            return;
        };
        let who = self.inventory_who(game);
        let whose = name(self.crew_count + resident);
        let mut open = true;
        let mut hired = false;
        egui::Window::new(format!("{HIRE_WINDOW} — {whose}"))
            .id(egui::Id::new("hire-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::LEFT_TOP, HIRE_AT)
            .frame(crate::theme::panel_frame())
            .show(ctx, |ui| {
                // The trade first, where there is one to name (feature
                // 86): a field medic is hired for what it does and not
                // for the gun it carries, and the premium on the month
                // below is that and nothing else.
                if terms.medic {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(FIELD_MEDIC).strong().color(theme::HEAL));
                        theme::question_mark(ui, FIELD_MEDIC_TIP);
                    });
                    ui.add_space(4.0);
                }
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Carries").small().color(theme::MUTED));
                    theme::question_mark(ui, HIRE_TIP);
                });
                ui.label(weapon_name(terms.gear.weapon.map(|w| w.kind)));
                for part in health::Part::ALL {
                    if let Some(piece) = terms.gear.worn(part) {
                        ui.label(armour_name(Some(piece.kind)));
                    }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("A month").small().color(theme::MUTED));
                    ui.label(egui::RichText::new(crate::format::euros(terms.fee)).strong());
                });
                let hint = if !terms.in_reach {
                    Some(format!("{} — {REACH_HINT}", name(who as u32)))
                } else if !terms.affordable {
                    Some(BROKE_HINT.to_string())
                } else {
                    None
                };
                let can = hint.is_none();
                if ui
                    .add_enabled(can, egui::Button::new(HIRE_BUTTON))
                    .clicked()
                {
                    hired = true;
                }
                if let Some(hint) = hint {
                    ui.add(
                        egui::Label::new(egui::RichText::new(hint).small().color(theme::MUTED))
                            .wrap(),
                    );
                }
            });
        if hired {
            self.orders.push(GearOrder::Hire {
                who: who as u32,
                resident,
            });
            open = false;
        }
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.open = None;
            self.terms = None;
        }
    }

    /// The talent tree (features 83 and 107), on the character sheet: the
    /// player's own crew member's class, level by level — a level's own
    /// slot across the width where it has one, and two side by side where
    /// the level is a choice — with the levels numbered down the left and
    /// the spine beside them lit as far as the level reached. A box is
    /// coloured for its state: learnt, given up, open for a point, or
    /// waiting on a level. A click picks one, and **under** the tree the
    /// picked slot says what it does, **what it is worth in numbers**,
    /// what state it is in, and — for one that is open — carries the
    /// button that spends the point. Under rather than beside it since the
    /// sheet hangs from the top of the canvas: what is written below the
    /// tree cannot move the tree from under the pointer. The tree asks
    /// nothing of the world it is not handed: everything here is
    /// `ClassView` and `world::class`'s own tables, the numbers said by
    /// [`crate::names::talent_numbers`] and
    /// [`crate::names::level_numbers`].
    fn talents(&mut self, ui: &mut egui::Ui, view: &ClassView) {
        let class = view.class;
        if class == world::Class::None {
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
        // A ranked kit (task 124) has four abilities a rank at a time in
        // place of the tree.
        if let Some(ranks) = view.ranks {
            self.ranked_skills(ui, view, ranks);
            return;
        }
        // A point a pick level reached and not chosen at.
        let points = points_left(view);
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(skill_points(points))
                        .size(SKILL_TEXT)
                        .color(if points > 0 {
                            theme::CAUTION
                        } else {
                            theme::MUTED
                        }),
                )
                .wrap(),
            );
            theme::question_mark(ui, SKILLS_TIP);
        });
        ui.add_space(4.0);

        // Every slot of the ten levels, top to bottom, with the words
        // that go on and under it.
        let mut slots: Vec<(Slot, &'static str, &'static str)> = Vec::new();
        for level in 1..=world::class::LEVELS {
            match world::class::pick_at(class, level) {
                Some((left, right)) => {
                    slots.push((
                        Slot::Pick(level, world::Side::Left),
                        talent_name(left),
                        talent_tip(left),
                    ));
                    slots.push((
                        Slot::Pick(level, world::Side::Right),
                        talent_name(right),
                        talent_tip(right),
                    ));
                }
                None => slots.push((
                    Slot::Fixed(level),
                    level_name(class, level).unwrap_or(""),
                    level_line(class, level).unwrap_or(""),
                )),
            }
        }
        // A slot picked before the class changed under it is no slot.
        if self
            .skill_pick
            .is_some_and(|slot| !slots.iter().any(|&(s, ..)| s == slot))
        {
            self.skill_pick = None;
        }

        self.skills_tree(ui, view, &slots);
        ui.add_space(6.0);
        let learn = self.skills_detail(ui, view, &slots);
        if let Some((level, side)) = learn {
            self.deploy_orders.push(DeployOrder::Pick {
                level: level as u32,
                side,
            });
            // The point spent, the tree says so next frame; the slot is
            // let go of so the column goes back to its hint.
            self.skill_pick = None;
        }
    }

    /// The Skills tab of a ranked kit (task 124): the skill points waiting,
    /// and each of the four abilities, Q C E R, with its rank as pips,
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
        for (slot, &rank) in ranks.iter().enumerate() {
            let slot = slot as u8;
            let top = world::class::MAX_RANK;
            let next = rank + 1;
            let open = view.points > 0
                && rank < top
                && world::class::rank_level(class, slot, next).is_some_and(|l| l <= view.level);
            egui::Frame::new()
                .fill(theme::RAISED)
                .stroke(egui::Stroke::new(1.0, if open { theme::ACCENT } else { theme::LINE }))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
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
                            let at = egui::pos2(rect.min.x + 6.0 + 12.0 * f32::from(k), rect.center().y);
                            if k < rank {
                                ui.painter().circle_filled(at, 4.0, theme::CAUTION);
                            } else {
                                ui.painter().circle_stroke(at, 4.0, egui::Stroke::new(1.0, theme::MUTED));
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
                    for r in 1..=top {
                        let level = world::class::rank_level(class, slot, r).unwrap_or(1);
                        let words = rank_numbers(class, slot, r).unwrap_or_default();
                        let colour = if r <= rank {
                            theme::INK
                        } else if level <= view.level {
                            theme::CAUTION
                        } else {
                            theme::MUTED
                        };
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(rank_line(r, level, &words))
                                    .size(SKILL_BOX_TEXT)
                                    .color(colour),
                            )
                            .wrap(),
                        );
                    }
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

    /// The tree itself: the spine and its numbers down the gutter, and a
    /// box a slot, coloured for its state and lit under the pointer or
    /// where the pick is. A click sets [`CrewPanels::skill_pick`]; the
    /// column beside it reads that.
    fn skills_tree(
        &mut self,
        ui: &mut egui::Ui,
        view: &ClassView,
        slots: &[(Slot, &'static str, &'static str)],
    ) {
        let state = |slot: Slot| SkillState::of(view, slot);
        let size = skill_tree_size();
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        let painter = ui.painter_at(rect);
        let row_y = |level: u8| rect.min.y + (level as f32 - 1.0) * (SKILL_BOX_H + SKILL_GAP_Y);
        let box_of = |slot: Slot| {
            let y = row_y(slot.level());
            let left = rect.min.x + SKILL_GUTTER;
            match slot {
                Slot::Fixed(_) => egui::Rect::from_min_size(
                    egui::pos2(left, y),
                    egui::vec2(2.0 * SKILL_BOX_W + SKILL_GAP_X, SKILL_BOX_H),
                ),
                Slot::Pick(_, world::Side::Left) => egui::Rect::from_min_size(
                    egui::pos2(left, y),
                    egui::vec2(SKILL_BOX_W, SKILL_BOX_H),
                ),
                Slot::Pick(_, world::Side::Right) => egui::Rect::from_min_size(
                    egui::pos2(left + SKILL_BOX_W + SKILL_GAP_X, y),
                    egui::vec2(SKILL_BOX_W, SKILL_BOX_H),
                ),
            }
        };
        // The spine down the gutter, lit as far as the level reached,
        // with the level's number beside each knot.
        let spine = rect.min.x + SKILL_GUTTER - 8.0;
        for level in 1..=world::class::LEVELS {
            let y = row_y(level) + SKILL_BOX_H / 2.0;
            let reached = level <= view.level;
            let ink = if reached { theme::ACCENT } else { theme::LINE };
            if level < world::class::LEVELS {
                let next = row_y(level + 1) + SKILL_BOX_H / 2.0;
                let on = level < view.level;
                painter.line_segment(
                    [egui::pos2(spine, y), egui::pos2(spine, next)],
                    egui::Stroke::new(1.5, if on { theme::ACCENT } else { theme::LINE }),
                );
            }
            painter.circle_filled(egui::pos2(spine, y), 3.0, ink);
            painter.text(
                egui::pos2(rect.min.x + 9.0, y),
                egui::Align2::CENTER_CENTER,
                level.to_string(),
                egui::FontId::proportional(SKILL_BOX_TEXT),
                if reached { theme::INK } else { theme::MUTED },
            );
        }
        let hovered = response
            .hover_pos()
            .and_then(|p| slots.iter().find(|&&(s, ..)| box_of(s).contains(p)));
        if let Some(p) = response.interact_pointer_pos()
            && response.clicked()
            && let Some(&(slot, ..)) = slots.iter().find(|&&(s, ..)| box_of(s).contains(p))
        {
            self.skill_pick = Some(slot);
        }
        for &(slot, label, _) in slots {
            let b = box_of(slot);
            let how = state(slot);
            let (fill, edge, ink) = match how {
                SkillState::Learnt => (theme::RAISED_ON, theme::ACCENT, theme::INK),
                SkillState::Open => (theme::RAISED, theme::CAUTION, theme::INK),
                SkillState::GivenUp | SkillState::Locked => {
                    (theme::PANEL_DEEP, theme::LINE, theme::MUTED)
                }
            };
            let lit = hovered.map(|&(s, ..)| s) == Some(slot) || self.skill_pick == Some(slot);
            painter.rect(
                b,
                4.0,
                fill,
                egui::Stroke::new(
                    if lit { 2.0 } else { 1.0 },
                    if lit { theme::INK } else { edge },
                ),
                egui::StrokeKind::Inside,
            );
            let font = egui::FontId::proportional(SKILL_BOX_TEXT);
            let mut job =
                egui::text::LayoutJob::simple(label.to_string(), font, ink, b.width() - 10.0);
            // A slot given up is struck through: the level was chosen at,
            // and the other side of it is gone for good.
            if how == SkillState::GivenUp {
                for section in &mut job.sections {
                    section.format.strikethrough = egui::Stroke::new(1.0, ink);
                }
            }
            let galley = painter.layout_job(job);
            painter.galley(
                egui::pos2(
                    b.center().x - galley.size().x / 2.0,
                    b.center().y - galley.size().y / 2.0,
                ),
                galley,
                ink,
            );
        }
        if let Some(&(slot, label, tip)) = hovered {
            let level = slot.level();
            let numbers = skill_numbers(view.class, slot);
            response
                .clone()
                .on_hover_text(format!("Level {level} · {label}\n{tip}\n{numbers}"));
        }
    }

    /// What the picked slot is, beside the tree: its name, the level it
    /// sits at, **what it is worth in numbers**, what it does in words,
    /// what state it is in, and the button that spends the point on one
    /// that is open. Nothing is picked and it is the hint instead.
    fn skills_detail(
        &mut self,
        ui: &mut egui::Ui,
        view: &ClassView,
        slots: &[(Slot, &'static str, &'static str)],
    ) -> Option<(u8, world::Side)> {
        let picked = self
            .skill_pick
            .and_then(|pick| slots.iter().find(|&&(s, ..)| s == pick))
            .copied();
        let Some((slot, label, tip)) = picked else {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(SKILLS_HINT)
                        .size(SKILL_TEXT)
                        .color(theme::MUTED),
                )
                .wrap(),
            );
            return None;
        };
        ui.label(
            egui::RichText::new(label)
                .size(SKILL_NAME_TEXT)
                .strong()
                .color(theme::INK),
        );
        ui.label(
            egui::RichText::new(skill_slot_line(
                slot.level(),
                matches!(slot, Slot::Pick(..)),
            ))
            .size(SKILL_TEXT)
            .color(theme::MUTED),
        );
        ui.add_space(4.0);
        // The figures first: what choosing this actually changes.
        ui.add(
            egui::Label::new(
                egui::RichText::new(skill_numbers(view.class, slot))
                    .size(SKILL_TEXT)
                    .color(theme::ACCENT),
            )
            .wrap(),
        );
        ui.add_space(4.0);
        ui.add(egui::Label::new(egui::RichText::new(tip).size(SKILL_TEXT)).wrap());
        ui.add_space(4.0);
        let state = SkillState::of(view, slot);
        let words = match state {
            SkillState::Learnt => match slot {
                Slot::Fixed(_) => SKILL_COMES_WITH.to_string(),
                Slot::Pick(..) => SKILL_LEARNT.to_string(),
            },
            SkillState::GivenUp => SKILL_GIVEN_UP.to_string(),
            SkillState::Open => SKILL_OPEN.to_string(),
            SkillState::Locked => skill_locked(slot.level()),
        };
        ui.add(
            egui::Label::new(egui::RichText::new(words).size(SKILL_TEXT).color(
                if state == SkillState::Open {
                    theme::CAUTION
                } else {
                    theme::MUTED
                },
            ))
            .wrap(),
        );
        if let (SkillState::Open, Slot::Pick(level, side)) = (state, slot) {
            ui.add_space(4.0);
            if ui
                .button(egui::RichText::new(SKILL_LEARN).size(SKILL_TEXT))
                .on_hover_text(SKILL_LEARN_TIP)
                .clicked()
            {
                return Some((level, side));
            }
        }
        None
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
        "broken — does nothing".into()
    } else {
        format!(
            "+{} hp · {} prot",
            piece.health.round(),
            piece.stats().protection
        )
    }
}

/// A cell's tooltip: the name, the numbers that matter — a piece's
/// health and protection, and what it has left — and the resource's line.
pub(crate) fn tip_of(item: PackItem, count: u32) -> String {
    // A tier above one is said after the name: "Basic helm — tier 2".
    let tiered = |name: &str, tier: bims::combat::Tier| match tier_word(tier) {
        Some(word) => format!("{name} — {word}"),
        None => name.to_string(),
    };
    match item {
        PackItem::Armour(piece) => {
            let stats = piece.stats();
            let state = if piece.broken() {
                "Broken — still worn, doing nothing".to_string()
            } else {
                format!("{} of {} hp left", piece.health.round(), stats.health)
            };
            let dodge = if piece.dodge() > 0.0 {
                format!(", {}% dodge", (piece.dodge() * 100.0).round())
            } else {
                String::new()
            };
            // And what a plate or a pair of greaves does besides (task
            // 116), on the same line, so the trader's row says it too.
            let effect = armour_effect(&piece);
            format!(
                "{} — {}\n+{} hp, {} protection{dodge}{effect} · {state}\n{}",
                tiered(armour_name(Some(piece.kind)), piece.tier),
                SLOT_NAMES[piece.kind.slot() as usize].to_lowercase(),
                stats.health,
                tidy_hundredths(stats.protection),
                item_tip(world::armour::resource_of(piece.kind))
            )
        }
        PackItem::Weapon(weapon) => {
            // The curve in a line, the way the Inventory says it, or a
            // blade's swing — the tier's numbers, not the kind's.
            let stats = weapon.stats();
            let numbers = if stats.melee {
                melee_text(&stats)
            } else {
                format!(
                    "Damage {}, range {} tiles",
                    damage_text(&stats),
                    tidy(stats.range)
                )
            };
            let name = tiered(weapon_name(Some(weapon.kind)), weapon.tier);
            let name = if count > 1 {
                format!("{name} × {count}")
            } else {
                name
            };
            format!(
                "{name}\n{numbers}\n{}",
                item_tip(world::armour::weapon_resource(weapon.kind))
            )
        }
        PackItem::Stack(code) => match ResourceId::from_code(code) {
            Some(id) if count > 1 => format!("{} × {count}\n{}", resource_name(id), item_tip(id)),
            Some(id) => format!("{}\n{}", resource_name(id), item_tip(id)),
            None => "Something the hold does not know".into(),
        },
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

/// A word with the key that does the same beside it: `Attack (F)`.
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
                egui::RichText::new(health_line(points, health::MAX_HEALTH, armour)).color(
                    if is_hurt(game, w) {
                        theme::BAD
                    } else if points < health::MAX_HEALTH {
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
/// The relics its Bim holds (feature 106): each by name and tier, with
/// what it does. Here and nowhere on the deck.
fn sheet_relics(ui: &mut egui::Ui, relics: &[world::Relic]) {
    theme::heading(ui, RELICS_HEADING);
    if relics.is_empty() {
        ui.label(egui::RichText::new(NO_RELICS).small().color(theme::MUTED));
        return;
    }
    for &relic in relics {
        ui.label(
            egui::RichText::new(relic_name(relic))
                .strong()
                .color(theme::INK),
        );
        ui.add(egui::Label::new(egui::RichText::new(relic_line(relic)).small()).wrap());
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
            for (i, part) in health::Part::ALL.into_iter().enumerate() {
                ui.label(egui::RichText::new(SLOT_NAMES[i]).color(theme::MUTED));
                match gear.worn(part) {
                    Some(piece) => ui.label(worn_piece_line(
                        armour_name(Some(piece.kind)),
                        piece.tier.code(),
                        piece.health,
                        piece.stats().health,
                    )),
                    None => ui.label(egui::RichText::new(NOTHING_WORN).color(theme::MUTED)),
                };
                ui.end_row();
            }
            ui.label(egui::RichText::new(SLOT_NAMES[3]).color(theme::MUTED));
            match gear.weapon {
                Some(weapon) => ui.label(held_weapon_line(
                    weapon_name(Some(weapon.kind)),
                    weapon.tier.code(),
                )),
                None => ui.label(egui::RichText::new(NOTHING_IN_HAND).color(theme::MUTED)),
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
    if !game.is_downed(patient) {
        return Some(REVIVE_NOT_DOWN.to_string());
    }
    if !game.is_alive(who) || game.is_downed(who) || game.is_outside(who) {
        return Some(HELPER_OUT.to_string());
    }
    if game.is_outside(patient) {
        return Some(PATIENT_OUT.to_string());
    }
    if game.is_carried(patient) {
        return Some(REVIVE_CARRIED.to_string());
    }
    reviver_of(game, patient, who).map(|other| revive_taken(&name(other as u32)))
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

        let hit = game.wound(1, health::Part::Body, 1_000.0);
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

    /// The Skills tree's rules (feature 83): a point for every pick level
    /// reached and not chosen at, a slot learnt where it was chosen, the
    /// other side of it given up for good, and everything above the level
    /// reached locked. The tree draws nothing it does not read here.
    #[test]
    fn a_skills_tree_counts_its_points_and_says_what_every_slot_is() {
        // The medic's: the engineer and the soldier have ranked kits.
        let view = |level: u8, picks: &[(u8, world::Side)]| ClassView {
            class: world::Class::Medic,
            level,
            picks: picks.to_vec(),
            ..Default::default()
        };
        // Level one: the first is learnt, everything above it waits, and
        // there is nothing to spend.
        let one = view(1, &[]);
        assert_eq!(points_left(&one), 0);
        assert_eq!(SkillState::of(&one, Slot::Fixed(1)), SkillState::Learnt);
        assert_eq!(SkillState::of(&one, Slot::Fixed(3)), SkillState::Locked);
        assert_eq!(
            SkillState::of(&one, Slot::Pick(2, world::Side::Left)),
            SkillState::Locked
        );
        // Level five, nothing chosen: three pick levels behind it — two,
        // four and five — and so three points.
        let five = view(5, &[]);
        assert_eq!(points_left(&five), 3);
        assert_eq!(
            SkillState::of(&five, Slot::Pick(2, world::Side::Left)),
            SkillState::Open
        );
        assert_eq!(SkillState::of(&five, Slot::Fixed(3)), SkillState::Learnt);
        assert_eq!(
            SkillState::of(&five, Slot::Pick(6, world::Side::Left)),
            SkillState::Locked
        );
        // One spent on the left of the second: a point fewer, that side
        // learnt and the other gone.
        let spent = view(5, &[(2, world::Side::Left)]);
        assert_eq!(points_left(&spent), 2);
        assert_eq!(
            SkillState::of(&spent, Slot::Pick(2, world::Side::Left)),
            SkillState::Learnt
        );
        assert_eq!(
            SkillState::of(&spent, Slot::Pick(2, world::Side::Right)),
            SkillState::GivenUp
        );
        // The top of the tree with every level chosen at: nothing left to
        // spend. The medic's seven pick levels, which is what a
        // tenth-level run opens with.
        let all: Vec<(u8, world::Side)> = (2..=world::class::LEVELS)
            .filter(|&l| world::class::is_pick_level(world::Class::Medic, l))
            .map(|l| (l, world::Side::Right))
            .collect();
        assert_eq!(all.len(), 7);
        assert_eq!(points_left(&view(10, &[])), 7);
        assert_eq!(points_left(&view(10, &all)), 0);
        // And a crew member with no class has no levels to spend at.
        assert_eq!(
            points_left(&ClassView {
                level: 10,
                ..Default::default()
            }),
            0
        );
    }
}

/// Where the Armory panel hangs from the top of the screen, and how wide
/// it may grow before its columns scroll.
const ARMORY_TOP: f32 = 110.0;
const ARMORY_W: f32 = 980.0;
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
    pub keys: u32,
    pub locked: bool,
}

/// What is being dragged across the Armory panel: a thing out of the
/// armory, or one off a slot.
#[derive(Clone, Copy, PartialEq, Debug)]
struct ArmoryDrag(world::GearSource);

/// The slot a thing on a loadout is in, named: the weapon, or the part.
fn slot_label(slot: world::GearSlot) -> &'static str {
    match slot.part() {
        Some(part) => SLOT_NAMES[part as usize],
        None => SLOT_NAMES[3],
    }
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
            for slot in [
                world::GearSlot::Weapon,
                world::GearSlot::Head,
                world::GearSlot::Body,
                world::GearSlot::Legs,
            ] {
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
                    Some(item) => response.on_hover_text(tip_of(item, 1)),
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
        && (column.may_change || !view.locked)
        && let Some(order) = drop_on_column(view, *drag, column)
    {
        asked.push(order);
    }
}

/// The armory, the money and the keys: every thing nobody wears, a
/// picture a thing, each a drag source onto a column; the whole a drop
/// zone that takes a thing off a slot.
fn armory_stock(ui: &mut egui::Ui, view: &ArmoryView, asked: &mut Vec<GearOrder>) {
    // Whether anything may be put on or taken off at all: on the map,
    // and in a mission while a Bim that may be changed is inside the ship.
    let open = view.columns.iter().any(|c| c.may_change);
    ui.horizontal(|ui| {
        theme::heading(ui, ARMORY_STOCK);
        ui.label(
            egui::RichText::new(format!(
                "{} · {} {}",
                crate::format::euros(view.money),
                view.keys,
                if view.keys == 1 {
                    ARMORY_KEY
                } else {
                    ARMORY_KEYS
                }
            ))
            .color(theme::MUTED),
        );
    });
    let frame = egui::Frame::new()
        .inner_margin(egui::Margin::same(6))
        .corner_radius(4.0)
        .fill(theme::PANEL_DEEP);
    let (_, dropped) = ui.dnd_drop_zone::<ArmoryDrag, ()>(frame, |ui| {
        ui.set_min_width(ARMORY_W - 24.0);
        ui.set_min_height(STASH_CELL + 8.0);
        if view.armory.is_empty() {
            ui.label(
                egui::RichText::new(ARMORY_NOTHING)
                    .small()
                    .color(theme::MUTED),
            );
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
            for stored in &view.armory {
                let item = stored.item;
                let from = world::GearSource::Armory { id: stored.id };
                let tip = armory_tip(stored);
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
                let response = response.on_hover_text(tip);
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
    });
    if let Some(drag) = dropped
        && open
        && let ArmoryDrag(world::GearSource::Worn { who, slot }) = *drag
    {
        asked.push(GearOrder::Unequip { who, part: slot });
    }
}

/// A thing in the armory, said: what it is, its tier, its numbers and,
/// for a piece, what it has left.
fn armory_tip(stored: &world::Stored) -> String {
    tip_of(stored.item, 1)
}

/// A thing's name alone.
fn item_name(item: PackItem) -> &'static str {
    match item {
        PackItem::Armour(p) => armour_name(Some(p.kind)),
        PackItem::Weapon(w) => weapon_name(Some(w.kind)),
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
