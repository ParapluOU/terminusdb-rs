//! The thinnest possible wrapper over the external SPARQL parser.
//!
//! We do not hand-write a SPARQL parser. `spargebra` parses SPARQL 1.1 into its
//! *algebra* form (a normalized `GraphPattern` tree), which the [`crate::lower`]
//! module then projects onto our own narrow [`crate::ir`].

use spargebra::{Query, SparqlParser};

use crate::error::{Result, SparqlError};

/// Parse a SPARQL query string into the spargebra algebra.
///
/// `base_iri`, if given, is used to resolve relative IRIs in the query.
pub fn parse(query: &str, base_iri: Option<&str>) -> Result<Query> {
    let mut parser = SparqlParser::new();
    if let Some(base) = base_iri {
        parser = parser
            .with_base_iri(base)
            .map_err(|e| SparqlError::Parse(format!("invalid base IRI: {e}")))?;
    }
    parser
        .parse_query(query)
        .map_err(|e| SparqlError::Parse(e.to_string()))
}
