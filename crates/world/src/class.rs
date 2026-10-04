//! Classes: what a player's crew member is, and what it learns (features
//! 74 to 78 and 88; every class a ranked kit since task 139).
//!
//! A crew member has **one class**, chosen by its player before the game
//! opens — a [`Class`] per player slot, kept on the world with the start
//! like the seed and changeable by `Command::SetClass` until the ship
//! first leaves its berth — and one [`Progress`] through it: experience,
//! the level that makes, and the ranks bought on the way up. A crew
//! member nobody steers, a station's resident has
//! [`Class::None`] and learns nothing.
//!
//! **A class owns abilities, never jobs or money.** Every Bim can do
//! every job, take every errand, place every site and use every weapon,
//! and every Bim brings the same money to the pool whatever its class.
//! A class only adds its own four abilities, which no other class can
//! use: [`can`] is the one rule, and it answers for an [`Ability`] and
//! nothing else.
//!
//! # Nothing is crafted for an ability (feature 90)
//!
//! What an ability spends is a [`Charge`]: a **counter** the world keeps
//! for each crew member (`World::charges_held`, task 127), never a thing
//! in a pack, that comes back on a cooldown of its own. A player never
//! stands at a bench or at a dock to use a skill — the engineer's
//! mines, Healing Sentry and satchel charges, the soldier's grenades
//! ([`GRENADE_CHARGES`] of them at [`GRENADE_COOLDOWN`] a charge) — and
//! what a class added later spends goes in [`Charge`] beside them. There
//! is no medicine to carry since task 120: a downed crewmate is revived
//! by standing beside it, and a medic's heal beam puts hit points back.
//! The abilities that spend nothing are held instead of thrown — a
//! beam, a riot shield — and the ones that are
//! neither wait out a cooldown of their own: the tank's Reflect Barrier
//! and Bastion at [`REFLECT_COOLDOWN`] and [`BASTION_COOLDOWN`] of their
//! ranks, the commander's Battle Cry and Rally at [`BATTLE_CRY_COOLDOWN`]
//! and [`RALLY_COOLDOWN`] of theirs, the medic's Heal Drone at
//! [`HEAL_DRONE_COOLDOWN`] of his.
//!
//! # Experience
//!
//! **The same for every class** (task 119). One thing gives it, and
//! nothing else:
//!
//! | what | xp | who |
//! |---|---|---|
//! | an enemy goes down within [`VICINITY_TILES`] — downed, or dead without being down first | [`XP_ENEMY_DOWN`] | every classed crew member in range |
//!
//! Each enemy counts once, at its first down or death: a machine
//! destroyed and a Manufacturer downed are worth the same twenty, and a
//! downed one dying after is worth nothing more. A crewmate or one of
//! a station's own going down gives nothing. The vicinity is measured on the deck the fight is
//! on, between the crew member and the enemy. No class has a source of
//! its own: building, laying a kit, healing, taking hits and hiring gave
//! the engineer, the medic, the tank and the commander experience of
//! their own until task 119, and give nobody any now.
//!
//! # Levels and ranks
//!
//! **Every class is a ranked kit** — the soldier since task 124, the
//! engineer 127, the commander 129, the medic 130 and the tank 139, which
//! took the old left-and-right talents away whole. Every class climbs
//! the same **twenty** levels ([`LEVELS`]) on [`LEVEL_XP`] — 100 for the
//! second, 16 740 for the sixteenth, 48 410 for the twentieth — and earns
//! **one skill point a level** up to the sixteenth ([`SKILL_LEVELS`]),
//! the first included; each of the four past it is ten hit points like
//! any level and five per cent more weapon damage ([`level_damage`]). A point buys one **rank** of one of four
//! abilities — Q, C, E and R, [`MAX_RANK`] ranks each — with
//! `Command::RankUp` ([`Progress::rank_up`]): in the game **Ctrl and the
//! ability's key, or a Ctrl-click on its box**, or the button on the
//! character sheet's Skills tab. A rank of Q, C or E wants level `2n − 1`
//! (1, 3, 5, 7); a rank of the ultimate R wants level 6, 9, 12 or 15
//! ([`rank_level`]). Points not spent carry over, without a limit, and a
//! rank is never taken back. A level reached is `WorldEvent::LevelUp`,
//! said once. Levels and ranks are kept through a death. A classless
//! crew member has no progression at all.
//!
//! # The engineer's four slots (task 127)
//!
//! A ranked kit like the soldier's: sixteen levels on
//! [`LEVEL_XP`], a skill point a level, Q, C and E rank `n` at
//! level `2n − 1` and the ultimate R at 6, 9, 12 and 15
//! ([`rank_level`]). Every number is a table here, one a rank, read with
//! [`by_rank`]; the deploy times are game minutes of working steps.
//!
//! **Q, Mine** (task 154; the EMP until then) — laid like the Healing
//! Sentry, in [`MINE_MINUTES`]: a mine on a tile of deck that goes off
//! the step an enemy — a machine or a hostile Bim, never the crew —
//! stands within [`MINE_TRIGGER`] (one tile) of it, every enemy in its
//! blast with a clear line taking its damage, half at the edge. It never
//! touches the crew or their sentries. One more laid past the standing
//! limit takes that engineer's oldest up:
//!
//! | rank | damage | blast | charges | standing | cooldown a charge |
//! |---|---|---|---|---|---|
//! | 1 | 40 | 1.5 tiles | 1 | 4 | 25 s |
//! | 2 | 50 | 1.5 tiles | 1 | 6 | 22 s |
//! | 3 | 60 | 2.0 tiles | 2 | 6 | 20 s |
//! | 4 | 75 | 2.0 tiles | 2 | 8 | 18 s |
//!
//! **C, Healing Sentry** — laid like a mine; heals every crew Bim on
//! its feet within its radius and its sight, below its full bar, at a
//! share of the medic's beam ([`HEAL_BEAM_HP`]); several reaching one
//! Bim do not stack, and the charges are the standing limit:
//!
//! | rank | heal (× beam) | radius | health | deploy | charges | cooldown a charge |
//! |---|---|---|---|---|---|---|
//! | 1 | 0.5 | 3 tiles | 120 | 6 min | 1 | 60 s |
//! | 2 | 0.75 | 4 tiles | 160 | 6 min | 1 | 60 s |
//! | 3 | 1.0 | 4 tiles | 200 | 4 min | 1 | 50 s |
//! | 4 | 1.25 | 5 tiles | 240 | 4 min | 1 | 40 s |
//!
//! **E, Satchel Charge** (task 154; the sandbags until then) — thrown the
//! grenade's way ([`GRENADE_RANGE`], its flight) onto a tile of deck,
//! where it lies until its engineer sets it off with the **remote
//! trigger** (`Command::Detonate`, Space): every satchel of his in the
//! room bursts at once, each on its own, so several thrown onto one tile
//! — they stack — hit as many times. Its blast is the mine's: enemies
//! alone, half at the edge. [`SATCHEL_CHARGES`] (two) at every rank:
//!
//! | rank | damage | radius | charges | cooldown a charge |
//! |---|---|---|---|---|
//! | 1 | 35 | 2.0 tiles | 2 | 30 s |
//! | 2 | 45 | 2.5 tiles | 2 | 27 s |
//! | 3 | 60 | 2.5 tiles | 2 | 24 s |
//! | 4 | 85 | 3.0 tiles | 2 | 20 s |
//!
//! **R, Sentry** (the ultimate) — laid in 3 min that a hit does not
//! interrupt, one standing (two at the Override Core's fifth rank), never
//! packed up; it stands until destroyed. The cooldown runs from the
//! laying, ready at every mission's start. Its health was doubled in task
//! 154, when the sandbags went:
//!
//! | rank | weapon | fire rate | health | cooldown |
//! |---|---|---|---|---|
//! | 1 | minigun, tier 2 | ×1.0 | 400 | 150 s |
//! | 2 | minigun, tier 3 | ×1.0 | 500 | 140 s |
//! | 3 | minigun, tier 3 | ×1.5 | 600 | 130 s |
//! | 4 | minigun, tier 3 | ×2.0 | 800 | 120 s |
//!
//! A hit on the engineer interrupts laying a mine or a Healing Sentry
//! at every rank.
//!
//! # The soldier's four abilities (task 124)
//!
//! The first ranked kit: sixteen levels on [`LEVEL_XP`], a skill point a
//! level, the gates of "Levels and ranks" above.
//!
//! **Q, Frag Grenade** (active, charges; range [`GRENADE_RANGE`] tiles,
//! fuse [`GRENADE_FUSE`] seconds):
//!
//! | rank | damage | radius | charges | cooldown a charge |
//! |---|---|---|---|---|
//! | 1 | 60 | 2.0 | 1 | 30 s |
//! | 2 | 75 | 2.5 | 2 | 30 s |
//! | 3 | 90 | 2.5 | 2 | 24 s |
//! | 4 | 110 | 3.0 | 2 | 20 s |
//!
//! **C, Weak Spot** (passive): every weapon hit it lands on an enemy — a
//! bolt, each bolt of a burst on its own, a schword's or a fist's blow,
//! never a grenade — may be **critical**: the weapon's flat damage at the
//! distance flown times the crit damage less one is added, after every
//! relic and ability factor and before the armour, so a relic's share is
//! never multiplied by the crit.
//!
//! | rank | crit chance | crit damage |
//! |---|---|---|
//! | 1 | 10% | 150% |
//! | 2 | 12% | 175% |
//! | 3 | 15% | 200% |
//! | 4 | 20% | 225% |
//!
//! **E, Stun Shot** (active, cooldown; October 2026 — Brace was E until
//! then): the soldier charges a shot for [`STUN_SHOT_CHARGE`] seconds,
//! holding his fire and walking as he likes, then fires it at the
//! spot he aimed at — no further than his weapon reaches, stopped short
//! of the first wall. It bursts there as wide as his grenade
//! ([`STUN_SHOT_RADIUS`]): every enemy in the burst with nothing opaque
//! between takes its damage and is stunned for [`STUN_SHOT_STUN`]
//! seconds — every machine, the Machine Heart's too
//! (`bims::droid::Droid::stun`), and the Manufacturers' people
//! (`bims::game::Game::stun_bim`). It harms none of the crew. Going down
//! calls the charge off (walking, an order or a roll no longer do), and
//! the cooldown runs only from a shot fired. A player steering him aims
//! it with the pointer while it charges: it flies the way he faces when
//! it fires, as far as the spot first aimed at.
//!
//! | rank | damage | radius | stun | cooldown |
//! |---|---|---|---|---|
//! | 1 | 15 | 2.0 tiles | 3 s | 30 s |
//! | 2 | 20 | 2.5 tiles | 3 s | 27 s |
//! | 3 | 25 | 2.5 tiles | 3 s | 24 s |
//! | 4 | 30 | 3.0 tiles | 3 s | 20 s |
//!
//! **R, Rampage** (ultimate, active; ready at every mission's start, and
//! may be used charging): the fire rate up, the damage taken down and full
//! aim on the move, for its seconds of the mission clock.
//!
//! | rank | duration | fire rate | damage taken | cooldown |
//! |---|---|---|---|---|
//! | 1 | 8 s | ×1.5 | ×0.80 | 150 s |
//! | 2 | 10 s | ×1.75 | ×0.75 | 135 s |
//! | 3 | 12 s | ×2.0 | ×0.70 | 120 s |
//! | 4 | 12 s, +1 s an enemy it downs during it, +6 s at most | ×2.0 | ×0.70 | 120 s |
//!
//! # The medic's four slots (task 130; reworked by task 153)
//!
//! A ranked kit like the soldier's, the engineer's and the commander's:
//! sixteen levels on [`LEVEL_XP`], a skill point a level, Q, C and
//! E rank `n` at level `2n − 1` and the ultimate R at 6, 9, 12 and 15.
//! **Two base traits** are his whatever his ranks: he revives a downed
//! crewmate in [`MEDIC_REVIVE_SECONDS`], and a Bim he revives gets up at
//! [`MEDIC_REVIVED_TO`] of its bar where anybody else's is at
//! `bims::health::REVIVED_TO`. There is no surge.
//!
//! **Every heal he gives** — the link, the drone, the circle — goes
//! through `World::medic_heal`: times his *Triage* on the Bim healed,
//! then times [`OVERRIDE_HEAL`] while he carries an *Override Core*
//! (task 153: the core's gift to a medic is half as much healing again,
//! whatever rank his circle is at).
//!
//! **Q, Heal Drone** (active, cooldown): a drone dropped at his feet flies
//! — over walls, it flies — to the friendly Bim on its feet lowest on its
//! bar, himself included, hovers over it and heals it slowly; when that
//! one is whole, down or gone it picks the next lowest, and with nobody
//! hurt it keeps by him. One drone a medic: a new one takes the old one's
//! place.
//!
//! | rank | heal a second | lasts | cooldown |
//! |---|---|---|---|
//! | 1 | 1.5 HP | 8 s | 25 s |
//! | 2 | 2 HP | 10 s | 22 s |
//! | 3 | 2.5 HP | 12 s | 20 s |
//! | 4 | 3 HP | 14 s | 18 s |
//!
//! **C, Triage** (passive): his heals are stronger on the badly hurt —
//! times one plus the rank's share times how much of its bar the healed
//! Bim is missing, so the full share on a Bim at nothing and none on a
//! whole one.
//!
//! | rank | at an empty bar | at half a bar |
//! |---|---|---|
//! | 1 | ×1.25 | ×1.125 |
//! | 2 | ×1.40 | ×1.20 |
//! | 3 | ×1.55 | ×1.275 |
//! | 4 | ×1.70 | ×1.35 |
//!
//! **E, Heal Beam** (active, toggle): the link of feature 76, on a
//! crewmate or himself, at [`HEAL_BEAM_HP`] an hour of the clock times
//! the rank's rate. He **fires at his full rate** while linked, and the
//! link heals **him as well**, as much as it gives a patient (task 153);
//! once, however many he holds, and once when the patient is himself.
//!
//! | rank | rate | a second at 1× | range | patients |
//! |---|---|---|---|---|
//! | 1 | ×1.0 | 2 HP | 6 tiles | 1 |
//! | 2 | ×1.5 | 3 HP | 7 tiles | 1 |
//! | 3 | ×2.0 | 4 HP | 8 tiles | 1 |
//! | 4 | ×2.5 | 5 HP | 9 tiles | 2, each at the full rate |
//!
//! **R, Healing Circle** (ultimate, toggle): switched on, every friendly
//! Bim on its feet within the rank's radius of him and in his sight
//! (walls block) — not himself — is healed at **the link's rate** (his E
//! rank's, the first's before one), through `medic_heal`; **he loses as
//! much** as a Bim is healed for before his Triage, every second it is on,
//! whoever stands in it — enough of it downs him, and that switches it
//! off — and every enemy standing in it takes [`HEALING_CIRCLE_BURN`] of
//! it as damage, a pulse every [`HEALING_CIRCLE_PULSE`] seconds. It goes
//! off by itself when he is down or unfit to act. The link's heal on him
//! is how a medic stands in his own circle for long.
//!
//! | rank | radius |
//! |---|---|
//! | 1 | 3 tiles |
//! | 2 | 3.5 tiles |
//! | 3 | 4 tiles |
//! | 4 | 4.5 tiles |
//!
//! The drone's cooldown runs on the mission clock, stops while paused, is
//! ready at every mission's start and is shortened by the cooldown relics
//! as every class cooldown is; every mission starts with the circle off.
//! See [`crate::medic`].
//!
//! # The tank's four slots (task 139; reworked by task 155)
//!
//! A ranked kit like the others': sixteen levels on [`LEVEL_XP`], a
//! skill point a level, Q, C and E rank `n` at level `2n − 1` and the
//! ultimate R at 6, 9, 12 and 15. **Two base traits** are his whatever
//! his ranks: armour he wears drains at [`TANK_DRAIN`] — half the rate,
//! so the same armour takes twice as much on him — and he sets out with
//! the pistol and a basic armour on.
//!
//! **Q, Riot Shield** (toggle): a flat plate of light held up in front of
//! him, facing the way he faces. A hostile bolt meeting it from the front
//! is stopped there, its damage off the shield's hit points, and
//! **bounced** back as his own — the angle out the angle in — onto
//! whatever it reaches. It restores [`RIOT_SHIELD_REGEN`] a second while
//! stowed, and up after [`RIOT_SHIELD_REGEN_DELAY`] seconds unstruck; at
//! nought it breaks and goes down, and cannot be raised again for
//! [`RIOT_SHIELD_BROKEN_COOLDOWN`] seconds (the cooldown relics and items
//! on it), restoring all the while.
//!
//! | rank | hit points | restores |
//! |---|---|---|
//! | 1 | 20 | 0.5 hp/s |
//! | 2 | 40 | 1.0 hp/s |
//! | 3 | 80 | 1.5 hp/s |
//! | 4 | 100 | 2.0 hp/s |
//!
//! **C, Plated** (passive): the damage of every hit on him multiplied
//! down, before the armour takes its share — with Rampage, Rally and
//! every other factor multiplied together — and hit points mended every
//! second.
//!
//! | rank | damage taken | mends | extra |
//! |---|---|---|---|
//! | 1 | ×0.90 | 0.2 hp/s | — |
//! | 2 | ×0.85 | 0.8 hp/s | — |
//! | 3 | ×0.80 | 1.4 hp/s | — |
//! | 4 | ×0.75 | 2.0 hp/s | armour drain on him ×0.5 again, a quarter in all |
//!
//! **E, Reflect Barrier** (active, cooldown): for its seconds every enemy
//! hit on him — a bolt, a beam, a blow — goes back on whoever struck it,
//! as much again as a hit of his ([`REFLECT_SHARE`]). He still takes it.
//!
//! | rank | lasts | cooldown |
//! |---|---|---|
//! | 1 | 3 s | 20 s |
//! | 2 | 4 s | 18 s |
//! | 3 | 5 s | 16 s |
//! | 4 | 6 s | 14 s |
//!
//! **R, Bastion** (ultimate, cooldown): every friend on his feet within
//! the radius — himself, the players and the bots — takes a shield of
//! its rank's [`BASTION_HP`] hit points, drawn on the end of the health
//! bar like armour, that drains its rank's [`BASTION_DRAIN`] a second
//! whatever strikes it, [`BASTION_SECONDS`] at most. With an *Override
//! Core* (the fifth rank, the fourth's shield) everybody it reached also
//! moves half again as fast ([`BASTION_HASTE`]) for those seconds.
//!
//! | rank | radius | shield | drains | cooldown |
//! |---|---|---|---|---|
//! | 1 | 6 tiles | 600 hp | 60 hp/s | 70 s |
//! | 2 | 7 tiles | 800 hp | 80 hp/s | 60 s |
//! | 3 | 8 tiles | 1000 hp | 100 hp/s | 50 s |
//! | 4 | 9 tiles | 1200 hp | 100 hp/s | 40 s |
//!
//! The cooldowns run on the mission clock, stop while paused, are ready
//! at every mission's start and are shortened by the cooldown relics as
//! every class cooldown is. See [`crate::tank`].
//!
//! # The commander's four slots (task 129)
//!
//! A ranked kit like the soldier's and the engineer's: sixteen levels on
//! [`LEVEL_XP`], a skill point a level, Q, C and E rank `n` at
//! level `2n − 1` and the ultimate R at 6, 9, 12 and 15. He has no base
//! trait: the cheaper hire went with the mercenaries (October 2026),
//! and his squad orders — attack, fall back, stand ground — were
//! removed.
//!
//! **Q, Battle Cry** (active, cooldown): every friendly Bim within
//! [`BATTLE_CRY_TILES`] of him **when he calls it** — himself, a player's
//! Bim, a bot, a reinforcement; never a sentry — fires
//! faster for its seconds. The reach is fixed at the call: one that walks
//! out keeps it, one that walks in does not get it.
//!
//! | rank | fire rate | lasts | cooldown |
//! |---|---|---|---|
//! | 1 | ×1.25 | 3 s | 20 s |
//! | 2 | ×1.30 | 4 s | 18 s |
//! | 3 | ×1.35 | 5 s | 16 s |
//! | 4 | ×1.40 | 6 s | 14 s |
//!
//! **C, Medivac** (active, cooldown): he calls a medic of the Republic's
//! in, on the free deck nearest him within [`REINFORCEMENT_REACH_TILES`]
//! — a classless crew member with the pistol, for that mission alone, a
//! reinforcement like the R's (`World::reinforcements`, marked `medic`),
//! who fights as any bot does and **runs to a player who goes down and
//! revives him** whatever the fight round the body, in a medic's time.
//! Those called before stay.
//!
//! | rank | armour | cooldown |
//! |---|---|---|
//! | 1 | none | 140 s |
//! | 2 | a tier-one plate vest | 130 s |
//! | 3 | a tier-three plate vest | 120 s |
//! | 4 | tier-three helm, vest and leg guards | 110 s |
//!
//! **E, Rally** (active, cooldown): every friendly Bim within
//! [`RALLY_TILES`] of him when he calls it, himself included, takes less
//! damage and moves faster for its seconds, the reach fixed at the call as
//! Battle Cry's is.
//!
//! | rank | damage taken | move speed | lasts | cooldown |
//! |---|---|---|---|---|
//! | 1 | ×0.85 | ×1.10 | 6 s | 45 s |
//! | 2 | ×0.80 | ×1.15 | 7 s | 40 s |
//! | 3 | ×0.75 | ×1.20 | 8 s | 35 s |
//! | 4 | ×0.70 | ×1.20 | 9 s | 30 s |
//!
//! **R, Reinforcements** (the ultimate, pressed): he calls Bims of the
//! Republic's in, on free deck next to him within
//! [`REINFORCEMENT_REACH_TILES`] — fewer where the tiles are short — each
//! a classless crew member with the rank's auto rifle and nothing to wear,
//! for that mission alone (`World::reinforcements`), as often as
//! [`REINFORCEMENT_COOLDOWN`] (140 s at every rank) lets him — those already
//! called stay:
//!
//! | rank | Bims | weapon |
//! |---|---|---|
//! | 1 | 2 | auto rifle, tier 1 |
//! | 2 | 3 | auto rifle, tier 1 |
//! | 3 | 3 | auto rifle, tier 2 |
//! | 4 | 4 | auto rifle, tier 3 |
//!
//! Both cooldowns run on the mission clock, stop while paused, are ready
//! at every mission's start and are shortened by the cooldown relics as
//! every class cooldown is.
//!
//! **His cry and his rally lift every friendly Bim they reach, a
//! player's own steered Bims included** — the crew's own bots and the
//! reinforcements alike. See [`crate::commander`].
//!
//! Every multiplier is a named constant here; what each rank *does* is
//! `crate::deploy` and the world's step for the engineer, the room's
//! one shooter (`bims::combat::Skill`, `World::skill_of`) for the
//! soldier, `crate::medic` with the world's step for the medic, and
//! `crate::tank` with the same `Skill` and the room's own Riot Shields
//! for the tank, and `crate::commander` with the same `Skill` for
//! the commander. No strings: the app
//! names the classes and the abilities (`CLASS_NAMES`,
//! `ranked_ability`).

