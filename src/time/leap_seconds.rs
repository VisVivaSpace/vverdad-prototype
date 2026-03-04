//! Leap second table for UTC ↔ TAI conversion.
//!
//! Contains a compiled-in table of all leap seconds from 1972 to present.
//! The table maps UTC dates to cumulative TAI-UTC offsets.
//!
//! TAI = UTC + leap_seconds (from table)
//! TT  = TAI + 32.184s (exact definition)

/// A leap second entry: (year, month, day, cumulative TAI-UTC offset in seconds).
///
/// Each entry marks the date when the offset changed to the new value.
/// The offset applies from that date forward until the next entry.
#[derive(Debug, Clone, Copy)]
pub struct LeapSecondEntry {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub tai_utc: f64,
}

/// Compiled-in leap second table (1972-01-01 through 2017-01-01).
///
/// Source: IERS Bulletin C / NAIF LSK kernel (naif0012.tls).
/// Table is current as of 2025 — no leap seconds have been added since
/// 2017-01-01 (TAI-UTC = 37 s).
///
/// # Maintenance
///
/// IERS Bulletin C is issued every six months and announces whether a
/// leap second will be introduced. If a new leap second is announced,
/// add a new entry at the end of this table.
///
// TODO: Implement a mechanism to update leap seconds at runtime
// (e.g., parse NAIF LSK kernel, download IERS Bulletin C, or accept
// a user-provided table).
pub const LEAP_SECOND_TABLE: &[LeapSecondEntry] = &[
    LeapSecondEntry {
        year: 1972,
        month: 1,
        day: 1,
        tai_utc: 10.0,
    },
    LeapSecondEntry {
        year: 1972,
        month: 7,
        day: 1,
        tai_utc: 11.0,
    },
    LeapSecondEntry {
        year: 1973,
        month: 1,
        day: 1,
        tai_utc: 12.0,
    },
    LeapSecondEntry {
        year: 1974,
        month: 1,
        day: 1,
        tai_utc: 13.0,
    },
    LeapSecondEntry {
        year: 1975,
        month: 1,
        day: 1,
        tai_utc: 14.0,
    },
    LeapSecondEntry {
        year: 1976,
        month: 1,
        day: 1,
        tai_utc: 15.0,
    },
    LeapSecondEntry {
        year: 1977,
        month: 1,
        day: 1,
        tai_utc: 16.0,
    },
    LeapSecondEntry {
        year: 1978,
        month: 1,
        day: 1,
        tai_utc: 17.0,
    },
    LeapSecondEntry {
        year: 1979,
        month: 1,
        day: 1,
        tai_utc: 18.0,
    },
    LeapSecondEntry {
        year: 1980,
        month: 1,
        day: 1,
        tai_utc: 19.0,
    },
    LeapSecondEntry {
        year: 1981,
        month: 7,
        day: 1,
        tai_utc: 20.0,
    },
    LeapSecondEntry {
        year: 1982,
        month: 7,
        day: 1,
        tai_utc: 21.0,
    },
    LeapSecondEntry {
        year: 1983,
        month: 7,
        day: 1,
        tai_utc: 22.0,
    },
    LeapSecondEntry {
        year: 1985,
        month: 7,
        day: 1,
        tai_utc: 23.0,
    },
    LeapSecondEntry {
        year: 1988,
        month: 1,
        day: 1,
        tai_utc: 24.0,
    },
    LeapSecondEntry {
        year: 1990,
        month: 1,
        day: 1,
        tai_utc: 25.0,
    },
    LeapSecondEntry {
        year: 1991,
        month: 1,
        day: 1,
        tai_utc: 26.0,
    },
    LeapSecondEntry {
        year: 1992,
        month: 7,
        day: 1,
        tai_utc: 27.0,
    },
    LeapSecondEntry {
        year: 1993,
        month: 7,
        day: 1,
        tai_utc: 28.0,
    },
    LeapSecondEntry {
        year: 1994,
        month: 7,
        day: 1,
        tai_utc: 29.0,
    },
    LeapSecondEntry {
        year: 1996,
        month: 1,
        day: 1,
        tai_utc: 30.0,
    },
    LeapSecondEntry {
        year: 1997,
        month: 7,
        day: 1,
        tai_utc: 31.0,
    },
    LeapSecondEntry {
        year: 1999,
        month: 1,
        day: 1,
        tai_utc: 32.0,
    },
    LeapSecondEntry {
        year: 2006,
        month: 1,
        day: 1,
        tai_utc: 33.0,
    },
    LeapSecondEntry {
        year: 2009,
        month: 1,
        day: 1,
        tai_utc: 34.0,
    },
    LeapSecondEntry {
        year: 2012,
        month: 7,
        day: 1,
        tai_utc: 35.0,
    },
    LeapSecondEntry {
        year: 2015,
        month: 7,
        day: 1,
        tai_utc: 36.0,
    },
    LeapSecondEntry {
        year: 2017,
        month: 1,
        day: 1,
        tai_utc: 37.0,
    },
];

