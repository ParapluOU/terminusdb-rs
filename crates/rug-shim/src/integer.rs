//! Integer type backed by num-bigint::BigInt

use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};
use std::fmt;
use std::str::FromStr;

use crate::parse::ParseIncomplete;
use crate::Order;

/// Arbitrary precision integer backed by num-bigint
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Integer(pub(crate) BigInt);

impl Integer {
    /// Creates a new Integer with value 0
    pub fn new() -> Self {
        Integer(BigInt::zero())
    }

    /// Returns the absolute value
    pub fn abs(self) -> Self {
        Integer(self.0.abs())
    }

    /// Returns a reference to the absolute value
    pub fn abs_ref(&self) -> AbsRef<'_> {
        AbsRef(&self.0)
    }

    /// Returns the number of significant bits
    pub fn significant_bits(&self) -> u32 {
        if self.0.is_zero() {
            0
        } else {
            self.0.bits() as u32
        }
    }

    /// Converts to u8, wrapping on overflow
    pub fn to_u8_wrapping(&self) -> u8 {
        let bytes = self.0.to_signed_bytes_le();
        if bytes.is_empty() {
            0
        } else {
            bytes[0]
        }
    }

    /// Converts to i64, returns None if out of range
    pub fn to_i64(&self) -> Option<i64> {
        self.0.to_i64()
    }

    /// Converts to u64, returns None if out of range
    pub fn to_u64(&self) -> Option<u64> {
        self.0.to_u64()
    }

    /// Converts to i32, returns None if out of range
    pub fn to_i32(&self) -> Option<i32> {
        self.0.to_i32()
    }

    /// Converts to u32, returns None if out of range
    pub fn to_u32(&self) -> Option<u32> {
        self.0.to_u32()
    }

    /// Parses a string in any radix
    pub fn parse<T: AsRef<[u8]>>(src: T) -> Result<ParseIncomplete, ()> {
        let s = std::str::from_utf8(src.as_ref()).map_err(|_| ())?;
        Ok(ParseIncomplete(s.to_string()))
    }

    /// Parses a string in the given radix
    pub fn parse_radix<T: AsRef<[u8]>>(src: T, radix: i32) -> Result<ParseIncomplete, ()> {
        let s = std::str::from_utf8(src.as_ref()).map_err(|_| ())?;
        // Store radix info in the parse result
        Ok(ParseIncomplete(format!("{}:{}", radix, s)))
    }

    /// Creates an Integer from digits in the given order
    pub fn from_digits<T: UnsignedPrimitive>(digits: &[T], order: Order) -> Self {
        let bytes: Vec<u8> = digits.iter().map(|d| d.to_u8()).collect();
        let bigint = match order {
            Order::Lsf | Order::LsfLe => BigInt::from_signed_bytes_le(&bytes),
            Order::Msf | Order::MsfBe => BigInt::from_signed_bytes_be(&bytes),
            Order::LsfBe => {
                let mut b = bytes;
                b.reverse();
                BigInt::from_signed_bytes_be(&b)
            }
            Order::MsfLe => {
                let mut b = bytes;
                b.reverse();
                BigInt::from_signed_bytes_le(&b)
            }
        };
        Integer(bigint)
    }
}

/// Reference to absolute value (for API compatibility)
pub struct AbsRef<'a>(&'a BigInt);

impl<'a> AbsRef<'a> {
    pub fn complete(self) -> Integer {
        Integer(self.0.abs())
    }
}

/// Trait for unsigned primitive types
pub trait UnsignedPrimitive: Copy {
    fn to_u8(self) -> u8;
}

impl UnsignedPrimitive for u8 {
    fn to_u8(self) -> u8 {
        self
    }
}

impl UnsignedPrimitive for u16 {
    fn to_u8(self) -> u8 {
        self as u8
    }
}

impl UnsignedPrimitive for u32 {
    fn to_u8(self) -> u8 {
        self as u8
    }
}

impl UnsignedPrimitive for u64 {
    fn to_u8(self) -> u8 {
        self as u8
    }
}

impl Default for Integer {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Integer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Integer {
    type Err = num_bigint::ParseBigIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Integer(BigInt::from_str(s)?))
    }
}