use crate::event::Refusal;

/// What a crew member is. Codes cross the seam and are never renumbered.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Class {
    /// No class: learns nothing, lays nothing. Everybody's until chosen.
    #[default]
    None = 0,
    /// The engineer: mines, a Healing Sentry, satchel charges and a
    /// sentry (task 154).
    Engineer = 1,
    /// The soldier: a line held, and grenades.
    Soldier = 2,
    /// The medic: a heal drone, triage, a heal beam and a healing
    /// circle (task 153).
    Medic = 3,
    /// The tank: a riot shield, plating, a reflect barrier and a Bastion
    /// (task 155).
    Tank = 4,
    /// The commander: a battle cry, a medic called in, a rally,
    /// reinforcements, and a cheaper hand at the dock.
    Commander = 5,
}

impl Class {
    pub const ALL: [Class; 6] = [
        Class::None,
        Class::Engineer,
        Class::Soldier,
        Class::Medic,
        Class::Tank,
        Class::Commander,
    ];

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Class> {
        Class::ALL.get(code as usize).copied()
    }

    /// Whether a player has to **unlock** it before it can be picked
    /// (feature 106, `crate::relic::Profile`): a won run unlocks relics,
    /// and a class added later can be marked here to be unlocked the same
    /// way. Every class there is now is open from the start.
    pub fn unlockable(self) -> bool {
        match self {
            Class::None
            | Class::Engineer
            | Class::Soldier
            | Class::Medic
            | Class::Tank
            | Class::Commander => false,
        }
    }

