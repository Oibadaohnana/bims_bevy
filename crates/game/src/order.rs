//! What a player asks of the room, as a value: one order, carried whole.
//!
//! Every way the mouse and the menus reach into the simulation is one of
//! these — a click on the deck, a right-click to walk somewhere, a row of
//! a menu, a box on the Management tab — and [`Game::order`] is
//! the one door they go through. That is what makes the room playable by
//! several people at once: an order is a number that can be written down,
//! sent down a wire and applied on every player's copy of the room in the
//! same order, so the copies stay one room. Nothing the app does to the
//! room *changes* it any other way; what it asks (`hit_at`, `is_selected`,
//! `bim_pos`) it may ask freely, since asking changes nothing.
//!
//! An order carries **who gave it** — the player's slot — because two
//! players do not share a selection: each has their own, and "send whoever
//! I have selected over there" means whoever *that* player has selected.
//! `Character::selected` is a mask, one bit a player, for the same reason.
//!
//! Positions are the room's own units, as `f32` — the same numbers the
//! pointer is turned into, so an order applied on another machine lands on
//! the same tile.

use crate::door;
use crate::game::{Game, ORDER_IGNORED};
use crate::health::Part;
use crate::math::vec2;
use crate::room::Switch;
use crate::task::{Kind, Saved};

/// One thing a player asked of the room. See the module note.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CrewOrder {
    /// A press and a release on the deck, from one corner to the other: a
    /// click when they are the same spot, a marquee when they are not.
    /// Picks whoever is under it for the player who gave it — a click on
    /// a fixture picks nobody and leaves the selection alone.
    Select {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    },
    /// Select the player's own crew member, and nobody else.
    SelectOwn,
    /// Recruit the player's own crew member, or let it go again.
    Recruit,
    /// A right-click on the deck: whoever the player has selected and
    /// takes orders goes there.
    Move {
        x: f32,
        y: f32,
    },
    /// A right-drag on the deck: the selected crew form a line from one
    /// end to the other.
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    },
    /// Walk crew member `who` to a spot, whoever is selected: what a
    /// container's window, a body's Loot row or the desk asks.
    SendTo {
        who: u32,
        x: f32,
        y: f32,
    },
    /// Take the post off crew member `who`, so it goes back about its
    /// errands.
    StandDown {
        who: u32,
    },
    /// Pick up the weapon lying on the deck by its id, into `who`'s pack.
    PickUp {
        who: u32,
        item: u32,
    },
    /// Dress every open wound on that part of `patient`.
    Bandage {
        who: u32,
        patient: u32,
        part: Part,
    },
    /// Dress **every** open wound on `patient`, out of `who`'s own pack
    /// (feature 87): the worst part now and the rest queued behind it.
    /// What the pop-up on a box of dressings sends.
    BandageAll {
        who: u32,
        patient: u32,
    },
    /// Treat the trauma on that part of `patient` with a medkit.
    Treat {
        who: u32,
        patient: u32,
        part: Part,
    },
    /// A powered door's switch, walked to.
    Door {
        who: u32,
        door: u32,
        order: door::Order,
    },
    /// Turn job `job`'s priority one notch, forwards or back.
    WorkPriority {
        job: u32,
        back: bool,
    },
    Autonomous {
        on: bool,
    },
}

impl CrewOrder {
    /// The crew member this order is an errand for — a walk, a row of a
    /// fixture's menu, a dressing — or `None` for an order that is not an
    /// errand: a selection, a recruit, a box on the Management tab. A
    /// move and a line go to whoever is selected and are not named here.
    /// What can be given with Shift, to wait its turn
    /// ([`Game::order_later`]), and what given plain drops the queue.
    pub fn errand_for(self) -> Option<u32> {
        match self {
            CrewOrder::SendTo { who, .. }
            | CrewOrder::PickUp { who, .. }
            | CrewOrder::Bandage { who, .. }
            | CrewOrder::BandageAll { who, .. }
            | CrewOrder::Treat { who, .. }
            | CrewOrder::Door { who, .. } => Some(who),
            CrewOrder::Select { .. }
            | CrewOrder::SelectOwn
            | CrewOrder::Recruit
            | CrewOrder::Move { .. }
            | CrewOrder::Line { .. }
            | CrewOrder::StandDown { .. }
            | CrewOrder::WorkPriority { .. }
            | CrewOrder::Autonomous { .. } => None,
        }
    }
}

