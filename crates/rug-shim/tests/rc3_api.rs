//! Behavioural tests for the `rug` API surface that TerminusDB v12.1.0-rc3
//! requires: `tdb-succinct` v0.2.1 uses reference-operand arithmetic and
//! `to_i128`, and `terminusdb-store-prolog` uses `ops::Pow` and
//! `remove_factor` when it renders rationals as decimals.
//!
//! These assert values, not just that the code type-checks — a shim op that
//! compiles but computes the wrong thing would silently corrupt decimal
//! serialization.

use rug::{ops::Pow, Integer};

#[test]
fn reference_operand_arithmetic_matches_by_value() {
    let ten = Integer::from(10);
    let three = Integer::from(3);

    assert_eq!(&ten - three.clone(), Integer::from(7));
    assert_eq!(ten.clone() - &three, Integer::from(7));
    assert_eq!(&ten + three.clone(), Integer::from(13));
    assert_eq!(ten.clone() + &three, Integer::from(13));
    assert_eq!(ten.clone() % &three, Integer::from(1));
    assert_eq!(&ten % three.clone(), Integer::from(1));

    // Negative operands: num-bigint's Rem is truncating, like GMP's tdiv.
    assert_eq!(Integer::from(-10) % &three, Integer::from(-1));
}

#[test]
fn assign_ops_with_reference_rhs() {
    let ten = Integer::from(10);

    let mut v = Integer::from(1);
    v += &ten;
    assert_eq!(v, Integer::from(11));
    v -= &ten;
    assert_eq!(v, Integer::from(1));

    // The carry loops in tdb-succinct's lexical_interval drive these to and
    // across zero, so check the sign change.
    let mut w = Integer::from(5);
    w -= &ten;
    assert_eq!(w, Integer::from(-5));
    assert!(w < 0);
}

#[test]
fn to_i128_round_trips_and_saturates_to_none() {
    assert_eq!(Integer::from(0).to_i128(), Some(0));
    assert_eq!(Integer::from(-1).to_i128(), Some(-1));

    let max = Integer::from(i128::MAX.to_string().parse::<Integer>().unwrap());
    assert_eq!(max.to_i128(), Some(i128::MAX));

    let min: Integer = i128::MIN.to_string().parse().unwrap();
    assert_eq!(min.to_i128(), Some(i128::MIN));

    // One past the range must be None, not a wrapped value: the caller turns
    // None into a MalformedEntry error.
    let too_big: Integer = "170141183460469231731687303715884105728".parse().unwrap();
    assert_eq!(too_big.to_i128(), None);
    let too_small: Integer = "-170141183460469231731687303715884105729".parse().unwrap();
    assert_eq!(too_small.to_i128(), None);
}

#[test]
fn pow_matches_repeated_multiplication() {
    assert_eq!(Integer::from(5).pow(0u32), Integer::from(1));
    assert_eq!(Integer::from(5).pow(1u32), Integer::from(5));
    assert_eq!(Integer::from(2).pow(10u32), Integer::from(1024));
    assert_eq!((&Integer::from(10)).pow(3u32), Integer::from(1000));

    // Beyond u64, which is the reason these go through a bignum at all.
    let expected: Integer = "100000000000000000000000000".parse().unwrap();
    assert_eq!(Integer::from(10).pow(26u32), expected);
}

#[test]
fn remove_factor_counts_and_divides() {
    let two = Integer::from(2);
    let five = Integer::from(5);

    // 40 = 2^3 * 5
    let (rest, twos) = Integer::from(40).remove_factor(&two);
    assert_eq!(twos, 3);
    assert_eq!(rest, Integer::from(5));
    let (rest, fives) = rest.remove_factor(&five);
    assert_eq!(fives, 1);
    assert_eq!(rest, Integer::from(1));

    // No occurrence at all leaves the value untouched.
    let (rest, twos) = Integer::from(7).remove_factor(&two);
    assert_eq!((rest, twos), (Integer::from(7), 0));

    // Negative input keeps its sign; only the magnitude is reduced.
    let (rest, twos) = Integer::from(-8).remove_factor(&two);
    assert_eq!(twos, 3);
    assert_eq!(rest, Integer::from(-1));

    // Degenerate factors must terminate rather than loop forever.
    assert_eq!(
        Integer::from(6).remove_factor(&Integer::from(1)),
        (Integer::from(6), 0)
    );
    assert_eq!(
        Integer::from(6).remove_factor(&Integer::from(0)),
        (Integer::from(6), 0)
    );
    assert_eq!(
        Integer::from(0).remove_factor(&two),
        (Integer::from(0), 0)
    );
}

/// The reason `remove_factor` + `pow` exist in the shim at all: rendering a
/// rational n/d as an exact decimal when d is 2^a*5^b. Mirrors the algorithm in
/// terminusdb-store-prolog's `value.rs`.
#[test]
fn exact_decimal_scaling_reproduces_store_prolog_algorithm() {
    fn scale(n: i64, d: i64) -> Option<(Integer, usize)> {
        let d = Integer::from(d);
        let (rest, twos) = d.remove_factor(&Integer::from(2));
        let (rest, fives) = rest.remove_factor(&Integer::from(5));
        if rest != 1 {
            return None;
        }
        let scale = twos.max(fives) as usize;
        let scaled = if twos >= fives {
            Integer::from(n) * Integer::from(5).pow(twos - fives)
        } else {
            Integer::from(n) * Integer::from(2).pow(fives - twos)
        };
        Some((scaled, scale))
    }

    // 1/8 = 0.125 -> scaled 125 at scale 3
    assert_eq!(scale(1, 8), Some((Integer::from(125), 3)));
    // 1/5 = 0.2 -> scaled 2 at scale 1
    assert_eq!(scale(1, 5), Some((Integer::from(2), 1)));
    // 3/40 = 0.075 -> scaled 75 at scale 3
    assert_eq!(scale(3, 40), Some((Integer::from(75), 3)));
    // 1/3 has no exact decimal form.
    assert_eq!(scale(1, 3), None);
}