    /// What it wears on the deck (feature 81): the kit the room draws
    /// over the coverall. Drawing only — `World::hand_the_room_the_outfits`
    /// says it every step and nothing else reads it — so a class added
    /// later wants an arm here or it looks like everybody else.
    pub fn outfit(self) -> bims::character::Outfit {
        use bims::character::Outfit;
        match self {
            Class::None => Outfit::Plain,
            Class::Engineer => Outfit::Engineer,
            Class::Soldier => Outfit::Soldier,
            Class::Medic => Outfit::Medic,
            Class::Tank => Outfit::Tank,
            Class::Commander => Outfit::Commander,
        }
    }
}

/// What a class alone may do: the only thing [`can`] ever refuses for a
/// class. Never a job, an errand, a site or a weapon.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ability {
    /// Lay a mine or a Healing Sentry, pack one up: the engineer's.
    Deploy,
    /// Throw a satchel charge, and set them off: the engineer's E (task
    /// 154; the EMP's place, and the sandbags' slot).
    Satchel,
    /// Lay the sentry: the engineer's ultimate (task 127).
    Sentry,
    /// Charge and fire a stun shot: the soldier's E (October 2026).
    StunShot,
    /// Throw a grenade: the soldier's.
    Throw,
    /// Hold a heal beam on a crewmate: the medic's.
    Beam,
    /// Drop a heal drone: the medic's Q (task 153).
    HealDrone,
    /// Switch a healing circle on or off: the medic's ultimate (task
    /// 153).
    HealingCircle,
    /// Send what strikes him back on the striker: the tank's E (task 155).
    Reflect,
    /// Hold up a shield that bounces bolts back: the tank's Q (task 155).
    RiotShield,
    /// Throw a draining shield over every friend near him: the tank's
    /// ultimate (task 155).
    Bastion,
    /// Call a rally: the commander's.
    Rally,
    /// Call a battle cry: the commander's (task 129).
    BattleCry,
    /// Call reinforcements in: the commander's ultimate.
    Reinforce,
    /// Call a medic of the Republic's in: the commander's C.
    Medivac,
    /// Go on a rampage: the soldier's ultimate (task 124).
    Rampage,
}

impl Ability {
    pub const ALL: [Ability; 16] = [
        Ability::Deploy,
        Ability::Satchel,
        Ability::Sentry,
        Ability::StunShot,
        Ability::Throw,
        Ability::Beam,
        Ability::HealDrone,
        Ability::HealingCircle,
        Ability::Reflect,
        Ability::RiotShield,
        Ability::Bastion,
        Ability::Rally,
        Ability::Rampage,
        Ability::BattleCry,
        Ability::Reinforce,
        Ability::Medivac,
    ];
}

/// Whether a class may use an ability. The one rule a class gates
/// anything by: everything not an [`Ability`] is everybody's.
pub fn can(class: Class, ability: Ability) -> bool {
    match ability {
        Ability::Deploy | Ability::Satchel | Ability::Sentry => class == Class::Engineer,
        Ability::StunShot | Ability::Throw | Ability::Rampage => class == Class::Soldier,
        Ability::Beam | Ability::HealDrone | Ability::HealingCircle => class == Class::Medic,
        Ability::Reflect | Ability::RiotShield | Ability::Bastion => class == Class::Tank,
        Ability::Rally | Ability::BattleCry | Ability::Reinforce | Ability::Medivac => {
            class == Class::Commander
        }
    }
}

