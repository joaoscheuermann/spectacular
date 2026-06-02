use std::time::{SystemTime, UNIX_EPOCH};

use crate::redaction::{redact_credential_urls, redact_failure_text};

const SECONDS_PER_DAY: i64 = 86_400;
const SECONDS_PER_HOUR: i64 = 3_600;
const SECONDS_PER_MINUTE: i64 = 60;

/// Formats a `SystemTime` as compact UTC RFC3339 without fractional seconds.
///
/// This uses a small std-only UTC conversion so lifecycle terminal rendering
/// does not need a timestamp dependency for plain display text.
pub fn format_timestamp(timestamp: SystemTime) -> String {
    let seconds = whole_seconds(timestamp);
    let days = seconds.div_euclid(SECONDS_PER_DAY);
    let seconds_of_day = seconds.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / SECONDS_PER_HOUR;
    let minute = seconds_of_day % SECONDS_PER_HOUR / SECONDS_PER_MINUTE;
    let second = seconds_of_day % SECONDS_PER_MINUTE;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Redacts secrets and normalizes terminal controls into one display line.
pub fn safe_message(message: &str) -> String {
    let redacted = redact_failure_text(message);
    let redacted = redact_credential_urls(&redacted);

    normalize_controls(&redacted)
}

/// Formats a timestamped lifecycle terminal line.
pub fn format_line(timestamp: SystemTime, message: &str) -> String {
    format!(
        "[{}] {}",
        format_timestamp(timestamp),
        safe_message(message)
    )
}

fn whole_seconds(timestamp: SystemTime) -> i64 {
    match timestamp.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().min(i64::MAX as u64) as i64,
        Err(error) => {
            let duration = error.duration();
            let seconds = duration.as_secs().min(i64::MAX as u64) as i64;
            let fractional_second = i64::from(duration.subsec_nanos() > 0);

            seconds.saturating_neg().saturating_sub(fractional_second)
        }
    }
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let adjusted_days = days + 719_468;
    let era = if adjusted_days >= 0 {
        adjusted_days
    } else {
        adjusted_days - 146_096
    } / 146_097;
    let day_of_era = adjusted_days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);

    (year, month as u32, day as u32)
}

fn normalize_controls(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut pending_space = false;

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            skip_escape_sequence(&mut chars);
            pending_space = true;
            continue;
        }

        if ch.is_control() || ch.is_whitespace() {
            pending_space = true;
            continue;
        }

        if pending_space && !normalized.is_empty() {
            normalized.push(' ');
        }

        pending_space = false;
        normalized.push(ch);
    }

    normalized
}

fn skip_escape_sequence(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    match chars.peek().copied() {
        Some('[') => {
            chars.next();
            for ch in chars.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&ch) {
                    break;
                }
            }
        }
        Some(']') => {
            chars.next();
            while let Some(ch) = chars.next() {
                if ch == '\u{7}' {
                    break;
                }

                if ch == '\u{1b}' && matches!(chars.peek(), Some('\\')) {
                    chars.next();
                    break;
                }
            }
        }
        Some(_) => {
            chars.next();
        }
        None => {}
    }
}
