//! String parsing for quantities and units.

use crate::units::definitions::lookup_unit;
use crate::units::error::{UnitError, UnitResult};
use crate::units::quantity::Quantity;
use crate::units::unit::Unit;

/// Parses a string into a Quantity.
pub fn parse_quantity(input: &str) -> UnitResult<Quantity> {
    let input = input.trim();

    if input.is_empty() {
        return Err(UnitError::ParseError {
            input: input.to_string(),
            reason: "empty input".to_string(),
        });
    }

    // Find where the number ends and the unit begins
    let (value_str, unit_str) = split_value_and_unit(input)?;

    // Parse the numeric value
    let value: f64 = value_str.parse().map_err(|_| UnitError::ParseError {
        input: input.to_string(),
        reason: format!("invalid number: '{}'", value_str),
    })?;

    // Parse the unit
    let unit = parse_unit(unit_str)?;

    Ok(Quantity::new(value, unit))
}

/// Parses a unit string into a Unit.
pub fn parse_unit(input: &str) -> UnitResult<Unit> {
    let input = input.trim();

    if input.is_empty() {
        return Err(UnitError::ParseError {
            input: input.to_string(),
            reason: "empty unit".to_string(),
        });
    }

    // First try direct lookup (handles pre-defined compound units like "mph", "km/h")
    if let Some(unit) = lookup_unit(input) {
        return Ok(unit);
    }

    // Parse compound unit
    parse_compound_unit(input)
}

/// Splits input into value string and unit string.
fn split_value_and_unit(input: &str) -> UnitResult<(&str, &str)> {
    let mut i = 0;
    let chars: Vec<char> = input.chars().collect();

    // Skip leading whitespace
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }

    // Handle optional leading sign
    if i < chars.len() && (chars[i] == '-' || chars[i] == '+') {
        i += 1;
    }

    // Parse integer part
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }

    // Parse decimal part
    if i < chars.len() && chars[i] == '.' {
        i += 1;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
    }

    // Parse exponent part (e.g., 1e10, 3.14E-5)
    if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
        i += 1;
        // Optional sign after exponent
        if i < chars.len() && (chars[i] == '-' || chars[i] == '+') {
            i += 1;
        }
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
    }

    if i == 0 {
        return Err(UnitError::ParseError {
            input: input.to_string(),
            reason: "no numeric value found".to_string(),
        });
    }

    let byte_pos = chars[..i].iter().collect::<String>().len();
    let value_str = &input[..byte_pos];
    let unit_str = input[byte_pos..].trim_start();

    if unit_str.is_empty() {
        return Err(UnitError::ParseError {
            input: input.to_string(),
            reason: "no unit found".to_string(),
        });
    }

    Ok((value_str, unit_str))
}

/// Parses a compound unit like "m/s", "kg*m/s^2", "m^2".
fn parse_compound_unit(input: &str) -> UnitResult<Unit> {
    let tokens = tokenize_unit(input)?;

    let mut result: Option<Unit> = None;
    let mut current_op: char = '*';
    let mut i = 0;

    while i < tokens.len() {
        let token = &tokens[i];

        match token.as_str() {
            "*" => {
                current_op = '*';
                i += 1;
            }
            "/" => {
                current_op = '/';
                i += 1;
            }
            _ => {
                let (unit, next_i) = parse_unit_term(&tokens, i, input)?;
                i = next_i;

                result = Some(match result {
                    None => unit,
                    Some(r) => {
                        if current_op == '*' {
                            Unit::multiply(&r, &unit)
                        } else {
                            Unit::divide(&r, &unit)
                        }
                    }
                });
                current_op = '*';
            }
        }
    }

    result.ok_or_else(|| UnitError::ParseError {
        input: input.to_string(),
        reason: "failed to parse compound unit".to_string(),
    })
}