/// What an ability **spends**: a **counter** the world keeps for each
/// crew member (`World::charges_held`, task 127) — never a thing in a
/// pack — that comes back on a cooldown of its own rather than being
/// made at a bench or bought at a dock (features 88 and 90). **No class
/// crafts for its abilities**: a class brings its charges with it,
/// spends them, and waits — the engineer's mines, Healing Sentry and
/// satchel charges, the soldier's grenade, and whatever a class added
/// later spends.
///
/// The number of charges and the seconds one takes to come back are
/// [`World::charges`](crate::World::charges) and
/// [`World::charge_cooldown`](crate::World::charge_cooldown), since both
/// are a rank away from the tables here; `World::restock_charges` is the
/// one step that raises a counter back.
///
/// The engineer's sentry kit (1) went with the kits in task 127 — the
/// sentry is an ultimate on a cooldown now — and the medkit and the
/// bandage were everybody's charges until task 120 took the medicine
/// out of the game. Codes cross the seam, and a counter is indexed by the
/// code ([`Charge::CODES`] of them). The sandbags (0) and the EMP (4)
/// went in task 154, and their codes went to what took their slots — the
/// satchel charge E's, the mine Q's — so a counter keeps its length and a
/// crew without charges hashes as it did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Charge {
    /// The engineer's satchel charges (E, task 154).
    Satchel = 0,
    /// The soldier's grenade (feature 90, Q).
    Grenade = 2,
    /// The engineer's Healing Sentry (C, task 127).
    HealingSentry = 3,
    /// The engineer's mines (Q, task 154).
    Mine = 4,
}

impl Charge {
    pub const ALL: [Charge; 4] = [
        Charge::Satchel,
        Charge::Grenade,
        Charge::HealingSentry,
        Charge::Mine,
    ];

    /// How many codes there are, the free ones counted: what a counter or
    /// a cooldown kept a charge is sized by, indexed by [`Charge::code`].
    pub const CODES: usize = 5;

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn from_code(code: u32) -> Option<Charge> {
        Charge::ALL.into_iter().find(|c| c.code() == code)
    }

    /// The one class that spends it.
    pub fn class(self) -> Class {
        match self {
            Charge::Satchel | Charge::HealingSentry | Charge::Mine => Class::Engineer,
            Charge::Grenade => Class::Soldier,
        }
    }

    /// The ability slot whose rank says how many there are and how fast
    /// they come back — a charge nothing can spend yet, at rank nought,
    /// does not come back either.
    pub fn slot(self) -> u8 {
        match self {
            Charge::Grenade | Charge::Mine => SLOT_Q,
            Charge::HealingSentry => SLOT_C,
            Charge::Satchel => SLOT_E,
        }
    }
}

// The left-and-right talents went with the tank's ranked kit (task 139),
// the last class that had them: `Side`, `Talent` (codes 0 to 68, never
// to be reused), the pick levels and `Command::PickTalent`.

/// How many levels every class climbs (October 2026: twenty; the ranked
/// kits' sixteen from task 124 until then).
pub const LEVELS: u8 = 20;

/// The last level that gives a skill point: by the sixteenth every rank
/// of the kit is bought. The levels past it give the hit points every
/// level does and [`LEVEL_DAMAGE`] on the weapon instead.
pub const SKILL_LEVELS: u8 = 16;

/// Cumulative experience for each level, by level less one: nothing for
/// the first, 100 for the second, 16 740 for the sixteenth and 48 410 for
/// the twentieth — every class's (task 139; `RANKED_LEVEL_XP` until
/// then). Each level's step is the one before's times 1.3, from 100 for
/// the second, rounded to ten (October 2026, the player's curve; it was
/// 3 200 for the sixteenth and 4 920 for the twentieth, the step growing
/// by twenty and then forty, then 1.2 times for a day: 7 200 and
/// 15 470).
pub const LEVEL_XP: [u32; LEVELS as usize] = [
    0, 100, 230, 400, 620, 910, 1_280, 1_760, 2_390, 3_210, 4_270, 5_650, 7_440, 9_770, 12_800,
    16_740, 21_860, 28_510, 37_160, 48_410,
];

/// What every level past [`SKILL_LEVELS`] adds to a player's weapon
/// damage (October 2026, the player's number): five per cent a level,
/// so a fifth more at the twentieth.
pub const LEVEL_DAMAGE: f32 = 0.05;

/// What `level` multiplies a weapon's damage by: one up to
/// [`SKILL_LEVELS`], then [`LEVEL_DAMAGE`] more a level, added.
pub fn level_damage(level: u8) -> f32 {
    1.0 + LEVEL_DAMAGE * level.saturating_sub(SKILL_LEVELS) as f32
}

/// Hit points a level puts on a player's bar (October 2026), every
/// level counted from the first: 110 at the first, 260 at the
/// sixteenth, 300 at the twentieth (the player's numbers). [`level_health`] is the sum; the
/// room is told it every step (`bims::game::Game::set_level_health`).
pub const LEVEL_HEALTH: f32 = 10.0;

/// The hit points `level` puts on the bar.
pub fn level_health(level: u8) -> f32 {
    LEVEL_HEALTH * level as f32
}

/// The ranks an ability of a ranked kit has.
pub const MAX_RANK: u8 = 4;

/// The four ability slots of a ranked kit, by key: Q, C, E and R. What
/// `Command::RankUp`'s `ability_slot` and `Progress::ranks` count by.
pub const SLOT_Q: u8 = 0;
pub const SLOT_C: u8 = 1;
pub const SLOT_E: u8 = 2;
pub const SLOT_R: u8 = 3;
/// How many slots a ranked kit has.
pub const SLOTS: usize = 4;

/// The level each rank of the ultimate — slot R — wants.
pub const ULTIMATE_LEVELS: [u8; MAX_RANK as usize] = [6, 9, 12, 15];

/// Whether a class has a **ranked kit**: four abilities bought a rank at
/// a time with a skill point a level. Every class since the tank's
/// (task 139); [`Class::None`] alone has nothing to learn.
pub fn ranked(class: Class) -> bool {
    class != Class::None
}

/// The level a rank of an ability slot of a ranked kit wants: Q, C and E
/// rank `n` at level `2n − 1`, the ultimate R at [`ULTIMATE_LEVELS`].
/// `None` for a class with no ranked kit, a slot past R, and a rank that
/// is not one to buy (nought, or past [`MAX_RANK`]).
pub fn rank_level(class: Class, ability_slot: u8, rank: u8) -> Option<u8> {
    if !ranked(class) || ability_slot as usize >= SLOTS || !(1..=MAX_RANK).contains(&rank) {
        return None;
    }
    Some(if ability_slot == SLOT_R {
        ULTIMATE_LEVELS[rank as usize - 1]
    } else {
        2 * rank - 1
    })
}

/// A rank's number off a table, one a rank: `None` at rank nought, the
/// ability not learnt, and the top at anything past it. The tables of
/// the ultimates are [`OVERRIDE_RANK`] long, the rest [`MAX_RANK`].
pub fn by_rank<T: Copy, const N: usize>(table: [T; N], rank: u8) -> Option<T> {
    match rank {
        0 => None,
        r => Some(table[(r as usize).min(N) - 1]),
    }
}

/// The rank of the ultimate no skill point buys (October 2026): what an
/// *Override Core* carried makes of a fourth (`bims::module`), one over
/// whatever is bought. Every ultimate's table has a row for it.
pub const OVERRIDE_RANK: u8 = 5;

/// The ultimate's rank with `bought` ranks of it and an *Override Core*
/// carried or not: one higher with it, up to [`OVERRIDE_RANK`], and
/// nothing without a rank bought.
pub fn ultimate_rank(bought: u8, override_core: bool) -> u8 {
    if override_core && bought > 0 {
        (bought + 1).min(OVERRIDE_RANK)
    } else {
        bought
    }
}

/// What an enemy going down within the vicinity is worth, to every
/// classed crew member in range, whatever its class: once an enemy — out
/// cold, or dead without being down first, which is every machine. A
/// Manufacturer downed is worth it, a machine destroyed the same, and
/// neither is worth anything more when it dies after: the death's own
/// `XP_ENEMY_DEAD` (task 119) went, and this went from ten to twenty,
/// then down to fifteen.
pub const XP_ENEMY_DOWN: u32 = 15;
/// How far the vicinity reaches, in tiles.
pub const VICINITY_TILES: f32 = 50.0;

// --- the engineer's numbers (task 127) ----------------------------------------
//
// One number a rank, ranks one to four, read with [`by_rank`] like the
// soldier's. The deploy times are game minutes of working steps, one a
// second at 1× (what `deploy::DEPLOY_SANDBAG_MINUTES` was).

