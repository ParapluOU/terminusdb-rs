//! Filter predicates: the thing a filtered read actually depends on.
//!
//! A GraphQL filter selects a *set*. Until now Reactor could only name that set
//! when the filter was a single `eq`, because `Index{class, prop, value}` is the
//! only term that says "the documents whose `prop` is `value`". Every other
//! shape — a range, a conjunction, a negation — fell back to [`Term::Class`],
//! which fires on any field change to any instance, so a filtered list and the
//! counts over it thrashed on every write.
//!
//! The way out is to stop trying to name the *value* and name the *predicate*
//! instead. [`crate::capture::resolve`] already hands the writer the whole
//! before and after document for every change, so membership can be **decided**
//! rather than guessed:
//!
//! ```text
//! entered   = !member(before) &&  member(after)
//! left      =  member(before) && !member(after)
//! unchanged =  member(before) == member(after)   // fires nothing
//! ```
//!
//! where `member(doc) = doc exists && pred(doc)`. That is exact for any
//! predicate we can evaluate, conjunctions and negations included, because we
//! hold every field and not just the one that moved.
//!
//! # Agreeing with the server is the whole problem
//!
//! An evaluator that disagrees with TerminusDB about collation, numeric
//! coercion, or a missing field under `ne` produces **silent staleness**: the
//! entry is never invalidated by the change that should have killed it, and
//! nothing anywhere reports a problem. So this module implements only what has
//! been *measured* against a live 12.1.0-rc3 node, the measurements are written
//! down below, and [`INDEXABLE_RANGES`] is re-checked against the server by
//! `tests/predicates.rs` — the table is the gate, not a comment.
//!
//! Everything outside the table is **unindexable by construction**: the
//! analyzer refuses to build a predicate from it and the read keeps its coarse
//! `Class(C)` dependency. A filter we only *mostly* understand is worse than one
//! we admit we do not understand at all.
//!
//! ## What the server does (measured, 12.1.0-rc3 community)
//!
//! * **A missing field makes every comparison false** — including `ne`. With
//!   `os` absent, `os: {ne: "x"}` does *not* match. This is the single most
//!   surprising rule and the one an intuitive implementation gets wrong.
//! * **`_not` is a true complement over the class**, so `_not: {os: {eq: "x"}}`
//!   *does* match a document with no `os`. `_not` and `ne` are therefore not
//!   interchangeable, and `_not{_not{p}}` is `p`.
//! * **Strings compare by code point**: `"" < "Apfel" < "Apple" < "B" <
//!   "apple"`. Rust's `str` ordering is byte-wise UTF-8, which is the same
//!   order.
//! * **Numbers compare numerically**, not lexically: `{gt: "9"}` matches `10`.
//! * **Several operators in one field filter are not ANDed.** `{ge: 0, lt: 10}`
//!   returns the `lt: 10` set; `{ge: 0, le: 0}` returns the `le: 0` set;
//!   `{eq: "x", ne: "x"}` returns the `eq` set. The server honours one operator
//!   and silently drops the rest, by its own declaration order. We refuse to
//!   reproduce that: a field filter with more than one operator is unindexable.
//! * **`_and: []` is everything, `_or: []` is nothing** — the standard reading.
//! * **An empty field filter (`{oi: {}}`) is a server-side panic**, so it never
//!   reaches us as a cacheable read.
//! * **Set-valued properties get a `CollectionXFilter`** with a different
//!   operator set, and nested filters on link properties are joins. Neither is
//!   implemented here; both are unindexable.
//!
//! ## Which datatypes can be filtered at all
//!
//! Not all of them, on this server, and the gaps are not where you would
//! guess. `xsd:long` and `xsd:unsignedInt` panic (`not a numerical value`,
//! `graphql/query.rs:672`); `xsd:float` panics on a datatype assertion
//! (`Float64 != Float32`); `xsd:decimal`'s `eq` compares the *lexical* form
//! (`10.250` does not match `10.25`) while its ordering is numeric; and on the
//! narrow integer widths (`xsd:byte`, `xsd:short`, `xsd:unsignedByte`) and the
//! derived ones (`xsd:nonNegativeInteger`, `xsd:positiveInteger`, …) `eq`
//! matches **some stored values and not others** while `ne` and the ordered
//! operators are correct.
//!
//! All of those are left out. Not because indexing them is hard, but because we
//! cannot demonstrate agreement with a server that crashes, contradicts itself,
//! or quietly answers a different question. Of the integer families only
//! `xsd:int` and `xsd:integer` agreed on every operator and every literal we
//! probed. See [`INDEXABLE_RANGES`].

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ids::{ClassName, PropName};

