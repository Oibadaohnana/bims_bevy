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

/// How long a trip takes, in whole words (feature 103): "2 days 5
/// hours", "5 hours 20 minutes", "40 minutes" — the world clock a trip
/// puts on, which is the one span in a run long enough for days.
pub fn trip_length(minutes: u64) -> String {
    let plural = |n: u64, word: &str| {
        if n == 1 {
            format!("{n} {word}")
        } else {
            format!("{n} {word}s")
        }
    };
    let days = minutes / 1440;
    let hours = (minutes % 1440) / 60;
    let mins = minutes % 60;
    match (days, hours, mins) {
        (0, 0, m) => plural(m, "minute"),
        (0, h, 0) => plural(h, "hour"),
        (0, h, m) => format!("{} {}", plural(h, "hour"), plural(m, "minute")),
        (d, 0, _) => plural(d, "day"),
        (d, h, _) => format!("{} {}", plural(d, "day"), plural(h, "hour")),
    }
}

/// A span of the **mission clock** (feature 103) as the seconds it is at
/// 1× — a minute of it is a real second — for a countdown along the
/// top: "1:30", "0:05". Rounded up, as a countdown is.
pub fn countdown(minutes: f64) -> String {
    let seconds = minutes.ceil().max(0.0) as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
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
            assert_eq!(roman(9), "IX");
        }
    }
}
