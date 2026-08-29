    use super::*;

    /// Build an exact `BigRational` from a decimal numerator/denominator pair.
    fn rat(n: &str, d: &str) -> BigRational {
        BigRational::new(
            BigInt::from_str_radix(n, 10).unwrap(),
            BigInt::from_str_radix(d, 10).unwrap(),
        )
    }

    /// EVERY row is `(numerator, denominator, expected_lexical)` captured from a REAL SWI-Prolog run
    /// of `casting:rational_to_decimal_string(N rdiv D, S, 20)` (with `prefer_rationals`). Locks the
    /// renderer byte-exact against the reference across terminating, non-terminating (rounded to 20
    /// fractional digits, round-half-up, trailing zeros trimmed), whole, negative, and
    /// large-denominator cases — including the `den = 2·10^20` boundary that rounds up at digit 20.
    #[test]
    fn rational_to_decimal_string_matches_swipl_vectors() {
        let vectors: &[(&str, &str, &str)] = &[
            ("7", "2", "3.5"),
            ("6", "2", "3"),
            ("1", "4", "0.25"),
            ("1", "3", "0.33333333333333333333"),
            ("2", "3", "0.66666666666666666667"),
            ("10", "4", "2.5"),
            ("-7", "2", "-3.5"),
            ("-1", "3", "-0.33333333333333333333"),
            ("1", "8", "0.125"),
            ("22", "7", "3.14285714285714285714"),
            ("1", "40", "0.025"),
            ("0", "5", "0"),
            ("5", "1", "5"),
            ("-5", "1", "-5"),
            ("1", "7", "0.14285714285714285714"),
            ("1", "6", "0.16666666666666666667"),
            ("1", "9", "0.11111111111111111111"),
            ("1", "11", "0.09090909090909090909"),
            ("1", "13", "0.07692307692307692308"),
            ("100", "3", "33.33333333333333333333"),
            ("999", "1000", "0.999"),
            ("1", "2", "0.5"),
            ("3", "8", "0.375"),
            ("-22", "7", "-3.14285714285714285714"),
            ("123456", "1000", "123.456"),
            ("1", "50", "0.02"),
            ("7", "20", "0.35"),
            ("1", "16", "0.0625"),
            ("355", "113", "3.14159292035398230088"),
            ("2", "7", "0.28571428571428571429"),
            ("1", "17", "0.05882352941176470588"),
            ("10", "3", "3.33333333333333333333"),
            ("-10", "3", "-3.33333333333333333333"),
            ("1", "80", "0.0125"),
            ("17", "64", "0.265625"),
            ("1", "1024", "0.0009765625"),
            ("999999", "7", "142857"),
            ("-1", "1000000", "-0.000001"),
            ("12345678901234567890", "3", "4115226300411522630"),
            ("1", "99999999999", "0.00000000001"),
            ("50000000000000000000", "3", "16666666666666666666.66666666666666666667"),
            ("1", "200000000000000000000", "0.00000000000000000001"),
        ];
        for (n, d, expected) in vectors {
            let got = rational_to_decimal_string_20(&rat(n, d)).unwrap();
            assert_eq!(&got, expected, "{n}/{d} rendered wrong");
        }
    }

    /// The decimal-string parser is the exact inverse used for stored decimal operands and decimal
    /// literals: parsing a canonical lexical then re-rendering must round-trip when it fits 20
    /// fractional digits.
    #[test]
    fn parse_decimal_round_trips_canonical_lexicals() {
        for s in ["3.5", "-3.5", "0.25", "42", "-42", "0", "0.025", "123.456", "-0.000001"] {
            let r = parse_decimal_to_rational(s).unwrap();
            assert_eq!(rational_to_decimal_string_20(&r).unwrap(), s, "round-trip {s}");
        }
        // Leading-dot and trailing-dot canonicalization.
        assert_eq!(parse_decimal_to_rational(".5").unwrap(), rat("1", "2"));
        // Non-canonical / scientific / garbage forms reject closed.
        for bad in ["1e5", "1.2.3", "abc", "", "-", "1,5", " 1"] {
            assert!(parse_decimal_to_rational(bad).is_err(), "should reject {bad:?}");
        }
    }

    /// The carry-overflow guard: a fraction that rounds up to a whole unit at 20 digits (requires a
    /// reduced denominator >= 2·10^20) FAILS CLOSED rather than reproduce the reference carry bug.
    #[test]
    fn rational_render_carry_overflow_fails_closed() {
        // (3·10^20 - 1)/(3·10^20): true value 0.9999...9(6r6...), rounds up to 1.0 at 20 digits.
        let den = "300000000000000000000";
        let num = "299999999999999999999";
        assert!(rational_to_decimal_string_20(&rat(num, den)).is_err());
    }