/// How two values of a property are ordered, resolved from its declared range
/// at analysis time.
///
/// Carried **in the predicate** rather than looked up at evaluation time, for
/// two reasons. The writer can then evaluate without touching the schema index,
/// which is what lets evaluation run on a worker pool; and a predicate built
/// against one reading of the schema can never be evaluated under another —
/// a schema change produces a different predicate, hence a different
/// [`PredicateId`], hence no cross-talk with the entries filed under the old
/// one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Order {
    /// Code-point string ordering (`xsd:string`).
    Text,
    /// Exact arbitrary-precision numeric ordering over the decimal text. Used
    /// for the integer families, where going through `f64` would make
    /// `10000000000000001` and `10000000000000000` equal.
    Integer,
    /// IEEE-754 double ordering (`xsd:double`), which is exactly what the
    /// server stores for that range.
    Float,
    /// Boolean equality (`xsd:boolean`). No ordering: GraphQL's
    /// `BooleanFilter` offers only `eq`/`ne`, so an ordered comparison on a
    /// boolean is not expressible and is rejected rather than invented.
    Bool,
}

/// The declared ranges a predicate may mention, and how each one compares.
///
/// **This table is the verification gate.** `tests/predicates.rs` inserts a
/// spread of documents of a class carrying one property per row, asks the
/// *server* for its filtered id set for every operator, and asserts this
/// module classifies the same set. Adding a row without the server agreeing
/// turns that test red.
///
/// Absent on purpose, with the reason, because "we forgot" and "we measured and
/// refused" must not look alike:
///
/// | range | why not |
/// |---|---|
/// | `xsd:long`, `xsd:unsignedInt`, `xsd:unsignedLong` | the server panics (`not a numerical value`) — a filter on one of these cannot be answered at all |
/// | `xsd:byte`, `xsd:short`, `xsd:unsignedByte`, `xsd:unsignedShort`, `xsd:nonNegativeInteger`, `xsd:positiveInteger`, `xsd:negativeInteger`, `xsd:nonPositiveInteger` | **`eq` is not reproducible.** It matches some stored values and not others: `nonNegativeInteger eq "1"` selects the document holding `1`, while `eq "0"` and `eq "10"` select nothing from documents holding `0` and `10` — and `ne` and the ordered operators are correct throughout. A disagreement rather than a crash, so it is the dangerous kind. We could not characterise the rule, so these are unindexed rather than approximated |
/// | `xsd:float` | the server panics on a datatype assertion (`Float64` vs `Float32`) |
/// | `xsd:decimal` | `eq` is lexical (`10.250` ≠ `10.25`) while ordering is numeric; we will not encode a contradiction |
/// | `xsd:dateTime` | `eq` is lexical (`…+00:00` ≠ `…Z`), so agreement would depend on how the writer spelled the timestamp |
/// | `sys:JSON`, enum classes, anything else | no measurement |
pub const INDEXABLE_RANGES: &[(&str, Order)] = &[
    ("xsd:string", Order::Text),
    ("xsd:boolean", Order::Bool),
    ("xsd:double", Order::Float),
    // IntFilter: a GraphQL `Int` literal. `xsd:int` is the only width of this
    // family that agrees with us on all six operators — see the table below.
    ("xsd:int", Order::Integer),
    // BigIntFilter: a GraphQL *string* literal holding a decimal integer. This
    // is the family `#[derive(TerminusDBModel)]` actually emits for `i32`, and
    // the only member of it whose `eq` we could reproduce.
    ("xsd:integer", Order::Integer),
];

/// The comparison discipline for a declared range, or `None` if we have not
/// verified it against the server.
pub fn order_of_range(range: &str) -> Option<Order> {
    INDEXABLE_RANGES
        .iter()
        .find(|(name, _)| *name == range)
        .map(|(_, order)| *order)
}

