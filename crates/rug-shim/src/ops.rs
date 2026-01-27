//! Arithmetic operations for Integer

use crate::Integer;
use num_bigint::BigInt;
use std::ops::{
    Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, RemAssign, Shl, ShlAssign, Shr,
    ShrAssign, Sub, SubAssign,
};

// Negation
impl Neg for Integer {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Integer(-self.0)
    }
}

// Addition
impl Add for Integer {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Integer(self.0 + rhs.0)
    }
}

impl Add<i32> for Integer {
    type Output = Self;
    fn add(self, rhs: i32) -> Self::Output {
        Integer(self.0 + rhs)
    }
}

impl Add<i64> for Integer {
    type Output = Self;
    fn add(self, rhs: i64) -> Self::Output {
        Integer(self.0 + rhs)
    }
}

impl AddAssign for Integer {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl AddAssign<i32> for Integer {
    fn add_assign(&mut self, rhs: i32) {
        self.0 += rhs;
    }
}

impl AddAssign<i64> for Integer {
    fn add_assign(&mut self, rhs: i64) {
        self.0 += rhs;
    }
}

impl AddAssign<u8> for Integer {
    fn add_assign(&mut self, rhs: u8) {
        self.0 += rhs;
    }
}

// Subtraction
impl Sub for Integer {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Integer(self.0 - rhs.0)
    }
}

impl Sub<i32> for Integer {
    type Output = Self;
    fn sub(self, rhs: i32) -> Self::Output {
        Integer(self.0 - rhs)
    }
}

impl SubAssign for Integer {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl SubAssign<i32> for Integer {
    fn sub_assign(&mut self, rhs: i32) {
        self.0 -= rhs;
    }
}

// Multiplication
impl Mul for Integer {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Integer(self.0 * rhs.0)
    }
}

impl Mul<i32> for Integer {
    type Output = Self;
    fn mul(self, rhs: i32) -> Self::Output {
        Integer(self.0 * rhs)
    }
}

impl Mul<Integer> for i32 {
    type Output = Integer;
    fn mul(self, rhs: Integer) -> Self::Output {
        Integer(BigInt::from(self) * rhs.0)
    }
}

impl MulAssign for Integer {
    fn mul_assign(&mut self, rhs: Self) {
        self.0 *= rhs.0;
    }
}

impl MulAssign<i32> for Integer {
    fn mul_assign(&mut self, rhs: i32) {
        self.0 *= rhs;
    }
}

// Division
impl Div for Integer {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        Integer(self.0 / rhs.0)
    }
}

impl Div<i32> for Integer {
    type Output = Self;
    fn div(self, rhs: i32) -> Self::Output {
        Integer(self.0 / rhs)
    }
}

impl DivAssign for Integer {
    fn div_assign(&mut self, rhs: Self) {
        self.0 /= rhs.0;
    }
}

// Remainder
impl Rem for Integer {
    type Output = Self;
    fn rem(self, rhs: Self) -> Self::Output {
        Integer(self.0 % rhs.0)
    }
}

impl Rem<i32> for Integer {
    type Output = Self;
    fn rem(self, rhs: i32) -> Self::Output {
        Integer(self.0 % rhs)
    }
}

impl RemAssign for Integer {
    fn rem_assign(&mut self, rhs: Self) {
        self.0 %= rhs.0;
    }
}

// Bit shifts
impl Shl<u32> for Integer {
    type Output = Self;
    fn shl(self, rhs: u32) -> Self::Output {
        Integer(self.0 << rhs)
    }
}

impl ShlAssign<u32> for Integer {
    fn shl_assign(&mut self, rhs: u32) {
        self.0 <<= rhs;
    }
}

impl Shl<usize> for Integer {
    type Output = Self;
    fn shl(self, rhs: usize) -> Self::Output {
        Integer(self.0 << rhs)
    }
}

impl ShlAssign<usize> for Integer {
    fn shl_assign(&mut self, rhs: usize) {
        self.0 <<= rhs;
    }
}

impl Shr<u32> for Integer {
    type Output = Self;
    fn shr(self, rhs: u32) -> Self::Output {
        Integer(self.0 >> rhs)
    }
}

impl ShrAssign<u32> for Integer {
    fn shr_assign(&mut self, rhs: u32) {
        self.0 >>= rhs;
    }
}

impl Shr<usize> for Integer {
    type Output = Self;
    fn shr(self, rhs: usize) -> Self::Output {
        Integer(self.0 >> rhs)
    }
}

impl ShrAssign<usize> for Integer {
    fn shr_assign(&mut self, rhs: usize) {
        self.0 >>= rhs;
    }
}

// Shifts with i32 (rug allows this)
impl Shl<i32> for Integer {
    type Output = Self;
    fn shl(self, rhs: i32) -> Self::Output {
        if rhs >= 0 {
            Integer(self.0 << (rhs as usize))
        } else {
            Integer(self.0 >> ((-rhs) as usize))
        }
    }
}

impl Shr<i32> for Integer {
    type Output = Self;
    fn shr(self, rhs: i32) -> Self::Output {
        if rhs >= 0 {
            Integer(self.0 >> (rhs as usize))
        } else {
            Integer(self.0 << ((-rhs) as usize))
        }
    }
}

impl ShlAssign<i32> for Integer {
    fn shl_assign(&mut self, rhs: i32) {
        if rhs >= 0 {
            self.0 <<= rhs as usize;
        } else {
            self.0 >>= (-rhs) as usize;
        }
    }
}

impl ShrAssign<i32> for Integer {
    fn shr_assign(&mut self, rhs: i32) {
        if rhs >= 0 {
            self.0 >>= rhs as usize;
        } else {
            self.0 <<= (-rhs) as usize;
        }
    }
}
