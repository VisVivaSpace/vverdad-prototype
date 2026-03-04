//! Error types for time operations.

use std::fmt;

/// Errors that can occur during time parsing and conversion.
#[derive(Debug, Clone, PartialEq)]
pub enum TimeError {
    ParseError { input: String, reason: String },
    InvalidTimeSystem { system: String },
    LeapSecondTableError { reason: String },
}

impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeError::ParseError { input, reason } => {
                write!(f, "Time parse error for '{}': {}", input, reason)
            }
            TimeError::InvalidTimeSystem { system } => {
                write!(f, "Invalid time system: '{}'", system)
            }
            TimeError::LeapSecondTableError { reason } => {
                write!(f, "Leap second table error: {}", reason)
            }
        }
    }
}

impl std::error::Error for TimeError {}

pub type TimeResult<T> = Result<T, TimeError>;
