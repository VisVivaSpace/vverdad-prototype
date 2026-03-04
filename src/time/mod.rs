//! Time module for aerospace epoch handling.
//!
//! Provides UTC and TDB time scales with eager parsing of date strings.
//! All epochs are stored as f64 days after J2000.0 (2000-01-01 12:00:00 TT).
//!
//! ## Time Scales
//!
//! - **UTC**: Coordinated Universal Time (civil time with leap seconds)
//! - **TDB**: Barycentric Dynamical Time (for solar system ephemerides)
//! - **TT**: Terrestrial Time (= TAI + 32.184s), stored as TDB
//! - **TAI**: International Atomic Time, stored as UTC
//!
//! ## Recognized String Formats
//!
//! Strings must end with a time system suffix (UTC, TDB, TT, TAI):
//! - ISO 8601: `"2030-12-12T00:00:00 UTC"`
//! - SPICE-style: `"12-DEC-2030 UTC"`

pub mod epoch;
pub mod error;
pub mod leap_seconds;
pub mod parse;

// Re-export main types and functions
pub use epoch::{
    j2000_days_to_jd, j2000_days_to_mjd, j2000_days_to_tdb_string, j2000_days_to_utc_string,
    tdb_to_utc, utc_to_tdb,
};
pub use error::{TimeError, TimeResult};
pub use parse::{ParsedEpoch, TimeSystem, try_parse_epoch};

use minijinja::value::Value as MJValue;

/// Registers all time filters with a Minijinja environment.
pub fn register_filters(env: &mut minijinja::Environment) {
    env.add_filter("to_utc", filter_to_utc);
    env.add_filter("to_tdb", filter_to_tdb);
    env.add_filter("jd", filter_jd);
    env.add_filter("mjd", filter_mjd);
}

/// Filter: Convert a TDB epoch to UTC.
///
/// Usage: `{{ epoch | to_utc }}`
fn filter_to_utc(value: MJValue) -> Result<MJValue, minijinja::Error> {
    let s = value.as_str().unwrap_or_default();
    if let Some(Ok(parsed)) = try_parse_epoch(s) {
        match parsed.system {
            TimeSystem::Tdb | TimeSystem::Tt => {
                if let Some(utc_days) = tdb_to_utc(parsed.days_j2000) {
                    return Ok(MJValue::from(j2000_days_to_utc_string(utc_days)));
                }
            }
            TimeSystem::Utc | TimeSystem::Tai => {
                // Already UTC (TAI is converted to UTC during parsing)
                return Ok(value);
            }
        }
    }
    // Pass through unchanged if not a recognized epoch
    Ok(value)
}

/// Filter: Convert a UTC epoch to TDB.
///
/// Usage: `{{ epoch | to_tdb }}`
fn filter_to_tdb(value: MJValue) -> Result<MJValue, minijinja::Error> {
    let s = value.as_str().unwrap_or_default();
    if let Some(Ok(parsed)) = try_parse_epoch(s) {
        match parsed.system {
            TimeSystem::Utc | TimeSystem::Tai => {
                if let Some(tdb_days) = utc_to_tdb(parsed.days_j2000) {
                    return Ok(MJValue::from(j2000_days_to_tdb_string(tdb_days)));
                }
            }
            TimeSystem::Tdb | TimeSystem::Tt => {
                // Already TDB (TT is converted to TDB during parsing)
                return Ok(value);
            }
        }
    }
    Ok(value)
}

/// Filter: Get the Julian Date of an epoch.
///
/// Usage: `{{ epoch | jd }}`
fn filter_jd(value: MJValue) -> Result<MJValue, minijinja::Error> {
    let s = value.as_str().unwrap_or_default();
    if let Some(Ok(parsed)) = try_parse_epoch(s) {
        let jd = j2000_days_to_jd(parsed.days_j2000);
        return Ok(MJValue::from(jd));
    }
    Ok(value)
}

/// Filter: Get the Modified Julian Date of an epoch.
///
/// Usage: `{{ epoch | mjd }}`
fn filter_mjd(value: MJValue) -> Result<MJValue, minijinja::Error> {
    let s = value.as_str().unwrap_or_default();
    if let Some(Ok(parsed)) = try_parse_epoch(s) {
        let mjd = j2000_days_to_mjd(parsed.days_j2000);
        return Ok(MJValue::from(mjd));
    }
    Ok(value)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_to_utc_from_tdb() {
        let val = MJValue::from("2030-12-12T00:00:00.000 TDB");
        let result = filter_to_utc(val).unwrap();
        let s = result.as_str().unwrap();
        assert!(s.contains("UTC"), "Expected UTC suffix, got: {}", s);
        assert!(
            s.contains("2030-12-1"),
            "Expected December 2030, got: {}",
            s
        );
    }

    #[test]
    fn test_filter_to_tdb_from_utc() {
        let val = MJValue::from("2030-12-12T00:00:00.000 UTC");
        let result = filter_to_tdb(val).unwrap();
        let s = result.as_str().unwrap();
        assert!(s.contains("TDB"), "Expected TDB suffix, got: {}", s);
    }

    #[test]
    fn test_filter_to_utc_passthrough() {
        let val = MJValue::from("hello world");
        let result = filter_to_utc(val).unwrap();
        assert_eq!(result.as_str().unwrap(), "hello world");
    }

    #[test]
    fn test_filter_jd() {
        // J2000.0 epoch itself should give JD 2451545.0
        let val = MJValue::from("2000-01-01T12:00:00.000 UTC");
        let result = filter_jd(val).unwrap();
        let jd: f64 = result.try_into().unwrap();
        assert!(
            (jd - 2451545.0).abs() < 0.001,
            "Expected ~2451545.0, got {}",
            jd
        );
    }

    #[test]
    fn test_filter_mjd() {
        let val = MJValue::from("2000-01-01T12:00:00.000 UTC");
        let result = filter_mjd(val).unwrap();
        let mjd: f64 = result.try_into().unwrap();
        assert!(
            (mjd - 51544.5).abs() < 0.001,
            "Expected ~51544.5, got {}",
            mjd
        );
    }

    #[test]
    fn test_filter_jd_passthrough() {
        let val = MJValue::from("not a date");
        let result = filter_jd(val).unwrap();
        assert_eq!(result.as_str().unwrap(), "not a date");
    }
}
