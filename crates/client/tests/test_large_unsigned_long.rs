#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! Regression cover for `u64` values above `i64::MAX`.
//!
//! TerminusDB 12.1.0-rc3 rejects a JSON integer larger than `i64::MAX` for any
//! integer-family field — including `xsd:unsignedLong`, whose own range check
//! (`is_unsigned_long/1` in `src/core/util/types.pl`) allows up to
//! `u64::MAX`. The boundary is exactly `i64::MAX`: `9223372036854775807` is
//! accepted and `9223372036854775808` comes back as a `could_not_interpret_as_type`
//! schema-check witness. rc3's new `cast_specificity/3` inference ladder is
//! what refuses it, so this did not fail on our previous pin.
//!
//! `PrimitiveValue::from(u64)` works around it by emitting a decimal string
//! past the cutoff, which the server's `string -> unsignedLong` cast handles.
//! These tests pin that the whole `u64` range survives a round trip; if the
//! server is fixed and the workaround is dropped, they still have to pass.

use anyhow::Result;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Counter {
    id: EntityIDFor<Self>,
    /// xsd:unsignedLong
    value: u64,
}

/// Every interesting magnitude in the `u64` range round-trips byte-exactly,
/// including the three values either side of the server's `i64::MAX` cutoff.
#[tokio::test]
async fn u64_round_trips_across_the_whole_range() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Counter,), _, _, _>("u64_range", |client, spec| async move {
            let cases: [(&str, u64); 6] = [
                ("zero", 0),
                ("small", 42),
                ("f64_exact_limit_plus_one", (1u64 << 53) + 1),
                ("i64_max", i64::MAX as u64),
                ("i64_max_plus_one", (i64::MAX as u64) + 1),
                ("u64_max", u64::MAX),
            ];

            for (label, value) in cases {
                let model = Counter {
                    id: EntityIDFor::new(label)?,
                    value,
                };
                let args = DocumentInsertArgs::from(spec.clone());
                client.insert_instance(&model, args).await?;

                let mut de = DefaultTDBDeserializer;
                let fetched: Counter = client.get_instance(label, &spec, &mut de).await?;

                assert_eq!(
                    fetched.value, value,
                    "u64 {} ({}) did not survive the round trip",
                    label, value
                );
            }

            Ok(())
        })
        .await
}

/// The serialization side of the workaround, without a server: at or below
/// `i64::MAX` we must still send a plain JSON number (so we do not change the
/// wire format for ordinary values), and only above it a decimal string.
#[test]
fn primitive_value_switches_representation_at_the_cutoff() {
    let below: PrimitiveValue = (i64::MAX as u64).into();
    assert!(
        matches!(below, PrimitiveValue::Number(_)),
        "i64::MAX must stay a JSON number, got {:?}",
        below
    );

    let above: PrimitiveValue = ((i64::MAX as u64) + 1).into();
    match above {
        PrimitiveValue::String(ref s) => assert_eq!(s, "9223372036854775808"),
        other => panic!("i64::MAX + 1 must be sent as a string, got {:?}", other),
    }

    let max: PrimitiveValue = u64::MAX.into();
    match max {
        PrimitiveValue::String(ref s) => assert_eq!(s, "18446744073709551615"),
        other => panic!("u64::MAX must be sent as a string, got {:?}", other),
    }

    // No precision is lost on the way out.
    let small: PrimitiveValue = 42u64.into();
    assert!(matches!(small, PrimitiveValue::Number(_)));
}