/// **Q, Mine** (task 154): what its blast does to every enemy in it at
/// its centre, a rank; half that at the edge.
/// Half again in October 2026, with every ability's damage, heal and
/// shield (was 40, 50, 60, 75).
pub const MINE_DAMAGE: [f32; 4] = [60.0, 75.0, 90.0, 112.5];
/// How far the blast reaches, in tiles, a rank.
pub const MINE_RADIUS: [f32; 4] = [1.5, 1.5, 2.0, 2.0];
/// How near an enemy has to come for it to go off, in tiles, at every
/// rank: one tile.
pub const MINE_TRIGGER: f32 = 1.0;
/// **Mine charges** a rank ([`Charge::Mine`]).
pub const MINE_CHARGES: [u32; 4] = [1, 1, 2, 2];
/// Seconds one spent mine charge takes to come back, a rank.
pub const MINE_COOLDOWN: [f64; 4] = [25.0, 22.0, 20.0, 18.0];
/// How many of one engineer's mines may lie at once, a rank: one more
/// laid takes the oldest up.
pub const MINE_STANDING: [u32; 4] = [4, 6, 6, 8];
/// Game minutes of working steps to lay one, at every rank: a second.
pub const MINE_MINUTES: f64 = 1.0;

/// **C, Healing Sentry**: what it heals, a share of the medic's beam
/// ([`HEAL_BEAM_HP`] an hour), a rank — so the two stay in proportion
/// when either is tuned.
pub const HEALING_SENTRY_RATE: [f32; 4] = [0.5, 0.75, 1.0, 1.25];
/// How far it heals, in tiles, a rank.
pub const HEALING_SENTRY_RADIUS: [f32; 4] = [3.0, 4.0, 4.0, 5.0];
/// Its health, one pool, a rank.
pub const HEALING_SENTRY_HEALTH: [f32; 4] = [120.0, 160.0, 200.0, 240.0];
/// Game minutes of working steps to lay one, a rank.
pub const HEALING_SENTRY_MINUTES: [f64; 4] = [6.0, 6.0, 4.0, 4.0];
/// **Healing Sentry charges** a rank — and how many of that engineer's
/// may stand, a further one laid destroying its oldest.
pub const HEALING_SENTRY_CHARGES: [u32; 4] = [1, 1, 1, 1];
/// Seconds one spent Healing Sentry charge takes to come back, a rank.
pub const HEALING_SENTRY_COOLDOWN: [f64; 4] = [60.0, 60.0, 50.0, 40.0];

/// **E, Satchel Charge** (task 154): what one bursting does to every
/// enemy in it at its centre, a rank; half that at the edge. Satchels
/// stacked on a tile each burst on their own.
/// Half again in October 2026, with every ability's damage, heal and
/// shield (was 35, 45, 60, 85).
pub const SATCHEL_DAMAGE: [f32; 4] = [52.5, 67.5, 90.0, 127.5];
/// How far one's blast reaches, in tiles, a rank.
pub const SATCHEL_RADIUS: [f32; 4] = [2.0, 2.5, 2.5, 3.0];
/// **Satchel charges** a rank ([`Charge::Satchel`]): two at every rank.
pub const SATCHEL_CHARGES: [u32; 4] = [2, 2, 2, 2];
/// Seconds one spent satchel charge takes to come back, a rank —
/// counted from the throw.
pub const SATCHEL_COOLDOWN: [f64; 4] = [30.0, 27.0, 24.0, 20.0];

/// **R, Sentry** (the ultimate): the minigun's tier, a rank.
pub const SENTRY_TIER: [bims::combat::Tier; 5] = [
    bims::combat::Tier::Two,
    bims::combat::Tier::Three,
    bims::combat::Tier::Three,
    bims::combat::Tier::Three,
    bims::combat::Tier::Three,
];
/// What its fire rate is multiplied by, a rank.
pub const SENTRY_FIRE_RATE: [f32; 5] = [1.0, 1.0, 1.5, 2.0, 2.0];
/// What its minigun's damage is multiplied by, a rank: half again
/// (October 2026, with every ability's damage, heal and shield) — the
/// minigun's own numbers are every gun's of the kind.
pub const SENTRY_DAMAGE: [f32; 5] = [1.5, 1.5, 1.5, 1.5, 1.5];
/// Its health, one pool, a rank: doubled in task 154, when the
/// sandbags went.
pub const SENTRY_HEALTH: [f32; 5] = [400.0, 500.0, 600.0, 800.0, 1000.0];
/// Tiles added to its minigun's range, a rank: five from the first. It
/// stands until it is destroyed — there is no timer.
pub const SENTRY_RANGE: [f32; 5] = [5.0, 5.0, 5.0, 5.0, 5.0];
/// Seconds of the mission clock from one laid to the next, a rank —
/// counted from the laying.
pub const SENTRY_COOLDOWN: [f64; 5] = ULTIMATE_COOLDOWN;
/// How many sentries one engineer may have standing, a rank: one, and two
/// at the [`OVERRIDE_RANK`] (October 2026) — the oldest goes when another
/// is laid past it.
pub const SENTRY_STANDING: [usize; 5] = [1, 1, 1, 1, 2];
/// Game minutes of working steps to lay it, at every rank. A hit does
/// not interrupt it.
pub const SENTRY_MINUTES: f64 = 3.0;

// --- the soldier's numbers (task 124) -----------------------------------------
//
// Every table is one number a rank, ranks one to four; read it with
// [`by_rank`], which answers `None` at rank nought.

/// **Q, Frag Grenade**: how far a grenade is thrown, in tiles, at every
/// rank.
pub const GRENADE_RANGE: f32 = 8.0;
/// Seconds from the throw to the burst, at every rank.
pub const GRENADE_FUSE: f32 = 2.0;
/// What the burst does at its centre, a rank; half that at the edge.
/// Half again in October 2026, with every ability's damage, heal and
/// shield (was 60, 75, 90, 110).
pub const GRENADE_DAMAGE: [f32; 4] = [90.0, 112.5, 135.0, 165.0];
/// How far the burst reaches, in tiles, a rank.
pub const GRENADE_RADIUS: [f32; 4] = [2.0, 2.5, 2.5, 3.0];
/// **Grenade charges** a rank (feature 90): how many grenades the soldier
/// fills back up to, one at a time on [`GRENADE_COOLDOWN`] a charge. A
/// soldier makes none and buys none — see [`Charge`].
pub const GRENADE_CHARGES: [u32; 4] = [1, 2, 2, 2];
/// Seconds of the mission clock one spent grenade charge takes to come
/// back, a rank.
pub const GRENADE_COOLDOWN: [f64; 4] = [30.0, 30.0, 24.0, 20.0];

/// **C, Weak Spot**: the chance a weapon hit on an enemy is critical, a
/// rank.
pub const WEAK_SPOT_CHANCE: [f32; 4] = [0.10, 0.12, 0.15, 0.20];
/// What a critical hit's flat damage is worth, a rank: 2.25 is 225%, so
/// one and a quarter times the flat damage is added to the hit.
/// Half again in October 2026, with every ability's damage, heal and
/// shield (was 1.5, 1.75, 2, 2.25).
pub const WEAK_SPOT_DAMAGE: [f32; 4] = [2.25, 2.625, 3.0, 3.375];

/// **E, Stun Shot**: seconds of the mission clock the shot charges for
/// before it fires, at every rank.
pub const STUN_SHOT_CHARGE: f64 = 2.0;
/// What the burst does to every enemy in it, a rank — the same at its
/// edge as at its centre.
/// Half again in October 2026, with every ability's damage, heal and
/// shield (was 15, 20, 25, 30).
pub const STUN_SHOT_DAMAGE: [f32; 4] = [22.5, 30.0, 37.5, 45.0];
/// How far the burst reaches, in tiles, a rank: the grenade's.
pub const STUN_SHOT_RADIUS: [f32; 4] = GRENADE_RADIUS;
/// Seconds every enemy in the burst is stunned, a rank.
pub const STUN_SHOT_STUN: [f32; 4] = [3.0, 3.0, 3.0, 3.0];
/// Seconds of the mission clock from one shot fired to the next, a rank.
pub const STUN_SHOT_COOLDOWN: [f64; 4] = [30.0, 27.0, 24.0, 20.0];
/// Seconds the shot flies from the muzzle to its burst.
pub const STUN_SHOT_FLIGHT: f32 = 0.15;

/// **R, Rampage**: how long it runs, in seconds of the mission clock, a
/// rank.
pub const RAMPAGE_SECONDS: [f64; 5] = [8.0, 10.0, 12.0, 12.0, 14.0];
/// What the fire rate is multiplied by while it runs, a rank.
/// Half again in October 2026, with every ability's damage, heal and
/// buff — the bonus or the cut (was 1.5, 1.75, 2, 2, 2.25).
pub const RAMPAGE_FIRE_RATE: [f32; 5] = [1.75, 2.125, 2.5, 2.5, 2.875];
/// What the damage taken is multiplied by while it runs, a rank.
/// Half again in October 2026, with every ability's damage, heal and
/// buff — the bonus or the cut (was 0.8, 0.75, 0.7, 0.7, 0.65).
pub const RAMPAGE_DAMAGE_TAKEN: [f32; 5] = [0.70, 0.625, 0.55, 0.55, 0.475];
/// Seconds of the mission clock from one Rampage to the next, a rank:
/// half of [`ULTIMATE_COOLDOWN`] (the player halved it).
pub const RAMPAGE_COOLDOWN: [f64; 5] = [35.0, 30.0, 25.0, 20.0, 17.5];
/// The rank from which an enemy the soldier downs during a Rampage adds
/// [`RAMPAGE_EXTEND_SECONDS`] to it.
pub const RAMPAGE_EXTEND_RANK: u8 = 4;
/// Seconds an enemy downed adds to a Rampage from its fourth rank.
pub const RAMPAGE_EXTEND_SECONDS: f64 = 1.0;
/// The most one Rampage is lengthened by, in seconds.
pub const RAMPAGE_EXTEND_MAX: f64 = 6.0;