/// A comparison operator. Exactly the six the server offers on every scalar
/// filter; `regex`, `startsWith`, `allOfTerms`, `anyOfTerms` and `relationship`
/// are deliberately absent — see the module docs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CmpOp {
    /// `eq`
    Eq,
    /// `ne`
    Ne,
    /// `lt`
    Lt,
    /// `le`
    Le,
    /// `gt`
    Gt,
    /// `ge`
    Ge,
}

impl CmpOp {
    /// The GraphQL spelling.
    pub fn as_str(&self) -> &'static str {
        match self {
            CmpOp::Eq => "eq",
            CmpOp::Ne => "ne",
            CmpOp::Lt => "lt",
            CmpOp::Le => "le",
            CmpOp::Gt => "gt",
            CmpOp::Ge => "ge",
        }
    }

    /// Parse a GraphQL operator name. `None` for one we do not index.
    pub fn parse(name: &str) -> Option<CmpOp> {
        Some(match name {
            "eq" => CmpOp::Eq,
            "ne" => CmpOp::Ne,
            "lt" => CmpOp::Lt,
            "le" => CmpOp::Le,
            "gt" => CmpOp::Gt,
            "ge" => CmpOp::Ge,
            _ => return None,
        })
    }

    /// True when this operator only needs equality, not an order.
    pub fn is_equality(&self) -> bool {
        matches!(self, CmpOp::Eq | CmpOp::Ne)
    }
}

/// A literal from a filter, kept in the lexical form the client wrote.
///
/// Numbers keep their text rather than becoming an `f64` or an `i64`: the
/// server's `BigIntFilter` takes an arbitrary-precision integer as a *string*,
/// and `arbitrary_precision` means a document value arrives with its spelling
/// intact. Parsing either side into a fixed-width number is how `1e308` and
/// `1e309` become equal.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Lit {
    /// A string literal.
    Text(String),
    /// A numeric literal, as written.
    Num(String),
    /// `true` / `false`.
    Bool(bool),
}

/// A predicate over one document, in the shape a GraphQL filter can express.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Pred {
    /// One scalar comparison on one property.
    Cmp {
        /// The property compared.
        prop: PropName,
        /// The operator.
        op: CmpOp,
        /// The literal compared against.
        value: Lit,
        /// How the two sides are ordered, from the property's declared range.
        order: Order,
    },
    /// Conjunction. An **empty** `All` is `true` — that is `_and: []`, and it is
    /// also what an empty filter object means.
    All(Vec<Pred>),
    /// Disjunction. An **empty** `Any` is `false`, which is `_or: []`.
    Any(Vec<Pred>),
    /// Negation. A true complement over the class, so it matches documents the
    /// inner predicate is false *or* inapplicable for.
    Not(Box<Pred>),
}

/// A predicate plus the class it selects from.
///
/// The class is part of the identity: `Person(filter: {age: {gt: 30}})` and
/// `Company(filter: {age: {gt: 30}})` are different sets, and a change to a
/// `Company` must not be evaluated against the `Person` predicate.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Predicate {
    /// The class whose instances the predicate selects from.
    pub class: ClassName,
    /// The predicate itself.
    pub pred: Pred,
}

/// A stable identity for a predicate: BLAKE3 over its canonical encoding.
///
/// Used as the registry key and in log output. Equality of ids is equality of
/// predicates — the id is derived from the structure, never assigned — so two
/// reads that state the same filter share one registration and one evaluation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PredicateId([u8; 16]);

impl PredicateId {
    /// The id's hex spelling.
    pub fn to_hex(self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl std::fmt::Debug for PredicateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PredicateId({})", self.to_hex())
    }
}

impl std::fmt::Display for PredicateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// Three-valued truth. `Unknown` is not a hedge: it is the honest answer when a
/// document holds something the measured rules do not cover — a string where a
/// number was declared, say — and it makes the writer fall back to coarse
/// invalidation for that document instead of guessing a membership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Truth {
    /// Definitely a member.
    True,
    /// Definitely not a member.
    False,
    /// We cannot say.
    Unknown,
}

impl Truth {
    fn of(b: bool) -> Truth {
        if b {
            Truth::True
        } else {
            Truth::False
        }
    }
}

