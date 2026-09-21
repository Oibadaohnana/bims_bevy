//! The research tree: what the crew know how to build and make, and how
//! they come to know more.
//!
//! Research is done by the ship's AI at a [`PartKind::ResearchDesk`], not
//! by anybody aboard — the crew have stopped being able to — and this
//! module is the tree it works through: one [`NodeDef`] a node, each
//! naming what it wants researched first, which **tier** it sits in, and
//! whether it is **locked** — wanting a key of that tier consumed for it. The crew set out
//! knowing everything a crew needs to live — the hydroponic bay, the
//! galley, the heads, the hull, the fission reactor, mining in a suit,
//! and medicine — and research the rest.
//!
//! # Keys
//!
//! A locked node stays locked until a **research key** of its tier has
//! been put into the crew's research desk and consumed there **for that
//! node** ([`Research::unlock`]). A key is a resource found on a friendly
//! station's research desk (`physics::ResourceId::ResearchKey` for tier
//! one), carried off in a pack where it takes [`KEY_CELLS`], and it is
//! consumed — **one key opens one node** and is gone, so two locked nodes
//! are two keys and two stations. A later tier's key
//! is bigger and wants a bigger desk; that is where the progression goes,
//! and it is why the key's size and the desk's slot are written down here
//! as a pair.
//!
//! # What a node gates
//!
//! [`node_of_part`] says which node each part waits on and
//! [`node_of_recipe`] the same for each row of `crate::recipes::RECIPES`;
//! `world` asks [`Research::part_allowed`] before a site is laid out and
//! [`Research::recipe_allowed`] before a bench is offered a recipe, and the
//! app hides the rest of the palette. A design accepted in the design
//! phase is never re-checked — the yard built it — so a ship that came
//! with a smelter keeps the smelter; it stands idle until the crew know
//! what to do at it.
//!
//! # Integers, and one state
//!
//! The tree is data and the crew's progress through it is [`Research`], a
//! few booleans and a count of minutes, which `world` keeps and hashes:
//! a server and a client have to agree about what a crew knows, or one
//! ship builds what the other refuses.

use crate::parts::PartKind;

/// How many tiers the tree has. The first is the starting point; the
/// others are to come, and every table here is sized so that they can.
pub const TIERS: u32 = 3;

/// A tier-one research key's size in a pack, cells across and down — and
/// the slot in a tier-one research desk, which is exactly that size.
pub const KEY_CELLS: (u32, u32) = (1, 2);

/// One node of the tree. The discriminants cross the seam as numbers —
/// `world::Command::Research`, the events — so they are written out and
/// never renumbered; a node is appended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Node {
    /// Everything a crew needs to live and to fly: the hull, the galley,
    /// the heads, the bunks, the bay, the fission reactor, the helm and
    /// the engines. Known at the start.
    Survival = 0,
    /// A walk outside with a pick: the suit locker and the suit. Known at
    /// the start, so the crew can mine from the first day.
    Mining = 1,
    /// Bandages and medkits at the drug lab. Known at the start.
    Medicine = 2,
    /// The smelter: ore into metal. Keyless, after mining.
    Smelting = 3,
    /// The workbench: metal into components. Keyless, after smelting.
    Workshop = 4,
    /// The fusion reactor. Keyless, after the workshop.
    FusionPower = 5,
    /// The armoury and every weapon and piece of armour: what it makes,
    /// and the three pieces the workbench makes. Behind a tier-one key.
    Armoury = 6,
    /// The emitter, at the workbench. Behind a tier-one key of its own.
    Emitters = 7,
    /// The hyperdrive: a jump to another star. After fusion power, behind
    /// a tier-one key of its own.
    Hyperdrive = 8,
}

impl Node {
    /// Every node, in discriminant order. `ALL[n as usize] == n`, which
    /// [`Node::def`] relies on and [`tree_is_sound`] checks.
    pub const ALL: [Node; 9] = [
        Node::Survival,
        Node::Mining,
        Node::Medicine,
        Node::Smelting,
        Node::Workshop,
        Node::FusionPower,
        Node::Armoury,
        Node::Emitters,
        Node::Hyperdrive,
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

/// The tree. Placeholder times throughout: hours for the workshop
/// nodes, a day for the fusion reactor, half a day for each locked node.
pub static RESEARCH: [NodeDef; NODES] = [
    NodeDef {
        node: Node::Survival,
        requires: &[],
        tier: 1,
        locked: false,
        minutes: 0,
    },
    NodeDef {
        node: Node::Mining,
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
        node: Node::Smelting,
        requires: &[Node::Mining],
        tier: 1,
        locked: false,
        minutes: 240,
    },
    NodeDef {
        node: Node::Workshop,
        requires: &[Node::Smelting],
        tier: 1,
        locked: false,
        minutes: 360,
    },
    NodeDef {
        node: Node::FusionPower,
        requires: &[Node::Workshop],
        tier: 1,
        locked: false,
        minutes: 1_440,
    },
    NodeDef {
        node: Node::Armoury,
        requires: &[Node::Workshop],
        tier: 1,
        locked: true,
        minutes: 720,
    },
    NodeDef {
        node: Node::Emitters,
        requires: &[Node::Workshop],
        tier: 1,
        locked: true,
        minutes: 600,
    },
    NodeDef {
        node: Node::Hyperdrive,
        requires: &[Node::FusionPower],
        tier: 1,
        locked: true,
        minutes: 900,
    },
];

/// Which node a part waits on. Everything not named is survival — known
/// at the start — which is the safe default: a new part the tree has not
/// heard of is a part the crew can build, not one nobody can.
pub fn node_of_part(kind: PartKind) -> Node {
    match kind {
        PartKind::SuitLocker => Node::Mining,
        PartKind::DrugLab => Node::Medicine,
        PartKind::Smelter => Node::Smelting,
        PartKind::Workbench => Node::Workshop,
        PartKind::FusionReactor => Node::FusionPower,
        PartKind::Armoury => Node::Armoury,
        PartKind::Hyperdrive => Node::Hyperdrive,
        _ => Node::Survival,
    }
}

/// Which node a row of `crate::recipes::RECIPES` waits on, by index. The
/// emitter is its own node; the armour at the workbench is the armoury's,
/// since it is what a fight wants; everything else waits on its bench.
pub fn node_of_recipe(index: usize) -> Node {
    match index {
        2 => Node::Emitters,
        7..=9 => Node::Armoury,
        i => crate::recipes::RECIPES
            .get(i)
            .map(|r| node_of_part(r.station))
            .unwrap_or(Node::Survival),
    }
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

    /// Put the AI onto a node. Starting a different node loses what was
    /// put into the last one: the AI thinks about one thing at a time.
    /// `false` when the node cannot be begun.
    pub fn begin(&mut self, node: Node) -> bool {
        if !self.available(node) {
            return false;
        }
        if self.current != Some(node) {
            self.progress = 0.0;
        }
        self.current = Some(node);
        true
    }

    /// Take the AI off whatever it is on. What was put in is lost, as it
    /// is when it is put onto something else: it thinks about one thing
    /// at a time, and an idle AI is thinking about nothing.
    pub fn cancel(&mut self) {
        self.current = None;
        self.progress = 0.0;
    }

    /// `minutes` more of the AI's time on the current node. The node it
    /// finished, if it finished one this call; the AI then stands idle.
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
