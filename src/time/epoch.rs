//! Epoch types and UTC ↔ TDB conversion functions.
//!
//! All epochs are stored as f64 days after J2000.0 (2000-01-01 12:00:00 TT).
//!
//! Time scale relationships:
//! - TT  = TAI + 32.184s (exact definition)
//! - TAI = UTC + leap_seconds (from table)
//! - TDB ≈ TT + periodic correction (~1.7ms max, mean zero)

use crate::time::leap_seconds::{
    J2000_JD, SECONDS_PER_DAY, TT_TAI_OFFSET, calendar_to_jd, jd_to_calendar, tai_utc_offset,
};

/// Convert a calendar date in UTC to days after J2000.0 (UTC scale).
///
/// Returns None if the date is before 1972-01-01 (no leap second data).
pub fn utc_calendar_to_j2000_days(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: f64,
) -> Option<f64> {
    // Verify we have leap second data for this date
    tai_utc_offset(year, month, day)?;
    let jd = calendar_to_jd(year, month, day, hour, minute, second);
    Some(jd - J2000_JD)
}

/// Convert a calendar date in TDB to days after J2000.0 (TDB scale).
pub fn tdb_calendar_to_j2000_days(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: f64,
) -> f64 {
    let jd = calendar_to_jd(year, month, day, hour, minute, second);
    jd - J2000_JD
}

/// Convert days-after-J2000 in UTC to a calendar date string.
///
/// Format: "YYYY-MM-DDTHH:MM:SS.fff UTC"
pub fn j2000_days_to_utc_string(days: f64) -> String {
    let jd = days + J2000_JD;
    let (year, month, day, hour, minute, second) = jd_to_calendar(jd);
    let millis = ((second - second.floor()) * 1000.0).round() as u32;
    let sec_int = second.floor() as u32;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03} UTC",
        year, month, day, hour, minute, sec_int, millis
    )
}

/// Convert days-after-J2000 in TDB to a calendar date string.
///
/// Format: "YYYY-MM-DDTHH:MM:SS.fff TDB"
pub fn j2000_days_to_tdb_string(days: f64) -> String {
    let jd = days + J2000_JD;
    let (year, month, day, hour, minute, second) = jd_to_calendar(jd);
    let millis = ((second - second.floor()) * 1000.0).round() as u32;
    let sec_int = second.floor() as u32;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03} TDB",
        year, month, day, hour, minute, sec_int, millis
    )
}

/// Convert UTC days-after-J2000 to TDB days-after-J2000.
///
/// UTC → TAI → TT → TDB
///
/// The TDB-TT correction is a periodic term with max amplitude ~1.7ms.
/// For most engineering purposes this is negligible but we include it
/// for correctness.
pub fn utc_to_tdb(utc_days: f64) -> Option<f64> {
    let jd_utc = utc_days + J2000_JD;
    let (year, month, day, _, _, _) = jd_to_calendar(jd_utc);

    let leap_seconds = tai_utc_offset(year, month, day)?;

    // UTC → TAI → TT (in seconds offset from UTC)
    let tt_minus_utc = leap_seconds + TT_TAI_OFFSET;

    // TT in J2000 days
    let tt_days = utc_days + tt_minus_utc / SECONDS_PER_DAY;

    // TDB ≈ TT + periodic correction
    // Fairhead & Bretagnon (1990) approximation, dominant term:
    // TDB - TT ≈ 0.001657 * sin(628.3076 * T + 6.2401) seconds
    // where T is Julian centuries of TDB from J2000.0
    let t = tt_days / 36525.0; // Julian centuries from J2000
    let tdb_tt_correction = 0.001657 * (628.3076 * t + 6.2401).sin();

    let tdb_days = tt_days + tdb_tt_correction / SECONDS_PER_DAY;
    Some(tdb_days)
}

/// Convert TDB days-after-J2000 to UTC days-after-J2000.
///
/// TDB → TT → TAI → UTC
///
/// This is iterative because the leap second lookup depends on the UTC date.
pub fn tdb_to_utc(tdb_days: f64) -> Option<f64> {
    // TDB → TT (remove periodic correction)
    let t = tdb_days / 36525.0;
    let tdb_tt_correction = 0.001657 * (628.3076 * t + 6.2401).sin();
    let tt_days = tdb_days - tdb_tt_correction / SECONDS_PER_DAY;

    // TT → UTC: need leap seconds, which depend on UTC date
    // Use TT date as initial approximation for leap second lookup
    let jd_approx = tt_days + J2000_JD;
    let (year, month, day, _, _, _) = jd_to_calendar(jd_approx);

    let leap_seconds = tai_utc_offset(year, month, day)?;
    let tt_minus_utc = leap_seconds + TT_TAI_OFFSET;

    let utc_days = tt_days - tt_minus_utc / SECONDS_PER_DAY;
    Some(utc_days)
}

