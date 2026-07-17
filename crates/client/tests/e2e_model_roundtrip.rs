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

/// Same as [`roundtrip`] but retrieves **unfolded**, so nested subdocuments /
/// tagged-union variants / content-addressed values come back inline rather than
/// as references.
async fn roundtrip_unfolded<M>(prefix: &str, id: &str, original: M) -> Result<()>
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
            let retrieved: M = client.get_instance_unfolded::<M>(&id, &spec, &mut de).await?;

            assert_eq!(
                retrieved,
                original,
                "unfolded roundtrip mismatch for {}",
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

// ---------------------------------------------------------------------------
// Collections / type families: Vec (List), HashSet/BTreeSet (Set).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct Collections {
    id: EntityIDFor<Self>,
    list: Vec<String>,
    hset: std::collections::HashSet<i32>,
    bset: std::collections::BTreeSet<String>,
}

#[tokio::test]
async fn roundtrip_collections() -> Result<()> {
    use std::collections::{BTreeSet, HashSet};
    roundtrip(
        "rt_collections",
        "c1",
        Collections {
            id: EntityIDFor::new("c1").unwrap(),
            list: vec!["a".into(), "b".into(), "c".into()],
            hset: HashSet::from([1, 2, 3]),
            bset: BTreeSet::from(["x".to_string(), "y".to_string()]),
        },
    )
    .await
}

#[tokio::test]
async fn roundtrip_collections_empty() -> Result<()> {
    use std::collections::{BTreeSet, HashSet};
    roundtrip(
        "rt_collections_empty",
        "c2",
        Collections {
            id: EntityIDFor::new("c2").unwrap(),
            list: vec![],
            hset: HashSet::new(),
            bset: BTreeSet::new(),
        },
    )
    .await
}

#[tokio::test]
async fn roundtrip_collections_singleton() -> Result<()> {
    use std::collections::{BTreeSet, HashSet};
    roundtrip(
        "rt_collections_single",
        "c3",
        Collections {
            id: EntityIDFor::new("c3").unwrap(),
            list: vec!["only".into()],
            hset: HashSet::from([42]),
            bset: BTreeSet::from(["solo".to_string()]),
        },
    )
    .await
}

// ---------------------------------------------------------------------------
// Map fields.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct Maps {
    id: EntityIDFor<Self>,
    kv: std::collections::HashMap<String, String>,
}

// KNOWN GAP (documented, not yet fixed): HashMap<String, String> has an
// internally inconsistent representation — it *serializes* as a Set of
// `HashMapStringEntry` subdocuments (impl/map.rs), but its `FromInstanceProperty`
// *deserializes* expecting a `sys:JSON` object (impl/hashmap.rs:163). So it cannot
// roundtrip until those two sides are reconciled. The schema-tree fix in this
// change makes the *insert* half work (HashMapStringEntry is now registered);
// read-back still needs a matching deserializer. Ignored until then.
#[ignore = "HashMap<String,String> serialize/deserialize representations disagree; see note"]
#[tokio::test]
async fn roundtrip_hashmap() -> Result<()> {
    use std::collections::HashMap;
    roundtrip(
        "rt_hashmap",
        "m1",
        Maps {
            id: EntityIDFor::new("m1").unwrap(),
            kv: HashMap::from([
                ("one".to_string(), "1".to_string()),
                ("two".to_string(), "2".to_string()),
            ]),
        },
    )
    .await
}

// ---------------------------------------------------------------------------
// Set cardinality (v12 @cardinality / @min_cardinality / @max_cardinality).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct Cardinal {
    id: EntityIDFor<Self>,
    #[tdb(cardinality = 2)]
    exactly_two: std::collections::HashSet<String>,
    #[tdb(min_cardinality = 1)]
    at_least_one: std::collections::HashSet<String>,
}

#[tokio::test]
async fn roundtrip_cardinality() -> Result<()> {
    use std::collections::HashSet;
    roundtrip(
        "rt_cardinality",
        "cd1",
        Cardinal {
            id: EntityIDFor::new("cd1").unwrap(),
            exactly_two: HashSet::from(["a".to_string(), "b".to_string()]),
            at_least_one: HashSet::from(["x".to_string()]),
        },
    )
    .await
}

// ---------------------------------------------------------------------------
// Links: Ref<T> / TdbLazy<T> graph edges (object properties, not values).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct Author {
    id: EntityIDFor<Self>,
    name: String,
}

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct Book {
    id: EntityIDFor<Self>,
    title: String,
    author: Ref<Author>,
}

