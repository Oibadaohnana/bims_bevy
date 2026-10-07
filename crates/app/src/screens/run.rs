//! The run's plumbing: the session every screen of a run shares, the
//! orders the players give it and the seam they go through, and the run's
//! opening from the lobby's settings.
//!
//! This was the ship designer's module until the designer went (October
//! 2026); what the game still needed of it is here.
//!
//! **Everything that changes the world goes through [`Net`].** It is a
//! local stand-in with a transport's shape: no click handler sends a
//! `Command` to the world directly, every order goes through
//! [`Net::order`], and with company the same call goes over the wire.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use bims::order::CrewOrder;
use ship::Session;
use shipdesign::parts::{PartKind, Rotation};
use world::Speed;
use world::world::Command;

use crate::canvas::root_ui;
use crate::crew::GearOrder;
use crate::net::{Packet, Wire};
use crate::{Screen, theme};
use wire::To;

use super::builder::Settings;

/// Longest a frame may pretend to be, so a window that was hidden does not
/// come back and pan the view across the room.
pub const MAX_FRAME_DT: f32 = 0.1;

/// The session: the game, shared by every screen of a run.
#[derive(Resource)]
pub struct ShipSession(pub Session);

/// Why there is nowhere to start, for the lost screen.
#[derive(Resource)]
pub struct Lost(pub String);

