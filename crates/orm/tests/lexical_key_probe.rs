#![recursion_limit = "512"]
//! Empirically probes how TerminusDB generates `lexical` document ids — over a
//! scalar field vs over a relation (link) field — and whether the resulting ids
//! can be fetched back by id. This pins down TDB's server-side algorithm so we
//! can decide what (if anything) terminusdb-rs must do to stay compliant.

use terminusdb_client::deserialize::DefaultTDBDeserializer;
use terminusdb_client::{DocumentInsertArgs, GetOpts};
use terminusdb_schema::{TdbLazy, ToTDBInstance};
use terminusdb_schema_derive::TerminusDBModel;
use terminusdb_test::test as db_test;

use terminusdb_schema; // required by the derive macro

/// Lexical key over a SCALAR field.
#[derive(Clone, Debug, Default, TerminusDBModel)]
#[tdb(key = "lexical", key_fields = "name")]
pub struct ScalarKeyed {
    pub name: String,
}

/// Lexical key over a RELATION (link) field.
#[derive(Clone, Debug, TerminusDBModel)]
#[tdb(key = "lexical", key_fields = "parent")]
pub struct LinkKeyed {
    pub parent: TdbLazy<ScalarKeyed>,
}

// Regression: document fetch must round-trip url-encoded lexical/hash key ids
// (via the POST/body form). Passes since the document-fetch fix.
#[db_test(db = "lexical_probe")]
async fn probe_lexical_key_generation(client: _, spec: _) -> anyhow::Result<()> {
    client
        .insert_entity_schema::<ScalarKeyed>(DocumentInsertArgs::from(spec.clone()))
        .await?;
    client
        .insert_entity_schema::<LinkKeyed>(DocumentInsertArgs::from(spec.clone()))
        .await?;

    // 1. Lexical over a SCALAR field (with a space, to see how values are encoded).
    let scalar = ScalarKeyed { name: "hello world".into() };
    let scalar_id = client
        .insert_instance(&scalar, DocumentInsertArgs::from(spec.clone()))
        .await?
        .root_id
        .clone();
    eprintln!("SCALAR_KEY_ID = {scalar_id}");

    // 2. Lexical over a LINK field (value is the parent's IRI).
    let link = LinkKeyed { parent: TdbLazy::new_id(&scalar_id)? };
    let link_id = client
        .insert_instance(&link, DocumentInsertArgs::from(spec.clone()))
        .await?
        .root_id
        .clone();
    eprintln!("LINK_KEY_ID = {link_id}");

    // 3. Round-trip: fetch each back by the id TDB just reported.
    //
    // FINDING: both FAIL. The lexical-key *generation* is fine (TDB url-encodes
    // field values: `hello world` -> `hello%20world`; a link's value is the
    // parent IRI, fully encoded). The DEFECT is the document FETCH: it can't
    // round-trip ids containing url-encoded characters. `url_builder::build`
    // splices the raw id into the URL without correct query-param encoding, so
    // TDB never matches the stored id. Affects ANY lexical/hash key whose field
    // values contain special chars; relations are the always-broken case.
    // Fetch via the plural path (get_instances -> get_documents), which the ORM
    // Phase-2 uses.
    let mut deser = DefaultTDBDeserializer;
    let scalar_back = client
        .get_instances::<ScalarKeyed>(vec![scalar_id.clone()], &spec, GetOpts::default(), &mut deser)
        .await;
    eprintln!("FETCH_SCALAR ok={} n={:?}", scalar_back.is_ok(), scalar_back.as_ref().map(|v| v.len()));

    let mut deser = DefaultTDBDeserializer;
    let link_back = client
        .get_instances::<LinkKeyed>(vec![link_id.clone()], &spec, GetOpts::default(), &mut deser)
        .await;
    eprintln!(
        "FETCH_LINK ok={} n={:?} err={:?}",
        link_back.is_ok(),
        link_back.as_ref().map(|v| v.len()),
        link_back.as_ref().err().map(|e| e.to_string())
    );

    assert_eq!(scalar_back?.len(), 1, "scalar-keyed doc must be fetchable by its id");
    assert_eq!(link_back?.len(), 1, "link-keyed doc must be fetchable by its id");

    // Also the SINGLE-doc path (get_instance -> get_document), used by db.get::<T>.
    let mut deser = DefaultTDBDeserializer;
    let scalar_single = client
        .get_instance::<ScalarKeyed>(&scalar_id, &spec, &mut deser)
        .await;
    eprintln!("FETCH_SCALAR_SINGLE ok={}", scalar_single.is_ok());
    let mut deser = DefaultTDBDeserializer;
    let link_single = client
        .get_instance::<LinkKeyed>(&link_id, &spec, &mut deser)
        .await;
    eprintln!("FETCH_LINK_SINGLE ok={}", link_single.is_ok());
    scalar_single?;
    link_single?;
    Ok(())
}