#[tokio::test]
async fn roundtrip_link() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Author, Book), _, _, _>("rt_link", |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            let author = Author {
                id: EntityIDFor::new("a1").unwrap(),
                name: "Ada".into(),
            };
            client.insert_instance(&author, args.clone()).await?;

            let book = Book {
                id: EntityIDFor::new("b1").unwrap(),
                title: "Notes".into(),
                author: Ref::from(EntityIDFor::<Author>::new("a1").unwrap()),
            };
            client.insert_instance(&book, args).await?;

            let mut de = DefaultTDBDeserializer;
            let retrieved: Book = client.get_instance::<Book>("b1", &spec, &mut de).await?;
            assert_eq!(retrieved.title, "Notes");
            // The link must survive as an edge to the author document.
            assert_eq!(retrieved.author.id().id(), "a1", "link id should roundtrip");
            Ok(())
        })
        .await
}

// ---------------------------------------------------------------------------
// Subdocuments.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(subdocument = true, key = "random")]
struct Address {
    street: String,
    city: String,
}

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct Person {
    id: EntityIDFor<Self>,
    name: String,
    address: Address,
}

#[tokio::test]
async fn roundtrip_subdocument() -> Result<()> {
    // Subdocuments come back as references unless retrieved unfolded.
    roundtrip_unfolded(
        "rt_subdocument",
        "p1",
        Person {
            id: EntityIDFor::new("p1").unwrap(),
            name: "Grace".into(),
            address: Address {
                street: "1 Compiler Way".into(),
                city: "Cambridge".into(),
            },
        },
    )
    .await
}

// ---------------------------------------------------------------------------
// Simple (unit-variant) enum.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
enum Color {
    Red,
    Green,
    Blue,
}

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct Painting {
    id: EntityIDFor<Self>,
    color: Color,
}

#[tokio::test]
async fn roundtrip_simple_enum() -> Result<()> {
    roundtrip(
        "rt_simple_enum",
        "pt1",
        Painting {
            id: EntityIDFor::new("pt1").unwrap(),
            color: Color::Green,
        },
    )
    .await
}

// ---------------------------------------------------------------------------
// Tagged union (unit + newtype + struct variants).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(subdocument = true)]
enum Shape {
    Nothing,
    Circle(f64),
    Rectangle { w: f64, h: f64 },
}

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct Drawing {
    id: EntityIDFor<Self>,
    shape: Shape,
}

async fn roundtrip_shape(prefix: &str, id: &str, shape: Shape) -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    let id = id.to_string();
    server
        .with_db_schema::<(Drawing,), _, _, _>(prefix, move |client, spec| async move {
            let original = Drawing {
                id: EntityIDFor::new(&id).unwrap(),
                shape,
            };
            let args = DocumentInsertArgs::from(spec.clone());
            client.insert_instance(&original, args).await?;
            let mut de = DefaultTDBDeserializer;
            let retrieved: Drawing =
                client.get_instance_unfolded::<Drawing>(&id, &spec, &mut de).await?;
            assert_eq!(retrieved, original);
            Ok(())
        })
        .await
}

#[tokio::test]
async fn roundtrip_tagged_union_newtype() -> Result<()> {
    roundtrip_shape("rt_tu_circle", "dr1", Shape::Circle(2.5)).await
}

#[tokio::test]
async fn roundtrip_tagged_union_struct() -> Result<()> {
    roundtrip_shape("rt_tu_rect", "dr2", Shape::Rectangle { w: 3.0, h: 4.0 }).await
}

#[tokio::test]
async fn roundtrip_tagged_union_unit() -> Result<()> {
    roundtrip_shape("rt_tu_nothing", "dr3", Shape::Nothing).await
}

// ---------------------------------------------------------------------------
// Key strategies with server-generated ids (Lexical / Hash / ValueHash).
// The id is derived server-side, so we use ServerIDFor + insert_and_retrieve.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(key = "lexical", key_fields = "code", id_field = "id")]
struct LexItem {
    id: ServerIDFor<Self>,
    code: String,
    label: String,
}

