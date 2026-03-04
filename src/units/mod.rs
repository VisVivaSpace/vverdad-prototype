//! Units module - Physical quantity handling with Minijinja filters
//!
//! This module provides:
//! - Parsing quantity strings (e.g., "5 kg", "100 kN")
//! - Unit conversion and arithmetic
//! - Minijinja filters for template rendering

pub mod definitions;
pub mod error;
#[macro_use]
pub mod macros;
pub mod parse;
pub mod quantity;
pub mod unit;

// Re-export main types
pub use error::{Dimensions, UnitError, UnitResult};
pub use quantity::Quantity;
pub use unit::Unit;

// Re-export parsing functions
pub use parse::{parse_quantity, parse_unit};

// Re-export all units and lookup from definitions
pub use definitions::*;

// =============================================================================
// Minijinja Filters
// =============================================================================

use minijinja::{Error as MJError, ErrorKind, Value as MJValue};

/// Converts a quantity string to a different unit.
///
/// Usage in templates: `{{ thrust | to("lbf") }}` -> "22480.9 lbf"
pub fn filter_to(value: MJValue, target: String) -> Result<MJValue, MJError> {
    let input = value
        .as_str()
        .ok_or_else(|| MJError::new(ErrorKind::InvalidOperation, "expected string value"))?;

    // Try to parse as quantity
    let qty = match parse_quantity(input) {
        Ok(q) => q,
        Err(_) => {
            // Not a valid quantity, return unchanged
            return Ok(value);
        }
    };

    // Parse target unit
    let target_unit = parse_unit(&target).map_err(|e| {
        MJError::new(
            ErrorKind::InvalidOperation,
            format!("invalid target unit '{}': {}", target, e),
        )
    })?;

    // Convert
    let converted = qty.convert_to(target_unit).map_err(|e| {
        MJError::new(
            ErrorKind::InvalidOperation,
            format!("conversion failed: {}", e),
        )
    })?;

    Ok(MJValue::from(converted.to_string()))
}

/// Extracts the numeric value of a quantity in the specified unit.
///
/// Usage in templates: `{{ thrust | value("N") }}` -> 100000.0
pub fn filter_value(value: MJValue, target: String) -> Result<MJValue, MJError> {
    let input = value
        .as_str()
        .ok_or_else(|| MJError::new(ErrorKind::InvalidOperation, "expected string value"))?;

    // Try to parse as quantity
    let qty = match parse_quantity(input) {
        Ok(q) => q,
        Err(_) => {
            // Not a valid quantity - try parsing as plain number
            if let Ok(num) = input.parse::<f64>() {
                return Ok(MJValue::from(num));
            }
            return Err(MJError::new(
                ErrorKind::InvalidOperation,
                format!("cannot parse '{}' as quantity", input),
            ));
        }
    };

    // Parse target unit
    let target_unit = parse_unit(&target).map_err(|e| {
        MJError::new(
            ErrorKind::InvalidOperation,
            format!("invalid target unit '{}': {}", target, e),
        )
    })?;

    // Convert and extract value
    let converted = qty.convert_to(target_unit).map_err(|e| {
        MJError::new(
            ErrorKind::InvalidOperation,
            format!("conversion failed: {}", e),
        )
    })?;

    Ok(MJValue::from(converted.value))
}

/// Extracts the unit symbol from a quantity string.
///
/// Usage in templates: `{{ thrust | unit }}` -> "kN"
pub fn filter_unit(value: MJValue) -> Result<MJValue, MJError> {
    let input = value
        .as_str()
        .ok_or_else(|| MJError::new(ErrorKind::InvalidOperation, "expected string value"))?;

    // Try to parse as quantity
    let qty = match parse_quantity(input) {
        Ok(q) => q,
        Err(_) => {
            // Not a valid quantity, return empty string
            return Ok(MJValue::from(""));
        }
    };

    Ok(MJValue::from(qty.unit.to_string()))
}

/// Converts a quantity to SI base units.
///
/// Usage in templates: `{{ thrust | si }}` -> "100000 N"
pub fn filter_si(value: MJValue) -> Result<MJValue, MJError> {
    let input = value
        .as_str()
        .ok_or_else(|| MJError::new(ErrorKind::InvalidOperation, "expected string value"))?;

    // Try to parse as quantity
    let qty = match parse_quantity(input) {
        Ok(q) => q,
        Err(_) => {
            // Not a valid quantity, return unchanged
            return Ok(value);
        }
    };

    // Get SI value and construct result with SI unit symbol
    let si_value = qty.si_value();
    let si_unit_symbol = get_si_unit_symbol(&qty.unit);

    Ok(MJValue::from(format!("{} {}", si_value, si_unit_symbol)))
}

/// Gets the SI unit symbol for a given unit based on its dimensions.
fn get_si_unit_symbol(unit: &Unit) -> &'static str {
    unit::si_symbol(&unit.dimensions)
}