// --- the seam, and the wire behind it ------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Order {
    Speed(Speed),
    /// A thing off the shelf of the trader the crew are at (task 114),
    /// onto a Bim's loadout or into the armory with `None` —
    /// `Command::BuyShelf`.
    BuyShelf {
        index: u32,
        to: Option<u32>,
    },
    /// A thing sold back at a trader (October 2026) — `Command::Sell`.
    Sell {
        from: world::GearSource,
    },
    /// An item at the trader (October 2026), bought onto the player's own
    /// Bim or the one it carries upgraded a tier — `Command::BuyItem`.
    BuyItem {
        kind: u32,
    },
    /// The item in the player's own Bim's item slot used at a room point
    /// (October 2026) — `Command::UseItem`.
    UseItem {
        item: u32,
        x: i32,
        y: i32,
    },
    /// Lay out a part to be built, at a design tile, turned so.
    Build {
        kind: PartKind,
        x: u32,
        y: u32,
        rotation: Rotation,
    },
    /// Call a site off.
    Cancel {
        site: u32,
    },
    /// Somebody's gear moved between missions (task 113): a thing on or
    /// off a slot, offered or taken. The loadouts and the armory are the
    /// world's, so even putting a helm on goes through the seam — every
    /// player's ship has to agree about where each thing is.
    Gear(GearOrder),
    /// An order to the crew's room — a click on the deck, a walk, a row of
    /// a fixture's menu, a Management box — `Command::Crew`. The crew's
    /// positions are in the checksum, so nothing reaches the room but
    /// through here (`bims::order`).
    Crew(CrewOrder),
    /// The same, given with Shift held: it waits its turn on the crew
    /// member's queue rather than displacing what it is on —
    /// `Command::CrewLater` (feature 69).
    CrewLater(CrewOrder),
    /// The player's class chosen or changed while playing, until the
    /// first undock — `Command::SetClass` (feature 74).
    SetClass(world::Class),
    /// The engineer sent to lay a mine or a Healing Sentry on a room
    /// tile — `Command::Deploy`, the Q and C keys over the deck.
    Deploy {
        kind: world::DeployKind,
        x: i32,
        y: i32,
    },
    /// The engineer's sentry laid on a room tile — `Command::Sentry`, the
    /// R key over the deck (task 127).
    Sentry {
        x: i32,
        y: i32,
    },
    /// The engineer's remote trigger: every satchel of his set off —
    /// `Command::Detonate`, the G key (task 154; Space until October 2026).
    Detonate,
    /// The way in the player's Bim stands at welded shut —
    /// `Command::Weld`, the V key (October 2026).
    Weld,
    /// A Sabotage's charge planted — `Command::Plant`, the V key.
    Plant,
    /// What the second set of attacks has within reach — a terminal, a
    /// cell door, a drum or a crate — `Command::Interact`, the V key.
    Interact,
    /// An Evacuation's flag taken up or put down — `Command::Flag`, the V
    /// key.
    Flag,
    /// A mine or a Healing Sentry taken back up, the charge back —
    /// `Command::PackUp`.
    PackUp(u32),
    /// The soldier's Stun Shot charged at a room tile —
    /// `Command::StunShot`, the E key (October 2026).
    StunShot {
        x: i32,
        y: i32,
    },
    /// The soldier's grenade thrown at a room tile — `Command::Throw`,
    /// the Q key over the deck.
    Throw {
        x: i32,
        y: i32,
    },
    /// A grenade, or with `satchel` an engineer's satchel charge, thrown
    /// at a room tile the Bim walks out to reach where it must —
    /// `Command::ThrowAt`, the click after the armed Q (or the engineer's
    /// E) key, or the key let go.
    ThrowAt {
        satchel: bool,
        x: i32,
        y: i32,
    },
    /// The soldier's Rampage — `Command::Rampage`, the R key (task 124).
    Rampage,
    /// A rank of the player's own ranked kit bought — `Command::RankUp`,
    /// Ctrl and a slot's key or a Ctrl-click on its box (task 124).
    RankUp {
        ability_slot: u32,
    },
    /// The medic's heal beam linked to a crew member, or unlinked —
    /// `Command::Beam`, the E key over one (feature 76).
    Beam(Option<u32>),
    /// The medic's Heal Drone — `Command::HealDrone`, the Q key (task
    /// 153).
    HealDrone,
    /// The medic's Healing Circle switched on or off —
    /// `Command::HealingCircle`, the R key (task 153).
    HealingCircle(bool),
    /// The tank's Riot Shield raised or put down — `Command::RiotShield`,
    /// the Q key (task 155).
    RiotShield(bool),
    /// The tank's Reflect Barrier — `Command::Reflect`, the E key.
    Reflect,
    /// The tank's Bastion — `Command::Bastion`, the R key.
    Bastion,
    /// The commander's rally — `Command::Rally`, the E key (task 129).
    Rally,
    /// The commander's Battle Cry — `Command::BattleCry`, the Q key (task
    /// 129).
    BattleCry,
    /// The commander's Reinforcements — `Command::Reinforce`, the R key.
    Reinforce,
    /// The commander's Medivac — `Command::Medivac`, the C key.
    Medivac,
    /// Every player's own standing order to the bots that follow them —
    /// `Command::Orders`, the F and T keys (feature 84).
    Orders(world::Standing),
    /// A medic taking a crewmate up into its arms, or setting one
    /// down with `None` — `Command::Carry`, the G key (feature 86).
    Carry(Option<u32>),
    /// A destination put to the crew on the world map, between missions
    /// — `Command::Propose` (feature 103).
    Propose {
        star: u32,
        station: u32,
    },
    /// A yes to the destination on the table, or one taken back —
    /// `Command::Accept`.
    AcceptTrip(bool),
    /// The *Back to ship* button — `Command::Return`.
    ReturnToShip,
    /// The bonus wave chosen in the ready check, or taken back (October
    /// 2026) — `Command::BonusWave`.
    BonusWave(bool),
    /// An answer to the departure check — `Command::LeaveBehind`.
    LeaveBehind(bool),
    /// *Ready* for a mission held for the ready check, or taken back —
    /// `Command::Ready`.
    Ready(bool),
    /// A relic put to the crew, or none — the reward screen's vote,
    /// `Command::ProposeRelic` (feature 106).
    ProposeRelic {
        relic: Option<world::Relic>,
    },
    /// A yes to the relic on the table, or one taken back —
    /// `Command::AcceptRelic`.
    AcceptRelic(bool),
    /// The host saying that player has left the game —
    /// `Command::PlayerGone`, carrying the gone player's slot rather than
    /// the sender's.
    PlayerGone(u32),
}