/// TT - TAI offset in seconds (exact by definition).
pub const TT_TAI_OFFSET: f64 = 32.184;

/// J2000.0 epoch: 2000-01-01 12:00:00 TT
/// This is defined as JD 2451545.0 TT.
pub const J2000_JD: f64 = 2451545.0;

/// Seconds per day.
pub const SECONDS_PER_DAY: f64 = 86400.0;

/// Look up the TAI-UTC offset (leap seconds) for a given calendar date.
///
/// Returns the cumulative number of leap seconds for dates >= 1972-01-01.
/// Returns None for dates before 1972-01-01 (before modern leap second system).
pub fn tai_utc_offset(year: i32, month: u32, day: u32) -> Option<f64> {
    let mut result = None;
    for entry in LEAP_SECOND_TABLE {
        if (year, month, day) >= (entry.year, entry.month, entry.day) {
            result = Some(entry.tai_utc);
        } else {
            break;
        }
    }
    result
}

/// Look up TAI-UTC offset for a given Julian Date (UTC).
pub fn tai_utc_offset_jd(jd_utc: f64) -> Option<f64> {
    let (year, month, day, _, _, _) = jd_to_calendar(jd_utc);
    tai_utc_offset(year, month, day)
}

/// Convert Julian Date to calendar date (year, month, day, hour, minute, second).
///
/// Algorithm from Meeus, "Astronomical Algorithms" (1991), Chapter 7.
pub fn jd_to_calendar(jd: f64) -> (i32, u32, u32, u32, u32, f64) {
    let jd_plus = jd + 0.5;
    let z = jd_plus.floor() as i64;
    let f = jd_plus - z as f64;

    let a = if z < 2299161 {
        z
    } else {
        let alpha = ((z as f64 - 1867216.25) / 36524.25).floor() as i64;
        z + 1 + alpha - alpha / 4
    };

    let b = a + 1524;
    let c = ((b as f64 - 122.1) / 365.25).floor() as i64;
    let d = (365.25 * c as f64).floor() as i64;
    let e = ((b - d) as f64 / 30.6001).floor() as i64;

    let day = (b - d - (30.6001 * e as f64).floor() as i64) as u32;
    let month = if e < 14 { e - 1 } else { e - 13 } as u32;
    let year = if month > 2 { c - 4716 } else { c - 4715 } as i32;

    let total_hours = f * 24.0;
    let hour = total_hours.floor() as u32;
    let total_minutes = (total_hours - hour as f64) * 60.0;
    let minute = total_minutes.floor() as u32;
    let second = (total_minutes - minute as f64) * 60.0;

    (year, month, day, hour, minute, second)
}

