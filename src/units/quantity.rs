//! Quantity type and arithmetic operations.

use crate::units::error::{UnitError, UnitResult};
use crate::units::unit::{Unit, si_symbol};
use std::fmt;
use std::ops::{Div, Mul, Neg};
use std::str::FromStr;

/// A physical quantity with a numeric value and unit.
#[derive(Clone, Copy, Debug)]
pub struct Quantity {
    pub value: f64,
    pub unit: Unit,
}

impl Quantity {
    /// Creates a new quantity with the given value and unit.
    pub fn new(value: f64, unit: Unit) -> Self {
        Quantity { value, unit }
    }

    /// Returns the value converted to SI base units.
    pub fn si_value(&self) -> f64 {
        self.value * self.unit.conversion
    }

    /// Adds two quantities. Returns an error if dimensions don't match.
    pub fn add(&self, other: &Quantity) -> UnitResult<Quantity> {
        if !self.unit.same_si_dimensions(&other.unit) {
            return Err(UnitError::DimensionMismatch {
                expected: self.unit.dimensions,
                found: other.unit.dimensions,
            });
        }

        let si_sum = self.si_value() + other.si_value();

        let mut result_dims = self.unit.dimensions;
        result_dims[7] = self.unit.dimensions[7];

        Ok(Quantity {
            value: si_sum,
            unit: Unit::new(result_dims, 1.0, si_symbol(&result_dims), "derived"),
        })
    }

    /// Subtracts two quantities. Returns an error if dimensions don't match.
    pub fn sub(&self, other: &Quantity) -> UnitResult<Quantity> {
        if !self.unit.same_si_dimensions(&other.unit) {
            return Err(UnitError::DimensionMismatch {
                expected: self.unit.dimensions,
                found: other.unit.dimensions,
            });
        }

        let si_diff = self.si_value() - other.si_value();

        let mut result_dims = self.unit.dimensions;
        result_dims[7] = self.unit.dimensions[7];

        Ok(Quantity {
            value: si_diff,
            unit: Unit::new(result_dims, 1.0, si_symbol(&result_dims), "derived"),
        })
    }

    /// Converts the quantity to a different unit.
    pub fn convert_to(&self, target: Unit) -> UnitResult<Quantity> {
        if !self.unit.same_si_dimensions(&target) {
            return Err(UnitError::IncompatibleUnits {
                from: self.unit.dimensions,
                to: target.dimensions,
            });
        }

        let si_value = self.si_value();
        let target_value = si_value / target.conversion;

        Ok(Quantity {
            value: target_value,
            unit: target,
        })
    }

    /// Extracts the dimensionless value as an f64.
    pub fn to_f64(&self) -> UnitResult<f64> {
        if !self.unit.is_dimensionless() {
            return Err(UnitError::NotDimensionless {
                found: self.unit.dimensions,
            });
        }
        Ok(self.si_value())
    }
}

impl Mul for Quantity {
    type Output = Quantity;

    fn mul(self, rhs: Quantity) -> Quantity {
        let si_product = self.si_value() * rhs.si_value();
        let result_unit = Unit::multiply(&self.unit, &rhs.unit);

        Quantity {
            value: si_product,
            unit: Unit::new(
                result_unit.dimensions,
                1.0,
                si_symbol(&result_unit.dimensions),
                "derived",
            ),
        }
    }
}

impl Div for Quantity {
    type Output = Quantity;

    fn div(self, rhs: Quantity) -> Quantity {
        let si_quotient = self.si_value() / rhs.si_value();
        let result_unit = Unit::divide(&self.unit, &rhs.unit);

        Quantity {
            value: si_quotient,
            unit: Unit::new(
                result_unit.dimensions,
                1.0,
                si_symbol(&result_unit.dimensions),
                "derived",
            ),
        }
    }
}

impl Neg for Quantity {
    type Output = Quantity;

    fn neg(self) -> Quantity {
        Quantity {
            value: -self.value,
            unit: self.unit,
        }
    }
}

impl Mul<Quantity> for f64 {
    type Output = Quantity;

    fn mul(self, rhs: Quantity) -> Quantity {
        Quantity {
            value: self * rhs.value,
            unit: rhs.unit,
        }
    }
}