/// The seam. [`Net::order`] is the client half and [`Net::receive`] the
/// other end, which applies orders **in arrival order**.
///
/// With a `wire` (feature 59, `crate::net`) the two halves are on two
/// machines: the **host** receives — its own orders at once, a guest's
/// as they arrive — and tells everybody what went (`Packet::Applied`),
/// which every guest then receives in that order; a **guest** posts an
/// ask and applies nothing itself until the host says so — unless it is
/// guessing (task 156, `crate::rollback`), when an order is played on its
/// own world at once and asked with the step it was played at. Without
/// one the two halves are the two calls one after the other.
pub struct Net {
    pub slot: u32,
    pub players: u32,
    /// The wire, while the game is played with company. `None` is a game
    /// of one — or a guest whose host has gone, which is the same thing
    /// from then on.
    pub wire: Option<Wire>,
    /// A guest's guesses, shared with the game screen's
    /// `crate::rollback::Rollback`: `None` where nothing is guessed — a
    /// trip's thread.
    pub ledger: Option<crate::rollback::Shared>,
}

impl Net {
    /// Post an order. The host applies it at once and tells the room; a
    /// guest asks the host and applies it when told — or, guessing, plays
    /// it on its own world first and asks with the step it played it at
    /// (`crate::rollback::predict`).
    ///
    /// The host tells the room **before** it applies: a trip's order
    /// builds the next site where it is applied — seconds — so told
    /// after, the guests began building theirs only once the host's was
    /// done; told first, every end builds at once. The order on the wire
    /// is the same.
    pub fn order(&self, session: &mut Session, order: Order) {
        match &self.wire {
            Some(wire) if !wire.host => {
                let stamp = self
                    .ledger
                    .as_ref()
                    .and_then(|l| crate::rollback::predict(l, session, self.slot, order));
                wire.send(To::Host, &Packet::Ask { order, stamp });
            }
            _ => {
                self.tell(self.slot, order, None);
                Self::receive(session, self.slot, order);
            }
        }
    }

    /// The host telling the room what went.
    fn tell(&self, from: u32, order: Order, asked: Option<crate::rollback::Asked>) {
        if let Some(wire) = &self.wire {
            wire.send(To::All, &Packet::Applied { from, order, asked });
        }
    }

    /// A guest's ask, arrived at the host: applied in arrival order — a
    /// stamped one at its step (`crate::rollback::Held`) — and told to the
    /// room, with what `asked` says of the stamp. Nothing on a guest, whose
    /// asks go to the host.
    pub fn asked(
        &self,
        session: &mut Session,
        from: u32,
        order: Order,
        asked: Option<crate::rollback::Asked>,
    ) {
        if !self.wire.as_ref().is_some_and(|w| w.host) {
            return;
        }
        self.tell(from, order, asked);
        Self::receive(session, from, order);
    }

    /// The host said this went: applied here the same way, in the same
    /// order. What a guest's world is made of.
    pub fn applied(&self, session: &mut Session, from: u32, order: Order) {
        if self.wire.as_ref().is_some_and(|w| !w.host) {
            Self::receive(session, from, order);
        }
    }

    /// Whether this end is the clock: a game of one, or the host.
    pub fn is_clock(&self) -> bool {
        self.wire.as_ref().is_none_or(|w| w.host)
    }

    /// An order applied here and told to nobody — the one thing a guest
    /// must never do, done on purpose: `BIMS_DESYNC_AT` (`dev.rs`) parts
    /// a guest's world from the host's with it, for the resync to mend
    /// (feature 67). Nothing else calls it.
    pub fn apply_unasked(&self, session: &mut Session, order: Order) {
        Self::receive(session, self.slot, order);
    }

