//! Deterministic, **recorded**, collision-detecting mapping from schema entity
//! names (class / property IRIs) to SQL identifiers.
//!
//! Mangling a TerminusDB name to an SQL identifier is lossy: IRI prefixes get
//! stripped, characters illegal in an unquoted identifier are replaced, and — to
//! agree with DataFusion's default identifier folding — the result is lowercased.
//! Because it is lossy, two distinct schema entities can collide onto the same SQL
//! identifier. We treat that as a **hard error at load time** and never pick a
//! winner (no last-write-wins). The reverse map is retained so the emitter can
//! recover the real IRI for a mangled name.

use std::collections::HashMap;

use terminusdb_schema::strip_schema_prefix;

use crate::error::SqlError;

/// Turn a schema entity name (class or property IRI / short name) into a candidate
/// SQL identifier. Not injective — [`IdentMap`] detects the collisions this can
/// produce.
pub fn mangle(source: &str) -> String {
    let local = strip_schema_prefix(source);
    let mut out = String::with_capacity(local.len());
    for ch in local.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    } else if out.as_bytes()[0].is_ascii_digit() {
        out.insert(0, '_');
    }
    // Fold case so lookups agree with DataFusion's default unquoted-identifier
    // normalisation (`enable_ident_normalization = true`).
    out.make_ascii_lowercase();
    out
}

/// A collision-detecting name map within a single naming domain (one for the
/// table namespace; one per table for its columns). Records which schema entity
/// owns each mangled SQL identifier so a second entity mangling onto it is a hard
/// error rather than a silent overwrite.
#[derive(Debug, Default, Clone)]
pub struct IdentMap {
    /// mangled SQL id → the source identity that owns it (the collision domain).
    by_sql: HashMap<String, String>,
}

impl IdentMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserve an SQL identifier for a synthetic column (e.g. `iri`) that has no
    /// schema-entity source, so a real property mangling onto it collides.
    pub fn reserve(&mut self, sql: &str, label: &str) {
        self.by_sql.insert(sql.to_string(), label.to_string());
    }

    /// Mangle `source` and record it. Returns the chosen SQL identifier, or
    /// [`SqlError::IdentifierCollision`] if a *different* source already owns it.
    /// Idempotent for the same source.
    pub fn insert(&mut self, source: &str) -> Result<String, SqlError> {
        let sql = mangle(source);
        match self.by_sql.get(&sql) {
            Some(existing) if existing != source => {
                return Err(SqlError::IdentifierCollision {
                    sql,
                    first: existing.clone(),
                    second: source.to_string(),
                });
            }
            _ => {}
        }
        self.by_sql.insert(sql.clone(), source.to_string());
        Ok(sql)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mangling_strips_prefix_and_lowercases() {
        assert_eq!(mangle("@schema:Person"), "person");
        assert_eq!(mangle("https://ns#DocumentType"), "documenttype");
        assert_eq!(mangle("firstName"), "firstname");
    }

    #[test]
    fn illegal_chars_and_leading_digit() {
        assert_eq!(mangle("first-name"), "first_name");
        assert_eq!(mangle("123abc"), "_123abc");
    }

    #[test]
    fn collision_is_a_hard_error_naming_both() {
        let mut m = IdentMap::new();
        assert_eq!(m.insert("@schema:Person").unwrap(), "person");
        // A different source that mangles to the same id must error, not overwrite.
        let err = m.insert("@schema:person").unwrap_err();
        match err {
            SqlError::IdentifierCollision { sql, first, second } => {
                assert_eq!(sql, "person");
                assert_eq!(first, "@schema:Person");
                assert_eq!(second, "@schema:person");
            }
            other => panic!("expected collision, got {other:?}"),
        }
    }

    #[test]
    fn same_source_is_idempotent() {
        let mut m = IdentMap::new();
        assert_eq!(m.insert("name").unwrap(), "name");
        assert_eq!(m.insert("name").unwrap(), "name");
    }

    #[test]
    fn reserved_id_collides_with_property() {
        let mut m = IdentMap::new();
        m.reserve("id", "<id column>");
        let err = m.insert("@schema:id").unwrap_err();
        assert!(matches!(err, SqlError::IdentifierCollision { .. }));
    }
}
