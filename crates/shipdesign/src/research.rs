//! The research tree: what the crew know how to build and make, and how
//! they come to know more.
//!
//! Research is done by the ship's AI at a [`PartKind::ResearchDesk`], not
//! by anybody aboard — the crew have stopped being able to — and this
//! module is the tree it works through: one [`NodeDef`] a node, each
//! naming what it wants researched first, which **tier** it sits in, and
//! whether it is **locked** — wanting a key of that tier consumed for it. The crew set out
//! knowing everything a crew needs to live and to fight — the hydroponic
//! bay, the galley, the heads, the hull, the fission reactor, the
//! workbench, the armoury, the suit locker and medicine — and research
//! the two things that are left: fusion power, and the hyperdrive behind
//! it.
//!
//! # Keys
//!
//! A locked node stays locked until a **research key** of its tier has
//! been put into the crew's research desk and consumed there **for that
//! node** ([`Research::unlock`]). A key is a resource found on a
//! station's research desk — the tier-one key
//! (`physics::ResourceId::ResearchKey`) on a friendly station's, the
//! tier-two (`ResearchKeyTwo`) on every hostile station's — carried off
//! in a pack where it takes [`KEY_CELLS`], and it is consumed — **one key
//! opens one node** and is gone, so two locked nodes are two keys and two
//! stations. A node wants a key of **its own tier** ([`Research::key_wanted`]):
//! the other tier's key in the desk opens nothing. Both keys are the
//! same size for now, and the desk holds one of either; a later key that
//! is bigger and wants a bigger desk is where the progression could go,
//! which is why the key's size and the desk's slot are written down here
//! as a pair. Tier three is declared ([`TIERS`]) and empty.
//!
//! # What a node gates
//!
//! [`node_of_part`] says which node each part waits on and
//! [`node_of_recipe`] the same for each row of `crate::recipes::RECIPES`;
//! `world` asks [`Research::part_allowed`] before a site is laid out and
//! [`Research::recipe_allowed`] before a bench is offered a recipe, and the
//! app hides the rest of the palette. A design accepted in the design
//! phase is never re-checked — the yard built it — so a ship that came
//! with a fusion reactor keeps it; it stands idle until the crew know
//! what to do at it.
//!
//! # The queue
//!
//! The AI thinks about one node at a time, but it can be given a list:
//! [`Research::enqueue`] puts a node at the back of the **queue**, and
//! whatever it needs that is not yet known, on the AI or queued goes in
//! ahead of it — so queueing the hyperdrive on a fresh crew queues
//! fusion power first — refused only when
//! something in that chain is still behind a key. [`Research::next`]
//! takes an idle AI onto the first queued node it can begin, and `world`
//! calls it every step the desk has power, so a node queued while the AI
//! is idle begins that step. Taking a node out — [`Research::dequeue`],
//! or [`Research::cancel`] on the one the AI is on — takes out with it
//! everything queued that needed it: the queue is always a plan that can
//! be carried out in order.
//!
//! # Integers, and one state
//!
//! The tree is data and the crew's progress through it is [`Research`], a
//! few booleans, a count of minutes and the queue, which `world` keeps
//! and hashes: a server and a client have to agree about what a crew
//! knows and what it will know next, or one ship builds what the other
//! refuses.

use crate::parts::PartKind;

/// How many tiers the tree has. The first is the starting point, the
/// second holds the upgrades node, and the third is declared and empty;
/// every table here is sized so that it can be filled.
pub const TIERS: u32 = 3;

/// A research key's size in a pack, cells across and down — either tier's
/// — and the slot in a research desk, which is exactly that size.
pub const KEY_CELLS: (u32, u32) = (1, 2);

