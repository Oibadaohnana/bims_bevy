//! The guest's playout buffer (task 148): the host's steps held back a
//! moment and played out at the world's own pace, so a connection that
//! brings them in clumps does not make the guest's world stop and lurch.
//!
//! # Why it is needed
//!
//! A guest's world is the host's, step for step (`net.rs`): the host says
//! how many steps went by, every frame, and a guest applied them the
//! moment they arrived. Over a steady line that is one step a frame and
//! smooth; over a shaky one — Wi-Fi, a phone's hotspot, a busy link — the
//! packets arrive in bunches, nothing for a tenth of a second and then six
//! frames' worth at once, and the guest's world froze and jumped with them.
//! The wire is TCP under a websocket, so nothing is ever *lost*: what goes
//! wrong is only *when* it arrives. A small buffer is the cure.
//!
//! # What it does
//!
//! Everything on a guest's timeline — the host's `Steps`, its `Applied`
//! orders and a whole `World` — goes into one queue in the order it came,
//! and leaves it in that order: the order is what keeps the copies equal,
//! and nothing here changes it. An order or a world leaves as soon as it
//! is at the front. Steps leave at the world's rate (`rate`, steps a
//! second — the host's own clock at 1×, 2×, …), so a guest at the same
//! refresh as the host plays one step a frame however they arrived.
//!
//! The buffer aims to hold [`Playout::target`] seconds of steps. Starting
//! at [`MIN_TARGET`], it grows by however long the queue ran dry while a
//! step was owed — a gap of 120 ms makes the next one 120 ms deeper — up
//! to the player's cap (the Esc sheet's *Network buffer*, kept beside the
//! keys). It comes back down by what it did not need: every [`QUIET`]
//! seconds without a gap, half of the least it held over them, less a
//! [`BAND`], is let go. The depth, followed over [`SMOOTH`] seconds, is
//! held at the target by playing a little slower ([`SLOWEST`]) or faster
//! ([`FASTEST`]) when it is off by more than [`BAND`]; more than [`SNAP`]
//! behind — after a trip, or a long outage — it catches up at once.
//!
//! The cost is latency: a guest sees the world, and its own orders come
//! back, `target` later. A steady line keeps it at a frame or two; the cap
//! is how much a player will trade for smoothness, and nought is off —
//! every packet applied as it lands, as before.
//!
//! Only the *when* of the steps is the buffer's: no step is skipped or
//! added, and the checksum is compared after the last step of the packet
//! it came with, so the copies stay what they were.

use std::collections::VecDeque;

use crate::net::{Event, Packet};

/// The least the buffer holds, in seconds: two frames at 60 Hz, which is
/// what a steady line's packets wander by against the guest's frames.
pub const MIN_TARGET: f64 = 0.033;
/// How far the depth may sit from the target, in seconds, before the
/// pace changes: a frame and a half, so a packet landing either side of
/// a frame's edge changes nothing.
pub const BAND: f64 = 0.025;
/// The pace's give: this many seconds off the target, past the band,
/// is twice the pace (before the clamps below).
pub const CATCH: f64 = 0.5;
/// The slowest the buffer plays while it fills, as a share of the pace.
pub const SLOWEST: f64 = 0.92;
/// The fastest it plays while it drains.
pub const FASTEST: f64 = 1.5;
/// Seconds past the target at which the buffer stops easing and catches
/// up at once.
pub const SNAP: f64 = 1.5;
/// How long the depth the pace is steered by takes to follow the queue's,
/// in seconds: a clumpy line fills the queue in a sawtooth, a clump at a
/// time, and the pace is not to chase each tooth — a step dropped before
/// every clump and two played after it would be the lurch again, smaller.
pub const SMOOTH: f64 = 1.0;
/// Seconds without a gap over which the buffer's spare is measured and
/// half of it let go.
pub const QUIET: f64 = 10.0;

/// One thing on the timeline: the host's steps, counted, or anything else
/// that keeps its place among them.
#[derive(Debug)]
enum Entry {
    Steps { n: u32, checksum: Option<u64> },
    Other(Box<Event>),
}

