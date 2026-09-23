//! A system remembers what the crew did in it.
//!
//! Two things used to be forgotten. A station's room is dropped when the
//! ship is fifty tiles from its hull (`World::settle_residents`) and
//! opened again on the way back with its people at their bunks — every
//! one of them, the dead included — so a garrison shot to the last man
//! stood up again the moment the ship had gone a little way off. And a
//! jump replaced every part of the world that belonged to the system —
//! the hostile list, the keys on the desks, the mining sites, the
//! plundered shelves, the lamps shot out, the chart — so a jump away and
//! back was a system as the generator rolled it, the key back on the
//! desk and the rocks back in the belt.
//!
//! Now both are kept. [`Losses`] is what a station has lost to the crew:
//! how many of its own are dead and how many of the mercenaries who
//! lived there are gone — hired away, or dead — added to whenever its
//! room is closed (`World::close_residents`) and read off by
//! `World::people_of` and `World::mercenaries_of` when it is opened
//! again, so a raided station opens with the survivors and an emptied one
//! opens empty. [`SystemMemory`] is everything else a system holds that
//! the crew have changed — the per-system fields of the world, lifted
//! out as one struct — filed by star when the ship jumps out
//! (`World::remember_system`) and put back when it jumps in
//! (`World::recall_system`), so a system is met as it was left. Both are
//! in `world_checksum` whole and in a save: a crew that emptied a station
//! and one that did not are two different galaxies.
//!
//! And [`Grave`] is where each of those dead is lying (feature 85). The
//! count alone opened a station with its survivors on a deck that had
//! been swept: the fight the crew had walked away from an hour before
//! had left no mark on the place. A grave is one body — where it fell,
//! in the station design's own units, what is still on it and what it
//! looked like — filed by `World::close_residents` off the room it is
//! closing and laid back out by `Residents::open`, so the dead lie where
//! they fell for as long as the station stands. What is on the body is
//! what was left on it: a pack the crew emptied comes back empty, and a
//! body is never loot twice.
//!
//! What is *still* not remembered is the room itself — where the
//! survivors stood, what was in their hands, half an errand. A room is
//! built afresh at every open the way it always was; the memory is the
//! count, the loss and the dead, the way the hold is a count and a piece
//! of armour is only a thing while it is out of it.

use bims::character::Look;
use bims::combat::Gear;
use worldgen::Node;

use crate::mining::MiningSite;
use crate::plunder::Plunder;
use crate::world::LampDamage;

/// What one station has lost to the crew, by the station's id: its own
/// people dead, and the mercenaries who lived there gone — hired, or
/// dead. Kept sorted by `station` on `World::losses`, and by star with
/// the rest of a system in [`SystemMemory`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Losses {
    pub station: u32,
    /// Of the station's own people — the residents, or the garrison —
    /// how many are dead. Comes off `World::people_of` when the room
    /// opens again.
    pub dead: u32,
    /// Of the mercenaries for hire who lived there, how many are gone:
    /// hired onto the crew, or dead where they stood. Comes off
    /// `World::mercenaries_of`; one dismissed back ashore puts one back.
    pub mercenaries: u32,
}

impl Losses {
    /// Nothing lost yet.
    pub fn none(station: u32) -> Losses {
        Losses {
            station,
            dead: 0,
            mercenaries: 0,
        }
    }

    /// Whether there is anything to remember: an entry with nothing
    /// lost is dropped, so two worlds that have lost the same read the
    /// same whichever rooms happened to open along the way.
    pub fn is_empty(&self) -> bool {
        self.dead == 0 && self.mercenaries == 0
    }
}

/// The losses at `station` in a sorted list, or none.
pub fn losses_at(losses: &[Losses], station: u32) -> Losses {
    match losses.binary_search_by_key(&station, |l| l.station) {
        Ok(i) => losses[i],
        Err(_) => Losses::none(station),
    }
}

