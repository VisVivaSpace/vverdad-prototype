//! Date string parsing for time-annotated values.
//!
//! Recognized formats (suffix-based only — strings WITHOUT a time system suffix stay as String):
//!
//! ISO 8601:
//! - "2030-12-12T00:00:00 UTC"
//! - "2030-12-12 00:00:00 TDB"
//! - "2030-12-12T00:00:00.000 UTC"
//! - "2030-12-12 UTC" (date only, time defaults to 00:00:00)
//!
//! SPICE-style:
//! - "12-DEC-2030 UTC"
//! - "2030-JUN-15 12:00:00.000 TDB"
//! - "12-DEC-2030 00:00:00 UTC"

use crate::time::epoch;
use crate::time::error::{TimeError, TimeResult};

/// Time system parsed from a string suffix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeSystem {
    Utc,
    Tdb,
    Tt,
    Tai,
}

/// Result of parsing a time string.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParsedEpoch {
    /// Days after J2000.0 in the native time system
    pub days_j2000: f64,
    /// The time system
    pub system: TimeSystem,
}

/// Try to parse a string as a time-annotated epoch.
///
/// Returns None if the string doesn't end with a recognized time system suffix.
/// Returns Some(Err) if it has a suffix but the date can't be parsed.
/// Returns Some(Ok) if successfully parsed.
pub fn try_parse_epoch(s: &str) -> Option<TimeResult<ParsedEpoch>> {
    let trimmed = s.trim();

    // Extract time system suffix
    let (date_part, system) = extract_time_system(trimmed)?;
    let date_part = date_part.trim();

    // Try to parse the date portion
    Some(parse_date_part(date_part, system))
}

/// Extract the time system suffix from a string.
///
/// Returns (date_part, time_system) if a valid suffix is found.
fn extract_time_system(s: &str) -> Option<(&str, TimeSystem)> {
    // Check for space-separated suffix (case-insensitive)
    if let Some(last_space) = s.rfind(' ') {
        let suffix = &s[last_space + 1..];
        let system = match suffix.to_uppercase().as_str() {
            "UTC" => Some(TimeSystem::Utc),
            "TDB" => Some(TimeSystem::Tdb),
            "TT" => Some(TimeSystem::Tt),
            "TAI" => Some(TimeSystem::Tai),
            _ => None,
        };
        if let Some(sys) = system {
            return Some((&s[..last_space], sys));
        }
    }
    None
}

/// Parse the date portion of a time string.
fn parse_date_part(date_str: &str, system: TimeSystem) -> TimeResult<ParsedEpoch> {
    // Try ISO 8601 first: "2030-12-12T00:00:00.000" or "2030-12-12 00:00:00.000" or "2030-12-12"
    if let Some(epoch) = try_parse_iso8601(date_str, system) {
        return epoch;
    }

    // Try SPICE-style: "12-DEC-2030" or "12-DEC-2030 00:00:00.000" or "2030-JUN-15 12:00:00"
    if let Some(epoch) = try_parse_spice(date_str, system) {
        return epoch;
    }

    Err(TimeError::ParseError {
        input: date_str.to_string(),
        reason: "Could not parse as ISO 8601 or SPICE-style date".to_string(),
    })
}

/// Returns the number of days in a given month, accounting for leap years.
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Month name abbreviation to number.
fn month_from_name(name: &str) -> Option<u32> {
    match name.to_uppercase().as_str() {
        "JAN" => Some(1),
        "FEB" => Some(2),
        "MAR" => Some(3),
        "APR" => Some(4),
        "MAY" => Some(5),
        "JUN" => Some(6),
        "JUL" => Some(7),
        "AUG" => Some(8),
        "SEP" => Some(9),
        "OCT" => Some(10),
        "NOV" => Some(11),
        "DEC" => Some(12),
        _ => None,
    }
}

/// Try to parse as ISO 8601 format.
///
/// Formats:
/// - "2030-12-12T00:00:00.000"
/// - "2030-12-12 00:00:00.000"
/// - "2030-12-12T00:00:00"
/// - "2030-12-12"
fn try_parse_iso8601(s: &str, system: TimeSystem) -> Option<TimeResult<ParsedEpoch>> {
    // Must start with YYYY-MM-DD (10 chars)
    if s.len() < 10 {
        return None;
    }

    // Check format: YYYY-MM-DD
    let year: i32 = s[0..4].parse().ok()?;
    if s.as_bytes()[4] != b'-' {
        return None;
    }
    let month: u32 = s[5..7].parse().ok()?;
    if s.as_bytes()[7] != b'-' {
        return None;
    }
    let day: u32 = s[8..10].parse().ok()?;

    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }

    let (hour, minute, second) = if s.len() > 10 {
        let sep = s.as_bytes()[10];
        if sep != b'T' && sep != b' ' {
            return None;
        }
        parse_time_part(&s[11..])?
    } else {
        (0, 0, 0.0)
    };

    Some(make_epoch(year, month, day, hour, minute, second, system))
}