// --- the medic's numbers (task 130) -------------------------------------------
//
// One number a rank, ranks one to four, read with [`by_rank`] like the
// soldier's, the engineer's and the commander's. The seconds are seconds
// of the mission clock, one a real second at 1×.

/// How far the heal beam reaches at its first rank, in tiles: the first
/// of [`HEAL_BEAM_RANGES`].
pub const HEAL_BEAM_RANGE: f32 = 6.0;
/// Per cent of its whole bar a beamed patient gains an hour of the clock
/// at the beam's first rank: three a second at 1× (task 130 made it two,
/// thirty until then; a share of the bar since October 2026, like every
/// heal; half again in October 2026, with every ability's damage and
/// heal). The ranks multiply it ([`HEAL_BEAM_RATE`]); the engineer's
/// Healing Sentry reads it unranked.
pub const HEAL_BEAM_HP: f32 = 180.0;
/// **Base trait**: how long a medic takes to revive a downed crewmate, in
/// seconds — anybody else's is `bims::health::REVIVE_SECONDS` (task 120).
/// A medic of the class and a field medic alike.
pub const MEDIC_REVIVE_SECONDS: f32 = 4.0;
/// **Base trait**: the share of its bar a Bim a medic of the class
/// revives gets up at — anybody else's is `bims::health::REVIVED_TO`.
/// A relic's *Rally Point* is its own.
pub const MEDIC_REVIVED_TO: f32 = 0.4;

/// **Q, Heal Drone** (task 153): per cent of its whole bar a second it
/// puts into the Bim it hovers over, a rank, before his Triage.
/// Half again in October 2026, with every ability's damage, heal and
/// shield (was 1.8, 2, 2.5, 3).
pub const HEAL_DRONE_HEAL: [f32; 4] = [2.7, 3.0, 3.75, 4.5];
/// Seconds of the mission clock a drone flies, a rank.
pub const HEAL_DRONE_SECONDS: [f64; 4] = [8.0, 10.0, 12.0, 14.0];
/// Seconds of the mission clock from one drone to the next, a rank.
pub const HEAL_DRONE_COOLDOWN: [f64; 4] = [25.0, 22.0, 20.0, 18.0];
/// How fast a drone flies, in tiles a second of the mission clock.
pub const HEAL_DRONE_SPEED: f32 = 6.0;
/// How near a drone has to be over its patient to heal it, in tiles.
pub const HEAL_DRONE_REACH: f32 = 0.6;

/// **C, Triage** (task 153): what a heal of his is lifted by on a Bim
/// at an empty bar, a rank — times the share of the bar it is missing, so
/// half of it at half a bar and none on a whole one.
/// Half again in October 2026, with every ability's damage, heal and
/// buff — the bonus or the cut (was 0.25, 0.4, 0.55, 0.7).
pub const TRIAGE: [f32; 4] = [0.375, 0.60, 0.825, 1.05];

/// What every heal of a medic carrying an *Override Core* is multiplied
/// by (task 153): the core's gift to his class, whatever rank his circle
/// is at.
pub const OVERRIDE_HEAL: f32 = 1.5;

/// **E, Heal Beam**: what [`HEAL_BEAM_HP`] is multiplied by, a rank.
pub const HEAL_BEAM_RATE: [f32; 4] = [1.0, 1.5, 2.0, 2.5];
/// How far the beam reaches, in tiles, a rank.
pub const HEAL_BEAM_RANGES: [f32; 4] = [HEAL_BEAM_RANGE, 7.0, 8.0, 9.0];
/// How far from the pointer, in tiles, the beam's key looks for a
/// friendly to link when nobody is under it (`World::beam_patient_near`):
/// the nearest the beam reaches within it is taken.
pub const HEAL_BEAM_PICK_REACH: f32 = 3.0;
/// How many patients the beam holds at once, each at the full rate, a
/// rank.
pub const HEAL_BEAM_PATIENTS: [usize; 4] = [1, 1, 1, 2];
/// The beam's rank from which each patient it holds also gains what the
/// medic's own items regenerate him by (`World::beam_item_rate`): a
/// *Reactor Heart*'s and a *Pressure Seal*'s hit points a second, the
/// Heart's quiet rate while he is unhurt.
pub const HEAL_BEAM_ITEM_RANK: u8 = 4;

/// **R, Healing Circle** (task 153): how far round him it reaches, in
/// tiles, a rank — the *Override Core*'s fifth the fourth's, since the
/// core's gift to a medic is [`OVERRIDE_HEAL`].
pub const HEALING_CIRCLE_RADIUS: [f32; 5] = [3.0, 3.5, 4.0, 4.5, 4.5];
/// The share of the circle's heal every enemy standing in it takes as
/// damage.
pub const HEALING_CIRCLE_BURN: f32 = 0.5;
/// What the circle drains its medic by, a share of what it heals a Bim by
/// before his Triage: two thirds, its heal as it was before every
/// ability's heal went half again (October 2026), so that made the circle
/// no dearer.
pub const HEALING_CIRCLE_COST: f32 = 2.0 / 3.0;
/// Seconds of the mission clock between two of the circle's burns: the
/// damage lands as a pulse, not a trickle of hits a step.
pub const HEALING_CIRCLE_PULSE: f64 = 0.5;

/// How long a revive takes (task 120): ten seconds, four for a `medic`.
pub fn revive_time(medic: bool) -> f32 {
    if medic {
        MEDIC_REVIVE_SECONDS
    } else {
        bims::health::REVIVE_SECONDS
    }
}

// --- the tank's numbers (task 139) --------------------------------------------
//
// One number a rank, ranks one to four, read with [`by_rank`] like the
// others'. The seconds are seconds of the mission clock, one a real
// second at 1×.

/// **Base trait**: what a piece of armour worn by a tank drains at: half
/// the damage it takes past its protection, so a piece absorbs twice as
/// much on him. Never doubled in the piece's own health, which moves
/// between Bims unchanged.
pub const TANK_DRAIN: f32 = 0.5;

/// **Q, Riot Shield**: the hit points the plate takes, a rank (task
/// 155).
/// Half again in October 2026, with every ability's damage, heal and
/// shield (was 20, 40, 80, 100).
pub const RIOT_SHIELD_HP: [f32; 4] = [30.0, 60.0, 120.0, 150.0];
/// Hit points a second the shield restores, stowed or up, a rank — half
/// a hit point at the first, two at the last.
/// Half again in October 2026, with every ability's damage, heal and
/// shield (was 0.5, 1, 1.5, 2).
pub const RIOT_SHIELD_REGEN: [f32; 4] = [0.75, 1.5, 2.25, 3.0];
/// Seconds of the mission clock a shield held up must go unstruck before
/// it restores; a stowed one restores at once.
pub const RIOT_SHIELD_REGEN_DELAY: f64 = 5.0;
/// Seconds of the mission clock a broken shield cannot be raised again,
/// from the hit that broke it — shorter with the cooldown relics and
/// items, as every class cooldown is. It restores all the while.
pub const RIOT_SHIELD_BROKEN_COOLDOWN: f64 = 10.0;

/// **C, Plated**: what the damage of a hit on him is multiplied by, a
/// rank — before the armour, as Rampage's is.
/// Half again in October 2026, with every ability's damage, heal and
/// buff — the bonus or the cut (was 0.9, 0.85, 0.8, 0.75).
pub const PLATED_DAMAGE_TAKEN: [f32; 4] = [0.85, 0.775, 0.70, 0.625];
/// And the per cent of his whole bar a second he mends, a rank (task 155).
/// Half again in October 2026, with every ability's damage, heal and
/// shield (was 0.2, 0.8, 1.4, 2).
pub const PLATED_REGEN: [f32; 4] = [0.3, 1.2, 2.1, 3.0];
/// The rank from which his armour drain is multiplied by
/// [`FORTRESS_DRAIN`] again (what *fortress* was).
pub const FORTRESS_RANK: u8 = 4;
/// What his armour drain is multiplied by again from [`FORTRESS_RANK`]:
/// a quarter of the rate in all.
pub const FORTRESS_DRAIN: f32 = 0.5;

/// **E, Reflect Barrier**: seconds of the mission clock it runs, a rank
/// (task 155).
pub const REFLECT_SECONDS: [f64; 4] = [3.0, 4.0, 5.0, 6.0];
/// Seconds of the mission clock from one barrier to the next, a rank.
pub const REFLECT_COOLDOWN: [f64; 4] = [20.0, 18.0, 16.0, 14.0];
/// The share of an enemy's hit on him that goes back on the striker:
/// half as much again as the hit.
/// Half again in October 2026, with every ability's damage, heal and
/// shield (was 1).
pub const REFLECT_SHARE: f32 = 1.5;

