//! Numbers as words: money, clocks, lengths of time, dates.
//!
//! The simulation hands over a count of euros, a number of minutes, a day
//! and a month; the euro sign, the digit grouping and the colon between the
//! hours and the minutes exist here and nowhere else.

use crate::names::MONTH_NAMES;

/// A big number with its digits in threes. One rule, in one place: money
/// wants it and so does a distance, and a distance is not money.
pub fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{a0}');
        }
        out.push(ch);
    }
    out
}

/// A number of euros, as words.
pub fn euros(value: u64) -> String {
    format!("€{}", grouped(value))
}

/// Minutes in a game day.
pub const MINUTES_PER_DAY: f32 = 24.0 * 60.0;

/// A clock reading, wrapped into one day.
pub fn clock_text(minutes: f32) -> String {
    let m = ((minutes % MINUTES_PER_DAY) + MINUTES_PER_DAY) % MINUTES_PER_DAY;
    format!(
        "{:02}:{:02}",
        (m / 60.0).floor() as u32,
        (m % 60.0).floor() as u32
    )
}

/// A length of time, said the way a person would say it.
pub fn span_text(minutes: f32) -> String {
    let total = minutes.round() as i64;
    if total < 60 {
        return format!("{total} min");
    }
    let hours = total / 60;
    let rest = total % 60;
    if rest == 0 {
        return format!("{hours} hour{}", if hours == 1 { "" } else { "s" });
    }
    format!("{hours}h {rest}m")
}

/// Game minutes as something a person can read. Days and hours, because a
/// trip across a system is days and a trip across a dock is minutes.
pub fn spell(minutes: f64) -> String {
    let whole = minutes.round() as i64;
    let days = whole / 1440;
    let hours = (whole % 1440) / 60;
    let mins = whole % 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins}m")
    }
}

/// A span of game minutes in whole words, for a warning read at a
/// glance: "6 hours", "1 hour 30 minutes", "30 minutes" — a part of a
/// minute counted as the whole one, as a countdown does — and "under a
/// minute" at nought. Days are hours here: a raider is never a day out.
pub fn in_words(minutes: f64) -> String {
    let whole = minutes.ceil().max(0.0) as i64;
    if whole < 1 {
        return "under a minute".into();
    }
    let hours = whole / 60;
    let mins = whole % 60;
    let plural = |n: i64, word: &str| {
        if n == 1 {
            format!("{n} {word}")
        } else {
            format!("{n} {word}s")
        }
    };
    match (hours, mins) {
        (0, m) => plural(m, "minute"),
        (h, 0) => plural(h, "hour"),
        (h, m) => format!("{} {}", plural(h, "hour"), plural(m, "minute")),
    }
}

/// A date the way a person would write it.
pub fn date_text(date: u32, month: u32, year: u32) -> String {
    let month = MONTH_NAMES
        .get(month.wrapping_sub(1) as usize)
        .copied()
        .unwrap_or("?");
    format!("{date} {month} {year}")
}

/// A small whole number as a roman numeral — which is how a body's ordinal
/// is read: the third planet out from Tanis-284 is Tanis-284 III.
pub fn roman(n: u32) -> String {
    let digits = [(10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let mut left = n;
    let mut out = String::new();
    for (value, glyph) in digits {
        while left >= value {
            out.push_str(glyph);
            left -= value;
        }
    }
    out
}

/// A small whole number as an ordinal, counted from one — "1st", "2nd",
/// "3rd", "4th", "11th", "22nd": a place in the research queue.
pub fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_is_grouped_in_threes_and_a_clock_wraps() {
        // --- money_is_grouped_in_threes ---
        {
            assert_eq!(euros(100_000), "€100\u{a0}000");
            assert_eq!(euros(999), "€999");
            assert_eq!(grouped(1_234_567), "1\u{a0}234\u{a0}567");
        }

        // --- a_clock_wraps_and_a_span_reads ---
        {
            assert_eq!(clock_text(1441.0), "00:01");
            assert_eq!(clock_text(-1.0), "23:59");
            assert_eq!(span_text(45.0), "45 min");
            assert_eq!(span_text(60.0), "1 hour");
            assert_eq!(span_text(150.0), "2h 30m");
            assert_eq!(spell(3000.0), "2d 2h");
            assert_eq!(in_words(360.0), "6 hours");
            assert_eq!(in_words(90.0), "1 hour 30 minutes");
            assert_eq!(in_words(30.0), "30 minutes");
            assert_eq!(in_words(1.0), "1 minute");
            assert_eq!(in_words(0.4), "1 minute", "counted up to the minute");
            assert_eq!(in_words(0.0), "under a minute");
            assert_eq!(in_words(59.2), "1 hour");
            assert_eq!(roman(9), "IX");
            assert_eq!(ordinal(1), "1st");
            assert_eq!(ordinal(2), "2nd");
            assert_eq!(ordinal(3), "3rd");
            assert_eq!(ordinal(4), "4th");
            assert_eq!(ordinal(11), "11th");
            assert_eq!(ordinal(22), "22nd");
        }
    }
}
