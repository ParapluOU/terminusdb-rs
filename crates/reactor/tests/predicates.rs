#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]
//! Filter predicates, checked against the server that has to agree with them.
//!
//! A predicate evaluator that disagrees with TerminusDB about collation,
//! numeric coercion, or a missing field under `ne` produces **silent
//! staleness**: the cached entry is never invalidated by the change that
//! should have killed it, and no counter anywhere moves. That is the failure
//! class this file exists to prevent, so the first test here is not about
//! caching at all — it asks the *server* which documents a filter selects, and
//! asserts our evaluator classifies the same set, for every operator and every
//! declared range we claim to index.
//!
//! The consequence is a hard rule, enforced by
//! [`terminusdb_reactor::predicate::INDEXABLE_RANGES`] being the thing this
//! test iterates: **a filter containing anything we have not verified is
//! unindexable by construction** and keeps its coarse `Class(C)` dependency.
//! Adding a row to that table without the server agreeing turns this red.

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use anyhow::{Context, Result};
use common::models::{person, Person};
use common::Reactor;
use serde_json::{json, Value};
use terminusdb_client::{DocumentInsertArgs, TerminusDBHttpClient};
use terminusdb_reactor::analyze::{plan, AnalyzeConfig, Plan, ReqView};
use terminusdb_reactor::predicate::{Order, Predicate, Truth, INDEXABLE_RANGES};
use terminusdb_reactor::{SchemaIndex, Term};

// ---------------------------------------------------------------- the fixture

/// One declared range, the values we store for it, and the filter literals we
/// probe it with.
///
/// Values are per-range because the domains differ: `xsd:unsignedByte` cannot
/// hold `-5` and `xsd:byte` cannot hold `1000`, and a schema failure would look
/// like a disagreement.
struct RangeFixture {
    /// The declared range, which must be a row of `INDEXABLE_RANGES`.
    range: &'static str,
    /// The property name in the probe class.
    prop: &'static str,
    /// Values, as document JSON. One document per entry, plus one that omits
    /// the property entirely.
    values: Vec<Value>,
    /// GraphQL literals to compare every operator against, written exactly as a
    /// client would have to write them: an `Int` token for `IntFilter`'s
    /// ranges, a *quoted* decimal for `BigIntFilter`'s, a `Float` for
    /// `FloatFilter`, a bare `true`/`false` for booleans.
    probes: Vec<&'static str>,
}

fn fixtures() -> Vec<RangeFixture> {
    // `small` is the IntFilter form (a bare Int literal); `big` is the
    // BigIntFilter form (a quoted decimal). Which one a range takes is the
    // server's choice and getting it wrong is a request error, not a wrong
    // answer — but the test writes the right one so the comparison actually
    // happens.
    vec![
        RangeFixture {
            range: "xsd:string",
            prop: "p_string",
            // "" < "Apfel" < "Apple" < "B" < "apple" in code-point order.
            values: vec![json!(""), json!("Apfel"), json!("Apple"), json!("apple")],
            probes: vec!["\"\"", "\"Apple\"", "\"B\"", "\"apple\""],
        },
        RangeFixture {
            range: "xsd:boolean",
            prop: "p_boolean",
            values: vec![json!(true), json!(false)],
            probes: vec!["true", "false"],
        },
        RangeFixture {
            range: "xsd:double",
            prop: "p_double",
            values: vec![json!(-2.5), json!(0.0), json!(1.5), json!(1.0e300)],
            probes: vec!["0", "1.5", "-2.5", "1.0e299"],
        },
        RangeFixture {
            range: "xsd:int",
            prop: "p_int",
            values: vec![json!(-5), json!(0), json!(10), json!(100000)],
            probes: vec!["0", "10", "-5", "99999"],
        },
        RangeFixture {
            range: "xsd:integer",
            prop: "p_integer",
            // Including a value no `f64` can distinguish from its neighbour, so
            // a lazy numeric coercion on either side would show up.
            values: vec![json!(-5), json!(0), json!(10), json!(10000000000000001i64)],
            probes: vec![
                "\"0\"",
                "\"10\"",
                "\"-5\"",
                "\"9\"",
                "\"10000000000000000\"",
            ],
        },
    ]
}