/// Convert calendar date to Julian Date.
///
/// Algorithm from Meeus, "Astronomical Algorithms" (1991), Chapter 7.
pub fn calendar_to_jd(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: f64) -> f64 {
    let (y, m) = if month <= 2 {
        (year as f64 - 1.0, month as f64 + 12.0)
    } else {
        (year as f64, month as f64)
    };

    let a = (y / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();

    let jd =
        (365.25 * (y + 4716.0)).floor() + (30.6001 * (m + 1.0)).floor() + day as f64 + b - 1524.5;

    let day_fraction = (hour as f64 + minute as f64 / 60.0 + second / 3600.0) / 24.0;
    jd + day_fraction
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tai_utc_offset_modern() {
        // After 2017-01-01: 37 leap seconds
        assert_eq!(tai_utc_offset(2024, 1, 1), Some(37.0));
        assert_eq!(tai_utc_offset(2030, 6, 15), Some(37.0));
    }

    #[test]
    fn test_tai_utc_offset_historical() {
        // 1972-01-01: first entry, 10 leap seconds
        assert_eq!(tai_utc_offset(1972, 1, 1), Some(10.0));
        // Mid-1972: 11 leap seconds
        assert_eq!(tai_utc_offset(1972, 7, 1), Some(11.0));
        // Before 1972: no modern leap seconds
        assert_eq!(tai_utc_offset(1971, 12, 31), None);
    }

    #[test]
    fn test_tai_utc_offset_boundary() {
        // Day before a leap second change
        assert_eq!(tai_utc_offset(2016, 12, 31), Some(36.0));
        // Day of the change
        assert_eq!(tai_utc_offset(2017, 1, 1), Some(37.0));
    }

    #[test]
    fn test_calendar_to_jd_j2000() {
        // J2000.0 = 2000-01-01 12:00:00 TT
        let jd = calendar_to_jd(2000, 1, 1, 12, 0, 0.0);
        assert!(
            (jd - J2000_JD).abs() < 1e-10,
            "Expected J2000 JD {}, got {}",
            J2000_JD,
            jd
        );
    }

    #[test]
    fn test_jd_to_calendar_j2000() {
        // Tier 1: noon JD → fractional part f = 0.0 → second = 0.0 exactly.
        let (year, month, day, hour, minute, second) = jd_to_calendar(J2000_JD);
        assert_eq!(year, 2000);
        assert_eq!(month, 1);
        assert_eq!(day, 1);
        assert_eq!(hour, 12);
        assert_eq!(minute, 0);
        assert_eq!(second, 0.0, "Expected exactly 0.0 seconds, got {}", second);
    }

    #[test]
    fn test_calendar_jd_roundtrip() {
        let jd = calendar_to_jd(2030, 12, 12, 0, 0, 0.0);
        let (year, month, day, hour, minute, second) = jd_to_calendar(jd);
        assert_eq!(year, 2030);
        assert_eq!(month, 12);
        assert_eq!(day, 12);
        assert_eq!(hour, 0);
        assert_eq!(minute, 0);
        assert!(second.abs() < 1e-6);
    }

    #[test]
    fn test_calendar_jd_roundtrip_with_time() {
        // Tier 2: f64 JD ~2.46e6 has ULP ~2^-52 * 2.46e6 ≈ 5.5e-10 days
        // ≈ 4.7e-5 seconds. Tighten from 1e-3 to 5e-5.
        let jd = calendar_to_jd(2033, 7, 4, 15, 30, 45.5);
        let (year, month, day, hour, minute, second) = jd_to_calendar(jd);
        assert_eq!(year, 2033);
        assert_eq!(month, 7);
        assert_eq!(day, 4);
        assert_eq!(hour, 15);
        assert_eq!(minute, 30);
        assert!(
            (second - 45.5).abs() < 5e-5,
            "Expected ~45.5, got {}",
            second
        );
    }

    // =====================================================================
    // A6: Leap seconds edge case tests
    // =====================================================================

    #[test]
    fn test_leap_second_boundary_dec31_jan1() {
        // Tier 1: exact integer values from table.
        // 2016-12-31 → 36, 2017-01-01 → 37
        assert_eq!(tai_utc_offset(2016, 12, 31), Some(36.0));
        assert_eq!(tai_utc_offset(2017, 1, 1), Some(37.0));
    }

    #[test]
    fn test_calendar_to_jd_known_values() {
        // Tier 1: Unix epoch = 1970-01-01 00:00:00 → JD 2440587.5
        let jd_unix = calendar_to_jd(1970, 1, 1, 0, 0, 0.0);
        assert_eq!(
            jd_unix, 2440587.5,
            "Unix epoch JD should be 2440587.5, got {}",
            jd_unix
        );

        // Tier 1: MJD origin = 1858-11-17 00:00:00 → JD 2400000.5
        let jd_mjd = calendar_to_jd(1858, 11, 17, 0, 0, 0.0);
        assert_eq!(
            jd_mjd, 2400000.5,
            "MJD origin JD should be 2400000.5, got {}",
            jd_mjd
        );
    }

    #[test]
    fn test_calendar_jd_roundtrip_parametric() {
        // Tier 2: multiple dates covering range of table.
        let dates = [
            (1972, 1, 1, 0, 0, 0.0),
            (2000, 1, 1, 12, 0, 0.0),
            (2030, 6, 15, 6, 30, 0.0),
            (2100, 12, 31, 23, 59, 59.0),
        ];
        for &(y, mo, d, h, mi, s) in &dates {
            let jd = calendar_to_jd(y, mo, d, h, mi, s);
            let (ry, rmo, rd, rh, rmi, rs) = jd_to_calendar(jd);
            assert_eq!(
                (ry, rmo, rd, rh, rmi),
                (y, mo, d, h, mi),
                "Roundtrip failed for {}-{:02}-{:02}T{:02}:{:02}",
                y,
                mo,
                d,
                h,
                mi
            );
            // Tier 2: second precision bounded by f64 JD ULP ≈ 5e-5 seconds
            assert!(
                (rs - s).abs() < 5e-5,
                "Second roundtrip for {}-{:02}-{:02}: expected {}, got {}",
                y,
                mo,
                d,
                s,
                rs
            );
        }
    }

    #[test]
    fn test_tai_utc_offset_jd_matches_calendar() {
        // Tier 1: JD-based lookup should match calendar-based lookup.
        let dates = [
            (2000, 1, 1, 12, 0, 0.0),
            (2017, 1, 1, 0, 0, 0.0),
            (2024, 6, 15, 0, 0, 0.0),
        ];
        for &(y, mo, d, h, mi, s) in &dates {
            let cal_offset = tai_utc_offset(y, mo, d);
            let jd = calendar_to_jd(y, mo, d, h, mi, s);
            let jd_offset = tai_utc_offset_jd(jd);
            assert_eq!(
                cal_offset, jd_offset,
                "Calendar vs JD offset mismatch for {}-{:02}-{:02}",
                y, mo, d
            );
        }
    }
}
