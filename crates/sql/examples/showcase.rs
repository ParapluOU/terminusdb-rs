//! # SQL → WOQL proof of concept
//!
//! A guided walkthrough for a TerminusDB maintainer. It boots an in-memory
//! TerminusDB, defines a small library graph (Authors and Books linked by a real
//! object property), seeds data, then runs a progression of SQL statements. For
//! **each** statement it prints:
//!
//! 1. the **SQL**,
//! 2. the **compiled WOQL** (readable DSL),
//! 3. the **live result** returned by TerminusDB.
//!
//! Run it:
//! ```text
//! cargo run -p terminusdb-sql --example showcase --features client
//! ```
//!
//! ## The idea
//!
//! We reuse DataFusion's logical layer to parse and type-check SQL, then compile
//! the resulting plan to WOQL. TerminusDB is the only engine — no SQL runs
//! in-process.
//!
//! | SQL                                   | TerminusDB meaning                         | WOQL                                    |
//! | ------------------------------------- | ------------------------------------------ | --------------------------------------- |
//! | table `book`                          | a concrete document class                  | `triple(S, rdf:type, @schema:Book)`     |
//! | column `title`                        | a datatype property (a value)              | `triple(S, @schema:title, V)`           |
//! | column `iri`                          | the document's subject IRI                 | the row's subject variable              |
//! | `book.author` (a `Ref<Author>`)       | an object property (a graph edge)          | `triple(S, @schema:author, AuthorIRI)`  |
//! | `JOIN … ON b.author = a.iri`           | follow the edge to the Author document      | shared variable (unification)           |
//! | `WHERE year > 1950`                   | a filter on a value                        | `greater(V, 1950)`                      |
//! | `ORDER BY … LIMIT …`                  | ordering + pagination                      | `order_by(…)`, `limit(…)`, `start(…)`   |
//! | an Optional column that is absent      | SQL NULL (datalog absence)                 | `optional(triple(…))`                   |

#![recursion_limit = "256"]

use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use terminusdb_sql::{explain, Catalog, QueryResponse, Session, SqlValue};

// --- schema ------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Author {
    id: EntityIDFor<Self>,
    name: String,
    country: String,
}

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Book {
    id: EntityIDFor<Self>,
    title: String,
    year: i32,
    /// Optional → a nullable column (absent = SQL NULL).
    genre: Option<String>,
    /// A real graph edge to an Author (not a string id).
    author: Ref<Author>,
}