/// The buffer, kept by the game screen on a guest. Empty and idle on the
/// host and in a game of one.
#[derive(Debug)]
pub struct Playout {
    queue: VecDeque<Entry>,
    /// Steps in the queue.
    buffered: u64,
    /// Steps owed to the world and not yet played, a fraction of one
    /// carried from frame to frame.
    owed: f64,
    /// The depth aimed at, in seconds.
    target: f64,
    /// The queue's depth followed over [`SMOOTH`] seconds: what the pace
    /// is steered by.
    smooth: f64,
    /// The cap the last frame was played under, in seconds.
    cap: f64,
    /// How long the queue has been dry with a step owed, this gap.
    starved: f64,
    /// Seconds since the last gap, or since spare was last let go.
    quiet: f64,
    /// The least the queue held after a frame's share over those seconds.
    low: f64,
    /// How the frames went, for `BIMS_AUTO`'s printout and the tests.
    pub tally: Tally,
}

/// How many frames played no step while the world ran, one, two, or three
/// or more, and how many gaps there were — what a smoke run reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub frames: [u32; 4],
    pub gaps: u32,
}

impl Default for Playout {
    fn default() -> Playout {
        Playout {
            queue: VecDeque::new(),
            buffered: 0,
            owed: 0.0,
            target: MIN_TARGET,
            smooth: 0.0,
            cap: 0.0,
            starved: 0.0,
            quiet: 0.0,
            low: f64::INFINITY,
            tally: Tally::default(),
        }
    }
}

impl Playout {
    /// Whether an event belongs on the timeline: the host's steps, its
    /// applied orders and its world. Everything else — a refusal, the
    /// roster, a resync asked — is nobody's turn and goes straight on.
    pub fn holds(event: &Event) -> bool {
        matches!(
            event,
            Event::Packet {
                packet: Packet::Steps { .. } | Packet::Applied { .. } | Packet::World { .. },
                ..
            }
        )
    }

    /// Put an arrival at the back of the queue.
    pub fn push(&mut self, event: Event) {
        match event {
            Event::Packet {
                packet: Packet::Steps { n, checksum },
                ..
            } => {
                // The end of a gap: the next one is met that much deeper.
                if self.starved > 0.0 {
                    self.target = (self.target + self.starved).min(self.cap.max(MIN_TARGET));
                    self.starved = 0.0;
                    self.quiet = 0.0;
                    self.low = f64::INFINITY;
                    self.tally.gaps += 1;
                }
                self.buffered += u64::from(n);
                self.queue.push_back(Entry::Steps { n, checksum });
            }
            other => self.queue.push_back(Entry::Other(Box::new(other))),
        }
    }

    /// Seconds of steps in the queue at `rate` steps a second.
    pub fn depth(&self, rate: f64) -> f64 {
        if rate > 0.0 {
            self.buffered as f64 / rate
        } else {
            0.0
        }
    }

    /// The depth aimed at, in seconds.
    pub fn target(&self) -> f64 {
        self.target
    }

    /// Everything, at once and in order: the buffer off, or the host gone
    /// and the clock this end's from here.
    pub fn flush(&mut self) -> Vec<Event> {
        self.buffered = 0;
        self.owed = 0.0;
        self.starved = 0.0;
        self.queue.drain(..).map(Entry::into_event).collect()
    }