/// Convert days-after-J2000 to Julian Date.
pub fn j2000_days_to_jd(days: f64) -> f64 {
    days + J2000_JD
}

/// Convert days-after-J2000 to Modified Julian Date.
///
/// MJD = JD - 2400000.5
pub fn j2000_days_to_mjd(days: f64) -> f64 {
    days + J2000_JD - 2400000.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utc_calendar_to_j2000_at_epoch() {
        // Tier 1: calendar_to_jd(2000,1,1,12,0,0) produces exactly J2000_JD.
        // Subtraction of identical constant yields exactly 0.0.
        let days = utc_calendar_to_j2000_days(2000, 1, 1, 12, 0, 0.0).unwrap();
        assert_eq!(
            days, 0.0,
            "J2000 noon UTC should be exactly 0.0 days, got {}",
            days
        );
    }

    #[test]
    fn test_utc_calendar_before_1972_returns_none() {
        assert!(utc_calendar_to_j2000_days(1970, 1, 1, 0, 0, 0.0).is_none());
    }

    #[test]
    fn test_tdb_calendar_to_j2000_at_epoch() {
        // Tier 1: same as UTC case — JD - J2000_JD = 0.0 exactly.
        let days = tdb_calendar_to_j2000_days(2000, 1, 1, 12, 0, 0.0);
        assert_eq!(days, 0.0);
    }

    #[test]
    fn test_utc_string_roundtrip() {
        let days = utc_calendar_to_j2000_days(2030, 12, 12, 0, 0, 0.0).unwrap();
        let s = j2000_days_to_utc_string(days);
        assert!(s.starts_with("2030-12-12T00:00:00"), "Got: {}", s);
        assert!(s.ends_with("UTC"));
    }

    #[test]
    fn test_tdb_string_roundtrip() {
        let days = tdb_calendar_to_j2000_days(2030, 12, 12, 0, 0, 0.0);
        let s = j2000_days_to_tdb_string(days);
        assert!(s.starts_with("2030-12-12T00:00:00"), "Got: {}", s);
        assert!(s.ends_with("TDB"));
    }

    #[test]
    fn test_utc_to_tdb_conversion() {
        // Tier 2: TDB-TT periodic term has max amplitude 0.001657s.
        // Total offset = 37 (leap) + 32.184 (TT-TAI) + periodic.
        // Tighten from 0.01s to 0.002s (just above max periodic amplitude).
        let utc_days = utc_calendar_to_j2000_days(2024, 1, 1, 0, 0, 0.0).unwrap();
        let tdb_days = utc_to_tdb(utc_days).unwrap();

        let diff_seconds = (tdb_days - utc_days) * SECONDS_PER_DAY;
        assert!(
            (diff_seconds - 69.184).abs() < 0.002,
            "UTC-TDB offset should be ~69.184s ± 0.002s, got {}",
            diff_seconds
        );
    }

    #[test]
    fn test_tdb_to_utc_conversion() {
        // Tier 2: roundtrip of closed-form ops. Error bounded by f64
        // rounding across ~4 arithmetic ops on values ~1e4 days.
        // Theoretical bound: ~4 * 2^-52 * 1e4 * 86400 ≈ 8e-8 seconds.
        let utc_days = utc_calendar_to_j2000_days(2024, 6, 15, 12, 0, 0.0).unwrap();
        let tdb_days = utc_to_tdb(utc_days).unwrap();
        let utc_roundtrip = tdb_to_utc(tdb_days).unwrap();

        let diff = (utc_roundtrip - utc_days).abs() * SECONDS_PER_DAY;
        assert!(
            diff < 1e-9,
            "UTC→TDB→UTC roundtrip error: {} seconds (expected < 1e-9)",
            diff
        );
    }

    #[test]
    fn test_j2000_days_to_jd() {
        // Tier 1: 0.0 + constant = constant exactly. 1.0 + 2451545.0 = 2451546.0 exactly.
        assert_eq!(j2000_days_to_jd(0.0), J2000_JD);
        assert_eq!(j2000_days_to_jd(1.0), J2000_JD + 1.0);
    }

    #[test]
    fn test_j2000_days_to_mjd() {
        // Tier 1: MJD at J2000.0 = 2451545.0 - 2400000.5 = 51544.5
        // All values are exactly representable in f64.
        let mjd = j2000_days_to_mjd(0.0);
        assert_eq!(mjd, 51544.5, "Expected MJD 51544.5, got {}", mjd);
    }

    // =====================================================================
    // A6: Time edge case tests
    // =====================================================================

    #[test]
    fn test_utc_to_tdb_at_j2000() {
        // Tier 2: At J2000 (2000-01-01 12:00 UTC), the offset is:
        //   32 leap seconds + 32.184s TT-TAI offset + periodic term.
        // Expected ≈ 64.184s ± 0.002s (periodic term bounded by 0.001657s).
        let utc_days = utc_calendar_to_j2000_days(2000, 1, 1, 12, 0, 0.0).unwrap();
        let tdb_days = utc_to_tdb(utc_days).unwrap();
        let diff_seconds = (tdb_days - utc_days) * SECONDS_PER_DAY;
        assert!(
            (diff_seconds - 64.184).abs() < 0.002,
            "UTC-TDB offset at J2000 should be ~64.184s, got {}",
            diff_seconds
        );
    }

    #[test]
    fn test_utc_to_tdb_far_future() {
        // Tier 2: 2100-01-01 — leap seconds should still be 37
        // (last entry in table is 2017-01-01).
        let utc_days = utc_calendar_to_j2000_days(2100, 1, 1, 0, 0, 0.0).unwrap();
        let tdb_days = utc_to_tdb(utc_days).unwrap();
        let diff_seconds = (tdb_days - utc_days) * SECONDS_PER_DAY;
        // 37 + 32.184 = 69.184, periodic bounded by 0.002s
        assert!(
            (diff_seconds - 69.184).abs() < 0.002,
            "UTC-TDB offset at 2100 should be ~69.184s, got {}",
            diff_seconds
        );
    }

    #[test]
    fn test_tdb_to_utc_roundtrip_at_j2000() {
        // Tier 2: roundtrip at epoch.
        let utc_days = utc_calendar_to_j2000_days(2000, 1, 1, 12, 0, 0.0).unwrap();
        let tdb_days = utc_to_tdb(utc_days).unwrap();
        let utc_roundtrip = tdb_to_utc(tdb_days).unwrap();
        let diff = (utc_roundtrip - utc_days).abs() * SECONDS_PER_DAY;
        assert!(
            diff < 1e-9,
            "UTC→TDB→UTC roundtrip at J2000 error: {} seconds",
            diff
        );
    }

    #[test]
    fn test_utc_to_tdb_before_1972_returns_none() {
        // Pre-1972 UTC date → None (no leap second data)
        assert!(utc_to_tdb(-11000.0).is_none()); // ~1969
    }

    #[test]
    fn test_utc_string_format_exact() {
        // Tier 1: verify string format for a known date
        let s = j2000_days_to_utc_string(0.0);
        assert_eq!(s, "2000-01-01T12:00:00.000 UTC");
    }

    #[test]
    fn test_tdb_string_format_exact() {
        // Tier 1: verify string format for a known date
        let s = j2000_days_to_tdb_string(0.0);
        assert_eq!(s, "2000-01-01T12:00:00.000 TDB");
    }

    // =====================================================================
    // A7: Numerical stability — time
    // =====================================================================

    #[test]
    fn test_utc_to_tdb_periodic_bound() {
        // Tier 3: verify the TDB-TT periodic correction stays bounded.
        // The Fairhead & Bretagnon dominant term has amplitude 0.001657s.
        // Test at multiple dates to verify the correction never exceeds 0.002s.
        for &utc_days in &[0.0, 3652.5, 7305.0, 18262.5, 36525.0] {
            if let Some(tdb_days) = utc_to_tdb(utc_days) {
                let diff = (tdb_days - utc_days) * SECONDS_PER_DAY;
                // Subtract the deterministic part (leap_seconds + TT_TAI_OFFSET)
                // to isolate the periodic correction
                let jd_utc = utc_days + J2000_JD;
                let (year, month, day, _, _, _) = jd_to_calendar(jd_utc);
                if let Some(leap_seconds) = tai_utc_offset(year, month, day) {
                    let deterministic = leap_seconds + TT_TAI_OFFSET;
                    let periodic = diff - deterministic;
                    assert!(
                        periodic.abs() < 0.002,
                        "Periodic correction at days={} should be < 0.002s, got {}",
                        utc_days,
                        periodic
                    );
                }
            }
        }
    }
}