impl Game {
    /// Carry out one order from player `slot`. The code is the room's
    /// answer: [`ORDER_IGNORED`] and the `ORDER_` codes for a walk — the
    /// app has words for the two that are refusals — and nought for
    /// everything else, whose refusals show on the deck rather than in a
    /// line. Given plain — without Shift, which is [`Game::order_later`]
    /// — an errand is the end of whatever was queued with Shift for that
    /// crew member: the queue is called off by any order given without
    /// the key, RimWorld's rule.
    pub fn order(&mut self, slot: u32, order: CrewOrder) -> u32 {
        let who = |w: u32| w as usize;
        if let Some(w) = order.errand_for()
            && who(w) < self.crew_count() as usize
        {
            self.drop_ordered(who(w));
            // An order to an errand ends a medic's beam (feature 76); a
            // walk does not — a medic may walk while it holds one.
            if !matches!(order, CrewOrder::SendTo { .. }) {
                self.set_beaming(who(w), false);
            }
        }
        match order {
            CrewOrder::Select { x0, y0, x1, y1 } => {
                self.drag_begin(x0, y0);
                self.drag_end(slot, x1, y1);
                0
            }
            CrewOrder::SelectOwn => {
                self.select_group(slot, 1);
                0
            }
            CrewOrder::Recruit => {
                self.toggle_recruited(slot);
                0
            }
            CrewOrder::Move { x, y } => self.order_move(slot, x, y),
            CrewOrder::Line { x0, y0, x1, y1 } => self.order_line(slot, vec2(x0, y0), vec2(x1, y1)),
            CrewOrder::SendTo { who: w, x, y } => {
                if who(w) < self.crew_count() as usize {
                    self.send_to(who(w), vec2(x, y));
                }
                0
            }
            CrewOrder::StandDown { who: w } => {
                self.stand_down(who(w));
                0
            }
            CrewOrder::PickUp { who: w, item } => {
                if who(w) < self.crew_count() as usize {
                    self.fetch(who(w), item);
                }
                0
            }
            CrewOrder::Bandage {
                who: w,
                patient,
                part,
            } => {
                if who(w) < self.crew_count() as usize && who(patient) < self.crew_count() as usize
                {
                    self.bandage(who(w), who(patient), part);
                }
                0
            }
            CrewOrder::BandageAll { who: w, patient } => {
                if who(w) < self.crew_count() as usize && who(patient) < self.crew_count() as usize
                {
                    self.bandage_all(who(w), who(patient));
                }
                0
            }
            CrewOrder::Treat {
                who: w,
                patient,
                part,
            } => {
                if who(w) < self.crew_count() as usize && who(patient) < self.crew_count() as usize
                {
                    self.treat(who(w), who(patient), part);
                }
                0
            }
            CrewOrder::Door {
                who: w,
                door,
                order,
            } => {
                if who(w) < self.crew_count() as usize {
                    self.order_door(who(w), door as usize, order);
                }
                0
            }
            CrewOrder::WorkPriority { job, back } => {
                if back {
                    self.cycle_work_priority_back(job);
                } else {
                    self.cycle_work_priority(job);
                }
                0
            }
            CrewOrder::Autonomous { on } => {
                self.set_autonomous(on);
                0
            }
        }
    }

    /// [`Game::order`] with Shift held: the errand goes on the *back* of
    /// the Bim's queue and waits its turn behind what it is on and
    /// whatever was queued before it, rather than displacing it (feature
    /// 69). A walk goes on as a [`Kind::Walk`] and is given the way a
    /// right-click gives one when its turn comes; any other errand is
    /// begun the way its row would have begun it, and dropped then if it
    /// cannot be — see [`Game::pump_queue`]. What is not an errand at all
    /// — a selection, a box on the Management tab — is done now, Shift
    /// or no Shift, through [`Game::order`]. A plain order afterwards
    /// drops what was queued (`drop_ordered`). The code is `order`'s: a
    /// walk with no way there is `ORDER_NOWHERE` with a cross on the deck,
    /// one queued `ORDER_MOVING` with a ping.
    pub fn order_later(&mut self, slot: u32, order: CrewOrder) -> u32 {
        let crew = self.crew_count() as usize;
        let who = |w: u32| w as usize;
        let (w, kind, minutes) = match order {
            CrewOrder::Move { x, y } => {
                let Some((squad, spots)) = self.huddle(slot, vec2(x, y)) else {
                    return ORDER_IGNORED;
                };
                return self.queue_squad(&squad, &spots);
            }
            CrewOrder::Line { x0, y0, x1, y1 } => {
                let Some((squad, spots)) = self.formation(slot, vec2(x0, y0), vec2(x1, y1)) else {
                    return ORDER_IGNORED;
                };
                return self.queue_squad(&squad, &spots);
            }
            CrewOrder::SendTo { who: w, x, y } => {
                if who(w) >= crew {
                    return ORDER_IGNORED;
                }
                return self.queue_walk(who(w), vec2(x, y), true);
            }
            CrewOrder::PickUp { who: w, item } => (w, Kind::Fetch { item }, 0.0),
            CrewOrder::Bandage {
                who: w,
                patient,
                part,
            } => (
                w,
                Kind::Bandage {
                    patient: who(patient),
                    part: part.code(),
                },
                0.0,
            ),
            // A whole body queues one dressing a part itself, so with
            // Shift it is the plain order given when its turn comes.
            CrewOrder::BandageAll { .. } => return self.order(slot, order),
            CrewOrder::Treat {
                who: w,
                patient,
                part,
            } => (
                w,
                Kind::Treat {
                    patient: who(patient),
                    part: part.code(),
                    bare: false,
                },
                0.0,
            ),
            CrewOrder::Door {
                who: w,
                door,
                order,
            } => (w, Kind::Switch(Switch::Door(door as usize, order)), 0.0),
            CrewOrder::Select { .. }
            | CrewOrder::SelectOwn
            | CrewOrder::Recruit
            | CrewOrder::StandDown { .. }
            | CrewOrder::WorkPriority { .. }
            | CrewOrder::Autonomous { .. } => return self.order(slot, order),
        };
        if who(w) < crew {
            self.queue_order(Saved::ordered(who(w), kind, minutes, None));
        }
        0
    }
}