/// One node of the tree. The discriminants cross the seam as numbers —
/// `world::Command::Research`, the events — so they are written out and
/// never renumbered; a node is appended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Node {
    /// Everything a crew needs to live and to fly: the hull, the galley,
    /// the heads, the bunks, the bay, the fission reactor, the helm, the
    /// engines, the suit locker, the workbench and the armoury. Known at
    /// the start.
    Survival = 0,
    /// Medkits at the drug lab. Known at the start.
    Medicine = 1,
    /// The fusion reactor. Keyless, and the one node a crew have to work
    /// for without a key.
    FusionPower = 2,
    /// The hyperdrive: a jump to another star. After fusion power, behind
    /// a tier-one key of its own.
    Hyperdrive = 3,
    /// The workbench's upgrades: two of a kind at one tier into one of
    /// the next, one to two and two to three alike. Behind a tier-two key
    /// — the one tier-two node, and the one thing the enemy's desks are
    /// worth walking to.
    Upgrades = 4,
}

impl Node {
    /// Every node, in discriminant order. `ALL[n as usize] == n`, which
    /// [`Node::def`] relies on and [`tree_is_sound`] checks.
    pub const ALL: [Node; 5] = [
        Node::Survival,
        Node::Medicine,
        Node::FusionPower,
        Node::Hyperdrive,
        Node::Upgrades,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Node> {
        Node::ALL.get(code as usize).copied()
    }

    pub fn def(self) -> &'static NodeDef {
        &RESEARCH[self as usize]
    }

    /// Known from the first day: nothing to research.
    pub fn known_at_start(self) -> bool {
        self.def().minutes == 0
    }
}

/// How many nodes there are, which is how long [`Research::done`] is.
pub const NODES: usize = Node::ALL.len();

/// One node, as data. No name: the words are the app's.
#[derive(Clone, Copy, Debug)]
pub struct NodeDef {
    pub node: Node,
    /// What has to be researched before this can be. Every one of them
    /// earlier in the table, so a walk down the table meets each node's
    /// prerequisites first.
    pub requires: &'static [Node],
    /// Which tier it is in, `1..=TIERS`.
    pub tier: u32,
    /// Locked: wants a key of its tier consumed at the desk for it before
    /// it can be begun.
    pub locked: bool,
    /// How long the AI takes over it, in game minutes. Nought for what
    /// the crew know at the start.
    pub minutes: u32,
}

/// The tree. Placeholder times throughout, unchanged since feature 64
/// doubled them: two days for the fusion reactor and a day and a quarter
/// for the hyperdrive behind it.
///
/// The money rework (feature 95) took **five nodes** out of it — mining,
/// smelting, the workshop, emitters and the armoury — because every one
/// of them gated a bench or a material that is gone: there is nothing to
/// mine, nothing to smelt, and the guns and armour they made are bought.
/// What was behind them is known at the start, the workbench, the armoury
/// and the suit locker included.
pub static RESEARCH: [NodeDef; NODES] = [
    NodeDef {
        node: Node::Survival,
        requires: &[],
        tier: 1,
        locked: false,
        minutes: 0,
    },
    NodeDef {
        node: Node::Medicine,
        requires: &[],
        tier: 1,
        locked: false,
        minutes: 0,
    },
    NodeDef {
        node: Node::FusionPower,
        requires: &[],
        tier: 1,
        locked: false,
        minutes: 2_880,
    },
    NodeDef {
        node: Node::Hyperdrive,
        requires: &[Node::FusionPower],
        tier: 1,
        locked: true,
        minutes: 1_800,
    },
    NodeDef {
        node: Node::Upgrades,
        requires: &[],
        tier: 2,
        locked: true,
        minutes: 1_440,
    },
];

/// Which node a part waits on. Everything not named is survival — known
/// at the start — which is the safe default: a new part the tree has not
/// heard of is a part the crew can build, not one nobody can.
///
/// Three parts are named and no more. The suit locker, the workbench and
/// the armoury were behind nodes the money rework took away, and are
/// known at the start with everything else.
pub fn node_of_part(kind: PartKind) -> Node {
    match kind {
        PartKind::DrugLab => Node::Medicine,
        PartKind::FusionReactor => Node::FusionPower,
        PartKind::Hyperdrive => Node::Hyperdrive,
        _ => Node::Survival,
    }
}

/// Which node a row of `crate::recipes::RECIPES` waits on, by index: its
/// bench's, which for the one row there is means medicine.
pub fn node_of_recipe(index: usize) -> Node {
    crate::recipes::RECIPES
        .get(index)
        .map(|r| node_of_part(r.station))
        .unwrap_or(Node::Survival)
}