/// Ranges the server *offers* a filter for and we deliberately do not index,
/// carried in the probe class so `unverified_operators_are_not_indexed` can
/// show the refusal is a measurement rather than an omission.
const REFUSED_RANGES: &[(&str, &str)] = &[
    // `eq` matches some stored values and not others on these — see
    // `INDEXABLE_RANGES` — while `ne` and the ordered operators are correct.
    // Not reproducible, so not approximated.
    ("p_byte", "xsd:byte"),
    ("p_short", "xsd:short"),
    ("p_ubyte", "xsd:unsignedByte"),
    ("p_nonneg", "xsd:nonNegativeInteger"),
    ("p_pos", "xsd:positiveInteger"),
    // The server panics (`not a numerical value`).
    ("p_long", "xsd:long"),
    // `eq` compares the lexical form while ordering is numeric.
    ("p_decimal", "xsd:decimal"),
    ("p_datetime", "xsd:dateTime"),
];

/// The probe class: one `Optional` property per indexable range, so a single
/// document can leave any of them absent — which is the case the measured rules
/// are most surprising about.
fn probe_schema(fixtures: &[RangeFixture]) -> Vec<Value> {
    let mut class = json!({
        "@type": "Class",
        "@id": "Probe",
        "@key": {"@type": "Lexical", "@fields": ["k"]},
        "k": "xsd:string",
    });
    for fixture in fixtures {
        class[fixture.prop] = json!({"@type": "Optional", "@class": fixture.range});
    }
    for (prop, range) in REFUSED_RANGES {
        class[*prop] = json!({"@type": "Optional", "@class": range});
    }
    vec![class]
}

/// One document per value index, plus a final document that omits every
/// optional property.
fn probe_documents(fixtures: &[RangeFixture]) -> Vec<Value> {
    let rows = fixtures
        .iter()
        .map(|f| f.values.len())
        .max()
        .unwrap_or_default();
    let mut docs = Vec::new();
    for row in 0..rows {
        let mut doc = json!({"@type": "Probe", "k": format!("d{row}")});
        for fixture in fixtures {
            if let Some(value) = fixture.values.get(row) {
                doc[fixture.prop] = value.clone();
            }
        }
        docs.push(doc);
    }
    // The empty one. Named last so failures read in document order.
    docs.push(json!({"@type": "Probe", "k": "gap"}));
    docs
}

// ------------------------------------------------------------------ the server

async fn graphql(client: &TerminusDBHttpClient, db: &str, query: &str) -> Result<Value> {
    let text = reqwest::Client::new()
        // `TerminusDBHttpClient::endpoint` already ends in `/api`.
        .post(format!("{}/graphql/admin/{db}", client.endpoint))
        .basic_auth("admin", Some("root"))
        .json(&json!({"query": query}))
        .send()
        .await?
        .text()
        .await?;
    serde_json::from_str(&text).with_context(|| format!("non-JSON GraphQL reply: {text}"))
}

/// Every document of a class, as the server renders it — which is exactly what
/// the writer evaluates predicates against.
///
/// A raw GET rather than a typed read: these documents have no Rust model, and
/// the point is to see the bytes the server actually produces (optional
/// properties absent rather than null, numbers at full precision).
async fn stored_documents(
    client: &TerminusDBHttpClient,
    db: &str,
    class: &str,
) -> Result<Vec<Value>> {
    let text = reqwest::Client::new()
        .get(format!(
            "{}/document/admin/{db}?type={class}&as_list=true",
            client.endpoint
        ))
        .basic_auth("admin", Some("root"))
        .send()
        .await?
        .text()
        .await?;
    serde_json::from_str(&text).with_context(|| format!("non-JSON document list: {text}"))
}

/// The set of `k`s the **server** says a filter selects, or an error string.
async fn server_selects(
    client: &TerminusDBHttpClient,
    db: &str,
    filter: &str,
) -> Result<std::result::Result<BTreeSet<String>, String>> {
    let query = format!("query {{ Probe(filter: {{{filter}}}) {{ k }} }}");
    let reply = graphql(client, db, &query).await?;
    if let Some(errors) = reply.get("errors") {
        return Ok(Err(errors.to_string()));
    }
    // An absent payload is NOT an empty selection. Reading it as one is the
    // failure that has bitten this project repeatedly (an empty
    // `/api/changes`, a mis-split `?ids=`): a comparison against a silently
    // empty answer passes for the wrong reason.
    let rows = reply
        .pointer("/data/Probe")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("no /data/Probe in the GraphQL reply: {reply}"))?;
    Ok(Ok(rows
        .iter()
        .filter_map(|row| row["k"].as_str().map(str::to_owned))
        .collect()))
}

// ------------------------------------------------------------------- our side

