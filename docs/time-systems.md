# Time System Support

VVERDAD includes built-in support for aerospace time systems. Date/time strings in data files are eagerly parsed at load time into typed epoch values, enabling time scale conversions and Julian date calculations in templates.

## Supported Time Scales

| Time Scale | Stored As | Description |
|-----------|-----------|-------------|
| **UTC** | `Value::Utc(f64)` | Coordinated Universal Time. Civil time with leap seconds. |
| **TDB** | `Value::Tdb(f64)` | Barycentric Dynamical Time. Used for solar system ephemerides (SPICE kernels). |
| **TT** | `Value::Tdb(f64)` | Terrestrial Time. Converted to TDB at parse time (TDB = TT + periodic correction). |
| **TAI** | `Value::Utc(f64)` | International Atomic Time. Converted to UTC at parse time (UTC = TAI - leap_seconds). |

Only UTC and TDB are stored internally. TT and TAI are converted to their respective storage scales during parsing.

## Internal Representation

All epochs are stored as **f64 days after J2000.0**.

The J2000.0 epoch is defined as:
- **2000-01-01 12:00:00.000 TT** (Terrestrial Time)
- Julian Date 2451545.0

This is the standard reference epoch used in astrodynamics and ephemeris computation.

## Recognized String Formats

Strings are only parsed as epochs if they end with a **space-separated time system suffix**. Strings without a suffix remain as `Value::String` and are never interpreted as dates.

### Supported Suffixes

`UTC`, `TDB`, `TT`, `TAI` (case-insensitive)

### ISO 8601 Format

```
"2030-12-12T00:00:00 UTC"
"2030-12-12 00:00:00 TDB"
"2030-12-12T00:00:00.000 UTC"
"2030-12-12T15:30:45.500 TDB"
"2030-12-12 UTC"                  (date only, time defaults to 00:00:00)
```

Pattern: `YYYY-MM-DD[T| ]HH:MM:SS[.fff] <SYSTEM>`

### SPICE-Style Format

```
"12-DEC-2030 UTC"
"2030-JUN-15 12:00:00.000 TDB"
"12-DEC-2030 00:00:00 UTC"
```

Pattern: `DD-MON-YYYY[ HH:MM:SS[.fff]] <SYSTEM>` or `YYYY-MON-DD[ HH:MM:SS[.fff]] <SYSTEM>`

Month abbreviations: JAN, FEB, MAR, APR, MAY, JUN, JUL, AUG, SEP, OCT, NOV, DEC

### Non-Matching Strings

These strings do NOT trigger time parsing and remain as `Value::String`:

```
"2030-12-12"              (no time system suffix)
"12-DEC-2030"             (no time system suffix)
"hello world"             (not a date)
"100 N"                   (parsed as quantity instead)
```

## Time Scale Conversions

### UTC to TDB

Path: UTC -> TAI -> TT -> TDB

1. **TAI = UTC + leap_seconds**: Leap seconds looked up from compiled-in table based on calendar date
2. **TT = TAI + 32.184s**: Exact by definition
3. **TDB = TT + periodic correction**: Fairhead & Bretagnon (1990) dominant term

The TDB-TT periodic correction:
```
TDB - TT = 0.001657 * sin(628.3076 * T + 6.2401) seconds
```
where T is Julian centuries of TDB from J2000.0 (T = days / 36525.0). Maximum amplitude is approximately 1.7 milliseconds.

### TDB to UTC

Path: TDB -> TT -> TAI -> UTC (reverse of above)

1. Remove TDB-TT periodic correction to get TT
2. Subtract 32.184s to get TAI
3. Subtract leap seconds (looked up from approximate UTC date) to get UTC

The TDB-to-UTC conversion uses an iterative approach because the leap second lookup depends on the UTC date, which is not yet known.

### TT to TDB (at parse time)

When a string with ` TT` suffix is parsed, it is converted to TDB and stored as `Value::Tdb`:
```
TDB = TT + (0.001657 * sin(628.3076 * T + 6.2401)) / 86400.0 days
```

### TAI to UTC (at parse time)

When a string with ` TAI` suffix is parsed, it is converted to UTC and stored as `Value::Utc`:
```
UTC = TAI - leap_seconds / 86400.0 days
```

## Leap Second Table

VVERDAD includes a compiled-in leap second table with 28 entries covering 1972-01-01 through 2017-01-01 (the last leap second as of 2024).

- Source: IERS Bulletin C / NAIF LSK kernel
- First entry: 1972-01-01, TAI-UTC = 10s
- Last entry: 2017-01-01, TAI-UTC = 37s
- Dates before 1972-01-01 are not supported (UTC parsing returns an error)
- Dates after 2017-01-01 use the last known offset (37s)

### Key Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `TT_TAI_OFFSET` | 32.184 s | TT - TAI (exact by definition) |
| `J2000_JD` | 2451545.0 | Julian Date of J2000.0 epoch |
| `SECONDS_PER_DAY` | 86400.0 | Seconds per day |

## Template Filters

The time module registers four Minijinja filters for use in templates:

### `|to_utc`

Converts a TDB epoch string to UTC. Passes through UTC values unchanged. Non-epoch strings pass through unchanged.

```jinja
{{ mission.departure_epoch | to_utc }}
{# "2030-12-12T00:00:00.000 TDB" -> "2030-12-11T23:58:50.816 UTC" (approx) #}
```

### `|to_tdb`

Converts a UTC epoch string to TDB. Passes through TDB values unchanged. Non-epoch strings pass through unchanged.

```jinja
{{ mission.departure_epoch | to_tdb }}
{# "2030-12-12T00:00:00.000 UTC" -> "2030-12-12T00:01:09.184 TDB" (approx) #}
```

### `|jd`

Returns the Julian Date (as a float) for any epoch string. Non-epoch strings pass through unchanged.

```jinja
{{ mission.departure_epoch | jd }}
{# "2000-01-01T12:00:00.000 UTC" -> 2451545.0 #}
```

### `|mjd`

Returns the Modified Julian Date (JD - 2400000.5) for any epoch string. Non-epoch strings pass through unchanged.

```jinja
{{ mission.departure_epoch | mjd }}
{# "2000-01-01T12:00:00.000 UTC" -> 51544.5 #}
```

## Default Rendering

When epoch values are serialized to strings (for template output), the format is:

- **UTC**: `"YYYY-MM-DDTHH:MM:SS.fff UTC"` (e.g., `"2030-12-12T00:00:00.000 UTC"`)
- **TDB**: `"YYYY-MM-DDTHH:MM:SS.fff TDB"` (e.g., `"2030-12-12T00:00:00.000 TDB"`)

These strings include millisecond precision and the time system suffix.

## Limitations

- Dates before 1972-01-01 cannot be represented in UTC (no leap second data)
- The leap second table is compiled-in and does not auto-update. If future leap seconds are announced (unlikely given the 2022 resolution to abolish them by 2035), the table must be updated in source code
- The TDB-TT conversion uses only the dominant Fairhead & Bretagnon term (1.7ms amplitude). For sub-millisecond accuracy in deep-space navigation, the full series would be needed
- Precision is limited by f64 representation of Julian Dates (~0.1ms for dates near present)
