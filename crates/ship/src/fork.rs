//! Two jobs of a frame at once, where the host has cores to spare (task
//! 122).
//!
//! [`Session::render`](crate::Session::render) builds the stations'
//! pictures while the crew's room draws itself: two pieces of work that
//! read nothing of each other's and write into buffers of their own, which
//! are put together afterwards in the order they always were — so the
//! picture is the same bit for bit however the two were run.
//!
//! **How** they are run is the host's to say, and this crate never starts
//! a thread of its own: [`set_join`] hands it a [`Join`] once at start-up
//! (the app's is Bevy's compute pool, so no second pool competes with
//! Bevy's), and until somebody does, [`serial`] runs the one and then the
//! other — which is what every test, probe, server and a target with no
//! threads gets.

use std::sync::OnceLock;

/// Run both jobs and come back when both are done, in any order and on
/// any threads: neither may depend on the other having run.
pub type Join = fn(&mut (dyn FnMut() + Send), &mut (dyn FnMut() + Send));

/// The one and then the other, on this thread.
pub fn serial(a: &mut (dyn FnMut() + Send), b: &mut (dyn FnMut() + Send)) {
    a();
    b();
}

/// The first on a thread of its own and the second on this one: what the
/// tests compare the serial picture against, and a host with no pool of
/// its own could use.
pub fn scoped(a: &mut (dyn FnMut() + Send), b: &mut (dyn FnMut() + Send)) {
    std::thread::scope(|s| {
        s.spawn(a);
        b();
    });
}

static JOIN: OnceLock<Join> = OnceLock::new();

/// Say how the two jobs of a frame are run, once, at start-up. A second
/// call changes nothing.
pub fn set_join(join: Join) {
    let _ = JOIN.set(join);
}

/// How the two jobs of a frame are run: the host's word, else [`serial`].
pub fn join() -> Join {
    JOIN.get().copied().unwrap_or(serial)
}