impl Predicate {
    /// Build the canonical encoding and its id.
    pub fn new(class: ClassName, pred: Pred) -> Predicate {
        Predicate { class, pred }
    }

    /// This predicate's id.
    pub fn id(&self) -> PredicateId {
        let text = self.encode();
        let hash = blake3::hash(text.as_bytes());
        let mut out = [0u8; 16];
        out.copy_from_slice(&hash.as_bytes()[..16]);
        PredicateId(out)
    }

    /// The canonical encoding: compact JSON of the structure.
    ///
    /// Deterministic because it is derived from the Rust structure, whose field
    /// and variant order is fixed, rather than from the client's filter text.
    /// Two clients who write the same filter with their keys in different
    /// orders therefore get the same predicate — which matters, because sharing
    /// a predicate is what makes the registry small.
    pub fn encode(&self) -> String {
        // Cannot fail: the structure contains only strings, bools and enums.
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Parse the canonical encoding.
    pub fn decode(text: &str) -> Option<Predicate> {
        serde_json::from_str(text).ok()
    }

    /// Every `(class, property)` pair this predicate mentions.
    ///
    /// The registry files the predicate under each of these, so a commit only
    /// evaluates predicates that mention a property which actually moved. That
    /// dispatch is the point of the whole feature: without it every write would
    /// evaluate every live predicate.
    pub fn props(&self) -> Vec<PropName> {
        let mut out = Vec::new();
        collect_props(&self.pred, &mut out);
        out.sort();
        out.dedup();
        out
    }

    /// Whether `doc` is a member of this predicate's set.
    ///
    /// `None` means the document does not exist (a create's before side, a
    /// delete's after side), and a document that does not exist is **not** a
    /// member whatever the predicate says. That is what makes creates and
    /// deletes fall out of the same `entered`/`left` arithmetic as updates
    /// instead of needing cases of their own — and it is why `Not` cannot be
    /// read as "matches a missing document": `_not` complements within the
    /// class, and a document that is not there is not in the class.
    pub fn member(&self, doc: Option<&Value>) -> Truth {
        match doc {
            None => Truth::False,
            Some(doc) => eval(&self.pred, doc),
        }
    }
}

fn collect_props(pred: &Pred, out: &mut Vec<PropName>) {
    match pred {
        Pred::Cmp { prop, .. } => out.push(prop.clone()),
        Pred::All(items) | Pred::Any(items) => {
            for item in items {
                collect_props(item, out);
            }
        }
        Pred::Not(inner) => collect_props(inner, out),
    }
}

/// Evaluate a predicate against a document body.
///
/// Kleene three-valued logic, so an `Unknown` leaf does not poison a conjunction
/// that is already false (or a disjunction already true). The precision matters:
/// an `Unknown` anywhere in the result makes the writer coarse-fire that
/// predicate, so narrowing where `Unknown` escapes is directly a narrowing of
/// over-invalidation.
fn eval(pred: &Pred, doc: &Value) -> Truth {
    match pred {
        Pred::Cmp {
            prop,
            op,
            value,
            order,
        } => eval_cmp(doc.get(prop.as_str()), *op, value, *order),
        Pred::All(items) => {
            let mut unknown = false;
            for item in items {
                match eval(item, doc) {
                    Truth::False => return Truth::False,
                    Truth::Unknown => unknown = true,
                    Truth::True => {}
                }
            }
            if unknown {
                Truth::Unknown
            } else {
                Truth::True
            }
        }
        Pred::Any(items) => {
            let mut unknown = false;
            for item in items {
                match eval(item, doc) {
                    Truth::True => return Truth::True,
                    Truth::Unknown => unknown = true,
                    Truth::False => {}
                }
            }
            if unknown {
                Truth::Unknown
            } else {
                Truth::False
            }
        }
        Pred::Not(inner) => match eval(inner, doc) {
            Truth::True => Truth::False,
            Truth::False => Truth::True,
            Truth::Unknown => Truth::Unknown,
        },
    }
}

/// One comparison against one document value.
///
/// `value` is the document's value for the property, or `None` if the property
/// is absent. **Absent is `False` for every operator, `ne` included** — the
/// measured server behaviour, and the rule that makes `ne` and `_not` differ.
fn eval_cmp(value: Option<&Value>, op: CmpOp, lit: &Lit, order: Order) -> Truth {
    let value = match value {
        // Absent, or explicitly null, which the document API renders for
        // nothing we index but which must not be read as a comparable value.
        None | Some(Value::Null) => return Truth::False,
        Some(value) => value,
    };
    match (order, value, lit) {
        (Order::Bool, Value::Bool(have), Lit::Bool(want)) => match op {
            CmpOp::Eq => Truth::of(have == want),
            CmpOp::Ne => Truth::of(have != want),
            // `BooleanFilter` has no ordering operators, so this is not
            // reachable from a parsed filter; refusing it keeps the evaluator
            // honest if that ever changes.
            _ => Truth::Unknown,
        },
        (Order::Text, Value::String(have), Lit::Text(want)) => {
            // Rust's `str` ordering is byte-wise over UTF-8, which is code
            // point order — what the server does.
            Truth::of(satisfies(op, have.as_str().cmp(want.as_str())))
        }
        (Order::Integer, Value::Number(have), Lit::Num(want)) => {
            match cmp_decimal_text(&have.to_string(), want) {
                Some(ordering) => Truth::of(satisfies(op, ordering)),
                None => Truth::Unknown,
            }
        }
        (Order::Float, Value::Number(have), Lit::Num(want)) => {
            match (have.as_f64(), want.parse::<f64>()) {
                (Some(a), Ok(b)) => match a.partial_cmp(&b) {
                    Some(ordering) => Truth::of(satisfies(op, ordering)),
                    // A NaN on either side. The server's behaviour here is not
                    // measured, so we decline to invent one.
                    None => Truth::Unknown,
                },
                _ => Truth::Unknown,
            }
        }
        // The document holds something of a different JSON shape than the
        // declared range implies — an array for a property we believed single,
        // a string where a number was declared. Our rules do not cover it.
        _ => Truth::Unknown,
    }
}

/// Does `ordering` (the document value compared *to* the literal) satisfy `op`?
fn satisfies(op: CmpOp, ordering: std::cmp::Ordering) -> bool {
    use std::cmp::Ordering::*;
    match op {
        CmpOp::Eq => ordering == Equal,
        CmpOp::Ne => ordering != Equal,
        CmpOp::Lt => ordering == Less,
        CmpOp::Le => ordering != Greater,
        CmpOp::Gt => ordering == Greater,
        CmpOp::Ge => ordering != Less,
    }
}

/// Compare two decimal numerals exactly, without going through a float.
///
/// Handles an optional sign, an integer part and an optional fraction.
/// Exponent notation returns `None` (undecidable) rather than a guess: the
/// integer ranges we index never produce it, and a wrong answer here would be
/// invisible. Arbitrary length is the point — `xsd:integer` has no width, so an
/// `i128` would be a silent ceiling.
fn cmp_decimal_text(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    let (a_neg, a_body) = split_sign(a.trim())?;
    let (b_neg, b_body) = split_sign(b.trim())?;
    let a_parts = split_decimal(a_body)?;
    let b_parts = split_decimal(b_body)?;

    // Zero has no sign: `-0` and `0` must compare equal, or `ge: 0` would miss
    // a stored `-0`.
    let a_zero = is_zero(&a_parts);
    let b_zero = is_zero(&b_parts);
    if a_zero && b_zero {
        return Some(Ordering::Equal);
    }
    match (a_neg && !a_zero, b_neg && !b_zero) {
        (true, false) => return Some(Ordering::Less),
        (false, true) => return Some(Ordering::Greater),
        (true, true) => return Some(cmp_magnitude(&b_parts, &a_parts)),
        (false, false) => {}
    }
    Some(cmp_magnitude(&a_parts, &b_parts))
}

/// Split a leading sign, rejecting anything that is not a decimal numeral.
fn split_sign(text: &str) -> Option<(bool, &str)> {
    let (neg, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }
    Some((neg, rest))
}

/// `("123", "45")` for `123.45`, with insignificant zeros trimmed.
fn split_decimal(text: &str) -> Option<(&str, &str)> {
    let (int, frac) = match text.split_once('.') {
        Some((int, frac)) => {
            if frac.contains('.') {
                return None;
            }
            (int, frac)
        }
        None => (text, ""),
    };
    Some((int.trim_start_matches('0'), frac.trim_end_matches('0')))
}

fn is_zero(parts: &(&str, &str)) -> bool {
    parts.0.is_empty() && parts.1.is_empty()
}

/// Compare two non-negative magnitudes given as (trimmed integer, trimmed
/// fraction).
fn cmp_magnitude(a: &(&str, &str), b: &(&str, &str)) -> std::cmp::Ordering {
    // More significant integer digits means larger, since leading zeros are
    // already gone.
    a.0.len()
        .cmp(&b.0.len())
        .then_with(|| a.0.cmp(b.0))
        .then_with(|| cmp_fraction(a.1, b.1))
}

/// Compare fractions digit by digit, shorter one zero-padded.
fn cmp_fraction(a: &str, b: &str) -> std::cmp::Ordering {
    let mut a = a.bytes();
    let mut b = b.bytes();
    loop {
        match (a.next(), b.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (x, y) => {
                let x = x.unwrap_or(b'0');
                let y = y.unwrap_or(b'0');
                if x != y {
                    return x.cmp(&y);
                }
            }
        }
    }
}

/// A predicate with its id precomputed, as the registry and the terms hold it.
pub type SharedPredicate = Arc<Predicate>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{cls, prop};
    use serde_json::json;

