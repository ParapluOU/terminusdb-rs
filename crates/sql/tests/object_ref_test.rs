//! The genuine object-property (graph link) path, end to end.
//!
//! Unlike `e2e_test.rs` (whose `employer: EntityIDFor<Company>` is a *string*
//! value), here `Book::author: TdbLazy<Author>` is a real graph edge: its schema
//! range is `Author` and its stored value is the linked author's subject IRI. This
//! exercises `ColumnKind::ObjectRef` and a real IRI equijoin `book.author =
//! author.iri`.
#![recursion_limit = "256"]

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use std::future::Future;

    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::*;
    use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
    use terminusdb_sql::{ColumnKind, QueryResponse, Session, SqlValue};

    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    struct Author {
        id: EntityIDFor<Self>,
        name: String,
    }

    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    struct Book {
        id: EntityIDFor<Self>,
        title: String,
        /// A real graph edge to an Author (not a string id). `Ref<T>` is the
        /// purpose-named alias of `TdbLazy<T>` — this test also proves the alias
        /// works as a model field end to end.
        author: Ref<Author>,
    }

    fn author(id: &str, name: &str) -> anyhow::Result<Author> {
        Ok(Author {
            id: EntityIDFor::new(id)?,
            name: name.to_string(),
        })
    }
    fn book(id: &str, title: &str, author_id: &str) -> anyhow::Result<Book> {
        Ok(Book {
            id: EntityIDFor::new(id)?,
            title: title.to_string(),
            author: Ref::from(EntityIDFor::<Author>::new(author_id)?),
        })
    }

    async fn with_session<F, Fut>(f: F) -> anyhow::Result<()>
    where
        F: FnOnce(Session<TerminusDBHttpClient>, BranchSpec) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Book, Author), _, _, _>("sql_objref", |client, spec| async move {
                let args = DocumentInsertArgs::from(spec.clone());
                // Authors first (the links point at them).
                client.insert_instance(&author("rowling", "Rowling")?, args.clone()).await?;
                client.insert_instance(&author("tolkien", "Tolkien")?, args.clone()).await?;
                client.insert_instance(&book("hp1", "Philosopher's Stone", "rowling")?, args.clone()).await?;
                client.insert_instance(&book("hp2", "Chamber of Secrets", "rowling")?, args.clone()).await?;
                client.insert_instance(&book("lotr", "Fellowship", "tolkien")?, args.clone()).await?;

                let session = Session::open(client.clone(), &spec.db, spec.branch.as_deref()).await?;
                f(session, spec).await
            })
            .await
    }

    fn col(resp: &QueryResponse, name: &str) -> Vec<String> {
        let idx = resp.columns.iter().position(|c| c == name).unwrap();
        resp.rows
            .iter()
            .map(|r| match &r[idx] {
                SqlValue::Str(s) | SqlValue::Node(s) => s.clone(),
                other => format!("{other:?}"),
            })
            .collect()
    }
    fn sorted(mut v: Vec<String>) -> Vec<String> {
        v.sort();
        v
    }

    /// The catalog classifies a `TdbLazy` field as an object reference.
    #[tokio::test]
    async fn author_is_an_object_reference_column() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            let book = session.catalog().table("book").unwrap();
            match &book.column("author").unwrap().kind {
                ColumnKind::ObjectRef { target_class_iri } => assert_eq!(target_class_iri, "Author"),
                other => panic!("expected ObjectRef, got {other:?}"),
            }
            Ok(())
        })
        .await
    }

    /// A real IRI equijoin: `book.author` (a link value) unifies with the
    /// author's synthetic `iri` (subject) column.
    #[tokio::test]
    async fn real_object_property_join() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            let r = session
                .run(
                    "SELECT b.title AS title, a.name AS author \
                     FROM book b JOIN author a ON b.author = a.iri \
                     WHERE a.name = 'Rowling'",
                )
                .await?;
            assert_eq!(
                sorted(col(&r, "title")),
                vec!["Chamber of Secrets", "Philosopher's Stone"]
            );
            // Every returned author must be Rowling.
            assert!(col(&r, "author").iter().all(|a| a == "Rowling"));
            Ok(())
        })
        .await
    }

    /// Prove the join key is the subject IRI: select both `b.author` (the stored
    /// link) and `a.iri` (the author's subject) and confirm they are the *same*
    /// IRI-shaped value per row — i.e. the match is IRI-on-IRI, not string=string.
    #[tokio::test]
    async fn join_key_is_the_subject_iri() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            let r = session
                .run(
                    "SELECT b.author AS link, a.iri AS subj, a.name AS name \
                     FROM book b JOIN author a ON b.author = a.iri \
                     ORDER BY b.title",
                )
                .await?;
            assert_eq!(r.rows.len(), 3, "one matched row per book");

            let link = col(&r, "link");
            let subj = col(&r, "subj");
            for (l, s) in link.iter().zip(subj.iter()) {
                assert_eq!(l, s, "link value equals the author subject IRI");
                // It is an IRI ("Author/..."), NOT the bare id string "rowling".
                assert!(l.contains("Author/"), "join key is an IRI, got `{l}`");
                assert_ne!(l, "rowling");
            }
            eprintln!("matched IRIs: {link:?}");
            Ok(())
        })
        .await
    }

    /// The join across all authors returns one (book, author) pair per book.
    #[tokio::test]
    async fn join_all_pairs() -> anyhow::Result<()> {
        with_session(|session, _spec| async move {
            let r = session
                .run(
                    "SELECT b.title AS title, a.name AS author \
                     FROM book b JOIN author a ON b.author = a.iri \
                     ORDER BY b.title",
                )
                .await?;
            assert_eq!(
                col(&r, "title"),
                vec!["Chamber of Secrets", "Fellowship", "Philosopher's Stone"]
            );
            assert_eq!(col(&r, "author"), vec!["Rowling", "Tolkien", "Rowling"]);
            Ok(())
        })
        .await
    }
}