/// Registers all unit filters with a Minijinja environment.
pub fn register_filters(env: &mut minijinja::Environment) {
    env.add_filter("to", filter_to);
    env.add_filter("value", filter_value);
    env.add_filter("unit", filter_unit);
    env.add_filter("si", filter_si);
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_to_basic() {
        let result = filter_to(MJValue::from("100 kN"), "lbf".to_string()).unwrap();
        let s = result.as_str().unwrap();
        assert!(s.contains("lbf"), "Expected lbf in result: {}", s);
    }

    #[test]
    fn test_filter_to_length() {
        let result = filter_to(MJValue::from("1 km"), "m".to_string()).unwrap();
        let s = result.as_str().unwrap();
        assert!(s.contains("1000"), "Expected 1000 in result: {}", s);
    }

    #[test]
    fn test_filter_value_basic() {
        let result = filter_value(MJValue::from("100 kN"), "N".to_string()).unwrap();
        let v: f64 = result.try_into().unwrap();
        assert!((v - 100000.0).abs() < 1e-6, "Expected 100000, got {}", v);
    }

    #[test]
    fn test_filter_unit_basic() {
        let result = filter_unit(MJValue::from("100 kN")).unwrap();
        let s = result.as_str().unwrap();
        assert_eq!(s, "kN");
    }

    #[test]
    fn test_filter_si_force() {
        let result = filter_si(MJValue::from("100 kN")).unwrap();
        let s = result.as_str().unwrap();
        assert!(s.contains("100000"), "Expected 100000 in result: {}", s);
        assert!(s.contains("N"), "Expected N in result: {}", s);
    }

    #[test]
    fn test_filter_to_invalid_quantity_passthrough() {
        // Non-quantity strings should pass through unchanged
        let result = filter_to(MJValue::from("hello world"), "m".to_string()).unwrap();
        assert_eq!(result.as_str().unwrap(), "hello world");
    }

    #[test]
    fn test_quantity_parsing() {
        let q = parse_quantity("100 kN").unwrap();
        assert!((q.value - 100.0).abs() < 1e-10);
        assert!((q.si_value() - 100000.0).abs() < 1e-6);
    }

    #[test]
    fn test_unit_conversion() {
        let q = parse_quantity("1 km").unwrap();
        let converted = q.convert_to(METER).unwrap();
        assert!((converted.value - 1000.0).abs() < 1e-10);
    }

    // =====================================================================
    // Unit conversion reference tests (NIST/IAU definitional constants)
    // =====================================================================

    #[test]
    fn test_ref_mile_to_meters() {
        // Tier 1: NIST exact definition. 1 mi = 1609.344 m
        assert_eq!(MILE.conversion, 1609.344);
    }

    #[test]
    fn test_ref_foot_inch_ratio() {
        // Tier 2: 0.3048 / 0.0254 = 12.0, but f64 division introduces
        // rounding at the last bit. Relative tolerance.
        let ratio = FOOT.conversion / INCH.conversion;
        let rel = (ratio - 12.0).abs() / 12.0;
        assert!(
            rel < 8.0 * f64::EPSILON,
            "ft/in ratio: expected 12.0, got {}",
            ratio
        );
    }

    #[test]
    fn test_ref_lbf_to_newtons() {
        // Tier 2: NIST conversion factor. 1 lbf = 4.448222 N
        let rel = (POUND_FORCE.conversion - 4.448_222).abs() / 4.448_222;
        assert!(
            rel < 1e-6,
            "lbf conversion: expected ~4.448222, got {}",
            POUND_FORCE.conversion
        );
    }

    #[test]
    fn test_ref_au_to_meters() {
        // Tier 1: IAU 2012 exact definition. 1 AU = 149597870700 m
        assert_eq!(AU.conversion, 149_597_870_700.0);
    }

    #[test]
    fn test_ref_180_deg_equals_pi_rad() {
        // Tier 2: exact definition. 180° = π rad
        let deg_180_in_rad = 180.0 * DEGREE.conversion;
        let rel = (deg_180_in_rad - std::f64::consts::PI).abs() / std::f64::consts::PI;
        assert!(
            rel < 8.0 * f64::EPSILON,
            "180 deg in rad: expected pi, got {}",
            deg_180_in_rad
        );
    }

    #[test]
    fn test_ref_kwh_to_joules() {
        // Tier 1: exact definition. 1 kWh = 3600000 J
        assert_eq!(KWH.conversion, 3_600_000.0);
    }

    #[test]
    fn test_ref_newton_dimensions() {
        // Tier 1: definition. N = kg*m/s^2 → [1,1,-2,0,0,0,0,0]
        assert_eq!(NEWTON.dimensions, [1, 1, -2, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_ref_parsed_kg_m_per_s2_equals_newton_dims() {
        // Tier 1: definition. Parsed "kg*m/s^2" should have same dims as N
        let parsed = parse_unit("kg*m/s^2").unwrap();
        assert_eq!(parsed.dimensions, NEWTON.dimensions);
    }
}