    /// This frame's share of the queue, in order: `dt` seconds of the
    /// window's clock, the world playing `rate` steps a second — nought
    /// while it is paused — and the buffer allowed to hold `cap` seconds,
    /// nought being off. `nominal` is the 1× rate, for steps queued
    /// behind a world this end thinks paused, which should not happen
    /// but must not stop the queue for good if it does.
    pub fn release(&mut self, dt: f64, rate: f64, nominal: f64, cap: f64) -> Vec<Event> {
        self.cap = cap.max(0.0);
        if self.cap <= 0.0 {
            self.target = 0.0;
            let out = self.flush();
            self.count(&out, rate > 0.0);
            return out;
        }
        self.target = self.target.clamp(MIN_TARGET.min(self.cap), self.cap);
        let running = rate > 0.0;
        let pace = if running {
            rate
        } else if matches!(self.queue.front(), Some(Entry::Steps { .. })) {
            nominal
        } else {
            0.0
        };
        if pace > 0.0 {
            let error = self.depth(pace) - self.target;
            if error > SNAP {
                self.owed += (error * pace).floor();
                self.smooth = self.target;
            }
            self.smooth += (self.depth(pace) - self.smooth) * (1.0 - (-dt / SMOOTH).exp());
            let error = self.smooth - self.target;
            let k = if error > BAND {
                (1.0 + (error - BAND) / CATCH).min(FASTEST)
            } else if error < -BAND {
                (1.0 + (error + BAND) / CATCH).max(SLOWEST)
            } else {
                1.0
            };
            self.owed += dt * pace * k;
        }
        let mut out = Vec::new();
        while let Some(front) = self.queue.front_mut() {
            match front {
                Entry::Other(_) => {
                    out.extend(self.queue.pop_front().map(Entry::into_event));
                }
                Entry::Steps { n, .. } => {
                    let can = self.owed.max(0.0).floor().min(f64::from(*n)) as u32;
                    if can == 0 {
                        break;
                    }
                    self.owed -= f64::from(can);
                    self.buffered -= u64::from(can);
                    if can == *n {
                        out.extend(self.queue.pop_front().map(Entry::into_event));
                    } else {
                        // Part of a packet: its checksum waits for the
                        // step it was taken after.
                        *n -= can;
                        out.push(
                            Entry::Steps {
                                n: can,
                                checksum: None,
                            }
                            .into_event(),
                        );
                        break;
                    }
                }
            }
        }
        // A step owed and none to play: a gap, measured until steps come.
        // The debt is let go rather than paid in a lurch when they do;
        // the depth catches up instead.
        if pace > 0.0 && self.queue.is_empty() && self.owed >= 1.0 {
            self.starved += dt;
            self.owed = self.owed.fract();
        }
        // What the buffer held and never needed comes off it, half at a
        // time, so a line that has settled is not paid for in latency.
        if self.starved == 0.0 && pace > 0.0 {
            self.quiet += dt;
            self.low = self.low.min(self.depth(pace));
            if self.quiet > QUIET {
                let spare = self.low - BAND;
                if spare > 0.0 {
                    self.target = (self.target - spare / 2.0).max(MIN_TARGET.min(self.cap));
                }
                self.quiet = 0.0;
                self.low = f64::INFINITY;
            }
        }
        self.count(&out, running);
        out
    }

    /// One frame into the tally, if the world ran.
    fn count(&mut self, out: &[Event], running: bool) {
        if !running {
            return;
        }
        let steps: u32 = out
            .iter()
            .map(|e| match e {
                Event::Packet {
                    packet: Packet::Steps { n, .. },
                    ..
                } => *n,
                _ => 0,
            })
            .sum();
        self.tally.frames[(steps as usize).min(3)] += 1;
    }
}