/// Run a filter through the real analyzer and return the predicate it built,
/// or `None` if it refused to index the shape.
fn our_predicate(schema: &SchemaIndex, filter: &str) -> Option<Arc<Predicate>> {
    let query = format!("query {{ Probe(filter: {{{filter}}}) {{ k }} }}");
    let body = bytes::Bytes::from(json!({"query": query}).to_string());
    let headers = http::HeaderMap::new();
    let method = http::Method::POST;
    let req = ReqView::new(&method, "/api/graphql/admin/probe", "", &headers, &body);
    let planned = plan(&req, schema, &AnalyzeConfig::default());
    let terms = match &planned {
        Plan::Read { terms, .. } => terms.clone(),
        _ => return None,
    };
    terms.iter().find_map(|term| match term {
        Term::Predicate(pred) => Some(Arc::clone(pred)),
        _ => None,
    })
}

/// The set of `k`s **our evaluator** says a predicate selects.
fn we_select(pred: &Predicate, docs: &[Value]) -> std::result::Result<BTreeSet<String>, String> {
    let mut out = BTreeSet::new();
    for doc in docs {
        let k = doc["k"].as_str().unwrap_or_default().to_owned();
        match pred.member(Some(doc)) {
            Truth::True => {
                out.insert(k);
            }
            Truth::False => {}
            // Never acceptable in this test: the whole point is that the
            // documents are of the ranges we claim to handle.
            Truth::Unknown => return Err(format!("undecidable for {k}")),
        }
    }
    Ok(out)
}

// ----------------------------------------------------------------- the gate

/// **Every operator, on every range we claim to index, against the server's own
/// answer.**
///
/// Four claims per case, and each one is a different way the feature could be
/// quietly wrong:
///
/// 1. the server can actually answer the filter (several ranges it advertises
///    make it panic — see `INDEXABLE_RANGES`);
/// 2. the analyzer builds a predicate for it, so the table and the analyzer
///    cannot drift apart;
/// 3. our evaluator decides every document (no `Unknown`);
/// 4. it selects exactly the set the server selected.
#[tokio::test]
async fn every_indexed_operator_agrees_with_the_server() -> Result<()> {
    let server = common::upstream().await?;
    let fixtures = fixtures();

    // The table is the gate, so start by checking the fixture covers it.
    for (range, _) in INDEXABLE_RANGES {
        assert!(
            fixtures.iter().any(|f| f.range == *range),
            "{range} is declared indexable but this test does not probe it — every row of \
             INDEXABLE_RANGES must be verified against the server"
        );
    }

    server
        .with_tmp_db("rx_pred_agree", |direct, spec| async move {
            let schema = probe_schema(&fixtures);
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_documents(schema.iter().collect(), args.clone().as_schema())
                .await
                .context("inserting the probe schema")?;
            let documents = probe_documents(&fixtures);
            direct
                .insert_documents(documents.iter().collect(), args.clone())
                .await
                .context("inserting the probe documents")?;

            // The schema as *we* read it, which is how the analyzer resolves a
            // property's range to a comparison discipline.
            let schema_docs: Vec<Value> = direct
                .get_schema_documents(&spec)
                .await
                .context("reading the schema back")?;
            let index = SchemaIndex::from_schema_documents(&schema_docs)?;

            // The documents as the server renders them, which is what the
            // writer evaluates against.
            let stored = stored_documents(&direct, &spec.db, "Probe")
                .await
                .context("reading the probe documents back")?;
            assert_eq!(
                stored.len(),
                documents.len(),
                "every probe document must have landed"
            );

            let ops = ["eq", "ne", "lt", "le", "gt", "ge"];
            let mut checked = 0usize;
            for fixture in &fixtures {
                let order = terminusdb_reactor::predicate::order_of_range(fixture.range)
                    .unwrap_or_else(|| panic!("{} is not in the table", fixture.range));
                for op in ops {
                    // `BooleanFilter` offers only equality, and we refuse to
                    // invent an order for it.
                    if order == Order::Bool && !matches!(op, "eq" | "ne") {
                        continue;
                    }
                    for probe in &fixture.probes {
                        let filter = format!("{}: {{{op}: {probe}}}", fixture.prop);
                        // A *bare* single `eq` is claimed by the older, narrower
                        // `Index{class, prop, value}` term, which predates this
                        // work and is tested elsewhere. Wrapping in `_and` is
                        // the same set — the server agrees, since the filter
                        // below is what it is asked — and routes the shape
                        // through the predicate path so this matrix covers
                        // `Eq`/`Ne` in the evaluator too.
                        let ours_filter = format!("_and: [{{{filter}}}]");
                        let theirs =
                            server_selects(&direct, &spec.db, &filter)
                                .await?
                                .map_err(|err| {
                                    anyhow::anyhow!(
                                        "the server cannot answer `{filter}` ({err}), so \
                                     {} must not be in INDEXABLE_RANGES",
                                        fixture.range
                                    )
                                })?;
                        let pred = our_predicate(&index, &ours_filter).ok_or_else(|| {
                            anyhow::anyhow!(
                                "the analyzer refused to index `{filter}`, but {} is in \
                                 INDEXABLE_RANGES — the table and the analyzer disagree",
                                fixture.range
                            )
                        })?;
                        let wrapped = server_selects(&direct, &spec.db, &ours_filter)
                            .await?
                            .map_err(|err| {
                                anyhow::anyhow!("the server refused `{ours_filter}`: {err}")
                            })?;
                        assert_eq!(
                            wrapped, theirs,
                            "`_and: [{{{filter}}}]` must select the same set as `{filter}`, \
                             or the matrix is checking a different question"
                        );
                        let ours = we_select(&pred, &stored)
                            .map_err(|err| anyhow::anyhow!("`{filter}`: {err}"))?;
                        assert_eq!(
                            ours, theirs,
                            "`{filter}` ({}): we select {ours:?}, the server selects {theirs:?}",
                            fixture.range
                        );
                        checked += 1;
                    }
                }
            }
            println!("FINDING {checked} operator/range/literal cases agree with the server");
            assert!(checked >= 100, "only {checked} cases: the matrix collapsed");
            Ok(())
        })
        .await
}