// From implementations
impl From<i8> for Integer {
    fn from(val: i8) -> Self {
        Integer(BigInt::from(val))
    }
}

impl From<i16> for Integer {
    fn from(val: i16) -> Self {
        Integer(BigInt::from(val))
    }
}

impl From<i32> for Integer {
    fn from(val: i32) -> Self {
        Integer(BigInt::from(val))
    }
}

impl From<i64> for Integer {
    fn from(val: i64) -> Self {
        Integer(BigInt::from(val))
    }
}

impl From<i128> for Integer {
    fn from(val: i128) -> Self {
        Integer(BigInt::from(val))
    }
}

impl From<u8> for Integer {
    fn from(val: u8) -> Self {
        Integer(BigInt::from(val))
    }
}

impl From<u16> for Integer {
    fn from(val: u16) -> Self {
        Integer(BigInt::from(val))
    }
}

impl From<u32> for Integer {
    fn from(val: u32) -> Self {
        Integer(BigInt::from(val))
    }
}

impl From<u64> for Integer {
    fn from(val: u64) -> Self {
        Integer(BigInt::from(val))
    }
}

impl From<u128> for Integer {
    fn from(val: u128) -> Self {
        Integer(BigInt::from(val))
    }
}

// Comparison with primitives
impl PartialEq<i32> for Integer {
    fn eq(&self, other: &i32) -> bool {
        self.0 == BigInt::from(*other)
    }
}

impl PartialEq<i64> for Integer {
    fn eq(&self, other: &i64) -> bool {
        self.0 == BigInt::from(*other)
    }
}

impl PartialOrd<i32> for Integer {
    fn partial_cmp(&self, other: &i32) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&BigInt::from(*other))
    }
}

impl PartialOrd<i64> for Integer {
    fn partial_cmp(&self, other: &i64) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&BigInt::from(*other))
    }
}

// TryInto implementations for converting Integer to primitives
use std::convert::TryFrom;

/// Error type for Integer conversion failures
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TryFromIntegerError(());

impl std::fmt::Display for TryFromIntegerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "integer value out of range for target type")
    }
}

impl std::error::Error for TryFromIntegerError {}

impl TryFrom<Integer> for usize {
    type Error = TryFromIntegerError;
    fn try_from(val: Integer) -> Result<Self, Self::Error> {
        val.0.to_biguint()
            .and_then(|u| usize::try_from(&u).ok())
            .ok_or(TryFromIntegerError(()))
    }
}

impl TryFrom<Integer> for u64 {
    type Error = TryFromIntegerError;
    fn try_from(val: Integer) -> Result<Self, Self::Error> {
        val.to_u64().ok_or(TryFromIntegerError(()))
    }
}

impl TryFrom<Integer> for i64 {
    type Error = TryFromIntegerError;
    fn try_from(val: Integer) -> Result<Self, Self::Error> {
        val.to_i64().ok_or(TryFromIntegerError(()))
    }
}

impl TryFrom<Integer> for u32 {
    type Error = TryFromIntegerError;
    fn try_from(val: Integer) -> Result<Self, Self::Error> {
        val.to_u32().ok_or(TryFromIntegerError(()))
    }
}

impl TryFrom<Integer> for i32 {
    type Error = TryFromIntegerError;
    fn try_from(val: Integer) -> Result<Self, Self::Error> {
        val.to_i32().ok_or(TryFromIntegerError(()))
    }
}

impl TryFrom<&Integer> for usize {
    type Error = TryFromIntegerError;
    fn try_from(val: &Integer) -> Result<Self, Self::Error> {
        val.0.to_biguint()
            .and_then(|u| usize::try_from(&u).ok())
            .ok_or(TryFromIntegerError(()))
    }
}

impl TryFrom<&Integer> for u64 {
    type Error = TryFromIntegerError;
    fn try_from(val: &Integer) -> Result<Self, Self::Error> {
        val.to_u64().ok_or(TryFromIntegerError(()))
    }
}

impl TryFrom<&Integer> for i64 {
    type Error = TryFromIntegerError;
    fn try_from(val: &Integer) -> Result<Self, Self::Error> {
        val.to_i64().ok_or(TryFromIntegerError(()))
    }
}
