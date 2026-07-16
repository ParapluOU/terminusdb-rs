//! End-to-end: seed a real embedded TerminusDB, then compile + run SQL through
//! the session and assert on decoded rows. Uses `TerminusDBServer` so it runs in
//! parallel with no `#[ignore]`.
//!
//! There is one live roundtrip assertion **per v1 language feature** — the SQL is
//! compiled to WOQL, executed against the server, decoded, and the exact rows are
//! checked. A differential test additionally cross-checks several statements
//! against a DataFusion in-memory oracle (the authority on what SQL means).
#![recursion_limit = "256"]

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use std::future::Future;

    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::*;
    use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
    use terminusdb_sql::{QueryResponse, Session, SqlValue};

    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    struct Company {
        id: EntityIDFor<Self>,
        name: String,
    }

    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    struct Person {
        id: EntityIDFor<Self>,
        name: String,
        age: i32,
        /// Optional → a nullable column; exercises SQL NULL vs datalog absence.
        nickname: Option<String>,
        /// A string reference to a Company id; exercises equijoins.
        employer: EntityIDFor<Company>,
    }

    fn company(id: &str, name: &str) -> anyhow::Result<Company> {
        Ok(Company {
            id: EntityIDFor::new(id)?,
            name: name.to_string(),
        })
    }
    fn person(
        id: &str,
        name: &str,
        age: i32,
        nickname: Option<&str>,
        employer: &str,
    ) -> anyhow::Result<Person> {
        Ok(Person {
            id: EntityIDFor::new(id)?,
            name: name.to_string(),
            age,
            nickname: nickname.map(str::to_string),
            employer: EntityIDFor::new(employer)?,
        })
    }

    /// Seed a fresh isolated db, then hand the test an opened `Session`.
    ///
    /// Data: two companies (Acme, Globex) and four people. `jane` has a nickname;
    /// the others do not (NULL). `zoe` references a company `ghost` that does not
    /// exist (so an inner join drops her, a left join keeps her with NULLs).
    async fn with_session<F, Fut>(f: F) -> anyhow::Result<()>
    where
        F: FnOnce(Session<TerminusDBHttpClient>, BranchSpec) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Person, Company), _, _, _>("sql_e2e", |client, spec| async move {
                let args = DocumentInsertArgs::from(spec.clone());
                client.insert_instance(&company("acme", "Acme")?, args.clone()).await?;
                client.insert_instance(&company("globex", "Globex")?, args.clone()).await?;
                client.insert_instance(&person("jane", "Jane", 30, Some("Janey"), "acme")?, args.clone()).await?;
                client.insert_instance(&person("john", "John", 25, None, "acme")?, args.clone()).await?;
                client.insert_instance(&person("mary", "Mary", 40, None, "globex")?, args.clone()).await?;
                client.insert_instance(&person("zoe", "Zoe", 35, None, "ghost")?, args.clone()).await?;

                let session = Session::open(client.clone(), &spec.db, spec.branch.as_deref()).await?;
                f(session, spec).await
            })
            .await
    }

    /// A column as plain strings (non-null cells only expected).
    fn col(resp: &QueryResponse, name: &str) -> Vec<String> {
        opt_col(resp, name)
            .into_iter()
            .map(|c| c.unwrap_or_else(|| "<NULL>".to_string()))
            .collect()
    }

    /// A column as `Option<String>` — `None` for a SQL NULL cell.
    fn opt_col(resp: &QueryResponse, name: &str) -> Vec<Option<String>> {
        let idx = resp.columns.iter().position(|c| c == name).unwrap();
        resp.rows
            .iter()
            .map(|r| match &r[idx] {
                SqlValue::Null => None,
                SqlValue::Str(s) | SqlValue::Node(s) | SqlValue::Decimal(s)
                | SqlValue::Temporal(s) => Some(s.clone()),
                SqlValue::Int(i) => Some(i.to_string()),
                SqlValue::Float(f) => Some(f.to_string()),
                SqlValue::Bool(b) => Some(b.to_string()),
                SqlValue::Json(j) => Some(j.to_string()),
            })
            .collect()
    }

    fn sorted<T: Ord>(mut v: Vec<T>) -> Vec<T> {
        v.sort();
        v
    }

    // === one live roundtrip assertion per v1 feature =========================

    #[tokio::test]
    async fn catalog_mirrors_live_schema() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            let cat = session.catalog();
            let mut tables: Vec<_> = cat.tables().map(|t| t.sql_name.clone()).collect();
            tables.sort();
            assert_eq!(tables, vec!["company", "person"]);

            let person = cat.table("person").unwrap();
            let mut cols: Vec<_> = person.columns.iter().map(|c| c.sql_name.clone()).collect();
            cols.sort();
            assert_eq!(cols, vec!["age", "employer", "id", "iri", "name", "nickname"]);
            assert!(!session.commit().is_empty());
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn projection_and_filter_gt() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            let r = session.run("SELECT name FROM person WHERE age > 26").await?;
            assert_eq!(sorted(col(&r, "name")), vec!["Jane", "Mary", "Zoe"]);
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn filter_equals_and_int_projection() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            let r = session.run("SELECT age FROM person WHERE name = 'Jane'").await?;
            assert_eq!(col(&r, "age"), vec!["30"]);
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn filter_not_equals() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            let r = session.run("SELECT name FROM person WHERE age <> 30").await?;
            assert_eq!(sorted(col(&r, "name")), vec!["John", "Mary", "Zoe"]);
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn filter_and() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            let r = session
                .run("SELECT name FROM person WHERE age > 26 AND age < 40")
                .await?;
            assert_eq!(sorted(col(&r, "name")), vec!["Jane", "Zoe"]);
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn filter_or() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            let r = session
                .run("SELECT name FROM person WHERE age = 25 OR age = 40")
                .await?;
            assert_eq!(sorted(col(&r, "name")), vec!["John", "Mary"]);
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn order_by_asc_and_desc_limit() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            let r = session.run("SELECT name FROM person ORDER BY age ASC").await?;
            assert_eq!(col(&r, "name"), vec!["John", "Jane", "Zoe", "Mary"]);
            let r = session
                .run("SELECT name FROM person ORDER BY age DESC LIMIT 2")
                .await?;
            assert_eq!(col(&r, "name"), vec!["Mary", "Zoe"]);
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn limit_and_offset() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            // age order: John(25), Jane(30), Zoe(35), Mary(40); skip 1, take 2.
            let r = session
                .run("SELECT name FROM person ORDER BY age ASC LIMIT 2 OFFSET 1")
                .await?;
            assert_eq!(col(&r, "name"), vec!["Jane", "Zoe"]);
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn distinct_vs_bag() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            // employer has a duplicate (jane & john both Acme). Bag keeps 4 rows.
            let bag = session.run("SELECT employer FROM person").await?;
            assert_eq!(bag.rows.len(), 4, "bag keeps duplicates");
            // DISTINCT collapses the two Acme rows: acme, globex, ghost → 3.
            let distinct = session.run("SELECT DISTINCT employer FROM person").await?;
            assert_eq!(distinct.rows.len(), 3, "DISTINCT dedupes employer");
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn nullable_column_absence_is_sql_null() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            // nickname is Optional: present for Jane, absent (NULL) for the rest,
            // but every person still appears (absence ≠ row-drop).
            let r = session.run("SELECT name, nickname FROM person").await?;
            assert_eq!(r.rows.len(), 4, "optional column does not drop rows");
            let pairs: Vec<(String, Option<String>)> = r
                .columns
                .iter()
                .position(|c| c == "name")
                .zip(r.columns.iter().position(|c| c == "nickname"))
                .map(|(ni, xi)| {
                    r.rows
                        .iter()
                        .map(|row| {
                            let name = match &row[ni] {
                                SqlValue::Str(s) | SqlValue::Node(s) => s.clone(),
                                v => format!("{v:?}"),
                            };
                            let nick = match &row[xi] {
                                SqlValue::Null => None,
                                SqlValue::Str(s) => Some(s.clone()),
                                v => Some(format!("{v:?}")),
                            };
                            (name, nick)
                        })
                        .collect()
                })
                .unwrap();
            let by_name: std::collections::HashMap<_, _> = pairs.into_iter().collect();
            assert_eq!(by_name["Jane"], Some("Janey".to_string()));
            assert_eq!(by_name["John"], None);
            assert_eq!(by_name["Mary"], None);
            assert_eq!(by_name["Zoe"], None);
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn inner_equijoin() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            let r = session
                .run(
                    "SELECT p.name AS pname, c.name AS cname \
                     FROM person p JOIN company c ON p.employer = c.id \
                     WHERE c.name = 'Acme'",
                )
                .await?;
            assert_eq!(sorted(col(&r, "pname")), vec!["Jane", "John"]);
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn left_join_yields_null_for_unmatched() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            // Zoe's employer 'ghost' matches no company → cname is NULL, but Zoe
            // is still present (left join keeps the left row).
            let r = session
                .run(
                    "SELECT p.name AS pname, c.name AS cname \
                     FROM person p LEFT JOIN company c ON p.employer = c.id",
                )
                .await?;
            assert_eq!(r.rows.len(), 4, "left join keeps all people");
            let pi = r.columns.iter().position(|c| c == "pname").unwrap();
            let ci = r.columns.iter().position(|c| c == "cname").unwrap();
            let mut pairs: Vec<(String, Option<String>)> = r
                .rows
                .iter()
                .map(|row| {
                    let p = match &row[pi] {
                        SqlValue::Str(s) | SqlValue::Node(s) => s.clone(),
                        v => format!("{v:?}"),
                    };
                    let c = match &row[ci] {
                        SqlValue::Null => None,
                        SqlValue::Str(s) | SqlValue::Node(s) => Some(s.clone()),
                        v => Some(format!("{v:?}")),
                    };
                    (p, c)
                })
                .collect();
            pairs.sort();
            assert_eq!(
                pairs,
                vec![
                    ("Jane".into(), Some("Acme".into())),
                    ("John".into(), Some("Acme".into())),
                    ("Mary".into(), Some("Globex".into())),
                    ("Zoe".into(), None),
                ]
            );
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn bag_semantics_no_dedupe() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            assert_eq!(session.run("SELECT name FROM person").await?.rows.len(), 4);
            Ok(())
        })
        .await
    }

    // === differential: DataFusion is the oracle for what SQL means ============

    use datafusion::arrow::array::{Int64Array, StringArray};
    use datafusion::arrow::datatypes::{DataType, Field, Schema};
    use datafusion::arrow::record_batch::RecordBatch;
    use datafusion::arrow::util::display::array_value_to_string;
    use datafusion::datasource::MemTable;
    use datafusion::prelude::SessionContext;
    use std::sync::Arc;

    /// An in-memory DataFusion instance holding the *same* rows we seed into
    /// TerminusDB — the oracle for what each SQL statement should return.
    fn oracle() -> SessionContext {
        let ctx = SessionContext::new();

        let pschema = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, false),
            Field::new("age", DataType::Int64, false),
            Field::new("employer", DataType::Utf8, false),
        ]));
        let people = RecordBatch::try_new(
            pschema.clone(),
            vec![
                Arc::new(StringArray::from(vec!["Jane", "John", "Mary", "Zoe"])),
                Arc::new(Int64Array::from(vec![30_i64, 25, 40, 35])),
                Arc::new(StringArray::from(vec!["acme", "acme", "globex", "ghost"])),
            ],
        )
        .unwrap();
        ctx.register_table(
            "person",
            Arc::new(MemTable::try_new(pschema, vec![vec![people]]).unwrap()),
        )
        .unwrap();

        let cschema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("name", DataType::Utf8, false),
        ]));
        let companies = RecordBatch::try_new(
            cschema.clone(),
            vec![
                Arc::new(StringArray::from(vec!["acme", "globex"])),
                Arc::new(StringArray::from(vec!["Acme", "Globex"])),
            ],
        )
        .unwrap();
        ctx.register_table(
            "company",
            Arc::new(MemTable::try_new(cschema, vec![vec![companies]]).unwrap()),
        )
        .unwrap();

        ctx
    }

    async fn oracle_rows(ctx: &SessionContext, sql: &str) -> Vec<Vec<String>> {
        let batches = ctx.sql(sql).await.unwrap().collect().await.unwrap();
        let mut rows = Vec::new();
        for b in &batches {
            for r in 0..b.num_rows() {
                let row = (0..b.num_columns())
                    .map(|c| array_value_to_string(b.column(c), r).unwrap())
                    .collect();
                rows.push(row);
            }
        }
        rows
    }

    fn tdb_rows(r: &QueryResponse) -> Vec<Vec<String>> {
        r.rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|v| match v {
                        SqlValue::Str(s) | SqlValue::Node(s) | SqlValue::Decimal(s)
                        | SqlValue::Temporal(s) => s.clone(),
                        SqlValue::Int(i) => i.to_string(),
                        SqlValue::Float(f) => f.to_string(),
                        SqlValue::Bool(b) => b.to_string(),
                        SqlValue::Null => "".to_string(),
                        SqlValue::Json(j) => j.to_string(),
                    })
                    .collect()
            })
            .collect()
    }

    /// Run the same SQL through TerminusDB and the oracle; compare (sorted unless
    /// the query has an ORDER BY). Uses only datatype columns whose values match
    /// between the two stores (not `id`/`iri`/`employer`, which TerminusDB stores
    /// as typed IRIs).
    #[tokio::test]
    async fn differential_against_datafusion() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            let ctx = oracle();
            let cases = [
                ("SELECT name, age FROM person WHERE age > 26", false),
                ("SELECT name FROM person WHERE age = 25", false),
                ("SELECT name FROM person WHERE age <> 30", false),
                ("SELECT name FROM person WHERE age > 26 AND age < 40", false),
                ("SELECT name FROM person WHERE age = 25 OR age = 40", false),
                ("SELECT name, age FROM person", false),
                ("SELECT name FROM person ORDER BY age", true),
                ("SELECT name FROM person ORDER BY age DESC LIMIT 2", true),
                ("SELECT name FROM person ORDER BY age ASC LIMIT 2 OFFSET 1", true),
                (
                    "SELECT p.name, c.name FROM person p JOIN company c ON p.employer = c.id",
                    false,
                ),
            ];
            for (sql, ordered) in cases {
                let mut ours = tdb_rows(&session.run(sql).await?);
                let mut theirs = oracle_rows(&ctx, sql).await;
                if !ordered {
                    ours.sort();
                    theirs.sort();
                }
                assert_eq!(ours, theirs, "divergence on `{sql}`");
            }
            Ok(())
        })
        .await
    }
}
