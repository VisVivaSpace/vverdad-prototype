//! Unit type and operations.

use crate::units::error::{Dimensions, UnitError};
use std::fmt;
use std::str::FromStr;

/// A unit of measurement with dimensions and conversion factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unit {
    pub dimensions: Dimensions,
    pub conversion: f64,
    pub symbol: &'static str,
    pub name: &'static str,
}

impl Unit {
    /// Creates a new unit with the given properties.
    pub const fn new(
        dimensions: Dimensions,
        conversion: f64,
        symbol: &'static str,
        name: &'static str,
    ) -> Self {
        Unit {
            dimensions,
            conversion,
            symbol,
            name,
        }
    }

    /// Creates a derived unit from multiplying two units.
    pub fn multiply(a: &Unit, b: &Unit) -> Unit {
        let mut dims = [0i8; 8];
        for (i, dim) in dims.iter_mut().enumerate() {
            *dim = a.dimensions[i] + b.dimensions[i];
        }
        Unit {
            dimensions: dims,
            conversion: a.conversion * b.conversion,
            symbol: build_compound_symbol(a.symbol, "*", b.symbol),
            name: "derived",
        }
    }

    /// Creates a derived unit from dividing two units.
    pub fn divide(a: &Unit, b: &Unit) -> Unit {
        let mut dims = [0i8; 8];
        for (i, dim) in dims.iter_mut().enumerate() {
            *dim = a.dimensions[i] - b.dimensions[i];
        }
        Unit {
            dimensions: dims,
            conversion: a.conversion / b.conversion,
            symbol: build_compound_symbol(a.symbol, "/", b.symbol),
            name: "derived",
        }
    }

    /// Returns true if this unit has the same SI dimensions (0-6) as another.
    pub fn same_si_dimensions(&self, other: &Unit) -> bool {
        self.dimensions[0..7] == other.dimensions[0..7]
    }

    /// Returns true if this unit is dimensionless (no SI dimensions).
    pub fn is_dimensionless(&self) -> bool {
        self.dimensions[0..7] == [0; 7]
    }
}

/// Builds a compound symbol string like "m/s" or "N*m" from two component symbols.
///
/// Returns "" if either component is empty. Uses Box::leak() to produce &'static str —
/// the leak is bounded by the number of unique compound units (typically single digits).
fn build_compound_symbol(a: &str, op: &str, b: &str) -> &'static str {
    if a.is_empty() || b.is_empty() {
        return "";
    }
    Box::leak(format!("{}{}{}", a, op, b).into_boxed_str())
}

/// Builds a symbol string for a unit raised to a power, like "m^2" or "s^-2".
///
/// Returns the original symbol unchanged if power is 1, or "" if symbol is empty.
pub fn build_power_symbol(symbol: &'static str, power: i8) -> &'static str {
    if symbol.is_empty() {
        return "";
    }
    if power == 1 {
        return symbol;
    }
    Box::leak(format!("{}^{}", symbol, power).into_boxed_str())
}

/// SI base unit symbols indexed by dimension position.
const SI_BASE_SYMBOLS: [&str; 7] = ["m", "kg", "s", "A", "K", "mol", "cd"];

/// Returns the SI unit symbol for a given dimension array.
///
/// First checks for well-known named units (N, J, W, Pa, Hz), then builds
/// a compound symbol from SI base units for anything else.
pub fn si_symbol(dims: &Dimensions) -> &'static str {
    // Check well-known named SI derived units first
    match *dims {
        [0, 0, 0, 0, 0, 0, 0, 0] => return "",
        [1, 0, 0, 0, 0, 0, 0, _] => return "m",
        [0, 1, 0, 0, 0, 0, 0, _] => return "kg",
        [0, 0, 1, 0, 0, 0, 0, _] => return "s",
        [0, 0, 0, 1, 0, 0, 0, _] => return "A",
        [0, 0, 0, 0, 1, 0, 0, _] => return "K",
        [0, 0, 0, 0, 0, 1, 0, _] => return "mol",
        [0, 0, 0, 0, 0, 0, 1, _] => return "cd",
        [1, 1, -2, 0, 0, 0, 0, _] => return "N",
        [2, 1, -2, 0, 0, 0, 0, _] => return "J",
        [2, 1, -3, 0, 0, 0, 0, _] => return "W",
        [-1, 1, -2, 0, 0, 0, 0, _] => return "Pa",
        [0, 0, -1, 0, 0, 0, 0, _] => return "Hz",
        _ => {}
    }

    // Build from SI base units: numerator (positive dims) / denominator (negative dims)
    let mut num_parts = Vec::new();
    let mut den_parts = Vec::new();

    for (i, &dim) in dims[0..7].iter().enumerate() {
        if dim > 0 {
            if dim == 1 {
                num_parts.push(SI_BASE_SYMBOLS[i].to_string());
            } else {
                num_parts.push(format!("{}^{}", SI_BASE_SYMBOLS[i], dim));
            }
        } else if dim < 0 {
            let abs = -dim;
            if abs == 1 {
                den_parts.push(SI_BASE_SYMBOLS[i].to_string());
            } else {
                den_parts.push(format!("{}^{}", SI_BASE_SYMBOLS[i], abs));
            }
        }
    }

    if num_parts.is_empty() && den_parts.is_empty() {
        return "";
    }

    let symbol = if den_parts.is_empty() {
        num_parts.join("*")
    } else if num_parts.is_empty() {
        // Pure denominator like 1/s^2 — use negative exponents
        den_parts
            .iter()
            .map(|s| {
                if s.contains('^') {
                    format!(
                        "{}^-{}",
                        &s[..s.find('^').unwrap()],
                        &s[s.find('^').unwrap() + 1..]
                    )
                } else {
                    format!("{}^-1", s)
                }
            })
            .collect::<Vec<_>>()
            .join("*")
    } else {
        format!("{}/{}", num_parts.join("*"), den_parts.join("*"))
    };

    Box::leak(symbol.into_boxed_str())
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.symbol.is_empty() {
            write!(f, "{}", self.name)
        } else {
            write!(f, "{}", self.symbol)
        }
    }
}

