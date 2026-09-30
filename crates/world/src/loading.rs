//! How far a site's building has got, for the app's loading bar.
//!
//! Opening a run or making a trip is seconds of work, nearly all of it
//! three rooms built when the ship ties up at a site (`World::join_rooms`):
//! the station's people's own room, the joined deck and the people's copy
//! of it. Measured in a release build, a room of the station's costs about
//! its build area squared over two hundred tenths of a second — 0.7 s at
//! a station, 2.7 s at the Machine Heart's fortress, 4.2 s at a planet's
//! town — and the joined deck about 0.4 s wherever it is. So a site's
//! rooms are counted in [`units`] of about a tenth of a second each.
//!
//! The app runs that work on a thread of its own and hands the thread a
//! [`Meter`] ([`watch`]); the world announces each site's units as it
//! starts on it ([`site`]) and each room's as it starts and finishes it
//! ([`begin`], [`end`]). The app reads the meter from its own thread.
//!
//! Nothing here is a rule: a thread with no meter (every test, the
//! server, the probes) counts into nothing, and nothing is read back into
//! the world.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use shipdesign::ShipDesign;

/// Where the building has got: a snapshot of a [`Meter`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    /// Sites begun so far — the one under way included.
    pub sites: u32,
    /// The site under way's units, all of them.
    pub total: u32,
    /// Its units done.
    pub done: u32,
    /// The units of the room being built now, nought between rooms.
    pub doing: u32,
}

impl Reading {
    /// The site under way's done over total, 0 to 1.
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.done as f32 / self.total as f32).min(1.0)
    }
}

/// Shared between the building thread and whoever watches it.
#[derive(Debug, Default)]
pub struct Meter(Mutex<Reading>);

impl Meter {
    /// Where it has got.
    pub fn read(&self) -> Reading {
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn with(&self, f: impl FnOnce(&mut Reading)) {
        f(&mut self.0.lock().unwrap_or_else(|e| e.into_inner()));
    }
}

thread_local! {
    static METER: RefCell<Option<Arc<Meter>>> = const { RefCell::new(None) };
}

/// Count this thread's building into `meter` from now on.
pub fn watch(meter: Arc<Meter>) {
    METER.with(|m| *m.borrow_mut() = Some(meter));
}

fn on(f: impl FnOnce(&mut Reading)) {
    METER.with(|m| {
        if let Some(meter) = m.borrow().as_ref() {
            meter.with(f);
        }
    });
}

/// The units a room laid on `design` is counted at: its build area
/// squared over two hundred, at least one.
pub fn units(design: &ShipDesign) -> u32 {
    let side = design.build_area;
    (side.saturating_mul(side) / 200).max(1)
}

/// The units of the joined deck, whatever the site (it measured the same
/// at a station, a town and the fortress).
pub const DECK_UNITS: u32 = 4;

/// A site begun: `total` units coming.
pub(crate) fn site(total: u32) {
    on(|r| {
        *r = Reading {
            sites: r.sites + 1,
            total,
            done: 0,
            doing: 0,
        };
    });
}

/// A room of `units` begun.
pub(crate) fn begin(units: u32) {
    on(|r| r.doing = units);
}

/// The room begun, done.
pub(crate) fn end() {
    on(|r| {
        r.done = (r.done + r.doing).min(r.total);
        r.doing = 0;
    });
}

/// `units` done without being begun: a room there was no need to build.
pub(crate) fn skip(units: u32) {
    on(|r| r.done = (r.done + units).min(r.total));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_watched_thread_counts_and_another_does_not() {
        let meter = Arc::new(Meter::default());
        let seen = meter.clone();
        std::thread::spawn(move || {
            watch(meter);
            site(10);
            begin(4);
            end();
            skip(3);
            begin(3);
        })
        .join()
        .unwrap();
        let r = seen.read();
        assert_eq!(
            r,
            Reading {
                sites: 1,
                total: 10,
                done: 7,
                doing: 3
            }
        );
        assert!((r.fraction() - 0.7).abs() < 1e-6);
        // This thread has no meter: nothing happens.
        site(5);
        end();
        assert_eq!(seen.read(), r);
    }
}