    /// The other end of the wire: the order queued on the world as player
    /// `from`'s command. Nothing is reordered, and nothing refused here —
    /// a command the world will not take says so as a `WorldEvent`.
    pub(crate) fn receive(session: &mut Session, from: u32, order: Order) {
        // Queued, never applied: a command lands at a step, the same step
        // for everybody.
        let Some(game) = &mut session.game else {
            return;
        };
        let slot = from;
        game.send(match order {
            Order::Speed(speed) => Command::SetSpeed { slot, speed },
            Order::BuyShelf { index, to } => Command::BuyShelf { slot, index, to },
            Order::Sell { from } => Command::Sell { slot, from },
            Order::BuyItem { kind } => Command::BuyItem { slot, kind },
            Order::UseItem { item, x, y } => Command::UseItem { slot, item, x, y },
            Order::Build {
                kind,
                x,
                y,
                rotation,
            } => Command::PlaceSite {
                slot,
                kind,
                origin: (x, y),
                rotation,
            },
            Order::Cancel { site } => Command::CancelSite { slot, site },
            Order::Gear(GearOrder::Equip { who, from }) => Command::Equip { slot, who, from },
            Order::Gear(GearOrder::EquipAt { who, from, at }) => Command::EquipAt {
                slot,
                who,
                from,
                at,
            },
            Order::Gear(GearOrder::Unequip { who, part }) => Command::Unequip { slot, who, part },
            Order::Gear(GearOrder::Offer { part, to }) => Command::Offer { slot, part, to },
            Order::Gear(GearOrder::AnswerOffer { from, part, yes }) => Command::AnswerOffer {
                slot,
                from,
                part,
                yes,
            },
            Order::Crew(order) => Command::Crew { slot, order },
            Order::CrewLater(order) => Command::CrewLater { slot, order },
            Order::SetClass(class) => Command::SetClass { slot, class },
            Order::Deploy { kind, x, y } => Command::Deploy { slot, kind, x, y },
            Order::Sentry { x, y } => Command::Sentry { slot, tile: (x, y) },
            Order::Detonate => Command::Detonate { slot },
            Order::Weld => Command::Weld { slot },
            Order::Plant => Command::Plant { slot },
            Order::Interact => Command::Interact { slot },
            Order::Flag => Command::Flag { slot },
            Order::PackUp(id) => Command::PackUp { slot, id },
            Order::StunShot { x, y } => Command::StunShot { slot, x, y },
            Order::Throw { x, y } => Command::Throw { slot, x, y },
            Order::ThrowAt { satchel, x, y } => Command::ThrowAt {
                slot,
                satchel,
                x,
                y,
            },
            Order::Rampage => Command::Rampage { slot },
            Order::RankUp { ability_slot } => Command::RankUp { slot, ability_slot },
            Order::Beam(patient) => Command::Beam { slot, patient },
            Order::HealDrone => Command::HealDrone { slot },
            Order::HealingCircle(on) => Command::HealingCircle { slot, on },
            Order::RiotShield(on) => Command::RiotShield { slot, on },
            Order::Reflect => Command::Reflect { slot },
            Order::Bastion => Command::Bastion { slot },
            Order::Rally => Command::Rally { slot },
            Order::BattleCry => Command::BattleCry { slot },
            Order::Reinforce => Command::Reinforce { slot },
            Order::Medivac => Command::Medivac { slot },
            Order::Orders(order) => Command::Orders { slot, order },
            Order::Carry(who) => Command::Carry { slot, who },
            Order::Propose { star, station } => Command::Propose {
                slot,
                star,
                station,
            },
            Order::AcceptTrip(yes) => Command::Accept { slot, yes },
            Order::ReturnToShip => Command::Return { slot },
            Order::BonusWave(on) => Command::BonusWave { slot, on },
            Order::LeaveBehind(yes) => Command::LeaveBehind { slot, yes },
            Order::Ready(yes) => Command::Ready { slot, yes },
            Order::ProposeRelic { relic } => Command::ProposeRelic {
                slot,
                relic: relic.map_or(u32::MAX, world::Relic::code),
            },
            Order::AcceptRelic(yes) => Command::AcceptRelic { slot, yes },
            Order::PlayerGone(gone) => Command::PlayerGone { slot: gone },
        });
    }
}

// --- the run's opening ----------------------------------------------------------