/// `change` applied to the losses at `station` in a sorted list: the
/// entry made if there is none, and dropped again if nothing is lost.
pub fn amend_losses(losses: &mut Vec<Losses>, station: u32, change: impl FnOnce(&mut Losses)) {
    let (i, mut entry) = match losses.binary_search_by_key(&station, |l| l.station) {
        Ok(i) => (i, losses[i]),
        Err(i) => (i, Losses::none(station)),
    };
    change(&mut entry);
    let present = losses.get(i).is_some_and(|l| l.station == station);
    match (present, entry.is_empty()) {
        (true, true) => {
            losses.remove(i);
        }
        (true, false) => losses[i] = entry,
        (false, true) => {}
        (false, false) => losses.insert(i, entry),
    }
}

/// One body lying on a station's deck (feature 85): where it fell, what
/// is still on it and what it looked like. Kept on `World::graves`,
/// sorted by `station` and in the order the bodies stood in the room
/// within one station, and by star with the rest in [`SystemMemory`].
///
/// The body itself is not kept — a `bims::bim::Bim` is a room's, and a
/// room is built afresh at every open. What is here is enough to lay one
/// out again: a corpse in the right place, in the right coverall, with
/// the gun still in its hand and whatever is left in its pack.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Grave {
    pub station: u32,
    /// Where it lies, in the station design's own world units — the
    /// frame `Residents::open` lays its room out in, which is what
    /// `crew::Aboard::position` answers in whether the rooms are joined
    /// or not.
    pub x: f64,
    pub y: f64,
    /// What is still on it: what it wore, the gun in its hand and its
    /// pack as the crew left it. Looted once is looted for good.
    pub gear: Gear,
    /// What it looked like, so the same person is lying there when the
    /// room is built again. Drawing only, and out of the checksum with
    /// the rest of what a body looks like.
    pub look: Look,
    /// Whether it was one of the mercenaries who lived there rather than
    /// one of the station's own: the coverall it is drawn in.
    pub hired: bool,
}

/// Every grave at `station` in a list sorted by station: a run of it.
pub fn graves_at(graves: &[Grave], station: u32) -> &[Grave] {
    let from = graves.partition_point(|g| g.station < station);
    let to = graves.partition_point(|g| g.station <= station);
    &graves[from..to]
}

/// The graves at `station` replaced by `laid`, the rest left alone and
/// the list still sorted by station. What `World::close_residents` does
/// with the dead of the room it is closing: the room is the whole truth
/// about that station's dead, so what it says replaces what was there.
pub fn set_graves(graves: &mut Vec<Grave>, station: u32, laid: Vec<Grave>) {
    let from = graves.partition_point(|g| g.station < station);
    let to = graves.partition_point(|g| g.station <= station);
    graves.splice(from..to, laid);
}

/// Everything a system holds that the crew have changed, as they left
/// it: the fields of the world that belong to the system rather than the
/// ship, lifted out when the ship jumps away and put back when it jumps
/// in. Kept sorted by `star` on `World::memories`.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SystemMemory {
    pub star: u32,
    /// `World::hostile`: whose people are enemies, sorted by id.
    pub hostile: Vec<u32>,
    /// `World::reinforcements`: the arena's extra garrison.
    pub reinforcements: u32,
    /// `World::station_keys`: which tier of key each desk still has.
    pub station_keys: Vec<u8>,
    /// `World::sites`: every belt's rocks as they were left.
    pub sites: Vec<MiningSite>,
    /// `World::plunder`: every enemy's shelf as it was left.
    pub plunder: Vec<Plunder>,
    /// The stations' lamps of `World::lamps` — the ship's own stay with
    /// the ship.
    pub lamps: Vec<LampDamage>,
    /// `World::discovered`: the chart of the system, sorted.
    pub discovered: Vec<Node>,
    /// `World::losses`: what each station has lost.
    pub losses: Vec<Losses>,
    /// `World::graves`: every body lying on a station of it (feature 85).
    pub graves: Vec<Grave>,
    /// `World::visited`: which of the system's nodes the ship has been
    /// at, sorted — what the map marks as somewhere the crew have been.
    pub visited: Vec<Node>,
    /// `World::infested`: which of the system's stations the machines
    /// hold and how far through their waves each is (feature 83). Per
    /// system like the rest, because a station id is a system's own: a
    /// jump that carried this list would hand the next system's station
    /// seven the last one's wave. **A station the crew cleared is in
    /// here with `cleared` set**, which is what keeps the crisis from
    /// ever re-arming it (feature 92).
    pub infested: Vec<crate::droid::Infestation>,
}

