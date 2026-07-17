//! End-to-end **roundtrip** coverage for the rich TerminusDB modelling surface.
//!
//! Each test defines a model with the derive, inserts its schema + an instance
//! into a fresh live TerminusDB (via the embedded `TerminusDBServer`), reads it
//! back into the Rust type, and asserts the retrieved value equals the original.
//! This is the "does every modelling feature actually survive a real
//! server roundtrip" suite — distinct from the serialization-only unit tests.
//!
//! Features covered here (each a `#[tokio::test]`):
//! scalars (all xsd datatypes), optionality, chrono temporals, xsd:decimal
//! (BigDecimal), sys:JSON, collections + cardinality, links (Ref/TdbLazy),
//! subdocuments, simple enums, tagged unions, inheritance, key strategies.

#![recursion_limit = "512"]
#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::{DefaultTDBDeserializer, DocumentInsertArgs};
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

/// Insert `original` (a model with a **fixed** id), read it back by that id, and
/// assert whole-struct equality. This is the workhorse for every model whose id
/// is client-assigned (key = random with an explicit `EntityIDFor`).
async fn roundtrip<M>(prefix: &str, id: &str, original: M) -> Result<()>
where
    M: TerminusDBModel + PartialEq + Send + Sync + 'static,
    (M,): ToTDBSchemas,
{
    let server = TerminusDBServer::test_instance().await?;
    let id = id.to_string();
    server
        .with_db_schema::<(M,), _, _, _>(prefix, move |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            client.insert_instance(&original, args).await?;

            let mut de = DefaultTDBDeserializer;
            let retrieved: M = client.get_instance::<M>(&id, &spec, &mut de).await?;

            assert_eq!(
                retrieved,
                original,
                "roundtrip mismatch for {}",
                std::any::type_name::<M>()
            );
            Ok(())
        })
        .await
}

// ---------------------------------------------------------------------------
// Scalars: every xsd datatype that has a ToSchemaClass mapping.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct Scalars {
    id: EntityIDFor<Self>,
    s: String,
    i32v: i32,
    i64v: i64,
    isizev: isize,
    u32v: u32,
    u64v: u64,
    usizev: usize,
    i8v: i8,
    u8v: u8,
    f32v: f32,
    f64v: f64,
    b: bool,
}

#[tokio::test]
async fn roundtrip_scalars() -> Result<()> {
    roundtrip(
        "rt_scalars",
        "s1",
        Scalars {
            id: EntityIDFor::new("s1").unwrap(),
            s: "hello world".into(),
            i32v: -2_000_000_000,
            i64v: -9_000_000_000_000_000_000,
            isizev: -123456,
            u32v: 4_000_000_000,
            u64v: 18_000_000_000_000_000_000,
            usizev: 123456,
            i8v: -128,
            u8v: 255,
            f32v: 3.5,
            f64v: 2.718281828459045,
            b: true,
        },
    )
    .await
}

// ---------------------------------------------------------------------------
// Optionality: Option<T> in both Some and None.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct Optionals {
    id: EntityIDFor<Self>,
    maybe_str: Option<String>,
    maybe_int: Option<i64>,
}

#[tokio::test]
async fn roundtrip_option_some() -> Result<()> {
    roundtrip(
        "rt_opt_some",
        "o1",
        Optionals {
            id: EntityIDFor::new("o1").unwrap(),
            maybe_str: Some("present".into()),
            maybe_int: Some(7),
        },
    )
    .await
}

#[tokio::test]
async fn roundtrip_option_none() -> Result<()> {
    roundtrip(
        "rt_opt_none",
        "o2",
        Optionals {
            id: EntityIDFor::new("o2").unwrap(),
            maybe_str: None,
            maybe_int: None,
        },
    )
    .await
}

// ---------------------------------------------------------------------------
// chrono temporals.
// ---------------------------------------------------------------------------

// NOTE: chrono::NaiveDate (xsd:date) has NO ToSchemaClass/ToInstanceProperty impl
// in terminusdb-schema — only DateTime<Utc> (xsd:dateTime) and NaiveTime (xsd:time)
// are wired. So xsd:date is not currently representable as a model field; that gap
// is tracked separately, not exercised here.
#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct Temporals {
    id: EntityIDFor<Self>,
    dt: chrono::DateTime<chrono::Utc>,
    time: chrono::NaiveTime,
}

#[tokio::test]
async fn roundtrip_temporals() -> Result<()> {
    use chrono::{NaiveTime, TimeZone, Utc};
    roundtrip(
        "rt_temporals",
        "t1",
        Temporals {
            id: EntityIDFor::new("t1").unwrap(),
            dt: Utc.with_ymd_and_hms(2021, 3, 14, 15, 9, 26).unwrap(),
            time: NaiveTime::from_hms_opt(15, 9, 26).unwrap(),
        },
    )
    .await
}

// ---------------------------------------------------------------------------
// xsd:decimal via BigDecimal (typed read-back, not raw JSON).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct Decimalish {
    id: EntityIDFor<Self>,
    amount: terminusdb_schema::BigDecimal,
}

#[tokio::test]
async fn roundtrip_bigdecimal() -> Result<()> {
    use std::str::FromStr;
    roundtrip(
        "rt_bigdecimal",
        "d1",
        Decimalish {
            id: EntityIDFor::new("d1").unwrap(),
            amount: terminusdb_schema::BigDecimal::from_str("12345.678901234567890").unwrap(),
        },
    )
    .await
}

// ---------------------------------------------------------------------------
// sys:JSON via serde_json::Value.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct JsonField {
    id: EntityIDFor<Self>,
    payload: serde_json::Value,
}

#[tokio::test]
async fn roundtrip_sys_json() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(JsonField,), _, _, _>("rt_sys_json", |client, spec| async move {
            let original = JsonField {
                id: EntityIDFor::new("j1").unwrap(),
                payload: serde_json::json!({"a": 1, "b": ["x", "y"], "c": {"nested": true}}),
            };
            let args = DocumentInsertArgs::from(spec.clone());
            client.insert_instance(&original, args).await?;

            // sys:JSON is stored content-addressed; it must be unfolded to inline
            // the value rather than returning a blob IRI.
            let mut de = DefaultTDBDeserializer;
            let retrieved: JsonField =
                client.get_instance_unfolded::<JsonField>("j1", &spec, &mut de).await?;
            assert_eq!(retrieved, original);
            Ok(())
        })
        .await
}