#[tokio::test]
async fn roundtrip_key_lexical() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(LexItem,), _, _, _>("rt_key_lexical", |client, spec| async move {
            let item = LexItem {
                id: ServerIDFor::new(),
                code: "abc-123".into(),
                label: "Widget".into(),
            };
            let args = DocumentInsertArgs::from(spec.clone());
            let (retrieved, _commit) = client.insert_instance_and_retrieve(&item, args).await?;
            assert!(retrieved.id.is_some(), "lexical key should be assigned");
            assert_eq!(retrieved.code, item.code);
            assert_eq!(retrieved.label, item.label);
            Ok(())
        })
        .await
}

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(key = "hash", key_fields = "first,second", id_field = "id")]
struct HashItem {
    id: ServerIDFor<Self>,
    first: String,
    second: String,
}

#[tokio::test]
async fn roundtrip_key_hash() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(HashItem,), _, _, _>("rt_key_hash", |client, spec| async move {
            let item = HashItem {
                id: ServerIDFor::new(),
                first: "alpha".into(),
                second: "beta".into(),
            };
            let args = DocumentInsertArgs::from(spec.clone());
            let (retrieved, _commit) = client.insert_instance_and_retrieve(&item, args).await?;
            assert!(retrieved.id.is_some(), "hash key should be assigned");
            assert_eq!(retrieved.first, item.first);
            assert_eq!(retrieved.second, item.second);
            Ok(())
        })
        .await
}

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(key = "value_hash", id_field = "id")]
struct VhItem {
    id: ServerIDFor<Self>,
    content: String,
}

#[tokio::test]
async fn roundtrip_key_value_hash() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(VhItem,), _, _, _>("rt_key_value_hash", |client, spec| async move {
            let item = VhItem {
                id: ServerIDFor::new(),
                content: "deterministic".into(),
            };
            let args = DocumentInsertArgs::from(spec.clone());
            let (retrieved, _commit) = client.insert_instance_and_retrieve(&item, args).await?;
            assert!(retrieved.id.is_some(), "value_hash key should be assigned");
            assert_eq!(retrieved.content, item.content);
            Ok(())
        })
        .await
}

// ---------------------------------------------------------------------------
// Inheritance (abstract base + concrete subclass).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(abstract_class = true, key = "random")]
struct NamedEntity {
    label: String,
}

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", inherits = "NamedEntity", key = "random")]
struct Gadget {
    id: EntityIDFor<Self>,
    label: String,
    voltage: i32,
}

#[tokio::test]
async fn roundtrip_inheritance() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(NamedEntity, Gadget), _, _, _>("rt_inheritance", |client, spec| async move {
            let gadget = Gadget {
                id: EntityIDFor::new("g1").unwrap(),
                label: "Toaster".into(),
                voltage: 230,
            };
            let args = DocumentInsertArgs::from(spec.clone());
            client.insert_instance(&gadget, args).await?;
            let mut de = DefaultTDBDeserializer;
            let retrieved: Gadget = client.get_instance::<Gadget>("g1", &spec, &mut de).await?;
            assert_eq!(retrieved, gadget);
            Ok(())
        })
        .await
}

// ---------------------------------------------------------------------------
// Composite: a model combining several features at once.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", key = "random")]
struct Composite {
    id: EntityIDFor<Self>,
    title: String,
    tags: Vec<String>,
    count: Option<i32>,
    address: Address,
    color: Color,
    author: Ref<Author>,
}

#[tokio::test]
async fn roundtrip_composite() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Author, Composite), _, _, _>("rt_composite", |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            let author = Author {
                id: EntityIDFor::new("ca1").unwrap(),
                name: "Edsger".into(),
            };
            client.insert_instance(&author, args.clone()).await?;

            let original = Composite {
                id: EntityIDFor::new("cx1").unwrap(),
                title: "Everything".into(),
                tags: vec!["a".into(), "b".into()],
                count: Some(3),
                address: Address {
                    street: "5 Dijkstra St".into(),
                    city: "Austin".into(),
                },
                color: Color::Blue,
                author: Ref::from(EntityIDFor::<Author>::new("ca1").unwrap()),
            };
            client.insert_instance(&original, args).await?;

            let mut de = DefaultTDBDeserializer;
            let retrieved: Composite =
                client.get_instance_unfolded::<Composite>("cx1", &spec, &mut de).await?;
            assert_eq!(retrieved.title, original.title);
            assert_eq!(retrieved.tags, original.tags);
            assert_eq!(retrieved.count, original.count);
            assert_eq!(retrieved.address, original.address);
            assert_eq!(retrieved.color, original.color);
            assert_eq!(retrieved.author.id().id(), "ca1");
            Ok(())
        })
        .await
}
