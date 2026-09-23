//! What a ship weighs, and what building on one costs.
//!
//! Every part has a [`mass`](crate::parts::PartDef::mass) and a
//! [`price`](crate::parts::PartDef::price), and the two are independent.
//! Before the money rework (feature 95) a part had a **recipe** instead of a
//! mass: it was built out of materials in the hold, it weighed what went into
//! it, and construction was a *move* rather than a purchase. There are no
//! materials any more — nothing is mined and nothing is refined — so this
//! module is the contract that replaced it.
//!
//! # The contract
//!
//! - **What a ship weighs** is the sum of [`crate::parts::part_mass`] over
//!   its parts, plus what is in the hold, plus the crew aboard. Nothing is
//!   in transit weighing nothing: there is no hauling, and a construction
//!   site holds no materials because there are none to hold.
//! - **Construction is paid in money**, out of the crew's one pool, and the
//!   price leaves the pool the moment the part goes down. The ship gets
//!   *heavier* by the part's mass when it does, which is the difference from
//!   the old rule and is deliberate: a part is bought, brought and bolted on.
//! - **Deconstruction returns the part's full price** to the pool the moment
//!   the part comes off, and the ship gets lighter by its mass. **There is no
//!   loss.** Scrapping at a penalty is a design decision nobody has taken,
//!   and a rule that quietly ate a tenth of every wall would be discovered as
//!   a crew that mysteriously go broke rebuilding.
//! - **Total mass changes through four things and no others**: trading while
//!   docked, food eaten or grown, crew joining or leaving, and construction
//!   or deconstruction. Nothing is burnt in flight — the engines run on the
//!   reactor. Anything else that changes it is a bug, and this list is the
//!   thing to check it against.
//! - **Money buys goods only while docked, and a part anywhere.** A desk is a
//!   place, so the shops want a station; a construction site is not, so a
//!   part is paid for docked, holding or landed alike. That is the one
//!   exception, and it is what makes building away from a station possible at
//!   all now that there is nothing in the hold to build out of.
//!
//! # Where the mass is, which is not the same question
//!
//! **The centre of mass is not conserved**, and neither is the inertia about
//! it: a part is at its tile centres, and the cargo and the crew are at the
//! centre of mass itself for want of anywhere better to put them. So bolting
//! an engine to the stern moves the centre of mass aft. Nothing reads that
//! yet — [`crate::mass`] is a single figure — and it is written down here so
//! that whatever does read it knows the total was the only thing promised.
//!
//! # What is not here
//!
//! Loose items on the deck, and scrap. [`site_price`] is the **rule** for
//! what a site costs, not the mechanism: the construction step in `world`
//! drives it with a Bim and a site in the middle of it — a site is only begun
//! while the pool, less the prices of the sites already begun, covers its
//! price (`World::affordable_site`), and the price leaves the pool in the one
//! call that puts the part down.

use economy::Money;

use crate::design::{Edit, ShipDesign};
use crate::parts::PartKind;

/// What laying `edit` out costs, in euros: the part's price for a
/// placement, and for deck plating the deck's price plus the frame's when
/// the tile has no frame yet — [`Edit::Plate`] lays both. Nought for
/// anything that is not construction.
///
/// The world reads this to know what a site will cost before it is begun
/// and what to take out of the pool when the part goes down, so the two
/// cannot disagree about what a wall is worth. It is the same shape
/// `recipe_for` had before the money rework, with a price where the
/// recipe was.
pub fn site_price(design: &ShipDesign, edit: Edit) -> Money {
    match edit {
        Edit::Place { kind, .. } => kind.def().price,
        Edit::Plate { origin } => {
            let tile = (origin.0 as i32, origin.1 as i32);
            let mut price = PartKind::Floor.def().price;
            if !design.grid().has_structure(tile) {
                price += PartKind::Structure.def().price;
            }
            price
        }
        Edit::Remove { .. } | Edit::Buy { .. } | Edit::Sell { .. } => 0,
    }
}

/// What taking `part_id` off gives back: its full price, or nought if
/// there is no such part. The other half of [`site_price`], and the
/// reason there is no loss written into either.
pub fn refund_for(design: &ShipDesign, part_id: u32) -> Money {
    design
        .part(part_id)
        .map(|p| p.kind.def().price)
        .unwrap_or(0)
}
