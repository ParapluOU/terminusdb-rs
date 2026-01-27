//! Pure Rust shim for rug crate
//!
//! This crate provides a drop-in replacement for `rug::Integer` using `num-bigint::BigInt`.
//! It enables cross-compilation to platforms where GMP is not available.
//!
//! Only the subset of rug's API actually used by tdb-succinct and terminus-store is implemented.

mod integer;
mod ops;
mod parse;

pub use integer::Integer;
pub use parse::ParseIncomplete;

// Re-export Order for API compatibility
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Lsf,
    LsfLe,
    LsfBe,
    Msf,
    MsfLe,
    MsfBe,
}