/// The cases the design notes expected to be surprising, pulled out of the
/// matrix above so a regression in any one of them names itself.
///
/// * **`ne` on a missing field is false.** A document with no value does not
///   match `ne`, which is not what "not equal" suggests.
/// * **`_not` *is* a complement**, so the same document does match
///   `_not: {p: {eq: v}}`. `ne` and `_not{eq}` are different predicates.
/// * **`_not` over a missing field under an ordered operator** likewise.
/// * **Numbers compare numerically**: `gt: "9"` selects `10`.
/// * **Negative numbers**, including across zero.
#[tokio::test]
async fn the_surprising_cases_agree_with_the_server() -> Result<()> {
    let server = common::upstream().await?;
    let fixtures = fixtures();
    server
        .with_tmp_db("rx_pred_edges", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_documents(
                    probe_schema(&fixtures).iter().collect(),
                    args.clone().as_schema(),
                )
                .await?;
            let documents = probe_documents(&fixtures);
            direct
                .insert_documents(documents.iter().collect(), args.clone())
                .await?;
            let schema_docs: Vec<Value> = direct.get_schema_documents(&spec).await?;
            let index = SchemaIndex::from_schema_documents(&schema_docs)?;
            let stored = stored_documents(&direct, &spec.db, "Probe").await?;

            let cases: Vec<(&str, &str)> = vec![
                ("ne on a present value", "p_string: {ne: \"Apple\"}"),
                ("ne on a missing value", "p_int: {ne: 5}"),
                ("_not over eq", "_not: {p_string: {eq: \"Apple\"}}"),
                ("_not over ne", "_not: {p_string: {ne: \"Apple\"}}"),
                ("_not over a range", "_not: {p_int: {ge: 1}}"),
                (
                    "double negation",
                    "_not: {_not: {p_string: {eq: \"Apple\"}}}",
                ),
                ("numeric, not lexical", "p_integer: {gt: \"9\"}"),
                ("beyond f64", "p_integer: {gt: \"10000000000000000\"}"),
                // `xsd:int` is an `IntFilter`, so its literal is a bare Int;
                // `xsd:integer` is a `BigIntFilter`, whose literal is a quoted
                // decimal. Sending the wrong form is a request error.
                ("negative", "p_int: {lt: 0}"),
                ("across zero", "p_int: {ge: -5}"),
                ("code-point collation", "p_string: {lt: \"B\"}"),
                ("the empty string", "p_string: {le: \"\"}"),
                (
                    "a conjunction over two properties",
                    "_and: [{p_int: {ge: 0}}, {p_string: {lt: \"B\"}}]",
                ),
                (
                    "implicit conjunction",
                    "p_int: {ge: 0}, p_string: {lt: \"B\"}",
                ),
                (
                    "a disjunction",
                    "_or: [{p_int: {lt: 0}}, {p_boolean: {eq: false}}]",
                ),
                (
                    "a negated conjunction over a missing field",
                    "_not: {_and: [{p_int: {ge: 1}}, {p_string: {eq: \"Apple\"}}]}",
                ),
            ];

            for (why, filter) in cases {
                let theirs = server_selects(&direct, &spec.db, filter)
                    .await?
                    .map_err(|err| {
                        anyhow::anyhow!("{why}: the server refused `{filter}`: {err}")
                    })?;
                let pred = our_predicate(&index, filter)
                    .ok_or_else(|| anyhow::anyhow!("{why}: `{filter}` must be indexable"))?;
                let ours =
                    we_select(&pred, &stored).map_err(|err| anyhow::anyhow!("{why}: {err}"))?;
                assert_eq!(
                    ours, theirs,
                    "{why} (`{filter}`): we select {ours:?}, the server selects {theirs:?}"
                );
                println!("FINDING {why}: `{filter}` -> {theirs:?} (agreed)");
            }

            // And the asymmetry the two `ne`/`_not` rows above exist to pin:
            // they must NOT select the same set, or the test would be passing
            // for the wrong reason.
            let ne = server_selects(&direct, &spec.db, "p_int: {ne: 5}")
                .await?
                .map_err(|e| anyhow::anyhow!(e))?;
            let not_eq = server_selects(&direct, &spec.db, "_not: {p_int: {eq: 5}}")
                .await?
                .map_err(|e| anyhow::anyhow!(e))?;
            assert!(
                not_eq.contains("gap") && !ne.contains("gap"),
                "the document with no value must match `_not{{eq}}` and not `ne`; got \
                 _not={not_eq:?} ne={ne:?}"
            );
            Ok(())
        })
        .await
}

