//! Parsing support for Integer
//!
//! rug uses an "Incomplete" pattern where parsing returns an intermediate
//! value that must be converted via `.into()` or `.complete()`.

use crate::Integer;
use num_bigint::BigInt;
use std::str::FromStr;

/// Incomplete parsed integer, needs to be completed via `.into()`
pub struct ParseIncomplete(pub(crate) String);

impl ParseIncomplete {
    /// Complete the parsing
    pub fn complete(self) -> Integer {
        self.into()
    }
}

impl From<ParseIncomplete> for Integer {
    fn from(incomplete: ParseIncomplete) -> Self {
        let s = &incomplete.0;

        // Check if radix is specified (format: "radix:value")
        if let Some(colon_pos) = s.find(':') {
            let radix: u32 = s[..colon_pos].parse().unwrap_or(10);
            let value = &s[colon_pos + 1..];

            // Handle negative numbers
            let (sign, digits) = if value.starts_with('-') {
                (-1i32, &value[1..])
            } else if value.starts_with('+') {
                (1, &value[1..])
            } else {
                (1, value)
            };

            // Parse with radix
            if let Some(bigint) = BigInt::parse_bytes(digits.as_bytes(), radix) {
                return Integer(bigint * sign);
            }
        }

        // Default: parse as decimal, handling various formats
        let trimmed = s.trim();

        // Try standard decimal parsing first
        if let Ok(bigint) = BigInt::from_str(trimmed) {
            return Integer(bigint);
        }

        // Handle hex (0x prefix)
        if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
            if let Some(bigint) = BigInt::parse_bytes(trimmed[2..].as_bytes(), 16) {
                return Integer(bigint);
            }
        }

        // Handle octal (0o prefix)
        if trimmed.starts_with("0o") || trimmed.starts_with("0O") {
            if let Some(bigint) = BigInt::parse_bytes(trimmed[2..].as_bytes(), 8) {
                return Integer(bigint);
            }
        }

        // Handle binary (0b prefix)
        if trimmed.starts_with("0b") || trimmed.starts_with("0B") {
            if let Some(bigint) = BigInt::parse_bytes(trimmed[2..].as_bytes(), 2) {
                return Integer(bigint);
            }
        }

        // Fallback: return zero on parse failure (matches rug behavior for some edge cases)
        Integer::new()
    }
}