impl SystemMemory {
    /// What is left of a system's memory once the machines have it
    /// (feature 92): the chart, the rocks, the shelves and the keys
    /// stand — the crew saw those and they are still there — but the
    /// people are gone, so whose side they were on and what the crew did
    /// to them is no longer anything the crew will meet. The
    /// infestations stay: they are how a station the crew cleared stays
    /// cleared.
    pub fn overrun(&mut self) {
        self.hostile.clear();
        self.losses.clear();
        self.graves.clear();
    }
}

/// The memory of `star` in a sorted list.
pub fn memory_of(memories: &[SystemMemory], star: u32) -> Option<&SystemMemory> {
    memories
        .binary_search_by_key(&star, |m| m.star)
        .ok()
        .map(|i| &memories[i])
}

/// `memory` filed in a sorted list, replacing what was there for its star.
pub fn file_memory(memories: &mut Vec<SystemMemory>, memory: SystemMemory) {
    match memories.binary_search_by_key(&memory.star, |m| m.star) {
        Ok(i) => memories[i] = memory,
        Err(i) => memories.insert(i, memory),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn losses_are_kept_sorted_and_an_empty_entry_is_dropped() {
        let mut losses = Vec::new();
        amend_losses(&mut losses, 7, |l| l.dead += 2);
        amend_losses(&mut losses, 3, |l| l.mercenaries += 1);
        amend_losses(&mut losses, 5, |l| l.dead += 0);
        assert_eq!(
            losses,
            vec![
                Losses {
                    station: 3,
                    dead: 0,
                    mercenaries: 1
                },
                Losses {
                    station: 7,
                    dead: 2,
                    mercenaries: 0
                },
            ]
        );
        assert_eq!(losses_at(&losses, 7).dead, 2);
        assert_eq!(losses_at(&losses, 5), Losses::none(5));
        amend_losses(&mut losses, 3, |l| l.mercenaries -= 1);
        assert_eq!(losses.len(), 1, "nothing lost is no entry");
        amend_losses(&mut losses, 7, |l| l.dead += 1);
        assert_eq!(losses_at(&losses, 7).dead, 3, "losses add up");
    }

    fn grave(station: u32, x: f64) -> Grave {
        Grave {
            station,
            x,
            y: 0.0,
            gear: Gear::default(),
            look: Look::of(0),
            hired: false,
        }
    }

    #[test]
    fn a_station_s_graves_are_a_run_of_the_list_and_are_replaced_whole() {
        let mut graves = vec![grave(3, 1.0), grave(7, 2.0), grave(7, 3.0)];
        assert_eq!(graves_at(&graves, 7).len(), 2);
        assert_eq!(graves_at(&graves, 5).len(), 0);
        // The room at 7 closes with one body on its deck: one is what
        // that station has now, whatever it had before.
        set_graves(&mut graves, 7, vec![grave(7, 9.0)]);
        assert_eq!(graves.len(), 2);
        assert_eq!(graves_at(&graves, 7)[0].x, 9.0);
        // A station nobody had died at keeps the list sorted.
        set_graves(&mut graves, 5, vec![grave(5, 4.0)]);
        assert_eq!(
            graves.iter().map(|g| g.station).collect::<Vec<_>>(),
            vec![3, 5, 7]
        );
        // And one whose deck is clear drops out of it altogether.
        set_graves(&mut graves, 5, Vec::new());
        assert_eq!(
            graves.iter().map(|g| g.station).collect::<Vec<_>>(),
            vec![3, 7]
        );
    }
}