/// A run from the lobby's settings (feature 102): what Start does. The
/// session opens straight onto the world — the default ship docked at the station the
/// lobby picked, `money_per_bim` a player's Bim in the pool
/// (`Session::run`) — with the names, the hair, the colours and the
/// classes the lobby dealt. Every machine
/// of a lobby calls this with the same numbers, the host at its own Start
/// and every guest at the host's, so the worlds are one world.
///
/// Seconds of work, so the builder runs it on a thread of its own behind
/// the loading screen (`screens::loading::Loading::start_run`), and
/// [`open_run`] hands the session on. Nothing of Bevy's in it.
pub fn build_run(s: &Settings, size: Vec2) -> Session {
    // The `end` command's run has its ten bots aboard from the start.
    let bots = if s.end { crate::dev::END_BOTS } else { 0 };
    let mut session = Session::run_with_bots(
        s.money_per_bim,
        s.players,
        bots,
        s.slot,
        s.seed,
        s.galaxy,
        s.spawn,
        &s.classes,
        size.x,
        size.y,
    );
    session.crew_names = s.names.clone();
    crate::names::set_crew_names(&session.crew_names);
    session.crew_hair = s.hair.clone();
    session.crew_tints = s.tints.clone();
    session.dress_crew();
    // And the difficulty the host picked, on every machine the same.
    if let Some(game) = &mut session.game {
        game.world.set_difficulty(s.difficulty);
        // And the ascension, the same.
        game.world.set_ascension(s.ascension);
        // And the host's rewards, dealt with company (`wavecfg::Dealt`).
        if let Some(rewards) = s.rewards {
            game.world.set_rewards(rewards);
        }
    }
    // The `end` command's (`crate::Launch::End`): at the Machine Heart,
    // after the difficulty so its waves are the set scaling's, the bots
    // classed at the top in tier-three kit — on every machine alike.
    if s.end && !session.end_for_probe(s.end_day) {
        eprintln!("end: the Machine Heart's fortress could not be laid");
    }
    // And the relic dials, on the `game` command as on any other.
    crate::dev::relic_dials(&mut session);
    // And the setup's Dev tab, on every machine alike.
    super::devtab::apply(&s.dev, &mut session);
    // The ready check: every mission with a fight in it — this first one
    // too — waits for every player's *Ready* (`BIMS_READY=0` says not).
    if let Some(game) = &mut session.game {
        game.world
            .set_ready_check(crate::dev::ready_check().unwrap_or(true));
    }
    session
}

/// The run's other half, on Bevy's thread: the session [`build_run`]
/// built handed to the game — the screen to go to handed back — or the
/// lost screen when there is nowhere to start.
pub fn open_run(commands: &mut Commands, s: &Settings, session: Session) -> Screen {
    let ok = session.spawn_ok() && session.game.is_some();
    if ok {
        commands.insert_resource(ShipSession(session));
        return Screen::Game;
    }
    let lost = match s.spawn {
        None => "The lobby did not say which station to start at.".into(),
        Some((star, station)) => {
            format!("There is no station {station} at star {star} in this galaxy.")
        }
    };
    commands.insert_resource(Lost(lost));
    Screen::Lost
}

pub struct RunPlugin;

impl Plugin for RunPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(EguiPrimaryContextPass, lost.run_if(in_state(Screen::Lost)));
    }
}

/// Nowhere to start. Shown instead of the game when the lobby named no
/// station, or one the galaxy has not got. The way back is the
/// whole of it: the lobby is where a start is chosen.
fn lost(
    mut contexts: EguiContexts,
    lost: Res<Lost>,
    mut next: ResMut<NextState<Screen>>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let mut root = root_ui(&ctx);
    egui::CentralPanel::default().show(&mut root, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.3);
            ui.label(egui::RichText::new("Nowhere to start").size(28.0).strong());
            ui.label(egui::RichText::new(&lost.0).color(theme::MUTED));
            ui.add_space(12.0);
            if ui.link("Back to the lobby").clicked() {
                next.set(Screen::Menu);
            }
        });
    });
    Ok(())
}