fn author(id: &str, name: &str, country: &str) -> anyhow::Result<Author> {
    Ok(Author {
        id: EntityIDFor::new(id)?,
        name: name.into(),
        country: country.into(),
    })
}
fn book(id: &str, title: &str, year: i32, genre: Option<&str>, author: &str) -> anyhow::Result<Book> {
    Ok(Book {
        id: EntityIDFor::new(id)?,
        title: title.into(),
        year,
        genre: genre.map(str::to_string),
        author: Ref::from(EntityIDFor::<Author>::new(author)?),
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_db_schema::<(Book, Author), _, _, _>("sql_showcase", |client, spec| async move {
            let args = DocumentInsertArgs::from(spec.clone());
            for a in [
                author("austen", "Jane Austen", "UK")?,
                author("tolkien", "J.R.R. Tolkien", "UK")?,
                author("orwell", "George Orwell", "UK")?,
                author("asimov", "Isaac Asimov", "US")?,
                author("newcomer", "New Writer", "US")?, // no books → left-join NULL
            ] {
                client.insert_instance(&a, args.clone()).await?;
            }
            for b in [
                book("pride", "Pride and Prejudice", 1813, Some("Romance"), "austen")?,
                book("lotr", "The Lord of the Rings", 1954, Some("Fantasy"), "tolkien")?,
                book("hobbit", "The Hobbit", 1937, Some("Fantasy"), "tolkien")?,
                book("1984", "Nineteen Eighty-Four", 1949, Some("Dystopia"), "orwell")?,
                book("foundation", "Foundation", 1951, None, "asimov")?, // no genre → NULL
            ] {
                client.insert_instance(&b, args.clone()).await?;
            }

            let session = Session::open(client.clone(), &spec.db, spec.branch.as_deref()).await?;

            banner("SQL → WOQL, live against TerminusDB");
            println!(
                "  database `{}` pinned at commit {}\n",
                spec.db,
                &session.commit()[..session.commit().len().min(12)]
            );

            let steps: &[(&str, &str)] = &[
                ("Project columns", "SELECT title, year FROM book"),
                ("Filter with a range", "SELECT title, year FROM book WHERE year > 1950"),
                ("Boolean AND", "SELECT title FROM book WHERE year > 1930 AND year < 1955"),
                ("Order + limit", "SELECT title, year FROM book ORDER BY year DESC LIMIT 3"),
                ("Optional column → SQL NULL", "SELECT title, genre FROM book"),
                ("DISTINCT (incl. the NULL)", "SELECT DISTINCT genre FROM book"),
                (
                    "Inner join across an object property (book.author → author.iri)",
                    "SELECT b.title, a.name FROM book b JOIN author a ON b.author = a.iri",
                ),
                (
                    "Join + filter on the joined table",
                    "SELECT b.title, a.name, a.country FROM book b \
                     JOIN author a ON b.author = a.iri WHERE a.country = 'US'",
                ),
                (
                    "LEFT JOIN keeps unmatched rows (New Writer has no book → NULL)",
                    "SELECT a.name, b.title FROM author a \
                     LEFT JOIN book b ON b.author = a.iri ORDER BY a.name",
                ),
            ];

            for (i, (desc, sql)) in steps.iter().enumerate() {
                run_step(&session, session.catalog(), i + 1, desc, sql).await?;
            }

            // The safety property: an untranslatable construct is a loud error,
            // never a silent wrong answer.
            banner("Unsupported constructs fail loudly");
            let bad = "SELECT COUNT(*) FROM book";
            println!("  SQL\n    {bad}\n");
            match session.run(bad).await {
                Ok(_) => println!("  (unexpectedly succeeded)"),
                Err(e) => println!("  → rejected: {e}"),
            }
            println!();

            Ok(())
        })
        .await
}

// --- one step: SQL, compiled WOQL, live rows ---------------------------------

async fn run_step(
    session: &Session<TerminusDBHttpClient>,
    catalog: &Catalog,
    n: usize,
    desc: &str,
    sql: &str,
) -> anyhow::Result<()> {
    println!("\n{}", "─".repeat(78));
    println!(" {n}. {desc}");
    println!("{}", "─".repeat(78));
    println!("\n  SQL\n    {}", sql.split_whitespace().collect::<Vec<_>>().join(" "));

    let ex = explain(sql, catalog).map_err(|e| anyhow::anyhow!("compile `{sql}`: {e}"))?;
    println!("\n  Compiled WOQL (DSL)\n    {}", indent(&ex.dsl, "    "));

    let result = session.run(sql).await?;
    println!("\n  Result ({} row{})", result.rows.len(), plural(result.rows.len()));
    print_table(&result);
    Ok(())
}

// --- pretty printing ---------------------------------------------------------

fn print_table(r: &QueryResponse) {
    let mut widths: Vec<usize> = r.columns.iter().map(|c| c.len()).collect();
    let cells: Vec<Vec<String>> = r
        .rows
        .iter()
        .map(|row| row.iter().map(cell_to_string).collect())
        .collect();
    for row in &cells {
        for (i, c) in row.iter().enumerate() {
            widths[i] = widths[i].max(c.len());
        }
    }
    let sep = |ch: char| {
        let mut s = String::from("    +");
        for w in &widths {
            s.push_str(&ch.to_string().repeat(w + 2));
            s.push('+');
        }
        s
    };
    let fmt_row = |vals: &[String]| {
        let mut s = String::from("    |");
        for (i, v) in vals.iter().enumerate() {
            s.push_str(&format!(" {:<width$} |", v, width = widths[i]));
        }
        s
    };
    println!("{}", sep('-'));
    println!("{}", fmt_row(&r.columns));
    println!("{}", sep('='));
    for row in &cells {
        println!("{}", fmt_row(row));
    }
    println!("{}", sep('-'));
}

fn cell_to_string(v: &SqlValue) -> String {
    match v {
        SqlValue::Null => "NULL".into(),
        SqlValue::Node(s) | SqlValue::Str(s) | SqlValue::Decimal(s) | SqlValue::Temporal(s) => {
            s.clone()
        }
        SqlValue::Int(i) => i.to_string(),
        SqlValue::Float(f) => f.to_string(),
        SqlValue::Bool(b) => b.to_string(),
        SqlValue::Json(j) => j.to_string(),
    }
}

fn indent(s: &str, pad: &str) -> String {
    s.replace('\n', &format!("\n{pad}"))
}
fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
fn banner(title: &str) {
    println!("\n{}", "═".repeat(78));
    println!(" {title}");
    println!("{}", "═".repeat(78));
}