/// The other half of the gate: operators and shapes we have **not** verified
/// must not be indexed, however tempting they look.
///
/// Asserted against the live server because two of these cases exist only
/// because of what the server does: `regex`/`startsWith` work fine and are
/// still left coarse (no measurement of the dialect), and two operators on one
/// field *is answered* — with one of them silently dropped.
#[tokio::test]
async fn unverified_operators_are_not_indexed() -> Result<()> {
    let server = common::upstream().await?;
    let fixtures = fixtures();
    server
        .with_tmp_db("rx_pred_unverified", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_documents(
                    probe_schema(&fixtures).iter().collect(),
                    args.clone().as_schema(),
                )
                .await?;
            direct
                .insert_documents(probe_documents(&fixtures).iter().collect(), args.clone())
                .await?;
            let schema_docs: Vec<Value> = direct.get_schema_documents(&spec).await?;
            let index = SchemaIndex::from_schema_documents(&schema_docs)?;

            for (why, filter) in [
                ("regex: dialect unmeasured", "p_string: {regex: \"A.*\"}"),
                ("xsd:byte: `eq` is not reproducible", "p_byte: {ge: 0}"),
                ("xsd:short: likewise", "p_short: {ge: 0}"),
                ("xsd:unsignedByte: likewise", "p_ubyte: {ge: 0}"),
                ("xsd:nonNegativeInteger: likewise", "p_nonneg: {ge: \"0\"}"),
                ("xsd:positiveInteger: likewise", "p_pos: {ge: \"1\"}"),
                ("xsd:long: the server panics", "p_long: {ge: 0}"),
                ("xsd:decimal: `eq` is lexical", "p_decimal: {ge: \"0\"}"),
                (
                    "xsd:dateTime: `eq` is lexical",
                    "p_datetime: {gt: \"2020-01-01T00:00:00Z\"}",
                ),
                ("startsWith: unmeasured", "p_string: {startsWith: \"A\"}"),
                (
                    "two operators on one field: the server drops one",
                    "p_int: {ge: 0, lt: 10}",
                ),
                ("membership by id", "_id: \"Probe/d0\""),
            ] {
                assert!(
                    our_predicate(&index, filter).is_none(),
                    "{why}: `{filter}` must not be indexed"
                );
            }

            // The evidence, recorded rather than paraphrased.
            //
            // `xsd:long` is a crash, which is stable enough to assert. The
            // integer families are a *disagreement*, which is the dangerous
            // kind because it looks like an answer: `eq` matches some stored
            // values and not others, while `ne` and the ordered operators are
            // right. The loop below prints what the server does for each, so a
            // future reader can re-check it in one run instead of rediscovering
            // it; it is not asserted value-by-value, because the rule is
            // precisely what we could not characterise.
            //
            // The population matters: with a *single* stored value `eq` is
            // correct, and it starts dropping values once the property holds
            // several. That data-dependence is the clearest statement of why
            // these ranges are refused — there is no rule here to implement.
            let population = [1, 7, 10];
            let narrow: Vec<Value> = population
                .iter()
                .map(|v| {
                    json!({
                        "@type": "Probe", "k": format!("narrow{v}"),
                        "p_byte": v, "p_nonneg": v, "p_pos": v
                    })
                })
                .collect();
            direct
                .insert_documents(narrow.iter().collect(), args.clone())
                .await?;
            // `xsd:long`'s failure is a *panic in the server*, and it needs a
            // stored value to reach. It is deliberately not provoked here —
            // panicking the node we share with every other test to re-prove
            // something `src/predicate.rs` records would be a poor trade — so
            // the refusal above stands on the measurement, not on this run.
            let mut disagreements = 0;
            for (prop, quoted) in [("p_byte", false), ("p_nonneg", true), ("p_pos", true)] {
                for value in population {
                    let literal = if quoted {
                        format!("\"{value}\"")
                    } else {
                        value.to_string()
                    };
                    let equal =
                        server_selects(&direct, &spec.db, &format!("{prop}: {{eq: {literal}}}"))
                            .await?;
                    let ordered =
                        server_selects(&direct, &spec.db, &format!("{prop}: {{ge: {literal}}}"))
                            .await?;
                    if let (Ok(equal), Ok(ordered)) = (&equal, &ordered) {
                        println!(
                            "FINDING {prop} eq {literal} -> {equal:?}, ge {literal} -> \
                             {ordered:?} (documents hold {population:?})"
                        );
                        // `eq v` must select exactly the document holding `v`.
                        let wanted = format!("narrow{value}");
                        if !(equal.len() == 1 && equal.contains(&wanted)) {
                            disagreements += 1;
                        }
                        assert!(
                            ordered.contains(&wanted),
                            "the ordered operators are the correct half of this: \
                             {prop} ge {literal} should contain {wanted}, got {ordered:?}"
                        );
                    }
                }
            }
            assert!(
                disagreements > 0,
                "none of the refused integer families disagreed with us this time. If `eq` \
                 on them has become reliable they can be verified and indexed — this \
                 assertion is how you find out."
            );

            // The finding that makes the two-operator refusal necessary: the
            // server answers it, and the answer is not the conjunction.
            let both = server_selects(&direct, &spec.db, "p_int: {ge: 0, lt: 10}").await?;
            let ge = server_selects(&direct, &spec.db, "p_int: {ge: 0}").await?;
            let lt = server_selects(&direct, &spec.db, "p_int: {lt: 10}").await?;
            if let (Ok(both), Ok(ge), Ok(lt)) = (both, ge, lt) {
                let conjunction: BTreeSet<String> = ge.intersection(&lt).cloned().collect();
                println!(
                    "FINDING two operators on one field: server={both:?}, ge={ge:?}, \
                     lt={lt:?}, conjunction would be {conjunction:?}"
                );
                assert_ne!(
                    both, conjunction,
                    "if the server ever starts ANDing several operators on one field, this \
                     refusal can be lifted — and this assertion is how you find out"
                );
            }
            Ok(())
        })
        .await
}