/// **R, Bastion**: how far it reaches, in tiles, a rank — the
/// *Override Core*'s fifth a tile further (task 155).
pub const BASTION_RADIUS: [f32; 5] = [6.0, 7.0, 8.0, 9.0, 10.0];
/// The hit points of the shield it throws over every friend in reach, a
/// rank (the player's words, October 2026: 600, 800, 1000, 1200; it was
/// 1000 at every rank). The *Override Core*'s fifth is the fourth's.
/// Half again in October 2026, with every ability's damage, heal and
/// shield (was 600, 800, 1000, 1200).
pub const BASTION_HP: [f32; 5] = [900.0, 1200.0, 1500.0, 1800.0, 1800.0];
/// What that shield loses a second whatever strikes it, a rank.
/// Half again in October 2026 with the hit points (was 60, 80, 100,
/// 100), so the seconds are what they were.
pub const BASTION_DRAIN: [f32; 5] = [90.0, 120.0, 150.0, 150.0, 150.0];
/// Seconds of the mission clock the shield, and the fifth rank's haste,
/// last at most, a rank: its hit points over its drain.
pub const BASTION_SECONDS: [f64; 5] = [10.0, 10.0, 10.0, 12.0, 12.0];
/// What the pace of everybody it reached is multiplied by at the
/// *Override Core*'s fifth rank, while the shield's seconds run.
/// Half again in October 2026, with every ability's damage, heal and
/// buff — the bonus or the cut (was 1.5).
pub const BASTION_HASTE: f32 = 1.75;
/// Seconds of the mission clock from one Bastion to the next, a rank.
pub const BASTION_COOLDOWN: [f64; 5] = ULTIMATE_COOLDOWN;

/// Every timed ultimate's cooldown, a rank: seventy seconds at the first
/// and ten fewer a rank after it (the Sentry and Bastion; the Rampage
/// has half of it, [`RAMPAGE_COOLDOWN`]; the
/// Cloak's own [`CLOAK_COOLDOWN`] is under it already, and Reinforcements
/// come once a mission).
pub const ULTIMATE_COOLDOWN: [f64; 5] = [70.0, 60.0, 50.0, 40.0, 35.0];

// --- the commander's numbers (task 129) ---------------------------------------
//
// One number a rank, ranks one to four, read with [`by_rank`] like the
// soldier's and the engineer's. The seconds are seconds of the mission
// clock, one a real second at 1×.

/// **Q, Battle Cry**: how far it reaches when he calls it, in tiles, at
/// every rank.
pub const BATTLE_CRY_TILES: f32 = 8.0;
/// What the fire rate of a Bim it reached is multiplied by, a rank.
/// Half again in October 2026, with every ability's damage, heal and
/// buff — the bonus or the cut (was 1.25, 1.3, 1.35, 1.4).
pub const BATTLE_CRY_FIRE_RATE: [f32; 4] = [1.375, 1.45, 1.525, 1.60];
/// Seconds of the mission clock it runs, a rank.
pub const BATTLE_CRY_SECONDS: [f64; 4] = [3.0, 4.0, 5.0, 6.0];
/// Seconds of the mission clock from one cry to the next, a rank.
pub const BATTLE_CRY_COOLDOWN: [f64; 4] = [20.0, 18.0, 16.0, 14.0];

/// **C, Medivac**: seconds of the mission clock from one medic called in
/// to the next, a rank — ready at every mission's start, shortened by the
/// cooldown relics as every class cooldown is.
pub const MEDIVAC_COOLDOWN: [f64; 4] = [140.0, 130.0, 120.0, 110.0];
/// The tier of the medic's armour, a rank: none at the first, tier one
/// at the second, tier three from the third. (A helm and leg guards came
/// on at the fourth too, until a Bim wore one armour, October 2026.)
pub const MEDIVAC_VEST: [Option<bims::combat::Tier>; 4] = [
    None,
    Some(bims::combat::Tier::One),
    Some(bims::combat::Tier::Three),
    Some(bims::combat::Tier::Three),
];

/// **E, Rally**: how far it reaches when he calls it, in tiles, at every
/// rank.
pub const RALLY_TILES: f32 = 8.0;
/// What the damage a Bim it reached takes is multiplied by, a rank.
/// Half again in October 2026, with every ability's damage, heal and
/// buff — the bonus or the cut (was 0.85, 0.8, 0.75, 0.7).
pub const RALLY_DAMAGE_TAKEN: [f32; 4] = [0.775, 0.70, 0.625, 0.55];
/// What the pace of a Bim it reached is multiplied by, a rank.
/// Half again in October 2026, with every ability's damage, heal and
/// buff — the bonus or the cut (was 1.1, 1.15, 1.2, 1.2).
pub const RALLY_PACE: [f32; 4] = [1.15, 1.225, 1.30, 1.30];
/// Seconds of the mission clock it runs, a rank.
pub const RALLY_SECONDS: [f64; 4] = [6.0, 7.0, 8.0, 9.0];
/// Seconds of the mission clock from one rally to the next, a rank.
pub const RALLY_COOLDOWN: [f64; 4] = [45.0, 40.0, 35.0, 30.0];

/// **R, Reinforcements**: how many Bims one call brings in, a rank.
pub const REINFORCEMENTS: [u32; 5] = [2, 3, 3, 4, 5];
/// The tier of the auto rifle each carries, a rank.
pub const REINFORCEMENT_TIER: [bims::combat::Tier; 5] = [
    bims::combat::Tier::One,
    bims::combat::Tier::One,
    bims::combat::Tier::Two,
    bims::combat::Tier::Three,
    bims::combat::Tier::Three,
];
/// The armour each reinforcement wears, a rank: none until the
/// [`OVERRIDE_RANK`] (October 2026), a tier-one plate there.
pub const REINFORCEMENT_VEST: [Option<bims::combat::Tier>; 5] =
    [None, None, None, None, Some(bims::combat::Tier::One)];
/// What the weapon damage of everybody a commander calls in — his R's
/// soldiers and his Medivac's medic — is multiplied by: half again
/// (October 2026, with every ability's damage, heal and buff), since
/// their rifle and pistol are every gun's of the kind.
pub const REINFORCEMENT_DAMAGE: f32 = 1.5;
/// How far from him, in tiles, a free tile of deck is looked for to stand
/// one on: fewer arrive where fewer are found.
pub const REINFORCEMENT_REACH_TILES: f32 = 5.0;
/// Seconds of the mission clock from one call for reinforcements to the
/// next, at every rank: ready at every mission's start, shortened by the
/// cooldown relics as every class cooldown is.
pub const REINFORCEMENT_COOLDOWN: f64 = 140.0;

/// The level `xp` makes: one to twenty on [`LEVEL_XP`], whatever the
/// class.
pub fn level_of(xp: u32) -> u8 {
    LEVEL_XP.iter().filter(|&&need| xp >= need).count().max(1) as u8
}

/// One crew member's way through its class: its experience, and the
/// ranks it has bought, a count a slot of its kit — Q, C, E and R.
#[derive(Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Progress {
    /// Cumulative experience.
    pub xp: u32,
    /// The ranks bought, a slot: Q, C, E and R ([`SLOT_Q`] …), nought to
    /// [`MAX_RANK`] each. Nought for a classless crew member.
    #[cfg_attr(feature = "serde", serde(default))]
    pub ranks: [u8; SLOTS],
}

impl Progress {
    /// The level the experience makes.
    pub fn level(&self) -> u8 {
        level_of(self.xp)
    }

    /// Experience still wanted for the next level; nought at the top.
    pub fn to_next(&self) -> u32 {
        let level = self.level();
        if level as usize >= LEVEL_XP.len() {
            return 0;
        }
        LEVEL_XP[level as usize].saturating_sub(self.xp)
    }

    /// `xp` more: every level reached by it, lowest first, for the world
    /// to say. Nothing past the top level.
    pub fn gain(&mut self, xp: u32) -> Vec<u8> {
        let was = self.level();
        self.xp = self.xp.saturating_add(xp);
        let now = self.level();
        ((was + 1)..=now).collect()
    }

    /// The rank bought of an ability slot: nought for none, and for a
    /// slot past R.
    pub fn rank(&self, ability_slot: u8) -> u8 {
        self.ranks.get(ability_slot as usize).copied().unwrap_or(0)
    }

    /// Every rank bought, of every slot: the skill points spent.
    pub fn ranks_bought(&self) -> u8 {
        self.ranks.iter().sum()
    }

    /// Skill points not spent (task 124): a point a level reached, the
    /// first included, up to [`SKILL_LEVELS`], less every rank bought.
    /// Nought for a classless crew member.
    pub fn points(&self, class: Class) -> u8 {
        if !ranked(class) {
            return 0;
        }
        self.level()
            .min(SKILL_LEVELS)
            .saturating_sub(self.ranks_bought())
    }

    /// Whether a rank of that slot may be bought now, and the rank it
    /// would be, or why not — in the order the refusals are said: a class
    /// (`NoClass`), a slot of the kit (`NoRankedKit`), a point to spend
    /// (`NoSkillPoint`), the slot not at [`MAX_RANK`] (`TopRank`), and the
    /// level the rank wants reached (`RankLocked`).
    pub fn can_rank_up(&self, class: Class, ability_slot: u8) -> Result<u8, Refusal> {
        if !ranked(class) {
            return Err(Refusal::NoClass);
        }
        if ability_slot as usize >= SLOTS {
            return Err(Refusal::NoRankedKit);
        }
        if self.points(class) == 0 {
            return Err(Refusal::NoSkillPoint);
        }
        let rank = self.rank(ability_slot);
        if rank >= MAX_RANK {
            return Err(Refusal::TopRank);
        }
        match rank_level(class, ability_slot, rank + 1) {
            Some(want) if self.level() >= want => Ok(rank + 1),
            _ => Err(Refusal::RankLocked),
        }
    }

