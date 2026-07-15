//! Thin wrapper over `xee-xpath-ast`: parse an XPath string into xee's AST.
//!
//! We reuse xee's spec-grade XPath 3.1 parser rather than writing our own; the
//! only job here is to surface a friendly error type.

use xee_xpath_ast::{ast, XPathParserContext};

use crate::error::{Result, XPathError};

/// Parse an XPath expression string into the `xee-xpath-ast` AST.
///
/// Uses the default static context (standard `fn:`/`xs:` namespaces, no
/// pre-declared variables), which is all our TerminusDB-flavoured subset needs.
pub fn parse(expr: &str) -> Result<ast::XPath> {
    let ctx = XPathParserContext::default();
    ctx.parse_xpath(expr)
        .map_err(|e| XPathError::Parse(format!("{e:?}")))
}