/// The crew's progress through the tree: what is researched, which locked
/// nodes have had their key, what the AI is on and how far it has got.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Research {
    /// By `Node` code.
    pub done: [bool; NODES],
    /// By `Node` code: a locked node whose key has been consumed.
    pub unlocked: [bool; NODES],
    /// What the AI is researching, if anything.
    pub current: Option<Node>,
    /// Minutes put into `current` so far.
    pub progress: f64,
    /// What the AI goes onto next, in order: never a node that is known
    /// or on the AI, and each one's prerequisites known, on the AI or
    /// earlier in this list — see the module note.
    pub queue: Vec<Node>,
}

impl Default for Research {
    fn default() -> Research {
        Research::new()
    }
}

impl Research {
    /// What a crew sets out knowing: every node with no time on it.
    pub fn new() -> Research {
        let mut done = [false; NODES];
        for node in Node::ALL {
            done[node as usize] = node.known_at_start();
        }
        Research {
            done,
            unlocked: [false; NODES],
            current: None,
            progress: 0.0,
            queue: Vec::new(),
        }
    }

    pub fn is_done(&self, node: Node) -> bool {
        self.done[node as usize]
    }

    /// Whether a node's lock is open: a key was consumed for it, or it
    /// never had one.
    pub fn is_unlocked(&self, node: Node) -> bool {
        !node.def().locked || self.unlocked[node as usize]
    }

    /// Whether a node is waiting only on its key: everything it needs
    /// is researched, and its own lock is shut.
    pub fn needs_key(&self, node: Node) -> bool {
        !self.is_done(node) && !self.is_unlocked(node)
    }

    /// Whether a node can be begun now: not done, everything it requires
    /// done, and its lock open if it has one.
    pub fn available(&self, node: Node) -> bool {
        let def = node.def();
        !self.is_done(node)
            && def.requires.iter().all(|&r| self.is_done(r))
            && self.is_unlocked(node)
    }

    /// Whether the crew may lay out a part of this kind.
    pub fn part_allowed(&self, kind: PartKind) -> bool {
        self.is_done(node_of_part(kind))
    }

    /// Whether a bench may be offered this row of the recipe table.
    pub fn recipe_allowed(&self, index: usize) -> bool {
        self.is_done(node_of_recipe(index))
    }

    /// Whether the workbench may take two of a kind up a tier: the
    /// upgrades node, which covers both steps.
    pub fn upgrades_allowed(&self) -> bool {
        self.is_done(Node::Upgrades)
    }

    /// Open a node's lock. What consuming a key does; the caller has
    /// taken a key of the node's tier out of the desk. `false` for a node
    /// that has no lock, is open already or is researched, in which case
    /// no key should go.
    pub fn unlock(&mut self, node: Node) -> bool {
        if self.is_unlocked(node) || self.is_done(node) {
            return false;
        }
        self.unlocked[node as usize] = true;
        true
    }

    /// Which tier of key a locked node wants: its own tier. `None` for a
    /// node with no lock.
    pub fn key_wanted(node: Node) -> Option<u32> {
        node.def().locked.then_some(node.def().tier)
    }

    /// Whether a node is spoken for: known, on the AI, or in the queue.
    pub fn planned(&self, node: Node) -> bool {
        self.is_done(node) || self.current == Some(node) || self.queue.contains(&node)
    }

    /// Whether every node from this one back to what is known is open:
    /// planned already, or with its lock open and its own prerequisites
    /// the same. What [`Research::enqueue`] wants of the chain it would
    /// queue.
    fn chain_open(&self, node: Node) -> bool {
        self.planned(node)
            || (self.is_unlocked(node) && node.def().requires.iter().all(|&r| self.chain_open(r)))
    }

    /// Whether [`Research::enqueue`] would take a node: not planned
    /// already, and nothing it needs — however far back — still behind
    /// a key.
    pub fn queueable(&self, node: Node) -> bool {
        !self.planned(node) && self.chain_open(node)
    }