impl Entry {
    fn into_event(self) -> Event {
        match self {
            // Who sent it is not kept: steps come from the host alone,
            // and the screen does not ask.
            Entry::Steps { n, checksum } => Event::Packet {
                from: 0,
                packet: Packet::Steps { n, checksum },
            },
            Entry::Other(event) => *event,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 60.0;
    const FRAME: f64 = 1.0 / 60.0;

    fn steps(n: u32, checksum: Option<u64>) -> Event {
        Event::Packet {
            from: 7,
            packet: Packet::Steps { n, checksum },
        }
    }

    fn stepped(out: &[Event]) -> u32 {
        out.iter()
            .map(|e| match e {
                Event::Packet {
                    packet: Packet::Steps { n, .. },
                    ..
                } => *n,
                _ => 0,
            })
            .sum()
    }

    /// The host's steps, one a frame for `frames` frames, arriving in
    /// clumps: held back `delay(frame)` frames each, in order. What every
    /// guest frame played.
    fn run(frames: u32, cap: f64, delay: impl Fn(u32) -> u32) -> (Vec<u32>, Playout) {
        let mut playout = Playout::default();
        let mut arrivals: Vec<(u32, u32)> = Vec::new();
        let mut last = 0;
        for sent in 0..frames {
            // TCP keeps the order: nothing overtakes what went before.
            last = (sent + delay(sent)).max(last);
            arrivals.push((last, sent));
        }
        let mut played = Vec::new();
        for frame in 0..frames + 120 {
            for _ in arrivals.iter().filter(|(at, _)| *at == frame) {
                playout.push(steps(1, None));
            }
            played.push(stepped(&playout.release(FRAME, RATE, RATE, cap)));
        }
        (played, playout)
    }

    #[test]
    fn a_steady_line_plays_a_step_a_frame_two_frames_behind_at_most() {
        let (played, playout) = run(600, 0.5, |_| 0);
        assert_eq!(played.iter().sum::<u32>(), 600);
        // After the first few frames, one a frame, every frame.
        assert!(
            played[10..590].iter().all(|&n| n == 1),
            "{:?}",
            &played[..40]
        );
        assert_eq!(playout.tally.gaps, 0);
        assert!(playout.target() <= MIN_TARGET + 1e-9);
    }

    #[test]
    fn a_shaky_line_is_smoothed_where_it_used_to_stop_and_lurch() {
        // Every twentieth packet held up eight frames, the rest behind it.
        let shaky = |sent: u32| if sent % 20 == 0 { 8 } else { 0 };
        let (unbuffered, _) = run(1200, 0.0, shaky);
        let (buffered, playout) = run(1200, 0.5, shaky);
        let stalls =
            |played: &[u32], from: usize| played[from..1150].iter().filter(|&&n| n != 1).count();
        assert_eq!(buffered.iter().sum::<u32>(), 1200);
        assert_eq!(unbuffered.iter().sum::<u32>(), 1200);
        // Without the buffer: a stall and a lurch at every hold-up.
        assert!(stalls(&unbuffered, 60) > 100, "{}", stalls(&unbuffered, 60));
        // With it: a gap or two while it learns how deep to be, then
        // a step a frame.
        assert!(playout.tally.gaps <= 3, "{:?}", playout.tally);
        assert!(stalls(&buffered, 300) <= 4, "{:?}", &buffered[300..400]);
        assert!(playout.target() > 0.1 && playout.target() < 0.2);
    }

    #[test]
    fn a_line_that_settles_gets_its_latency_back() {
        // Shaky for ten seconds, steady for fifty.
        let (played, playout) = run(
            3600,
            0.5,
            |sent| {
                if sent < 600 && sent % 20 == 0 { 8 } else { 0 }
            },
        );
        assert_eq!(played.iter().sum::<u32>(), 3600);
        assert!(playout.target() < 0.05, "{}", playout.target());
        assert!(played[3000..3590].iter().all(|&n| n == 1));
    }

    #[test]
    fn the_cap_bounds_the_buffer_and_nought_is_off() {
        let (_, playout) = run(600, 0.05, |sent| if sent % 30 == 0 { 20 } else { 0 });
        assert!(playout.target() <= 0.05 + 1e-9);
        let mut off = Playout::default();
        off.push(steps(3, Some(9)));
        assert_eq!(stepped(&off.release(FRAME, RATE, RATE, 0.0)), 3);
    }

    #[test]
    fn the_order_is_kept_and_a_split_packet_keeps_its_checksum_for_its_last_step() {
        let mut playout = Playout::default();
        playout.push(steps(4, Some(42)));
        playout.push(Event::Packet {
            from: 7,
            packet: Packet::Refused { why: 3 },
        });
        playout.push(steps(1, None));
        let mut seen = Vec::new();
        for _ in 0..30 {
            for event in playout.release(FRAME, RATE, RATE, 0.5) {
                match event {
                    Event::Packet {
                        packet: Packet::Steps { n, checksum },
                        ..
                    } => seen.push((n, checksum)),
                    Event::Packet {
                        packet: Packet::Refused { .. },
                        ..
                    } => seen.push((0, None)),
                    _ => {}
                }
            }
        }
        let total: u32 = seen.iter().map(|(n, _)| n).sum();
        assert_eq!(total, 5);
        let refused = seen.iter().position(|s| *s == (0, None)).unwrap();
        // The checksum came with the fourth step, and the refusal after it.
        let before: u32 = seen[..refused].iter().map(|(n, _)| n).sum();
        assert_eq!(before, 4);
        assert_eq!(seen[refused - 1].1, Some(42));
        assert!(seen[..refused - 1].iter().all(|(_, c)| c.is_none()));
    }

    #[test]
    fn far_behind_it_catches_up_at_once_and_paused_it_waits() {
        let mut playout = Playout::default();
        for _ in 0..300 {
            playout.push(steps(1, None));
        }
        let first = stepped(&playout.release(FRAME, RATE, RATE, 0.5));
        assert!(first > 150, "{first}");
        let mut paused = Playout::default();
        paused.push(Event::Packet {
            from: 7,
            packet: Packet::Refused { why: 0 },
        });
        assert_eq!(paused.release(FRAME, 0.0, RATE, 0.5).len(), 1);
        assert_eq!(paused.tally.gaps, 0);
        assert!(paused.release(FRAME, 0.0, RATE, 0.5).is_empty());
        assert_eq!(paused.starved, 0.0);
    }
}