    fn cmp(p: &str, op: CmpOp, v: Lit, order: Order) -> Pred {
        Pred::Cmp {
            prop: prop(p),
            op,
            value: v,
            order,
        }
    }

    fn pred(p: Pred) -> Predicate {
        Predicate::new(cls("U"), p)
    }

    fn n(s: &str) -> Lit {
        Lit::Num(s.to_owned())
    }

    fn t(s: &str) -> Lit {
        Lit::Text(s.to_owned())
    }

    // ---------------------------------------------------- the measured rules

    /// The rule an intuitive implementation gets wrong: a missing field is
    /// false under *every* operator, `ne` included. Measured against
    /// 12.1.0-rc3; `tests/predicates.rs` re-checks it against the server.
    #[test]
    fn a_missing_field_is_false_for_every_operator() {
        let doc = json!({"k": "q"});
        for op in [
            CmpOp::Eq,
            CmpOp::Ne,
            CmpOp::Lt,
            CmpOp::Le,
            CmpOp::Gt,
            CmpOp::Ge,
        ] {
            assert_eq!(
                pred(cmp("os", op, t("x"), Order::Text)).member(Some(&doc)),
                Truth::False,
                "{} on a missing field must be false, not {:?}",
                op.as_str(),
                pred(cmp("os", op, t("x"), Order::Text)).member(Some(&doc))
            );
        }
    }