/// Tokenizes a unit string into parts.
fn tokenize_unit(input: &str) -> UnitResult<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in input.chars() {
        match ch {
            '*' | '/' | '^' => {
                if !current.is_empty() {
                    tokens.push(current);
                    current = String::new();
                }
                tokens.push(ch.to_string());
            }
            ' ' => {
                if !current.is_empty() {
                    tokens.push(current);
                    current = String::new();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

/// Parses a unit term with optional exponent.
fn parse_unit_term(
    tokens: &[String],
    start: usize,
    original_input: &str,
) -> UnitResult<(Unit, usize)> {
    if start >= tokens.len() {
        return Err(UnitError::ParseError {
            input: original_input.to_string(),
            reason: "unexpected end of unit".to_string(),
        });
    }

    let base_str = &tokens[start];
    let base_unit = lookup_unit(base_str).ok_or_else(|| UnitError::ParseError {
        input: original_input.to_string(),
        reason: format!("unknown unit: '{}'", base_str),
    })?;

    // Check for exponent
    if start + 2 < tokens.len() && tokens[start + 1] == "^" {
        let exp_str = &tokens[start + 2];
        let exp: i8 = exp_str.parse().map_err(|_| UnitError::ParseError {
            input: original_input.to_string(),
            reason: format!("invalid exponent: '{}'", exp_str),
        })?;

        let raised = raise_unit(&base_unit, exp);
        Ok((raised, start + 3))
    } else {
        Ok((base_unit, start + 1))
    }
}

/// Raises a unit to a power.
fn raise_unit(unit: &Unit, power: i8) -> Unit {
    let mut dims = [0i8; 8];
    for (i, dim) in dims.iter_mut().enumerate() {
        *dim = unit.dimensions[i] * power;
    }

    let conversion = if power >= 0 {
        unit.conversion.powi(power as i32)
    } else {
        1.0 / unit.conversion.powi((-power) as i32)
    };

    Unit::new(
        dims,
        conversion,
        crate::units::unit::build_power_symbol(unit.symbol, power),
        "derived",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_quantity() {
        let q = parse_quantity("12.5 m").unwrap();
        assert!((q.value - 12.5).abs() < 1e-10);
        assert_eq!(q.unit.dimensions, [1, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_parse_negative_value() {
        let q = parse_quantity("-3.14 rad").unwrap();
        assert!((q.value - (-3.14)).abs() < 1e-10);
    }

    #[test]
    fn test_parse_scientific_notation() {
        let q = parse_quantity("1.5e10 m").unwrap();
        assert!((q.value - 1.5e10).abs() < 1e5);
    }

    #[test]
    fn test_parse_compound_unit() {
        let unit = parse_unit("m/s").unwrap();
        assert_eq!(unit.dimensions, [1, 0, -1, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_parse_compound_with_exponent() {
        let unit = parse_unit("m^2").unwrap();
        assert_eq!(unit.dimensions, [2, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_parse_complex_compound() {
        let unit = parse_unit("kg*m/s^2").unwrap();
        assert_eq!(unit.dimensions, [1, 1, -2, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_parse_predefined_compound() {
        let unit = parse_unit("mph").unwrap();
        assert_eq!(unit.dimensions, [1, 0, -1, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_parsed_compound_symbol_preserved() {
        let unit = parse_unit("m/s").unwrap();
        assert_eq!(unit.symbol, "m/s");
    }

    #[test]
    fn test_parsed_compound_symbol_with_prefix() {
        let unit = parse_unit("km/s").unwrap();
        assert_eq!(unit.symbol, "km/s");
    }

    #[test]
    fn test_parsed_compound_symbol_with_exponent() {
        // "m^2" is an alias for predefined SQUARE_METER, which uses Unicode superscript
        let unit = parse_unit("m^2").unwrap();
        assert_eq!(unit.symbol, "m\u{00B2}");
    }

    #[test]
    fn test_parsed_complex_compound_symbol() {
        let unit = parse_unit("kg*m/s^2").unwrap();
        assert_eq!(unit.symbol, "kg*m/s^2");
    }

    #[test]
    fn test_parsed_quantity_displays_compound() {
        let q = parse_quantity("2831 m/s").unwrap();
        assert_eq!(format!("{}", q), "2831 m/s");
    }

    #[test]
    fn test_parsed_quantity_displays_km_per_s() {
        let q = parse_quantity("5.5 km/s").unwrap();
        assert_eq!(format!("{}", q), "5.5 km/s");
    }
}