impl FromStr for Unit {
    type Err = UnitError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::units::parse::parse_unit(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::definitions::*;

    #[test]
    fn test_divide_symbol() {
        let unit = Unit::divide(&METER, &SECOND);
        assert_eq!(unit.symbol, "m/s");
        assert_eq!(format!("{}", unit), "m/s");
    }

    #[test]
    fn test_divide_symbol_with_prefix() {
        let unit = Unit::divide(&KILOMETER, &SECOND);
        assert_eq!(unit.symbol, "km/s");
        assert_eq!(format!("{}", unit), "km/s");
    }

    #[test]
    fn test_multiply_symbol() {
        let unit = Unit::multiply(&NEWTON, &METER);
        assert_eq!(unit.symbol, "N*m");
        assert_eq!(format!("{}", unit), "N*m");
    }

    #[test]
    fn test_compound_symbol_empty_propagates() {
        // If either operand has empty symbol, result should be empty
        let empty = Unit::new([0; 8], 1.0, "", "test");
        let result = Unit::multiply(&METER, &empty);
        assert_eq!(result.symbol, "");
    }

    #[test]
    fn test_si_symbol_known_units() {
        assert_eq!(si_symbol(&[1, 0, 0, 0, 0, 0, 0, 0]), "m");
        assert_eq!(si_symbol(&[0, 1, 0, 0, 0, 0, 0, 0]), "kg");
        assert_eq!(si_symbol(&[0, 0, 1, 0, 0, 0, 0, 0]), "s");
        assert_eq!(si_symbol(&[1, 1, -2, 0, 0, 0, 0, 0]), "N");
        assert_eq!(si_symbol(&[2, 1, -2, 0, 0, 0, 0, 0]), "J");
        assert_eq!(si_symbol(&[2, 1, -3, 0, 0, 0, 0, 0]), "W");
        assert_eq!(si_symbol(&[-1, 1, -2, 0, 0, 0, 0, 0]), "Pa");
        assert_eq!(si_symbol(&[0, 0, -1, 0, 0, 0, 0, 0]), "Hz");
    }

    #[test]
    fn test_si_symbol_velocity() {
        // Velocity is not a named SI unit — should build from base units
        let sym = si_symbol(&[1, 0, -1, 0, 0, 0, 0, 0]);
        assert_eq!(sym, "m/s");
    }

    #[test]
    fn test_si_symbol_acceleration() {
        let sym = si_symbol(&[1, 0, -2, 0, 0, 0, 0, 0]);
        assert_eq!(sym, "m/s^2");
    }

    #[test]
    fn test_si_symbol_unknown_compound() {
        // Something exotic like m^3*kg/s — should build from base units
        let sym = si_symbol(&[3, 1, -1, 0, 0, 0, 0, 0]);
        assert_eq!(sym, "m^3*kg/s");
    }

    #[test]
    fn test_build_power_symbol() {
        assert_eq!(build_power_symbol("m", 2), "m^2");
        assert_eq!(build_power_symbol("s", -2), "s^-2");
        assert_eq!(build_power_symbol("m", 1), "m");
        assert_eq!(build_power_symbol("", 2), "");
    }
}
