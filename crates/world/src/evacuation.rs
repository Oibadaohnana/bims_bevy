//! **Evacuation** (October 2026, a mission the map shapes): a defence
//! whose fight is getting the site's people to the crew's ship.
//!
//! - **Who**: the site's own people and [`data::EVACUEES`] refugees beside
//!   them (`Residents::open`'s `evacuees`, past the bunks) — every body of
//!   the site's room that is not a defender, a grave or one of the
//!   Manufacturers'. They take no arms: the room keeps them sheltering, and
//!   the world posts them by **the flag** instead of a bunk.
//! - **The flag**: one, standing where the site's first person stands when
//!   the defence begins. `Command::Flag` (the Use key, V) takes it up from
//!   within [`data::FLAG_REACH_TILES`] — a player's Bim, never a bot — and
//!   puts it down where the Bim stands when pressed again; the carrier
//!   still shoots. Downed or unfit, the carrier drops it where it stands.
//!   Every person not yet aboard is posted a little way round it
//!   ([`World::evacuation_step`]) — round the carrier while it is carried,
//!   so they follow; where it lies when it is dropped, so they hold there.
//! - **Aboard**: a person standing on the crew's ship (its own design
//!   under it on the crew's deck) is aboard for good, and posted no more.
//! - **The waves** come on a clock, [`data::EVAC_WAVE_STEPS`] apart and
//!   stacking, until it is over; the machines go for the crew and the
//!   people alike.
//! - **Held** when every one still alive is aboard (one at the least):
//!   `TownHeld`, and the bounty waiting for the clear cut to the share of
//!   the people saved. **Failed** when none is left alive and none aboard:
//!   the defence lost, nothing paid.
//!
//! The state is the defence's (`defense::Evacuation`), saved and hashed.
//! A child of `crate::world`, as `mission.rs` is. **Nothing here draws
//! from a stream.**

use super::*;
use crate::defense::Evacuation;

/// The Evacuation's flag as the app draws it: where it stands in the
/// site's own design units, and whether it is carried; the people aboard
/// and how many there were.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvacuationLook {
    pub flag: DVec2,
    pub carried: bool,
    pub aboard: u32,
    pub total: u32,
    pub alive: u32,
}

impl World {
    /// The Evacuation at the site the crew are at, with its id.
    fn evacuation_here(&self) -> Option<(u32, &Evacuation)> {
        let id = self.ship.state.station()?;
        Some((id, self.defense(id)?.evacuation.as_ref()?))
    }

    /// The people an Evacuation is for, by body index in the site's room:
    /// alive, not a defender, a grave or one of the Manufacturers'.
    fn evacuees(&self) -> Vec<usize> {
        let Some(residents) = self.residents.as_ref() else {
            return Vec::new();
        };
        let room = &residents.aboard.room;
        (0..room.crew_count() as usize)
            .filter(|&who| {
                residents.is_own(who) && !room.is_manufacturer(who) && room.is_alive(who)
            })
            .collect()
    }

    /// Where a body of the site's room stands on the crew's deck.
    fn on_crew_deck(&self, who: usize) -> Option<bims::math::Vec2> {
        let residents = self.residents.as_ref()?;
        let p = residents.aboard.position(who as u32);
        let q = self.aboard.from_station(p)?;
        Some(bims::math::vec2(q.x as f32, q.y as f32))
    }

    /// Whether a point of the crew's deck is on the crew's ship.
    fn on_the_ship(&self, at: bims::math::Vec2) -> bool {
        let (foreign, p) = self.aboard.design_of(dvec2(at.x as f64, at.y as f64));
        if foreign {
            return false;
        }
        let t = shipdesign::TILE as f64;
        let tile = ((p.x / t).floor() as i32, (p.y / t).floor() as i32);
        self.ship
            .design
            .grid()
            .get(shipdesign::Layer::Structure, tile)
            != 0
    }

    /// The Evacuation laid as the defence `id` begins: the people counted
    /// and the flag where the first of them stands. `None` where there is
    /// nobody to bring out.
    pub(super) fn begin_evacuation(&self) -> Option<Evacuation> {
        let people = self.evacuees();
        let first = *people.first()?;
        let at = self.on_crew_deck(first)?;
        Some(Evacuation {
            total: people.len() as u32,
            aboard: Vec::new(),
            holder: None,
            at: (at.x.round() as i32, at.y.round() as i32),
        })
    }

    /// The Evacuation as the app draws it.
    pub fn evacuation_look(&self) -> Option<EvacuationLook> {
        let (_, e) = self.evacuation_here()?;
        let at = match e.holder {
            Some(who) => self.aboard.room.bim_pos(who as usize),
            None => bims::math::vec2(e.at.0 as f32, e.at.1 as f32),
        };
        let flag = self
            .aboard
            .to_station(dvec2(at.x as f64, at.y as f64))
            .unwrap_or(dvec2(0.0, 0.0));
        Some(EvacuationLook {
            flag,
            carried: e.holder.is_some(),
            aboard: e.aboard.len() as u32,
            total: e.total,
            alive: self.evacuees().len() as u32,
        })
    }

