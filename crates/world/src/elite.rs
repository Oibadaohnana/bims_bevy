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
//! [`data::ELITE_GUARDIAN_WAVE`] a Guardian a tier — one at tier one, two
//! at tier two, three at tier three ([`data::ELITE_GUARDIANS`],
//! [`with_guardian`]). **The reward**: only an elite drops relics — the
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

/// A wave's machines at an elite: in wave [`data::ELITE_GUARDIAN_WAVE`] at
/// least [`data::ELITE_GUARDIANS`] Guardians for `tier` (one, two or
/// three), each one the tier did not give in a Trooper's place (the last
/// one's, so the other Troopers' arms are dealt as before) — or the last
/// machine's but a Guardian where there is no Trooper — and never more
/// than the wave has machines. After the Wardens, where
/// [`bims::droid::wave_kinds`] puts its Guardians. Any other wave as it is.
pub fn with_guardian(mut kinds: Vec<DroidKind>, wave: u32, tier: Tier) -> Vec<DroidKind> {
    if wave != data::ELITE_GUARDIAN_WAVE {
        return kinds;
    }
    let want = data::ELITE_GUARDIANS[tier.code() as usize - 1] as usize;
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

#[cfg(test)]
mod tests {
    use super::*;

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
                let plain = bims::droid::wave_kinds(n, tier);
                assert_eq!(with_guardian(plain.clone(), 1, tier), plain, "wave one");
                assert_eq!(with_guardian(plain.clone(), 3, tier), plain);
                let second = with_guardian(plain.clone(), 2, tier);
                assert_eq!(second.len(), plain.len(), "as many machines");
                assert_eq!(
                    guardians(&second),
                    want.min(n as usize).max(guardians(&plain)),
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
        // A tier-three wave with Guardians enough of its own is left alone.
        let big = bims::droid::wave_kinds(32, Tier::Three);
        assert_eq!(guardians(&big), 4);
        assert_eq!(with_guardian(big.clone(), 2, Tier::Three), big);
    }
}