    /// And the companion rule: `_not` is a complement over the class, so it
    /// *does* match the document with no value. `ne` and `_not{eq}` are not
    /// the same predicate.
    #[test]
    fn not_complements_while_ne_does_not() {
        let missing = json!({"k": "q"});
        let present = json!({"k": "p", "os": "x"});

        let ne = pred(cmp("os", CmpOp::Ne, t("x"), Order::Text));
        let not_eq = pred(Pred::Not(Box::new(cmp(
            "os",
            CmpOp::Eq,
            t("x"),
            Order::Text,
        ))));

        assert_eq!(ne.member(Some(&missing)), Truth::False);
        assert_eq!(not_eq.member(Some(&missing)), Truth::True);
        assert_eq!(ne.member(Some(&present)), Truth::False);
        assert_eq!(not_eq.member(Some(&present)), Truth::False);
    }

    #[test]
    fn a_nonexistent_document_is_never_a_member() {
        // Even under a negation, which would otherwise be vacuously true.
        let not_eq = pred(Pred::Not(Box::new(cmp(
            "os",
            CmpOp::Eq,
            t("x"),
            Order::Text,
        ))));
        assert_eq!(not_eq.member(None), Truth::False);
        assert_eq!(pred(Pred::All(vec![])).member(None), Truth::False);
    }

