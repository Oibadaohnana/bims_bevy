//! **Ascensions** (October 2026, the player's): a run's level of
//! difficulty past the game as it plays, nought to [`MOST`], picked on the
//! game setup and dealt to every machine with the rest of the settings
//! (`World::set_ascension`). Winning a run at a level opens the next one
//! in the player's profile (`crate::relic::Profile::record_win`).
//!
//! Each level keeps every level under it, as Slay the Spire's do:
//!
//! 0. the game as it plays;
//! 1. **Swarming Elites** — each fight's place on the floor
//!    [`ELITE_CHANCE`] in a hundred more likely an elite's, where the
//!    floor is laid (`World::lay_floor`);
//! 2. **Tougher Enemies** — every enemy laid with [`ENEMY_HEALTH`] per
//!    cent more hit points (`World::enemy_health_factor`);
//! 3. **Aua Making Enemies** — every hit on the crew [`ENEMY_DAMAGE`] per cent
//!    harder (`World::skill_of`'s `damage_taken`);
//! 4. **More Enemies** — the scaling's growth turned up so the Heart's
//!    waves are [`HEART_ENEMIES`] per cent more ([`scale`]), day one's as
//!    it was;
//! 5. **One More** — every area a wave more a site ([`EXTRA_WAVES`]).
//!
//! Nothing here draws from a stream, and level nought is the game exactly.

use crate::droid::WaveScaling;

/// The highest ascension.
pub const MOST: u32 = 5;

/// Swarming Elites (one): in a hundred, the odds a fight's place on the
/// floor past area 0 is put on an elite's system where one is left —
/// on top of the one in ten the systems are elites' anyway.
pub const ELITE_CHANCE: u32 = 20;
/// Tougher Enemies (two): the enemies' hit points, in per cent more.
pub const ENEMY_HEALTH: i32 = 10;
/// Aua Making Enemies (three): what every hit on the crew does, in per cent
/// more.
pub const ENEMY_DAMAGE: i32 = 10;
/// More Enemies (four): the Heart's waves, in per cent more.
pub const HEART_ENEMIES: u64 = 10;
/// One More (five): waves more a site, in every area.
pub const EXTRA_WAVES: u32 = 1;

/// The odds in a hundred a floor's fight place is made an elite's at
/// `level`.
pub fn elite_chance(level: u32) -> u32 {
    if level >= 1 { ELITE_CHANCE } else { 0 }
}

/// The per cent more hit points every enemy is laid with at `level`.
pub fn enemy_health(level: u32) -> i32 {
    if level >= 2 { ENEMY_HEALTH } else { 0 }
}

/// The per cent more every hit on the crew does at `level`.
pub fn enemy_damage(level: u32) -> i32 {
    if level >= 3 { ENEMY_DAMAGE } else { 0 }
}

/// The wave formula as `level` plays it: at four the growth of every area
/// turned up by one share so a player brings [`HEART_ENEMIES`] per cent
/// more on the Heart's day (day one's count as it was; with no growth at
/// all, the count itself), at five every area's waves one more.
pub fn scale(level: u32, mut s: WaveScaling) -> WaveScaling {
    if level >= 4 {
        let heart = s.heart_day();
        let before = s.per_player_hundredths_on(heart);
        let want = (before * (100 + HEART_ENEMIES)).div_ceil(100);
        let base = u64::from(s.enemies_per_player) * 100;
        let grown = before.saturating_sub(base);
        if grown == 0 {
            s.enemies_per_player = (want.div_ceil(100)).min(u64::from(u32::MAX)) as u32;
        } else {
            let more = want - base;
            for area in [
                &mut s.area_0,
                &mut s.tier_1_area,
                &mut s.tier_2_area,
                &mut s.tier_3_area,
            ] {
                let h = area.growth_hundredths();
                area.set_growth_hundredths((h * more).div_ceil(grown));
            }
        }
    }
    if level >= 5 {
        for area in [
            &mut s.area_0,
            &mut s.tier_1_area,
            &mut s.tier_2_area,
            &mut s.tier_3_area,
        ] {
            area.waves = area.waves.max(1).saturating_add(EXTRA_WAVES);
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_nought_is_the_game_as_it_plays() {
        let s = WaveScaling::DEFAULT;
        assert_eq!(scale(0, s), s);
        assert_eq!(scale(3, s), s);
        assert_eq!(elite_chance(0), 0);
        assert_eq!(enemy_health(1), 0);
        assert_eq!(enemy_damage(2), 0);
    }

    #[test]
    fn each_level_keeps_the_ones_under_it() {
        for level in 1..=MOST {
            assert_eq!(elite_chance(level), ELITE_CHANCE);
        }
        assert_eq!(enemy_health(MOST), ENEMY_HEALTH);
        assert_eq!(enemy_damage(MOST), ENEMY_DAMAGE);
        assert_eq!(
            scale(MOST, WaveScaling::DEFAULT).heart_day(),
            WaveScaling::DEFAULT.heart_day()
        );
    }

    /// Four: a tenth more at the Heart, day one as it was, and the floor's
    /// rows where they were.
    #[test]
    fn four_brings_a_tenth_more_to_the_heart() {
        for s in [WaveScaling::DEFAULT, WaveScaling::with_tier_days(10, 20)] {
            let up = scale(4, s);
            let heart = s.heart_day();
            assert_eq!(up.heart_day(), heart);
            assert_eq!(up.per_player_on(1), s.per_player_on(1));
            let (was, now) = (
                s.per_player_hundredths_on(heart),
                up.per_player_hundredths_on(heart),
            );
            assert!(now * 100 >= was * 110, "{was} -> {now}");
            assert!(now * 100 <= was * 112, "{was} -> {now}");
        }
        // No growth at all: the count itself.
        let mut flat = WaveScaling::DEFAULT;
        for area in [
            &mut flat.area_0,
            &mut flat.tier_1_area,
            &mut flat.tier_2_area,
            &mut flat.tier_3_area,
        ] {
            area.growth_per_day = 0.0;
        }
        flat.enemies_per_player = 20;
        assert_eq!(scale(4, flat).enemies_per_player, 22);
    }

    #[test]
    fn five_is_a_wave_more_in_every_area() {
        let s = WaveScaling::DEFAULT;
        let up = scale(5, s);
        for (a, b) in s.areas().iter().zip(up.areas()) {
            assert_eq!(b.waves, a.waves.max(1) + 1);
        }
    }
}