    /// Buy a rank of that slot with a skill point: the rank it is now,
    /// or why not ([`Progress::can_rank_up`]). Never taken back.
    pub fn rank_up(&mut self, class: Class, ability_slot: u8) -> Result<u8, Refusal> {
        let rank = self.can_rank_up(class, ability_slot)?;
        self.ranks[ability_slot as usize] = rank;
        Ok(rank)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_class_owns_its_abilities_and_nothing_else() {
        assert!(can(Class::Engineer, Ability::Deploy));
        assert!(!can(Class::Engineer, Ability::StunShot));
        assert!(!can(Class::Engineer, Ability::Throw));
        assert!(can(Class::Engineer, Ability::Satchel) && can(Class::Engineer, Ability::Sentry));
        assert!(!can(Class::Soldier, Ability::Satchel) && !can(Class::Medic, Ability::Sentry));
        assert!(!can(Class::Soldier, Ability::Deploy));
        assert!(can(Class::Soldier, Ability::StunShot));
        assert!(can(Class::Soldier, Ability::Throw));
        assert!(!can(Class::Soldier, Ability::Beam));
        assert!(can(Class::Medic, Ability::Beam));
        assert!(can(Class::Medic, Ability::HealDrone) && can(Class::Medic, Ability::HealingCircle));
        assert!(!can(Class::Medic, Ability::Deploy));
        assert!(!can(Class::Medic, Ability::Throw));
        assert!(!can(Class::Engineer, Ability::HealDrone));
        assert!(!can(Class::Soldier, Ability::HealingCircle));
        assert!(can(Class::Tank, Ability::Reflect));
        assert!(can(Class::Tank, Ability::RiotShield));
        assert!(can(Class::Tank, Ability::Bastion));
        assert!(!can(Class::Soldier, Ability::Bastion));
        assert!(!can(Class::Tank, Ability::Beam));
        assert!(!can(Class::Tank, Ability::Deploy));
        assert!(!can(Class::Medic, Ability::Reflect));
        assert!(!can(Class::Soldier, Ability::RiotShield));
        assert!(can(Class::Commander, Ability::Rally));
        assert!(can(Class::Commander, Ability::BattleCry));
        assert!(!can(Class::Soldier, Ability::BattleCry));
        assert!(!can(Class::Commander, Ability::RiotShield));
        assert!(!can(Class::Commander, Ability::Deploy));
        assert!(!can(Class::Medic, Ability::Rally));
        for ability in Ability::ALL {
            assert!(!can(Class::None, ability));
        }
        for class in Class::ALL {
            assert_eq!(Class::from_code(class.code()), Some(class));
        }
    }

    /// Every class is a ranked kit (task 139): twenty levels, the
    /// sixteenth at 16 740 and the top at 48 410, a point a level up to the
    /// sixteenth, and every rank's gate refusing a level early and
    /// allowing on the level — the tank's as the soldier's.
    #[test]
    fn every_class_climbs_twenty_levels_and_buys_a_rank_a_point() {
        assert_eq!(LEVELS, 20);
        assert_eq!(LEVEL_XP.len(), 20);
        assert_eq!(LEVEL_XP[15], 16_740, "the sixteenth at 16 740");
        assert_eq!(LEVEL_XP[19], 48_410, "the top at 48 410");
        assert!(LEVEL_XP.windows(2).all(|w| w[0] < w[1]));
        // Each step is the one before's times 1.3, rounded to ten.
        let mut step = 100.0_f64;
        for w in LEVEL_XP.windows(2) {
            assert_eq!(w[1] - w[0], (step / 10.0).round() as u32 * 10);
            step *= 1.3;
        }
        assert_eq!(level_of(0), 1);
        assert_eq!(level_of(99), 1);
        assert_eq!(level_of(16_739), 15);
        assert_eq!(level_of(16_740), 16);
        assert_eq!(level_of(48_409), 19);
        assert_eq!(level_of(1_000_000), 20);
        for class in Class::ALL {
            assert_eq!(ranked(class), class != Class::None, "{class:?}");
        }
        let mut p = Progress::default();
        assert_eq!(p.points(Class::None), 0, "a classless bot learns nothing");
        assert_eq!(
            p.can_rank_up(Class::None, SLOT_Q),
            Err(Refusal::NoClass),
            "nor ranks anything"
        );
        for class in Class::ALL.into_iter().filter(|&c| c != Class::None) {
            assert_eq!(p.points(class), 1, "{class:?}: a point at the first level");
            assert_eq!(p.can_rank_up(class, SLOT_Q), Ok(1), "{class:?}");
            assert_eq!(p.can_rank_up(class, SLOT_R), Err(Refusal::RankLocked));
            assert_eq!(p.can_rank_up(class, 4), Err(Refusal::NoRankedKit));
        }
        let s = Class::Tank;
        assert_eq!(p.rank_up(s, SLOT_Q), Ok(1));
        assert_eq!(p.rank_up(s, SLOT_C), Err(Refusal::NoSkillPoint));
        assert_eq!(p.gain(16_740), (2..=16).collect::<Vec<u8>>());
        assert_eq!(p.to_next(), 5_120);
        assert_eq!(p.points(s), 15);
        // The levels past the sixteenth give no point: sixteen buy every
        // rank there is.
        assert_eq!(p.gain(31_670), (17..=20).collect::<Vec<u8>>());
        assert_eq!(p.to_next(), 0);
        assert_eq!(p.points(s), 15);
        assert_eq!(
            SKILL_LEVELS as usize,
            SLOTS * MAX_RANK as usize,
            "a point a rank"
        );
        assert_eq!(level_damage(1), 1.0);
        assert_eq!(level_damage(16), 1.0);
        assert!((level_damage(17) - 1.05).abs() < 1e-6);
        assert!((level_damage(20) - 1.20).abs() < 1e-6);
        assert_eq!(level_health(20), 200.0);
        for _ in 0..3 {
            p.rank_up(s, SLOT_Q).unwrap();
        }
        assert_eq!(p.rank(SLOT_Q), MAX_RANK);
        assert_eq!(p.rank_up(s, SLOT_Q), Err(Refusal::TopRank));
        // Every gate, of every class: refused a level early, allowed on
        // the level.
        for class in Class::ALL.into_iter().filter(|&c| c != Class::None) {
            for slot in 0..SLOTS as u8 {
                for rank in 1..=MAX_RANK {
                    let want = rank_level(class, slot, rank).unwrap();
                    let mut q = Progress::default();
                    q.ranks[slot as usize] = rank - 1;
                    if want > 1 {
                        q.xp = LEVEL_XP[want as usize - 2];
                        assert_eq!(
                            q.can_rank_up(class, slot),
                            Err(Refusal::RankLocked),
                            "{class:?} {slot} {rank}"
                        );
                    }
                    q.xp = LEVEL_XP[want as usize - 1];
                    assert_eq!(
                        q.can_rank_up(class, slot),
                        Ok(rank),
                        "{class:?} {slot} {rank}"
                    );
                }
            }
        }
        assert_eq!(rank_level(s, SLOT_E, 3), Some(5));
        assert_eq!(rank_level(s, SLOT_R, 1), Some(6));
        assert_eq!(rank_level(s, SLOT_R, 4), Some(15));
        assert_eq!(rank_level(s, SLOT_Q, 5), None);
        assert_eq!(rank_level(Class::None, SLOT_Q, 1), None);
        assert_eq!(by_rank(GRENADE_DAMAGE, 0), None);
        assert_eq!(by_rank(GRENADE_DAMAGE, 4), Some(165.0));
    }

    /// The tank's tables (task 155), as the player gave them.
    #[test]
    fn the_tank_s_tables_are_the_spec_s() {
        assert_eq!(RIOT_SHIELD_HP, [30.0, 60.0, 120.0, 150.0]);
        assert_eq!(RIOT_SHIELD_REGEN, [0.75, 1.5, 2.25, 3.0]);
        assert_eq!(
            (RIOT_SHIELD_REGEN_DELAY, RIOT_SHIELD_BROKEN_COOLDOWN),
            (5.0, 10.0)
        );
        assert_eq!(PLATED_DAMAGE_TAKEN, [0.85, 0.775, 0.70, 0.625]);
        assert_eq!(PLATED_REGEN, [0.3, 1.2, 2.1, 3.0]);
        assert_eq!(TANK_DRAIN * FORTRESS_DRAIN, 0.25, "a quarter in all");
        assert_eq!(REFLECT_SHARE, 1.5, "half as much again as he takes");
        assert_eq!(BASTION_HP, [900.0, 1200.0, 1500.0, 1800.0, 1800.0]);
        assert_eq!(BASTION_DRAIN, [90.0, 120.0, 150.0, 150.0, 150.0]);
        for r in 0..5 {
            assert_eq!(
                BASTION_SECONDS[r],
                (BASTION_HP[r] / BASTION_DRAIN[r]) as f64,
                "rank {}: the shield drains out in its seconds",
                r + 1
            );
        }
        assert_eq!(BASTION_RADIUS[0], 6.0, "six tiles at the first rank");
        assert_eq!(
            BASTION_HASTE, 1.75,
            "three quarters again as fast with the core"
        );
        assert_eq!(BASTION_COOLDOWN, [70.0, 60.0, 50.0, 40.0, 35.0]);
    }
}