    #[test]
    fn strings_compare_by_code_point() {
        // The order the server showed: "" < "Apfel" < "Apple" < "B" < "apple".
        let order = ["", "Apfel", "Apple", "B", "apple"];
        for (i, a) in order.iter().enumerate() {
            for (j, b) in order.iter().enumerate() {
                let p = pred(cmp("os", CmpOp::Lt, t(b), Order::Text));
                let doc = json!({"os": a});
                assert_eq!(
                    p.member(Some(&doc)),
                    Truth::of(i < j),
                    "{a:?} < {b:?} should be {}",
                    i < j
                );
            }
        }
    }

    #[test]
    fn numbers_compare_numerically_not_lexically() {
        let doc = json!({"n": 10});
        // Lexically "10" < "9"; numerically 10 > 9. The server says numeric.
        assert_eq!(
            pred(cmp("n", CmpOp::Gt, n("9"), Order::Integer)).member(Some(&doc)),
            Truth::True
        );
        assert_eq!(
            pred(cmp("n", CmpOp::Lt, n("9"), Order::Integer)).member(Some(&doc)),
            Truth::False
        );
    }

    #[test]
    fn integer_comparison_is_exact_beyond_f64() {
        // Two values that are the same `f64`.
        let a = "10000000000000001";
        let b = "10000000000000000";
        assert_eq!(a.parse::<f64>().unwrap(), b.parse::<f64>().unwrap());
        let doc = json!({ "n": serde_json::Number::from_string_unchecked(a.to_owned()) });
        assert_eq!(
            pred(cmp("n", CmpOp::Gt, n(b), Order::Integer)).member(Some(&doc)),
            Truth::True,
            "an f64 round trip would make these equal"
        );
        assert_eq!(
            pred(cmp("n", CmpOp::Eq, n(b), Order::Integer)).member(Some(&doc)),
            Truth::False
        );
    }

    #[test]
    fn negative_numbers_and_signed_zero() {
        for (have, want, op, expect) in [
            ("-5", "0", CmpOp::Lt, true),
            ("-5", "-6", CmpOp::Gt, true),
            ("-5", "-5", CmpOp::Eq, true),
            ("0", "-0", CmpOp::Eq, true),
            ("-0", "0", CmpOp::Ge, true),
            ("-5", "0", CmpOp::Ge, false),
            ("-1", "-10", CmpOp::Gt, true),
        ] {
            let doc = json!({ "n": serde_json::Number::from_string_unchecked(have.to_owned()) });
            assert_eq!(
                pred(cmp("n", op, n(want), Order::Integer)).member(Some(&doc)),
                Truth::of(expect),
                "{have} {} {want}",
                op.as_str()
            );
        }
    }

    #[test]
    fn empty_conjunction_is_true_and_empty_disjunction_is_false() {
        let doc = json!({"k": "p"});
        assert_eq!(pred(Pred::All(vec![])).member(Some(&doc)), Truth::True);
        assert_eq!(pred(Pred::Any(vec![])).member(Some(&doc)), Truth::False);
    }

    #[test]
    fn a_wrong_shaped_value_is_unknown_not_a_guess() {
        // A string where a number was declared: our rules do not cover it, and
        // guessing would be the silent-staleness failure.
        let doc = json!({"n": "10"});
        assert_eq!(
            pred(cmp("n", CmpOp::Gt, n("9"), Order::Integer)).member(Some(&doc)),
            Truth::Unknown
        );
        // But a conjunction already false stays false: Kleene, not poisoning.
        let p = pred(Pred::All(vec![
            cmp("k", CmpOp::Eq, t("nope"), Order::Text),
            cmp("n", CmpOp::Gt, n("9"), Order::Integer),
        ]));
        assert_eq!(p.member(Some(&doc)), Truth::False);
        // And a disjunction already true stays true.
        let p = pred(Pred::Any(vec![
            cmp("k", CmpOp::Eq, t("p"), Order::Text),
            cmp("n", CmpOp::Gt, n("9"), Order::Integer),
        ]));
        assert_eq!(p.member(Some(&json!({"k": "p", "n": "10"}))), Truth::True);
    }

    #[test]
    fn exponent_notation_is_undecidable_under_integer_order() {
        let doc = json!({ "n": serde_json::Number::from_string_unchecked("1e10".to_owned()) });
        assert_eq!(
            pred(cmp("n", CmpOp::Gt, n("9"), Order::Integer)).member(Some(&doc)),
            Truth::Unknown
        );
    }