/// Try to parse as SPICE-style format.
///
/// Formats:
/// - "12-DEC-2030"
/// - "12-DEC-2030 00:00:00.000"
/// - "2030-JUN-15 12:00:00"
fn try_parse_spice(s: &str, system: TimeSystem) -> Option<TimeResult<ParsedEpoch>> {
    let parts: Vec<&str> = s.splitn(2, ' ').collect();
    let date_part = parts[0];

    // Split date by '-'
    let date_segments: Vec<&str> = date_part.split('-').collect();
    if date_segments.len() != 3 {
        return None;
    }

    // Try DD-MON-YYYY format
    if let Some(month) = month_from_name(date_segments[1]) {
        let (day, year) = if date_segments[0].len() <= 2 && date_segments[2].len() == 4 {
            // DD-MON-YYYY
            (
                date_segments[0].parse::<u32>().ok()?,
                date_segments[2].parse::<i32>().ok()?,
            )
        } else if date_segments[0].len() == 4 && date_segments[2].len() <= 2 {
            // YYYY-MON-DD
            (
                date_segments[2].parse::<u32>().ok()?,
                date_segments[0].parse::<i32>().ok()?,
            )
        } else {
            return None;
        };

        // Validate day for the given month/year
        if day < 1 || day > days_in_month(year, month) {
            return None;
        }

        let (hour, minute, second) = if parts.len() > 1 {
            parse_time_part(parts[1])?
        } else {
            (0, 0, 0.0)
        };

        return Some(make_epoch(year, month, day, hour, minute, second, system));
    }

    None
}

/// Parse a time portion "HH:MM:SS.fff" or "HH:MM:SS"
fn parse_time_part(s: &str) -> Option<(u32, u32, f64)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() < 2 {
        return None;
    }

    let hour: u32 = parts[0].parse().ok()?;
    let minute: u32 = parts[1].parse().ok()?;
    let second: f64 = if parts.len() > 2 {
        parts[2].parse().ok()?
    } else {
        0.0
    };

    if hour > 23 || minute > 59 || second >= 61.0 {
        return None;
    }

    Some((hour, minute, second))
}