/// Whether a walk's code is a refusal worth a line: what `order_move` comes
/// back with when there is no way there.
pub fn walk_refused(code: u32) -> bool {
    code == crate::game::ORDER_NOWHERE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Game, ORDER_MOVING};
    use crate::room::{ROOM_H, ROOM_W, TILE};

    fn room() -> Game {
        Game::bare(7, ROOM_W, ROOM_H)
    }

    #[test]
    fn an_order_is_the_call_it_stands_for_and_a_selection_is_one_player_s() {
        let mut game = room();
        game.set_autonomous(false);
        game.set_players(2);
        let james = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));
        let kate = game.put_for_probe(1, vec2(ROOM_W * 0.5, ROOM_H * 0.5));

        // Two players, two selections: a sweep by player 1 over both picks
        // both for player 1 and nobody for player 0.
        game.order(
            1,
            CrewOrder::Select {
                x0: james.x - TILE,
                y0: james.y - TILE,
                x1: kate.x + TILE,
                y1: kate.y + TILE,
            },
        );
        assert_eq!(game.selected_all(1), vec![0, 1]);
        assert_eq!(game.selected_all(0), Vec::<usize>::new());
        game.order(0, CrewOrder::SelectOwn);
        assert_eq!(game.selected_all(0), vec![0]);
        assert_eq!(game.selected_all(1), vec![0, 1], "player 1's is untouched");

        // In peace a move by player 1 goes to Kate — their own — and never
        // to James, whoever they have selected; player 0's goes to James.
        let a = vec2(ROOM_W * 0.35, ROOM_H * 0.8);
        assert_eq!(
            game.order(1, CrewOrder::Move { x: a.x, y: a.y }),
            ORDER_MOVING
        );
        assert!((game.destination_for_probe(1).unwrap() - a).len() < TILE);
        assert!(
            game.destination_for_probe(0)
                .is_none_or(|d| (d - a).len() > TILE),
            "James is player 0's"
        );
        let b = vec2(ROOM_W * 0.6, ROOM_H * 0.8);
        assert_eq!(
            game.order(0, CrewOrder::Move { x: b.x, y: b.y }),
            ORDER_MOVING
        );
        assert!((game.destination_for_probe(0).unwrap() - b).len() < TILE);

        // Recruiting is the player's own, and reads back by slot.
        game.order(1, CrewOrder::Recruit);
        assert!(game.is_recruited(1) && !game.is_recruited(0));
        game.order(1, CrewOrder::Recruit);
        assert!(!game.is_recruited(1));

        // A box on the Management tab, and a walk by name.
        game.order(0, CrewOrder::Autonomous { on: true });
        assert!(game.is_autonomous());
        let c = vec2(ROOM_W * 0.4, ROOM_H * 0.3);
        game.order(
            1,
            CrewOrder::SendTo {
                who: 1,
                x: c.x,
                y: c.y,
            },
        );
        assert!(game.post_of(1).is_some());
        game.order(1, CrewOrder::StandDown { who: 1 });
        assert!(game.post_of(1).is_none());
        // Nobody: nothing, and no panic.
        game.order(
            0,
            CrewOrder::SendTo {
                who: 9,
                x: c.x,
                y: c.y,
            },
        );
        assert!(!walk_refused(ORDER_MOVING) && walk_refused(crate::game::ORDER_NOWHERE));
    }

    /// Feature 69: an order given with Shift waits its turn. Two walks
    /// queued go one after the other, and are read off the deck
    /// (`queued_walks`) until they are walked; an errand queued behind them
    /// is begun the way its row begins one once the Bim is there — and
    /// dropped, not mimed, when there is nothing for it by then; a plain
    /// order calls the whole queue off.
    #[test]
    fn a_shift_order_waits_its_turn_and_a_plain_one_calls_the_queue_off() {
        use crate::game::{JOB_FETCH, JOB_WALK};
        const DT: f32 = 1.0 / 60.0;
        let mut game = room();
        game.set_autonomous(false);
        let start = game.put_for_probe(0, vec2(ROOM_W * 0.3, ROOM_H * 0.5));

        // First, an errand queued where there is nothing for it — no such
        // weapon on the deck — is dropped when its turn comes, the way the
        // row would have refused it, not walked through with empty hands.
        game.order_later(0, CrewOrder::PickUp { who: 0, item: 99 });
        assert_eq!(game.ordered_count(0), 1);
        game.simulate(DT);
        assert_eq!(game.ordered_count(0), 0, "nothing in the pot: dropped");
        assert_eq!(game.activity(0), 0);
        game.order(0, CrewOrder::SelectOwn);

        // Two walks with Shift: the first is on at once — there was
        // nothing to wait behind — and the second waits on the queue.
        let a = vec2(ROOM_W * 0.3, ROOM_H * 0.8);
        let b = vec2(ROOM_W * 0.6, ROOM_H * 0.8);
        assert_eq!(
            game.order_later(0, CrewOrder::Move { x: a.x, y: a.y }),
            ORDER_MOVING
        );
        assert_eq!(
            game.order_later(0, CrewOrder::Move { x: b.x, y: b.y }),
            ORDER_MOVING
        );
        let walks = game.queued_walks(0);
        assert_eq!(walks.len(), 2, "both read off the deck: {walks:?}");
        assert!((walks[0] - a).len() < TILE && (walks[1] - b).len() < TILE);
        assert_eq!(game.agenda_job(0, 0), JOB_WALK);
        // And a weapon to pick up behind them, lying back where the Bim
        // started.
        let gun = game.drop_for_probe(start, crate::combat::WeaponKind::LaserPistol.basic(), 1);
        game.order_later(0, CrewOrder::PickUp { who: 0, item: gun });
        assert_eq!(game.agenda_len(0), 3);
        assert_eq!(game.ordered_count(0), 3);

        // The first walk is taken up the next step; the second stays.
        game.simulate(DT);
        assert!((game.destination_for_probe(0).unwrap() - a).len() < TILE);
        assert_eq!(game.queued_walks(0).len(), 1);
        // Walked there, the second is given.
        let mut steps = 0;
        while !game.arrived_for_probe(0) && steps < 3000 {
            game.simulate(DT);
            steps += 1;
        }
        for _ in 0..3 {
            game.simulate(DT);
        }
        assert!(
            (game.destination_for_probe(0).unwrap() - b).len() < TILE,
            "on to the second: {:?}",
            game.destination_for_probe(0)
        );
        assert!(game.queued_walks(0).is_empty());
        // And there, the fetch is begun the way the row begins one.
        let mut steps = 0;
        while !game.arrived_for_probe(0) && steps < 3000 {
            game.simulate(DT);
            steps += 1;
        }
        for _ in 0..3 {
            game.simulate(DT);
        }
        assert_eq!(game.activity(0), JOB_FETCH, "off for it, once it got there");
        assert_eq!(game.ordered_count(0), 0);

        // A plain order is the end of what was queued: the fetch is put
        // down to be picked up (it is the Bim's own work now), the queued
        // walk behind it is not.
        game.order_later(0, CrewOrder::Move { x: a.x, y: a.y });
        assert_eq!(game.ordered_count(0), 1);
        assert_eq!(
            game.order(
                0,
                CrewOrder::Move {
                    x: start.x,
                    y: start.y
                }
            ),
            ORDER_MOVING
        );
        assert_eq!(game.ordered_count(0), 0, "the plain walk called it off");
        assert_eq!(game.agenda_len(0), 1, "the fetch waits to be picked up");
        assert!(game.queued_walks(0).is_empty());
        // What is not an errand is done now, Shift or no.
        game.order_later(0, CrewOrder::Autonomous { on: true });
        assert!(game.is_autonomous());
    }
}