    #[test]
    fn doubles_compare_as_f64() {
        let doc = json!({"d": 1.5});
        assert_eq!(
            pred(cmp("d", CmpOp::Ge, n("1.5"), Order::Float)).member(Some(&doc)),
            Truth::True
        );
        assert_eq!(
            pred(cmp("d", CmpOp::Gt, n("1.5"), Order::Float)).member(Some(&doc)),
            Truth::False
        );
        let big = json!({ "d": serde_json::Number::from_string_unchecked("1e+300".to_owned()) });
        assert_eq!(
            pred(cmp("d", CmpOp::Gt, n("1.0e299"), Order::Float)).member(Some(&big)),
            Truth::True
        );
    }

    #[test]
    fn booleans_compare_by_equality_only() {
        let doc = json!({"b": true});
        assert_eq!(
            pred(cmp("b", CmpOp::Eq, Lit::Bool(true), Order::Bool)).member(Some(&doc)),
            Truth::True
        );
        assert_eq!(
            pred(cmp("b", CmpOp::Ne, Lit::Bool(true), Order::Bool)).member(Some(&doc)),
            Truth::False
        );
        assert_eq!(
            pred(cmp("b", CmpOp::Lt, Lit::Bool(true), Order::Bool)).member(Some(&doc)),
            Truth::Unknown
        );
    }

    // -------------------------------------------------------- identity, props

    #[test]
    fn the_id_is_derived_from_the_structure() {
        let a = pred(Pred::All(vec![
            cmp("n", CmpOp::Ge, n("0"), Order::Integer),
            cmp("os", CmpOp::Lt, t("B"), Order::Text),
        ]));
        let b = a.clone();
        assert_eq!(a.id(), b.id());

        // A different class, a different operator, a different order: all
        // different predicates.
        let other_class = Predicate::new(cls("V"), a.pred.clone());
        assert_ne!(a.id(), other_class.id());
        let other_order = pred(Pred::All(vec![
            cmp("n", CmpOp::Ge, n("0"), Order::Float),
            cmp("os", CmpOp::Lt, t("B"), Order::Text),
        ]));
        assert_ne!(a.id(), other_order.id());
    }

    #[test]
    fn the_canonical_encoding_round_trips() {
        let p = pred(Pred::Any(vec![
            Pred::Not(Box::new(cmp("os", CmpOp::Eq, t("x,y"), Order::Text))),
            Pred::All(vec![cmp("b", CmpOp::Ne, Lit::Bool(false), Order::Bool)]),
        ]));
        let text = p.encode();
        assert_eq!(Predicate::decode(&text), Some(p));
    }

    #[test]
    fn props_lists_every_mentioned_property_once() {
        let p = pred(Pred::All(vec![
            cmp("a", CmpOp::Eq, t("1"), Order::Text),
            Pred::Any(vec![
                cmp("b", CmpOp::Eq, t("2"), Order::Text),
                Pred::Not(Box::new(cmp("a", CmpOp::Ne, t("3"), Order::Text))),
            ]),
        ]));
        assert_eq!(p.props(), vec![prop("a"), prop("b")]);
    }

    #[test]
    fn only_verified_ranges_are_indexable() {
        assert_eq!(order_of_range("xsd:string"), Some(Order::Text));
        assert_eq!(order_of_range("xsd:integer"), Some(Order::Integer));
        assert_eq!(order_of_range("xsd:double"), Some(Order::Float));
        // The ones the server cannot answer, and the ones we never measured.
        for range in [
            "xsd:long",
            "xsd:unsignedInt",
            "xsd:unsignedLong",
            "xsd:float",
            "xsd:decimal",
            "xsd:dateTime",
            // Measured and refused: `eq` matches some values and not others.
            "xsd:byte",
            "xsd:short",
            "xsd:unsignedByte",
            "xsd:unsignedShort",
            "xsd:nonNegativeInteger",
            "xsd:positiveInteger",
            "xsd:negativeInteger",
            "xsd:nonPositiveInteger",
            "sys:JSON",
            "Colour",
        ] {
            assert_eq!(order_of_range(range), None, "{range} must not be indexable");
        }
    }
}