/// Create a ParsedEpoch from calendar components and time system.
///
/// For UTC and TAI: computes UTC J2000 days (TAI is converted to UTC first)
/// For TDB and TT: computes TDB J2000 days (TT is converted to TDB)
fn make_epoch(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: f64,
    system: TimeSystem,
) -> TimeResult<ParsedEpoch> {
    match system {
        TimeSystem::Utc => {
            let days = epoch::utc_calendar_to_j2000_days(year, month, day, hour, minute, second)
                .ok_or_else(|| TimeError::ParseError {
                    input: format!("{:04}-{:02}-{:02}", year, month, day),
                    reason: "Date before 1972-01-01 (no leap second data)".to_string(),
                })?;
            Ok(ParsedEpoch {
                days_j2000: days,
                system: TimeSystem::Utc,
            })
        }
        TimeSystem::Tdb => {
            let days = epoch::tdb_calendar_to_j2000_days(year, month, day, hour, minute, second);
            Ok(ParsedEpoch {
                days_j2000: days,
                system: TimeSystem::Tdb,
            })
        }
        TimeSystem::Tt => {
            // TT is stored as TDB (the difference is <2ms, but we apply it)
            // TDB ≈ TT + periodic correction
            let tt_days = epoch::tdb_calendar_to_j2000_days(year, month, day, hour, minute, second);
            // Apply TDB-TT correction (same formula as in epoch.rs)
            let t = tt_days / 36525.0;
            let tdb_tt_correction = 0.001657 * (628.3076 * t + 6.2401).sin();
            let tdb_days = tt_days + tdb_tt_correction / crate::time::leap_seconds::SECONDS_PER_DAY;
            Ok(ParsedEpoch {
                days_j2000: tdb_days,
                system: TimeSystem::Tdb,
            })
        }
        TimeSystem::Tai => {
            // TAI → UTC: subtract leap seconds, then store as UTC
            use crate::time::leap_seconds::{SECONDS_PER_DAY, tai_utc_offset};
            let tai_days =
                epoch::tdb_calendar_to_j2000_days(year, month, day, hour, minute, second);
            let leap = tai_utc_offset(year, month, day).ok_or_else(|| TimeError::ParseError {
                input: format!("{:04}-{:02}-{:02}", year, month, day),
                reason: "Date before 1972-01-01 (no leap second data)".to_string(),
            })?;
            let utc_days = tai_days - leap / SECONDS_PER_DAY;
            Ok(ParsedEpoch {
                days_j2000: utc_days,
                system: TimeSystem::Utc,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_suffix_returns_none() {
        assert!(try_parse_epoch("2030-12-12").is_none());
        assert!(try_parse_epoch("hello world").is_none());
        assert!(try_parse_epoch("100 N").is_none());
        assert!(try_parse_epoch("12-DEC-2030").is_none());
    }

    #[test]
    fn test_iso8601_utc() {
        let result = try_parse_epoch("2030-12-12T00:00:00 UTC").unwrap().unwrap();
        assert_eq!(result.system, TimeSystem::Utc);
        // 2030-12-12 should be about 11302 days after J2000
        assert!(
            result.days_j2000 > 11000.0 && result.days_j2000 < 12000.0,
            "Expected ~11302, got {}",
            result.days_j2000
        );
    }

    #[test]
    fn test_iso8601_tdb() {
        let result = try_parse_epoch("2030-12-12 00:00:00 TDB").unwrap().unwrap();
        assert_eq!(result.system, TimeSystem::Tdb);
    }

    #[test]
    fn test_iso8601_date_only_utc() {
        let result = try_parse_epoch("2030-12-12 UTC").unwrap().unwrap();
        assert_eq!(result.system, TimeSystem::Utc);
    }

    #[test]
    fn test_iso8601_with_millis() {
        let result = try_parse_epoch("2030-12-12T15:30:45.500 UTC")
            .unwrap()
            .unwrap();
        assert_eq!(result.system, TimeSystem::Utc);
    }

    #[test]
    fn test_spice_style_utc() {
        let result = try_parse_epoch("12-DEC-2030 UTC").unwrap().unwrap();
        assert_eq!(result.system, TimeSystem::Utc);
    }

    #[test]
    fn test_spice_style_with_time() {
        let result = try_parse_epoch("12-DEC-2030 00:00:00 UTC")
            .unwrap()
            .unwrap();
        assert_eq!(result.system, TimeSystem::Utc);
    }

    #[test]
    fn test_spice_style_yyyy_mon_dd() {
        let result = try_parse_epoch("2030-JUN-15 12:00:00.000 TDB")
            .unwrap()
            .unwrap();
        assert_eq!(result.system, TimeSystem::Tdb);
    }

    #[test]
    fn test_tt_converts_to_tdb() {
        let result = try_parse_epoch("2030-12-12T00:00:00 TT").unwrap().unwrap();
        // TT input is stored as TDB
        assert_eq!(result.system, TimeSystem::Tdb);
    }

    #[test]
    fn test_tai_converts_to_utc() {
        let result = try_parse_epoch("2030-12-12T00:00:00 TAI").unwrap().unwrap();
        // TAI input is stored as UTC
        assert_eq!(result.system, TimeSystem::Utc);
    }

    #[test]
    fn test_case_insensitive_suffix() {
        let result = try_parse_epoch("2030-12-12 utc").unwrap().unwrap();
        assert_eq!(result.system, TimeSystem::Utc);
    }

    #[test]
    fn test_iso8601_and_spice_same_date() {
        let iso = try_parse_epoch("2030-12-12T00:00:00 UTC").unwrap().unwrap();
        let spice = try_parse_epoch("12-DEC-2030 00:00:00 UTC")
            .unwrap()
            .unwrap();
        assert!(
            (iso.days_j2000 - spice.days_j2000).abs() < 1e-10,
            "ISO and SPICE should give same result: {} vs {}",
            iso.days_j2000,
            spice.days_j2000
        );
    }

    #[test]
    fn test_invalid_date_feb30_rejected() {
        // February 30 doesn't exist in any year — returns Some(Err) since the
        // suffix is valid but the date is not
        let result = try_parse_epoch("2030-02-30 UTC");
        assert!(
            matches!(result, Some(Err(_))),
            "Expected Some(Err(...)) for invalid date, got: {:?}",
            result
        );
    }

    #[test]
    fn test_invalid_date_feb29_non_leap_rejected() {
        // 2030 is not a leap year
        let result = try_parse_epoch("2030-02-29 UTC");
        assert!(
            matches!(result, Some(Err(_))),
            "Expected Some(Err(...)) for non-leap Feb 29, got: {:?}",
            result
        );
    }

    #[test]
    fn test_valid_date_feb29_leap_year() {
        // 2024 is a leap year
        let result = try_parse_epoch("2024-02-29 UTC").unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_date_spice_style_rejected() {
        // 31-JUN doesn't exist (June has 30 days) — returns Some(Err)
        let result = try_parse_epoch("31-JUN-2030 UTC");
        assert!(
            matches!(result, Some(Err(_))),
            "Expected Some(Err(...)) for invalid SPICE date, got: {:?}",
            result
        );
    }

    #[test]
    fn test_pre_1972_utc_fails() {
        let result = try_parse_epoch("1970-01-01T00:00:00 UTC").unwrap();
        assert!(result.is_err());
    }
}
