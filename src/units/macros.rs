//! Unit definition macro and dimension constants.

use crate::units::error::Dimensions;

// =============================================================================
// Dimension Constants
// =============================================================================

/// Length: m
pub const LENGTH: Dimensions = [1, 0, 0, 0, 0, 0, 0, 0];

/// Mass: kg
pub const MASS: Dimensions = [0, 1, 0, 0, 0, 0, 0, 0];

/// Time: s
pub const TIME: Dimensions = [0, 0, 1, 0, 0, 0, 0, 0];

/// Electric current: A
pub const CURRENT: Dimensions = [0, 0, 0, 1, 0, 0, 0, 0];

/// Temperature: K
pub const TEMPERATURE: Dimensions = [0, 0, 0, 0, 1, 0, 0, 0];

/// Amount of substance: mol
pub const AMOUNT: Dimensions = [0, 0, 0, 0, 0, 1, 0, 0];

/// Luminous intensity: cd
pub const LUMINOSITY: Dimensions = [0, 0, 0, 0, 0, 0, 1, 0];

/// Velocity: m/s
pub const VELOCITY: Dimensions = [1, 0, -1, 0, 0, 0, 0, 0];

/// Acceleration: m/s^2
pub const ACCELERATION: Dimensions = [1, 0, -2, 0, 0, 0, 0, 0];

/// Force: N = kg*m/s^2
pub const FORCE: Dimensions = [1, 1, -2, 0, 0, 0, 0, 0];

/// Energy: J = kg*m^2/s^2
pub const ENERGY: Dimensions = [2, 1, -2, 0, 0, 0, 0, 0];

/// Power: W = kg*m^2/s^3
pub const POWER: Dimensions = [2, 1, -3, 0, 0, 0, 0, 0];

/// Pressure: Pa = kg/(m*s^2)
pub const PRESSURE: Dimensions = [-1, 1, -2, 0, 0, 0, 0, 0];

/// Frequency: Hz = 1/s
pub const FREQUENCY: Dimensions = [0, 0, -1, 0, 0, 0, 0, 0];

/// Electric charge: C = A*s
pub const CHARGE: Dimensions = [0, 0, 1, 1, 0, 0, 0, 0];

/// Electric potential: V = kg*m^2/(A*s^3)
pub const VOLTAGE: Dimensions = [2, 1, -3, -1, 0, 0, 0, 0];

/// Electric resistance: Ω = kg*m^2/(A^2*s^3)
pub const RESISTANCE: Dimensions = [2, 1, -3, -2, 0, 0, 0, 0];

/// Capacitance: F = A^2*s^4/(kg*m^2)
pub const CAPACITANCE: Dimensions = [-2, -1, 4, 2, 0, 0, 0, 0];

/// Inductance: H = kg*m^2/(A^2*s^2)
pub const INDUCTANCE: Dimensions = [2, 1, -2, -2, 0, 0, 0, 0];

/// Magnetic field: T = kg/(A*s^2)
pub const MAGNETIC_FIELD: Dimensions = [0, 1, -2, -1, 0, 0, 0, 0];

/// Magnetic flux: Wb = kg*m^2/(A*s^2)
pub const MAGNETIC_FLUX: Dimensions = [2, 1, -2, -1, 0, 0, 0, 0];

/// Area: m^2
pub const AREA: Dimensions = [2, 0, 0, 0, 0, 0, 0, 0];

/// Volume: m^3
pub const VOLUME: Dimensions = [3, 0, 0, 0, 0, 0, 0, 0];

/// Dimensionless (pure number)
pub const DIMENSIONLESS: Dimensions = [0, 0, 0, 0, 0, 0, 0, 0];

/// Angle: rad (dimension 7 = 1)
pub const ANGLE: Dimensions = [0, 0, 0, 0, 0, 0, 0, 1];

/// Solid angle: sr (dimension 7 = 2)
pub const SOLID_ANGLE: Dimensions = [0, 0, 0, 0, 0, 0, 0, 2];

// =============================================================================
// Unit Definition Macro
// =============================================================================

/// Defines units and generates both const definitions and lookup table entries.
///
/// # Syntax
///
/// ```ignore
/// define_units! {
///     CONST_NAME: DimensionType = conversion_factor, "symbol", ["name", "alias1", "alias2"];
/// }
/// ```
///
/// - `CONST_NAME`: The constant name (e.g., `KILOMETER`)
/// - `DimensionType`: One of the dimension constants (e.g., `LENGTH`, `FORCE`)
/// - `conversion_factor`: Factor to convert to SI base units (e.g., `1000.0` for km)
/// - `"symbol"`: The primary symbol (e.g., `"km"`)
/// - `["name", ...]`: The canonical name (first) and optional aliases (rest)
///
/// # Example
///
/// ```ignore
/// define_units! {
///     KILOMETER: LENGTH = 1000.0, "km", ["kilometer", "kilometers"];
///     MILE: LENGTH = 1609.344, "mi", ["mile", "miles"];
/// }
/// ```
#[macro_export]
macro_rules! define_units {
    ($(
        $(#[$meta:meta])*
        $name:ident : $dims:expr_2021 => $conversion:expr_2021, $symbol:expr_2021, [$canonical:expr_2021 $(, $alias:expr_2021)*];
    )*) => {
        // Generate const definitions
        $(
            $(#[$meta])*
            pub const $name: Unit = Unit::new($dims, $conversion, $symbol, $canonical);
        )*

        /// Returns a slice of all known unit symbols and their corresponding Units.
        pub fn unit_lookup() -> &'static [(&'static str, Unit)] {
            &[
                $(
                    ($symbol, $name),
                    ($canonical, $name),
                    $(($alias, $name),)*
                )*
            ]
        }

        /// Looks up a unit by its symbol or name.
        /// Returns None if not found.
        pub fn lookup_unit(symbol: &str) -> Option<Unit> {
            let table = unit_lookup();
            for (s, u) in table {
                if *s == symbol {
                    return Some(*u);
                }
            }
            None
        }
    };
}

pub use define_units;
