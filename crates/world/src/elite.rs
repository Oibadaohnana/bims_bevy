//! Elites: one system in ten holds a fight above the rest.
//!
//! **Stateless, like the trader's and the Manufacturers' rolls.** A star's
//! system is an elite's where the galaxy's seed rolls it
//! ([`data::ELITE_SYSTEM_CHANCE`] in a hundred, a salt of its own) and it
//! is not the crew's own ([`holds`]). The elite is the station the system
//! offers (`World::elite_station`, the primary of task 135): the
//! machines' from the first day — never the Manufacturers', never an
//! outpost's coin — so it is always an attack, and the town beside it the
//! defence.
//!
//! **The fight**: at least [`data::ELITE_WAVES`] waves, and in wave
//! [`data::ELITE_GUARDIAN_WAVE`] its Guardians ([`with_guardian`]): in the
//! floor's tier-one zone a Guardian a tier — one at tier one, two at tier
//! two, three at tier three ([`data::ELITE_GUARDIANS`]); in the tier-two
//! and tier-three zones the scaling's `tier2_guardians` / `tier3_guardians`
//! for each player (October 2026), with `tier2_elites` / `tier3_elites`
//! Bombers on top ([`with_bombers`]) and a Conductor ([`with_conductor`]).
//! **The reward**: only an elite drops relics — the
//! reward screen on its clear and a cache on its research desk; every
//! other fight drops none (`World::relics_on_leaving`, `World::infest`).

use bims::combat::Tier;
use bims::droid::DroidKind;

use crate::data;

/// Whether the galaxy's roll makes a star's system an elite's: odds of
/// [`data::ELITE_SYSTEM_CHANCE`] in a hundred, off the galaxy's seed and
/// the star — no stream a fight draws from.
pub fn rolled(galaxy_seed: u64, star: u32) -> bool {
    let seed =
        worldgen::rng::mix(galaxy_seed ^ 0x_454C_4954_4553) ^ worldgen::rng::mix(u64::from(star));
    worldgen::rng::Rng::new(seed).below(100) < data::ELITE_SYSTEM_CHANCE
}

/// Whether `star`'s system holds an elite: [`rolled`], and not the crew's
/// `home` star.
pub fn holds(galaxy_seed: u64, home: u32, star: u32) -> bool {
    star != home && rolled(galaxy_seed, star)
}

/// The Guardians an elite's Guardian wave holds in a tier-one zone, by
/// the site's `tier`: [`data::ELITE_GUARDIANS`].
pub fn guardians_at(tier: Tier) -> u32 {
    data::ELITE_GUARDIANS[tier.code() as usize - 1]
}

/// A wave's machines at an elite: in wave [`data::ELITE_GUARDIAN_WAVE`] at
/// least `want` Guardians ([`guardians_at`] in a tier-one zone, the
/// scaling's per player in the others), each one not there already in a
/// Trooper's place (the last
/// one's, so the other Troopers' arms are dealt as before) — or the last
/// machine's but a Guardian where there is no Trooper — and never more
/// than the wave has machines. After the Wardens. Any other wave as it is —
/// and a plain wave has no Guardian ([`bims::droid::wave_kinds`]), so these
/// are the only Guardians a run meets.
pub fn with_guardian(mut kinds: Vec<DroidKind>, wave: u32, want: u32) -> Vec<DroidKind> {
    if wave != data::ELITE_GUARDIAN_WAVE {
        return kinds;
    }
    let want = want as usize;
    while kinds.iter().filter(|&&k| k == DroidKind::Guardian).count() < want {
        let Some(at) = kinds
            .iter()
            .rposition(|&k| k == DroidKind::Trooper)
            .or_else(|| kinds.iter().rposition(|&k| k != DroidKind::Guardian))
        else {
            break;
        };
        kinds.remove(at);
        let at = kinds
            .iter()
            .take_while(|&&k| k == DroidKind::Warden)
            .count();
        kinds.insert(at, DroidKind::Guardian);
    }
    kinds
}

/// An elite's **Bombers** besides its Guardians (October 2026, the
/// player's: "If the other elite parameter is set to 1 then one bomber
/// spawns"): `bombers` of them on top of wave
/// [`data::ELITE_GUARDIAN_WAVE`], at its end with the wave's own; any
/// other wave as it is.
pub fn with_bombers(mut kinds: Vec<DroidKind>, wave: u32, bombers: u32) -> Vec<DroidKind> {
    if wave == data::ELITE_GUARDIAN_WAVE {
        kinds.extend(std::iter::repeat_n(DroidKind::Bomber, bombers as usize));
    }
    kinds
}