// ------------------------------------------------- the cache, end to end

/// The behaviour the whole feature is for: a **filtered count** survives a
/// field change that does not move any document across the filter's boundary,
/// and dies when one does.
///
/// A count is the purest form of the claim, because its value is a statement
/// about membership and nothing else. Before this change the read depended on
/// `Class(Person)` — fired by any field change to any instance — so a filtered
/// count was discarded on every write, which is worst exactly where it hurts
/// most, since paging metadata is re-read on every write.
#[tokio::test]
async fn a_filtered_count_survives_a_move_inside_the_range_and_dies_on_a_crossing() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_pred_range", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 40)?, args.clone())
                .await?;
            direct
                .insert_instance(&person("bob", "Bob", 20)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/graphql/admin/{}", spec.db);
            // `age` is `xsd:integer`, so its filter is a `BigIntFilter` and its
            // literals are quoted decimals. (A bare `30` is a request error on
            // 12.1.0-rc3, not a different query.)
            let query = json!({"query": "query { _count(Person: {age: {ge: \"30\"}}) }"});
            let count = |probe: &common::Probe| -> i64 {
                probe
                    .json()
                    .unwrap_or_default()
                    .pointer("/data/_count")
                    .and_then(Value::as_i64)
                    .unwrap_or_else(|| panic!("no count in {}", probe.text()))
            };

            let first = reactor.post(&path, query.clone()).await?;
            assert_eq!(first.status, 200, "body: {}", first.text());
            assert_eq!(count(&first), 1, "only Ann is over 30");
            assert!(
                reactor.post(&path, query.clone()).await?.is_hit(),
                "the count must be cached before we can test what breaks it"
            );

            if !up.supports_changes().await {
                println!(
                    "FINDING precision not demonstrable on {}: /api/changes is absent, so \
                     every commit fires Everything.",
                    up.version().await.unwrap_or_else(|_| "unknown".into())
                );
                return Ok(());
            }

            // A move *within* the set: 40 -> 50 is still >= 30.
            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Ann", 50)?, args.clone())
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let inside = reactor.post(&path, query.clone()).await?;
            assert!(
                inside.is_hit(),
                "a field change that does not cross the filter's boundary must not \
                 invalidate a filtered count — this is the entire feature. Got {} ({})",
                inside.cache_status(),
                inside.text()
            );

            // A change to a document that stays *outside* the set, likewise.
            let before = reactor.head();
            direct
                .insert_instance(&person("bob", "Bobby", 25)?, args.clone())
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            assert!(
                reactor.post(&path, query.clone()).await?.is_hit(),
                "nor must a change to a non-member that stays a non-member"
            );

            // A crossing out: 50 -> 10 leaves the set.
            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Ann", 10)?, args.clone())
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let out = reactor.post(&path, query.clone()).await?;
            assert_eq!(
                count(&out),
                0,
                "a crossing must be reflected: {} ({})",
                out.text(),
                out.cache_status()
            );

            // And a crossing in: a non-member entering.
            let before = reactor.head();
            direct
                .insert_instance(&person("bob", "Bobby", 90)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let back = reactor.post(&path, query).await?;
            assert_eq!(
                count(&back),
                1,
                "a document entering the set must be reflected: {} ({})",
                back.text(),
                back.cache_status()
            );
            Ok(())
        })
        .await
}

