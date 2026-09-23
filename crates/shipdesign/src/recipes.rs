//! What the crew still make, and out of what.
//!
//! One table, [`RECIPES`], and since the money rework (feature 95) **one
//! row in it**: two vegetables into a medkit at the drug lab. Everything
//! else a crew ever made is bought now — `economy`, and a station's desk —
//! and the workbench makes nothing at all: what it does is **combine** two
//! of a kind at one tier into one of the next, which is the world's
//! (`world::World::upgrade`) and not a recipe.
//!
//! The shape of a recipe is unchanged: a Bim stands at the row's
//! [`station`](Recipe::station) for its [`minutes`](Recipe::minutes), the
//! inputs come out of the hold and the output goes in. The chain is the
//! room's (`bims::task::Kind::Craft`) and the hold is the world's, so
//! **this crate does neither** — it says what a recipe is, and
//! `crates/world` moves the cargo when the room says a Bim has finished
//! one. A native server will move the same cargo off the same table.
//!
//! # Mass
//!
//! Crafting **conserves mass**: an output weighs exactly what went into
//! it, and `every_recipe_holds_together` in the tests holds the resource
//! table to that — a medkit is two vegetables, so a medkit weighs two
//! vegetables. There is no exception left: the smelter vented slag and the
//! smelter is gone.
//!
//! # A recipe is data, a job is a station
//!
//! There is no chain per product. A new thing to make is one row here, and
//! the row says which station; the work list has one job per station
//! (`bims::work::Job::Craft` is all of them today). A target per output —
//! "keep twenty medkits" — is the world's, set by the player, and it is
//! what turns a row into an errand: a recipe is on offer while the hold has
//! fewer of its output than the target, the inputs for one, room for the
//! output, a bench of its station aboard, and that station powered.
//!
//! # What a made thing is worth
//!
//! Whatever `economy::trade_price` says, which is a hand-written number
//! like every other book value now. There was a rule — inputs plus labour,
//! `made_book` and `LABOUR_BP_PER_HOUR` — and it went with the production
//! chains it was written to keep honest: with one recipe left there is no
//! chain to print money along.

use physics::ResourceId;

use crate::parts::PartKind;

/// One thing the crew can make.
#[derive(Clone, Copy, Debug)]
pub struct Recipe {
    /// Which part a Bim stands at to make it. Has to draw power — a
    /// workstation that ran in a brownout would be one the reactor was
    /// not needed for.
    pub station: PartKind,
    /// Units of each resource taken out of the hold. Never empty, and no
    /// resource twice.
    pub inputs: &'static [(ResourceId, u32)],
    /// What goes in: one resource, so many units.
    pub output: (ResourceId, u32),
    /// How long a Bim stands at the bench, in game minutes.
    pub minutes: u32,
}

/// The table. Indexed by position, and the index is what crosses the wasm
/// boundary — `ship_recipe_*` — so a recipe is appended, never inserted.
pub static RECIPES: [Recipe; 1] = [
    // The one thing left: medicine at the drug lab, made from the first
    // day. It kept the quarter of an hour it always had, and its inputs
    // are the two vegetables alone now that there is no component to put
    // in it — which is why a medkit weighs two vegetables.
    Recipe {
        station: PartKind::DrugLab,
        inputs: &[(ResourceId::Vegetable, 2)],
        output: (ResourceId::Medkit, 1),
        minutes: 15,
    },
];

impl Recipe {
    /// What the inputs weigh, all together.
    pub fn input_mass(&self) -> f64 {
        self.inputs
            .iter()
            .map(|&(id, units)| units as f64 * id.mass_per_unit())
            .sum()
    }

    /// What the output weighs.
    pub fn output_mass(&self) -> f64 {
        self.output.1 as f64 * self.output.0.mass_per_unit()
    }
}

/// Whether the table holds together: every recipe on a station that draws,
/// with something in, one thing out that is not also in, no input twice,
/// every count above nought, and the mass rule — what comes out weighs
/// exactly what went in.
pub fn recipes_are_sound() -> bool {
    RECIPES.iter().all(|r| {
        let station = r.station.def().draws();
        let inputs = !r.inputs.is_empty()
            && r.inputs.iter().enumerate().all(|(i, &(id, units))| {
                units > 0 && id != r.output.0 && !r.inputs[..i].iter().any(|&(seen, _)| seen == id)
            });
        let output = r.output.1 > 0;
        let mass = (r.output_mass() - r.input_mass()).abs() < 1e-9;
        station && inputs && output && r.minutes > 0 && mass
    })
}

/// Whether a part is a **workstation**: somewhere a Bim stands and
/// works. Every part a recipe is made at, and the two that are worked at
/// for other reasons since the money rework (feature 95) — the
/// **workbench**, where two of a kind are combined into one of the next
/// and a piece of armour is repaired, and the **armoury**, which is the
/// gun cabinet a crew fetch from and stow into. `bims::aboard` builds the
/// room's benches off this, and a bench is what makes a cabinet a
/// container the crew can reach into.
pub fn is_workstation(kind: PartKind) -> bool {
    at(kind).next().is_some() || matches!(kind, PartKind::Workbench | PartKind::Armoury)
}

/// Every recipe made at `station`, by index into [`RECIPES`].
pub fn at(station: PartKind) -> impl Iterator<Item = (usize, &'static Recipe)> {
    RECIPES
        .iter()
        .enumerate()
        .filter(move |(_, r)| r.station == station)
}
