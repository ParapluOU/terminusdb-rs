#![recursion_limit = "512"]
//! Integration tests for ORM `order_by` against a real embedded TerminusDB.
//!
//! Ordering is only meaningful end-to-end if every link in the chain holds:
//! `orderBy` reaches the GraphQL (top-level via `discover_primary_ids`, and on
//! a reverse-relation field), the `ASC`/`DESC` enum renders *unquoted*, and the
//! Phase-2 batch fetch preserves the Phase-1 order instead of scrambling it
//! through a `HashSet`. We assert the actual returned order, both directions —
//! the type/class-name resolution in `get`/`get_ordered` (`to_schema().
//! class_name()`) is easy to get subtly wrong, so it's checked against data.

use serde::{Deserialize, Serialize};
use terminusdb_client::DocumentInsertArgs;
use terminusdb_orm::prelude::*;
use terminusdb_orm::RelationOpts;
use terminusdb_schema::{TdbLazy, TerminusOrdering, ToGql, ToTDBInstance};
use terminusdb_schema_derive::TerminusDBModel;
use terminusdb_test::test as db_test;

use terminusdb_schema; // required by the derive

#[derive(Debug, Clone, Default, TerminusDBModel)]
#[tdb(key = "Lexical")]
pub struct OrdBook {
    pub title: String,
}

/// `book: TdbLazy<OrdBook>` makes `OrdChapter` a reverse relation of `OrdBook`.
#[derive(Debug, Clone, TerminusDBModel)]
pub struct OrdChapter {
    pub heading: String,
    pub book: TdbLazy<OrdBook>,
    pub sort_key: f64,
}

// Hand-written filter/ordering types (the derive generates these only for crates
// that set TERMINUSDB_DERIVE_FILTERS=1; here we want to exercise the ORM
// mechanics directly, decoupled from that codegen).

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct OrdChapterOrdering {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_key: Option<TerminusOrdering>,
}
impl TdbGQLOrdering<OrdChapter> for OrdChapterOrdering {}
impl ToGql for OrdChapterOrdering {
    fn to_gql(&self) -> String {
        match &self.sort_key {
            Some(o) => format!("{{sort_key: {}}}", o.to_gql()),
            None => "{}".to_string(),
        }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct OrdChapterFilter {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}
impl TdbGQLFilter<OrdChapter> for OrdChapterFilter {}
impl ToGql for OrdChapterFilter {
    fn to_gql(&self) -> String {
        match &self.id {
            Some(id) => format!("{{_id: {}}}", id.to_gql()),
            None => "{}".to_string(),
        }
    }
}

/// Insert one book + three chapters whose `sort_key`s are inserted OUT of
/// order, so any preserved ordering must come from `orderBy`, not insert order.
async fn seed(
    client: &terminusdb_client::TerminusDBHttpClient,
    spec: &terminusdb_client::BranchSpec,
) -> anyhow::Result<String> {
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<OrdBook>(args.clone()).await?;
    client.insert_entity_schema::<OrdChapter>(args.clone()).await?;

    let book_id = client
        .insert_instance(&OrdBook { title: "Book".into() }, args.clone())
        .await?
        .root_id
        .clone();

    // Deliberately scrambled insertion order.
    for (heading, sort_key) in [("third", 3.0), ("first", 1.0), ("second", 2.0)] {
        let chapter = OrdChapter {
            heading: heading.into(),
            book: TdbLazy::new_id(&book_id)?,
            sort_key,
        };
        client.insert_instance(&chapter, args.clone()).await?;
    }
    Ok(book_id)
}

/// Reverse relation `Book -> its Chapters`, ordered by `sort_key`. This is the
/// exact shape `orm_children_ordered` (papiro `pages`/`region_samples`/
/// `page_layers`) relies on. Asserts both ASC and DESC.
#[db_test(db = "ordering_reverse_relation")]
async fn reverse_relation_orders_children_by_sort_key(client: _, spec: _) -> anyhow::Result<()> {
    let book_id = seed(&client, &spec).await?;

    let headings = |result: &OrmResult| -> anyhow::Result<Vec<String>> {
        Ok(result
            .get_ordered::<OrdChapter>()?
            .into_iter()
            .map(|(_, c)| c.heading)
            .collect())
    };

    let asc = OrdBook::find(EntityIDFor::<OrdBook>::new_untyped(&book_id)?)
        .with_opts::<OrdChapter, (), OrdChapterOrdering>(
            RelationOpts::new().order_by(OrdChapterOrdering {
                sort_key: Some(TerminusOrdering::Asc),
            }),
        )
        .with_client(&client)
        .execute(&spec)
        .await?;
    assert_eq!(
        headings(&asc)?,
        vec!["first", "second", "third"],
        "reverse relation should return chapters ASC by sort_key"
    );

    let desc = OrdBook::find(EntityIDFor::<OrdBook>::new_untyped(&book_id)?)
        .with_opts::<OrdChapter, (), OrdChapterOrdering>(
            RelationOpts::new().order_by(OrdChapterOrdering {
                sort_key: Some(TerminusOrdering::Desc),
            }),
        )
        .with_client(&client)
        .execute(&spec)
        .await?;
    assert_eq!(
        headings(&desc)?,
        vec!["third", "second", "first"],
        "reverse relation should return chapters DESC by sort_key"
    );

    Ok(())
}

/// Top-level `Model::query(..).order_by(..)` path, which routes through
/// `discover_primary_ids` (applies `orderBy`) then the order-preserving fetch.
#[db_test(db = "ordering_top_level_query")]
async fn top_level_query_orders_by_sort_key(client: _, spec: _) -> anyhow::Result<()> {
    seed(&client, &spec).await?;

    let desc = OrdChapter::query(OrdChapterFilter::default())
        .order_by(OrdChapterOrdering {
            sort_key: Some(TerminusOrdering::Desc),
        })
        .with_client(&client)
        .execute(&spec)
        .await?;

    let headings: Vec<String> = desc.get::<OrdChapter>()?.into_iter().map(|c| c.heading).collect();
    assert_eq!(
        headings,
        vec!["third", "second", "first"],
        "top-level query should return all chapters DESC by sort_key"
    );

    Ok(())
}