impl Mul<f64> for Quantity {
    type Output = Quantity;

    fn mul(self, rhs: f64) -> Quantity {
        Quantity {
            value: self.value * rhs,
            unit: self.unit,
        }
    }
}

impl Div<f64> for Quantity {
    type Output = Quantity;

    fn div(self, rhs: f64) -> Quantity {
        Quantity {
            value: self.value / rhs,
            unit: self.unit,
        }
    }
}

impl PartialEq for Quantity {
    fn eq(&self, other: &Self) -> bool {
        if !self.unit.same_si_dimensions(&other.unit) {
            return false;
        }
        let a = self.si_value();
        let b = other.si_value();
        // Tier 2: si_value() is a single multiply. Allow 8*epsilon relative
        // tolerance for the conversion multiply + comparison rounding.
        let max_abs = a.abs().max(b.abs());
        if max_abs == 0.0 {
            return true;
        }
        (a - b).abs() / max_abs < 8.0 * f64::EPSILON
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.value, self.unit)
    }
}

impl Mul<Unit> for f64 {
    type Output = Quantity;

    fn mul(self, rhs: Unit) -> Quantity {
        Quantity::new(self, rhs)
    }
}

impl FromStr for Quantity {
    type Err = UnitError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::units::parse::parse_quantity(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::definitions::*;
    use crate::units::macros::*;

    /// Helper: assert f64 values are equal within a relative tolerance.
    /// Tier 2: allows `tol` relative error from a single arithmetic chain.
    fn assert_rel(a: f64, b: f64, tol: f64, msg: &str) {
        let max_abs = a.abs().max(b.abs());
        if max_abs == 0.0 {
            assert!(
                a == 0.0 && b == 0.0,
                "{}: expected 0, got a={}, b={}",
                msg,
                a,
                b
            );
            return;
        }
        let rel = (a - b).abs() / max_abs;
        assert!(
            rel < tol,
            "{}: expected rel error < {}, got {} (a={}, b={})",
            msg,
            tol,
            rel,
            a,
            b
        );
    }

    // =====================================================================
    // Addition
    // =====================================================================

    #[test]
    fn test_add_same_unit() {
        // Tier 2: one add
        let a = Quantity::new(100.0, NEWTON);
        let b = Quantity::new(50.0, NEWTON);
        let c = a.add(&b).unwrap();
        assert_rel(c.si_value(), 150.0, 8.0 * f64::EPSILON, "100 N + 50 N");
    }

    #[test]
    fn test_add_different_units() {
        // Tier 2: 2 converts + add. 1 km + 500 m = 1500 m SI
        let a = Quantity::new(1.0, KILOMETER);
        let b = Quantity::new(500.0, METER);
        let c = a.add(&b).unwrap();
        assert_rel(c.si_value(), 1500.0, 8.0 * f64::EPSILON, "1 km + 500 m");
    }

    #[test]
    fn test_add_dimension_mismatch() {
        // Tier 1: exact error
        let a = Quantity::new(1.0, METER);
        let b = Quantity::new(1.0, SECOND);
        assert!(a.add(&b).is_err());
    }

    // =====================================================================
    // Subtraction
    // =====================================================================

    #[test]
    fn test_sub_basic() {
        // Tier 2: one sub
        let a = Quantity::new(200.0, NEWTON);
        let b = Quantity::new(50.0, NEWTON);
        let c = a.sub(&b).unwrap();
        assert_rel(c.si_value(), 150.0, 8.0 * f64::EPSILON, "200 N - 50 N");
    }

    #[test]
    fn test_sub_different_units() {
        // Tier 2: 2 converts + sub. 1 km - 500 m = 500 m SI
        let a = Quantity::new(1.0, KILOMETER);
        let b = Quantity::new(500.0, METER);
        let c = a.sub(&b).unwrap();
        assert_rel(c.si_value(), 500.0, 8.0 * f64::EPSILON, "1 km - 500 m");
    }

    // =====================================================================
    // Multiplication
    // =====================================================================

    #[test]
    fn test_mul_quantities() {
        // Tier 2: 10 N * 5 m = 50 N·m (= 50 J in SI)
        let a = Quantity::new(10.0, NEWTON);
        let b = Quantity::new(5.0, METER);
        let c = a * b;
        assert_rel(c.si_value(), 50.0, 8.0 * f64::EPSILON, "10 N * 5 m");
    }

    #[test]
    fn test_mul_dimensions_combine() {
        // Tier 1: exact integer dimensions. N * m → energy dimensions [2,1,-2,...]
        let a = Quantity::new(1.0, NEWTON);
        let b = Quantity::new(1.0, METER);
        let c = a * b;
        assert_eq!(
            c.unit.dimensions, ENERGY,
            "N * m should have energy dimensions"
        );
    }

    #[test]
    fn test_scalar_mul_f64_times_quantity() {
        // Tier 2: 2.0 * (100 N) = 200 N
        let q = Quantity::new(100.0, NEWTON);
        let r = 2.0 * q;
        assert_rel(r.si_value(), 200.0, 8.0 * f64::EPSILON, "2.0 * 100 N");
        assert_eq!(r.unit.dimensions, FORCE);
    }

    #[test]
    fn test_scalar_mul_quantity_times_f64() {
        // Tier 2: (100 N) * 2.0 = 200 N
        let q = Quantity::new(100.0, NEWTON);
        let r = q * 2.0;
        assert_rel(r.si_value(), 200.0, 8.0 * f64::EPSILON, "100 N * 2.0");
        assert_eq!(r.unit.dimensions, FORCE);
    }

    // =====================================================================
    // Division
    // =====================================================================

    #[test]
    fn test_div_quantities() {
        // Tier 2: 100 m / 10 s = 10 m/s
        let a = Quantity::new(100.0, METER);
        let b = Quantity::new(10.0, SECOND);
        let c = a / b;
        assert_rel(c.si_value(), 10.0, 8.0 * f64::EPSILON, "100 m / 10 s");
        assert_eq!(c.unit.dimensions, VELOCITY);
    }

    #[test]
    fn test_div_dimensionless() {
        // Tier 1 dims, Tier 2 value: 5 m / 5 m → dimensionless 1.0
        let a = Quantity::new(5.0, METER);
        let b = Quantity::new(5.0, METER);
        let c = a / b;
        assert_eq!(c.unit.dimensions, DIMENSIONLESS);
        assert_rel(c.si_value(), 1.0, 8.0 * f64::EPSILON, "5 m / 5 m");
    }

    #[test]
    fn test_scalar_div() {
        // Tier 2: 100 N / 4.0 = 25 N
        let q = Quantity::new(100.0, NEWTON);
        let r = q / 4.0;
        assert_rel(r.si_value(), 25.0, 8.0 * f64::EPSILON, "100 N / 4.0");
        assert_eq!(r.unit.dimensions, FORCE);
    }

    // =====================================================================
    // Negation
    // =====================================================================

    #[test]
    fn test_neg() {
        // Tier 1: bit flip, exact
        let q = Quantity::new(100.0, NEWTON);
        let r = -q;
        assert_eq!(r.value, -100.0);
        assert_eq!(r.unit, NEWTON);
    }

    // =====================================================================
    // Conversion
    // =====================================================================

    #[test]
    fn test_convert_to_compatible() {
        // Tier 2: 1 km → 1000 m
        let q = Quantity::new(1.0, KILOMETER);
        let r = q.convert_to(METER).unwrap();
        assert_rel(r.value, 1000.0, 8.0 * f64::EPSILON, "1 km to m");
    }

    #[test]
    fn test_convert_to_incompatible() {
        // Tier 1: exact error
        let q = Quantity::new(1.0, METER);
        assert!(q.convert_to(SECOND).is_err());
    }

    #[test]
    fn test_to_f64_dimensionless() {
        // Tier 1: extract f64 from percent
        let q = Quantity::new(50.0, PERCENT);
        let v = q.to_f64().unwrap();
        assert_rel(v, 0.5, 8.0 * f64::EPSILON, "50% as f64");
    }

    #[test]
    fn test_to_f64_not_dimensionless() {
        // Tier 1: exact error
        let q = Quantity::new(100.0, NEWTON);
        assert!(q.to_f64().is_err());
    }

    // =====================================================================
    // Equality (PartialEq)
    // =====================================================================

    #[test]
    fn test_equality_different_units() {
        // Tier 2 via PartialEq: 1 km == 1000 m
        let a = Quantity::new(1.0, KILOMETER);
        let b = Quantity::new(1000.0, METER);
        assert_eq!(a, b);
    }

    // =====================================================================
    // Display and FromStr
    // =====================================================================

    #[test]
    fn test_display_format() {
        // Tier 1: exact string
        let q = Quantity::new(100.0, NEWTON);
        assert_eq!(format!("{}", q), "100 N");
    }

    #[test]
    fn test_display_div_result() {
        // Quantity division should display SI symbol, not "derived"
        let a = Quantity::new(100.0, METER);
        let b = Quantity::new(10.0, SECOND);
        let c = a / b;
        let s = format!("{}", c);
        assert!(s.contains("m/s"), "Expected 'm/s' in display, got: {}", s);
        assert!(
            !s.contains("derived"),
            "Should not contain 'derived', got: {}",
            s
        );
    }

    #[test]
    fn test_display_mul_result() {
        // Quantity multiplication should display SI symbol
        let a = Quantity::new(10.0, NEWTON);
        let b = Quantity::new(5.0, METER);
        let c = a * b;
        let s = format!("{}", c);
        // N*m has energy dimensions → "J"
        assert!(s.contains("J"), "Expected 'J' in display, got: {}", s);
        assert!(
            !s.contains("derived"),
            "Should not contain 'derived', got: {}",
            s
        );
    }

    #[test]
    fn test_display_add_result() {
        let a = Quantity::new(100.0, NEWTON);
        let b = Quantity::new(50.0, NEWTON);
        let c = a.add(&b).unwrap();
        let s = format!("{}", c);
        assert!(s.contains("N"), "Expected 'N' in display, got: {}", s);
        assert!(
            !s.contains("derived"),
            "Should not contain 'derived', got: {}",
            s
        );
    }

    #[test]
    fn test_from_str() {
        // Tier 1 unit, Tier 2 value
        let q: Quantity = "100 N".parse().unwrap();
        assert_eq!(q.unit.dimensions, FORCE);
        assert_rel(q.value, 100.0, 8.0 * f64::EPSILON, "parse '100 N'");
    }

    // =====================================================================
    // Extreme values (numerical stability)
    // =====================================================================

    #[test]
    fn test_large_values() {
        // Tier 2: relative tolerance handles large magnitudes correctly
        let a = Quantity::new(1e15, METER);
        let b = Quantity::new(1e15, METER);
        let c = a.add(&b).unwrap();
        assert_rel(c.si_value(), 2e15, 8.0 * f64::EPSILON, "1e15 m + 1e15 m");
    }

    #[test]
    fn test_small_values() {
        // Tier 2: verify no underflow for very small quantities
        let a = Quantity::new(1e-18, KILOGRAM);
        let b = Quantity::new(1e-18, KILOGRAM);
        let c = a * b;
        assert_rel(
            c.si_value(),
            1e-36,
            8.0 * f64::EPSILON,
            "1e-18 kg * 1e-18 kg",
        );
    }

    #[test]
    fn test_mixed_magnitude_add() {
        // Tier 2: 1e12 m + 1 m. Precision loss is expected due to f64 mantissa
        // (~52 bits → ~15.9 decimal digits). 1e12 + 1 = 1000000000001.0 is
        // exactly representable, so this should be exact.
        let a = Quantity::new(1e12, METER);
        let b = Quantity::new(1.0, METER);
        let c = a.add(&b).unwrap();
        assert_eq!(c.si_value(), 1_000_000_000_001.0);
    }

    #[test]
    fn test_quantity_zero_handling() {
        // Tier 1/2: 0 N + 100 N = 100 N, 0 N * 100 N = 0
        let zero = Quantity::new(0.0, NEWTON);
        let q = Quantity::new(100.0, NEWTON);
        let sum = zero.add(&q).unwrap();
        assert_rel(sum.si_value(), 100.0, 8.0 * f64::EPSILON, "0 N + 100 N");
        let prod = zero * q;
        assert_eq!(prod.si_value(), 0.0);
    }
}