/// The same claim for a read that returns documents.
///
/// The difference is worth being explicit about, because it bounds what the
/// predicate term buys. A list that returns members' fields depends on three
/// things, not one: the predicate (membership), a `Doc(id)` per document
/// actually returned (the response augmentation), and a
/// `ClassField(C, p)` per *selected* property — because the response contains
/// that property for every member, and a non-member acquiring it is a
/// membership question the predicate answers but a member changing it is not.
///
/// So the change below moves a property the selection does **not** contain, on
/// a document outside the set. Before this change `Class(Person)` was in the
/// dependency set and any field edit to any `Person` discarded the list; now
/// only the three terms above can.
#[tokio::test]
async fn a_filtered_list_survives_a_change_to_a_document_outside_the_set() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_pred_list", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 40)?, args.clone())
                .await?;
            direct
                .insert_instance(&person("bob", "Bob", 20)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/graphql/admin/{}", spec.db);
            let query = json!({"query": "query { Person(filter: {age: {ge: \"30\"}}) { name } }"});

            let first = reactor.post(&path, query.clone()).await?;
            assert_eq!(first.status, 200, "body: {}", first.text());
            assert!(
                first.text().contains("Ann") && !first.text().contains("Bob"),
                "the filter must actually filter: {}",
                first.text()
            );
            assert!(reactor.post(&path, query.clone()).await?.is_hit(), "cached");

            if !up.supports_changes().await {
                println!("FINDING precision not demonstrable: /api/changes is absent");
                return Ok(());
            }

            // Bob is not in the set, his change does not put him in it, and it
            // moves a property the selection does not read.
            let before = reactor.head();
            direct
                .insert_instance(&person("bob", "Bob", 25)?, args.clone())
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let after = reactor.post(&path, query.clone()).await?;
            assert!(
                after.is_hit(),
                "a change to an instance the filter excludes must not invalidate a \
                 filtered list. Got {} ({})",
                after.cache_status(),
                after.text()
            );

            // But Bob entering the set must be.
            let before = reactor.head();
            direct
                .insert_instance(&person("bob", "Bob", 55)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let after = reactor.post(&path, query).await?;
            assert!(
                after.text().contains("Bob"),
                "a document entering the set must be reflected: {} ({})",
                after.text(),
                after.cache_status()
            );
            Ok(())
        })
        .await
}

