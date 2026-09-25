//! Construction: a part of the ship laid out by the player and put
//! together by the crew, **paid for out of the crew's one pool**.
//!
//! A [`BuildSite`] is a part that is not there yet — a kind, a tile and a
//! turn, the same three numbers a placement in the design phase is. A Bim
//! walks to it, works the build time, the part goes down through
//! `shipdesign::apply` exactly as the design phase would have placed it,
//! and **the price leaves the pool at that moment**. Stage 7 of
//! [`crate::World::step`] is where all of that lands.
//!
//! # There is nothing to haul
//!
//! Before the money rework (feature 95) a site was built out of the hold:
//! the crew carried its recipe to it a load at a time, what was "carried
//! to a site" was a reservation on the hold, and the whole recipe left it
//! in one call. There are no materials any more — nothing is mined and
//! nothing is refined — so a site holds nothing, reserves nothing, and a
//! site given up costs nothing to give up because nothing was ever spent
//! on it.
//!
//! What has taken its place is the **pool**: a site is only begun while
//! the money, less the prices of the sites already begun, covers its price
//! (`World::affordable_site`), and a crew that cannot afford one sees
//! [`crate::Refusal::NotEnoughMoney`] and the site waits. Money is the one
//! thing that may be spent away from a station — see the contract in
//! `shipdesign::materials`.
//!
//! The ship is never under way while it is built on: nothing flies it
//! (feature 104), and a trip is resolved between missions.

use economy::Money;
use shipdesign::parts::{PartKind, Rotation};
use shipdesign::{Edit, ShipDesign, site_price};

/// One part waiting to be built.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BuildSite {
    /// Its identity. Ids only ever climb and are never reissued, so a
    /// command naming a site that has since been built or cancelled is
    /// refused rather than landing on another.
    pub id: u32,
    pub kind: PartKind,
    pub origin: (u32, u32),
    pub rotation: Rotation,
}

impl BuildSite {
    pub fn new(id: u32, kind: PartKind, origin: (u32, u32), rotation: Rotation) -> BuildSite {
        BuildSite {
            id,
            kind,
            origin,
            rotation,
        }
    }

    /// The edit that puts the part down: deck plating lays its own frame,
    /// the way the designer's plating tool does.
    pub fn edit(&self) -> Edit {
        match self.kind {
            PartKind::Floor => Edit::Plate {
                origin: self.origin,
            },
            kind => Edit::Place {
                kind,
                origin: self.origin,
                rotation: self.rotation,
            },
        }
    }

    /// What it costs, against `design` — plating a bare tile costs the
    /// frame as well as the deck.
    pub fn price(&self, design: &ShipDesign) -> Money {
        site_price(design, self.edit())
    }

    /// Every tile the part will cover.
    pub fn tiles(&self) -> Vec<(u32, u32)> {
        shipdesign::parts::covered(self.kind, self.rotation)
            .into_iter()
            .map(|(dx, dy)| (self.origin.0 + dx, self.origin.1 + dy))
            .collect()
    }
}

/// How long putting a part together takes, in game minutes: a base plus a
/// share per **hundred euros** of the part's price, so a wall is a few
/// minutes and a heavy engine an afternoon. It was per unit of materials
/// until the money rework (feature 95); the price stands in for the bulk
/// of a part the way its recipe did. Placeholder, like every other number
/// aboard.
pub fn build_minutes(price: Money) -> f64 {
    let hundreds = price as f64 / 100.0;
    crate::data::BUILD_MINUTES_BASE + crate::data::BUILD_MINUTES_PER_HUNDRED * hundreds
}

/// Why a site may not be laid out where it was asked for. What
/// [`crate::World::can_place_site`] answers, for a blueprint to be drawn
/// red before anything is sent; the command itself comes back as a
/// [`crate::Refusal`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SiteRefusal {
    /// The rules would not put the part there: a `shipdesign::EditError`
    /// code.
    WontFit(u32),
    /// The part would go, and the ship would then have a fault it has not
    /// got now: a `shipdesign::IssueCode` code.
    Fault(u32),
    /// Nothing is built onto the ship in a run (feature 102): the shipyard
    /// is switched off (`World::shipyard_enabled`).
    NoShipyard,
}
