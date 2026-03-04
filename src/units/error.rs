//! Error types for unit operations.

use std::fmt;

/// Dimension vector: 7 SI base dimensions + 1 pseudo-dimension for dimensionless categories.
pub type Dimensions = [i8; 8];

/// Errors that can occur during unit operations.
#[derive(Debug, Clone, PartialEq)]
pub enum UnitError {
    DimensionMismatch {
        expected: Dimensions,
        found: Dimensions,
    },
    IncompatibleUnits {
        from: Dimensions,
        to: Dimensions,
    },
    NotDimensionless {
        found: Dimensions,
    },
    ParseError {
        input: String,
        reason: String,
    },
}

impl fmt::Display for UnitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnitError::DimensionMismatch { expected, found } => {
                write!(
                    f,
                    "Dimension mismatch: expected {:?}, found {:?}",
                    expected, found
                )
            }
            UnitError::IncompatibleUnits { from, to } => {
                write!(
                    f,
                    "Incompatible units: cannot convert from {:?} to {:?}",
                    from, to
                )
            }
            UnitError::NotDimensionless { found } => {
                write!(
                    f,
                    "Expected dimensionless quantity, found dimensions {:?}",
                    found
                )
            }
            UnitError::ParseError { input, reason } => {
                write!(f, "Parse error for '{}': {}", input, reason)
            }
        }
    }
}

impl std::error::Error for UnitError {}

pub type UnitResult<T> = Result<T, UnitError>;