    /// Put a node at the back of the queue, with whatever it needs that
    /// is not yet planned ahead of it, prerequisites first. `false`, and
    /// nothing queued, when it is planned already or something in that
    /// chain is behind a key. The AI goes onto the head of the queue at
    /// the next [`Research::next`].
    pub fn enqueue(&mut self, node: Node) -> bool {
        if !self.queueable(node) {
            return false;
        }
        self.push_chain(node);
        true
    }

    fn push_chain(&mut self, node: Node) {
        if self.planned(node) {
            return;
        }
        for &r in node.def().requires {
            self.push_chain(r);
        }
        self.queue.push(node);
    }

    /// Take a node out of the queue, and with it everything queued that
    /// needed it: what came out, the node first — empty for a node not in
    /// the queue.
    pub fn dequeue(&mut self, node: Node) -> Vec<Node> {
        if !self.queue.contains(&node) {
            return Vec::new();
        }
        self.queue.retain(|&n| n != node);
        let mut out = vec![node];
        out.extend(self.prune());
        out
    }

    /// Put an idle AI onto the first queued node it can begin. The node
    /// it went onto, if it went onto one; `None` while it is busy or the
    /// queue holds nothing it can begin.
    pub fn next(&mut self) -> Option<Node> {
        if self.current.is_some() {
            return None;
        }
        let at = self.queue.iter().position(|&n| self.available(n))?;
        let node = self.queue.remove(at);
        self.current = Some(node);
        self.progress = 0.0;
        Some(node)
    }

    /// Take the AI off whatever it is on. What was put in is lost: it
    /// thinks about one thing at a time, and an idle AI is thinking
    /// about nothing. Whatever was queued behind it that needed it comes
    /// out of the queue too, and is the returned list; the AI goes onto
    /// what is left at the next [`Research::next`].
    pub fn cancel(&mut self) -> Vec<Node> {
        self.current = None;
        self.progress = 0.0;
        self.prune()
    }

    /// Drop from the queue whatever cannot be carried out any more: a
    /// node known already, or one needing a node that is neither known,
    /// on the AI nor queued — and then whatever needed *that*. What came
    /// out, in queue order.
    fn prune(&mut self) -> Vec<Node> {
        let mut out = Vec::new();
        loop {
            let kept: Vec<Node> = self
                .queue
                .iter()
                .copied()
                .filter(|&n| {
                    !self.is_done(n)
                        && n.def().requires.iter().all(|&r| {
                            self.is_done(r) || self.current == Some(r) || self.queue.contains(&r)
                        })
                })
                .collect();
            if kept.len() == self.queue.len() {
                return out;
            }
            out.extend(self.queue.iter().copied().filter(|n| !kept.contains(n)));
            self.queue = kept;
        }
    }

    /// `minutes` more of the AI's time on the current node. The node it
    /// finished, if it finished one this call; the AI then stands idle
    /// until the next [`Research::next`].
    pub fn advance(&mut self, minutes: f64) -> Option<Node> {
        let node = self.current?;
        if !self.available(node) {
            // What it was on has been researched or locked from under it:
            // nothing to do until told again.
            self.current = None;
            return None;
        }
        self.progress += minutes;
        if self.progress + 1e-9 < node.def().minutes as f64 {
            return None;
        }
        self.done[node as usize] = true;
        self.current = None;
        self.progress = 0.0;
        Some(node)
    }

    /// How far along the current node is, nought to one; nought idle.
    pub fn fraction(&self) -> f64 {
        match self.current {
            Some(node) if node.def().minutes > 0 => {
                (self.progress / node.def().minutes as f64).clamp(0.0, 1.0)
            }
            _ => 0.0,
        }
    }
}

/// Whether the table holds together: one entry per node in order, every
/// prerequisite earlier in the table than what wants it, tiers in range,
/// nothing known at the start locked or wanting anything, and every
/// locked node in a tier a key exists for.
pub fn tree_is_sound() -> bool {
    RESEARCH.len() == NODES
        && Node::ALL.iter().enumerate().all(|(i, &node)| {
            let def = &RESEARCH[i];
            def.node == node
                && def.tier >= 1
                && def.tier <= TIERS
                && def.requires.iter().all(|&r| (r as usize) < i)
                && (def.minutes > 0 || (!def.locked && def.requires.is_empty()))
        })
}