/// A conjunction survives one conjunct moving while the conjunction's value
/// stays put, and dies when the conjunction flips.
///
/// This is the case that cannot be done by indexing a value: it needs the whole
/// document, because whether `age >= 30 AND name < "B"` holds after a change to
/// `age` depends on `name`, which did not change.
#[tokio::test]
async fn a_conjunction_survives_one_conjunct_moving_and_dies_when_it_flips() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_pred_and", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            // Zara is over 30 but her name is not < "B", so she is not a
            // member however her age moves.
            direct
                .insert_instance(&person("zara", "Zara", 40)?, args.clone())
                .await?;
            direct
                .insert_instance(&person("anna", "Anna", 40)?, args.clone())
                .await?;

            let reactor = Reactor::start(up, &spec.db).await?;
            let path = format!("/api/graphql/admin/{}", spec.db);
            let query = json!({"query": "query { Person(filter: {_and: [{age: {ge: \"30\"}}, \
                                         {name: {lt: \"B\"}}]}) { name } }"});

            let first = reactor.post(&path, query.clone()).await?;
            assert_eq!(first.status, 200, "body: {}", first.text());
            assert!(
                first.text().contains("Anna") && !first.text().contains("Zara"),
                "the conjunction must filter: {}",
                first.text()
            );
            assert!(reactor.post(&path, query.clone()).await?.is_hit(), "cached");

            if !up.supports_changes().await {
                println!("FINDING precision not demonstrable: /api/changes is absent");
                return Ok(());
            }

            // Zara's age moves across one conjunct's boundary — but she is
            // still not named < "B", so the conjunction is unchanged for her.
            let before = reactor.head();
            direct
                .insert_instance(&person("zara", "Zara", 20)?, args.clone())
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let after = reactor.post(&path, query.clone()).await?;
            assert!(
                after.is_hit(),
                "one conjunct flipping for a document the other conjunct excludes must not \
                 invalidate: that decision needs the whole document, which is exactly what \
                 the writer evaluates. Got {} ({})",
                after.cache_status(),
                after.text()
            );

            // Now flip the conjunction for a real member.
            let before = reactor.head();
            direct
                .insert_instance(&person("anna", "Anna", 20)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());
            let after = reactor.post(&path, query).await?;
            assert!(
                !after.text().contains("Anna"),
                "the conjunction flipped, so the read must be refreshed: {} ({})",
                after.text(),
                after.cache_status()
            );
            Ok(())
        })
        .await
}

/// Over the configured evaluation bound, invalidation goes coarse and says so.
///
/// Backpressure has to degrade *precision*, not latency: the commit path waits
/// for evaluation, so an unbounded one would stall the watermark, and a stalled
/// watermark stalls every read.
#[tokio::test]
async fn over_the_bound_invalidation_goes_coarse_and_the_counter_ticks() -> Result<()> {
    let server = common::upstream().await?;
    let up = &server;
    server
        .with_schema::<(Person,), _, _, _>("rx_pred_bound", |direct, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            direct
                .insert_instance(&person("alice", "Ann", 40)?, args.clone())
                .await?;

            // One `(change, predicate)` pair per commit is allowed; the test
            // registers two predicates, so any commit touching `age` is over.
            let reactor = Reactor::start_with(up, &spec.db, |config| {
                config.predicates.max_evaluations_per_commit = 1;
            })
            .await?;
            let path = format!("/api/graphql/admin/{}", spec.db);
            let over_30 =
                json!({"query": "query { Person(filter: {age: {ge: \"30\"}}) { name } }"});
            let over_99 =
                json!({"query": "query { Person(filter: {age: {ge: \"99\"}}) { name } }"});

            for query in [&over_30, &over_99] {
                reactor.post(&path, query.clone()).await?;
                assert!(reactor.post(&path, query.clone()).await?.is_hit(), "cached");
            }
            assert_eq!(
                reactor.engine.metrics().predicate_evaluations_skipped.get(),
                0,
                "nothing skipped yet"
            );

            if !up.supports_changes().await {
                println!("FINDING precision not demonstrable: /api/changes is absent");
                return Ok(());
            }

            // 40 -> 50 crosses neither boundary, so a precise run keeps both.
            let before = reactor.head();
            direct
                .insert_instance(&person("alice", "Ann", 50)?, args)
                .await?;
            assert!(reactor
                .await_watermark_change(before.as_ref())
                .await
                .is_some());

            let skipped = reactor.engine.metrics().predicate_evaluations_skipped.get();
            assert!(
                skipped >= 2,
                "over the bound, the pairs must be counted as skipped, got {skipped}"
            );
            assert!(
                reactor.engine.metrics().predicate_fires_coarse.get() >= 1,
                "and the commit must be reported as a coarse fire"
            );
            let after = reactor.post(&path, over_30).await?;
            assert!(
                after.is_miss(),
                "and the entry must be gone — coarse invalidation is still invalidation. \
                 Got {}",
                after.cache_status()
            );
            Ok(())
        })
        .await
}