/// An elite's **Conductor** (task 157): from the floor's tier-two zone on
/// (`zone`, `World::zone_tier`), in wave [`data::ELITE_GUARDIAN_WAVE`], one
/// in the last Trooper's place — or the last machine's but a Guardian's
/// where there is no Trooper — put after the Wardens and the Guardians, the
/// Guardians left as they came. A wave with one already, any other wave
/// and a tier-one zone's as they are.
pub fn with_conductor(mut kinds: Vec<DroidKind>, wave: u32, zone: Tier) -> Vec<DroidKind> {
    if wave != data::ELITE_GUARDIAN_WAVE
        || zone < Tier::Two
        || kinds.contains(&DroidKind::Conductor)
    {
        return kinds;
    }
    let Some(at) = kinds
        .iter()
        .rposition(|&k| k == DroidKind::Trooper)
        .or_else(|| kinds.iter().rposition(|&k| k != DroidKind::Guardian))
    else {
        return kinds;
    };
    kinds.remove(at);
    let at = kinds
        .iter()
        .take_while(|&&k| matches!(k, DroidKind::Warden | DroidKind::Guardian))
        .count();
    kinds.insert(at, DroidKind::Conductor);
    kinds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tier_two_elite_s_guardian_wave_has_a_conductor_in_a_trooper_s_place() {
        for n in [3u32, 6, 12] {
            let plain = bims::droid::wave_kinds(n);
            let guarded = with_guardian(plain.clone(), 2, guardians_at(Tier::Two));
            let kinds = with_conductor(guarded.clone(), 2, Tier::Two);
            assert_eq!(kinds.len(), guarded.len(), "a wave of {n}");
            assert_eq!(
                kinds.iter().filter(|&&k| k == DroidKind::Conductor).count(),
                1
            );
            let guardians =
                |k: &[DroidKind]| k.iter().filter(|&&k| k == DroidKind::Guardian).count();
            assert_eq!(guardians(&kinds), guardians(&guarded), "the Guardians stay");
            // Wave one, a tier-one zone and a second call leave it be.
            assert_eq!(with_conductor(plain.clone(), 1, Tier::Two), plain);
            assert_eq!(with_conductor(guarded.clone(), 2, Tier::One), guarded);
            assert_eq!(with_conductor(kinds.clone(), 2, Tier::Three), kinds);
        }
    }

    #[test]
    fn about_one_system_in_ten_is_an_elite_s_and_never_home() {
        let stars = 600u32;
        for seed in [1u64, 7, 42] {
            let got = (0..stars).filter(|&s| rolled(seed, s)).count() as u32;
            let percent = got * 100 / stars;
            assert!((6..=14).contains(&percent), "seed {seed}: {percent}%");
            for star in 0..stars {
                assert_eq!(rolled(seed, star), rolled(seed, star));
                assert!(!holds(seed, star, star), "never the crew's own");
            }
        }
    }

    fn guardians(kinds: &[DroidKind]) -> usize {
        kinds.iter().filter(|&&k| k == DroidKind::Guardian).count()
    }

    #[test]
    fn the_second_wave_has_one_guardian_a_tier_in_a_trooper_s_place() {
        for (tier, want) in [(Tier::One, 1), (Tier::Two, 2), (Tier::Three, 3)] {
            for n in 1..=16u32 {
                let plain = bims::droid::wave_kinds(n);
                let tier = guardians_at(tier);
                assert_eq!(with_guardian(plain.clone(), 1, tier), plain, "wave one");
                assert_eq!(with_guardian(plain.clone(), 3, tier), plain);
                let second = with_guardian(plain.clone(), 2, tier);
                assert_eq!(second.len(), plain.len(), "as many machines");
                assert_eq!(
                    guardians(&second),
                    want.min(n as usize),
                    "{tier:?} {n}: {second:?}"
                );
                // The Wardens, then the Guardians, then the rest.
                let wardens = second
                    .iter()
                    .take_while(|&&k| k == DroidKind::Warden)
                    .count();
                assert!(
                    second[wardens..]
                        .iter()
                        .take(guardians(&second))
                        .all(|&k| k == DroidKind::Guardian),
                    "after the Wardens: {second:?}"
                );
                // The Troopers given up are the last ones.
                let troopers =
                    |k: &[DroidKind]| k.iter().filter(|&&k| k == DroidKind::Trooper).count();
                assert!(troopers(&second) <= troopers(&plain));
            }
        }
        // A wave with Guardians enough already (a forced one) is left alone.
        let mut big = bims::droid::wave_kinds(32);
        big[0] = DroidKind::Guardian;
        assert_eq!(with_guardian(big.clone(), 2, 1), big);
    }

    #[test]
    fn the_elite_bombers_come_on_top_of_the_guardian_wave_alone() {
        let plain = bims::droid::wave_kinds(8);
        let bombers = |k: &[DroidKind]| k.iter().filter(|&&k| k == DroidKind::Bomber).count();
        for n in 0..=3u32 {
            let second = with_bombers(plain.clone(), 2, n);
            assert_eq!(second.len(), plain.len() + n as usize);
            assert_eq!(bombers(&second), n as usize);
            assert_eq!(with_bombers(plain.clone(), 1, n), plain, "wave one");
        }
    }
}