    /// What the flag asks of that player's Bim: fit to act (`OutOfReach`),
    /// an Evacuation under way and the flag its own to put down, or lying
    /// within reach to take up (`NoFlagNear`). True to take it up.
    pub fn can_flag(&self, slot: u32) -> Result<bool, Refusal> {
        if !self.fit_to_act(slot) || slot >= self.players() {
            return Err(Refusal::OutOfReach);
        }
        let Some((id, e)) = self.evacuation_here() else {
            return Err(Refusal::NoFlagNear);
        };
        if self.defense(id).is_some_and(|d| d.over()) {
            return Err(Refusal::NoFlagNear);
        }
        match e.holder {
            Some(who) if who == slot => Ok(false),
            Some(_) => Err(Refusal::NoFlagNear),
            None => {
                let me = self.aboard.room.bim_pos(slot as usize);
                let at = bims::math::vec2(e.at.0 as f32, e.at.1 as f32);
                if (at - me).len() <= data::FLAG_REACH_TILES * shipdesign::TILE as f32 {
                    Ok(true)
                } else {
                    Err(Refusal::NoFlagNear)
                }
            }
        }
    }

    /// The flag taken up, or put down where the Bim stands — see
    /// `Command::Flag`. True when it was taken up.
    pub(super) fn flag(&mut self, slot: u32) -> Result<bool, Refusal> {
        let take = self.can_flag(slot)?;
        let me = self.aboard.room.bim_pos(slot as usize);
        let Some(id) = self.ship.state.station() else {
            return Err(Refusal::NoFlagNear);
        };
        if let Some(e) = self.defense_mut(id).and_then(|d| d.evacuation.as_mut()) {
            if take {
                e.holder = Some(slot);
            } else {
                e.holder = None;
                e.at = (me.x.round() as i32, me.y.round() as i32);
            }
        }
        Ok(take)
    }

    /// The Evacuation's step, after the waves: the flag with its carrier
    /// (dropped by one down or unfit), whoever has reached the ship
    /// counted aboard, the rest posted round the flag, and the end — held
    /// or failed.
    pub(super) fn evacuation_step(&mut self, events: &mut Vec<WorldEvent>) {
        let Some((id, e)) = self.evacuation_here().map(|(id, e)| (id, e.clone())) else {
            return;
        };
        if !self.aboard.is_joined() || self.defense(id).is_some_and(|d| d.over()) {
            return;
        }
        // The flag with its carrier.
        let mut e = e;
        if let Some(who) = e.holder {
            let at = self.aboard.room.bim_pos(who as usize);
            e.at = (at.x.round() as i32, at.y.round() as i32);
            if !self.fit_to_act(who) {
                e.holder = None;
                events.push(WorldEvent::FlagCarried { who, taken: false });
            }
        }
        // Who is aboard now.
        let people = self.evacuees();
        for &who in &people {
            if e.aboard.contains(&(who as u32)) {
                continue;
            }
            if self
                .on_crew_deck(who)
                .is_some_and(|at| self.on_the_ship(at))
            {
                e.aboard.push(who as u32);
                e.aboard.sort_unstable();
                events.push(WorldEvent::Evacuated {
                    station: id,
                    aboard: e.aboard.len() as u32,
                });
            }
        }
        // The rest posted round the flag, in the site's room.
        let flag = bims::math::vec2(e.at.0 as f32, e.at.1 as f32);
        let spot_in_site = self.aboard.to_station(dvec2(flag.x as f64, flag.y as f64));
        if let (Some(spot), Some(residents)) = (spot_in_site, self.residents.as_mut()) {
            let middle = residents.aboard.to_room(spot);
            let tile = shipdesign::TILE as f32;
            for (k, &who) in people
                .iter()
                .filter(|&&who| !e.aboard.contains(&(who as u32)))
                .enumerate()
            {
                let at = middle + defense::enemy_spot(k + 1) * (tile * 1.2);
                let room = &mut residents.aboard.room;
                let far = room
                    .post_of(who)
                    .is_none_or(|post| (post - at).len() > tile * 1.5);
                if far {
                    room.post_at(who, at);
                }
            }
        }
        // The end: every one alive aboard, or none left at all.
        let alive = people.len() as u32;
        let saved = e
            .aboard
            .iter()
            .filter(|&&w| people.contains(&(w as usize)))
            .count() as u32;
        let total = e.total.max(1);
        let held = saved > 0 && saved == alive;
        let failed = alive == 0;
        if let Some(d) = self.defense_mut(id) {
            d.evacuation = Some(e);
            if held {
                d.won = true;
            } else if failed {
                d.lost = true;
            }
        }
        if held {
            // The reward the share of them saved.
            self.run.pending_bounty = self.run.pending_bounty * u64::from(saved) / u64::from(total);
            self.bonus_wave_fought();
            events.push(WorldEvent::TownHeld { station: id });
        } else if failed {
            self.run.pending_bounty = 0;
            events.push(WorldEvent::TownFell { station: id });
        }
    }

    /// The flag put in that player's Bim's hands outright, for the tests.
    pub fn give_flag_for_probe(&mut self, slot: u32) {
        let Some(id) = self.ship.state.station() else {
            return;
        };
        if let Some(e) = self.defense_mut(id).and_then(|d| d.evacuation.as_mut()) {
            e.holder = Some(slot);
        }
    }
}
