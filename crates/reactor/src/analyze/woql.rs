//! `/api/woql` analysis.
//!
//! WOQL arrives as JSON-LD and we analyze it **as JSON**, not by deserializing
//! into [`terminusdb_woql2::query::Query`] — that type derives
//! `TerminusDBModel`/`FromTDBInstance`, not serde's `Deserialize`, so there is
//! nothing to deserialize into. That turns out to be the right shape for the
//! job anyway: an analyzer whose duty is to be conservative about everything it
//! does not recognize is naturally a whitelist walk over a tree, and staying on
//! the raw JSON keeps us off the re-serialization path entirely (invariant 1).
//!
//! The whitelist itself is taken from woql2 **at runtime**, via
//! `Query::to_schema_tree()`, so a renamed or added query class cannot silently
//! drift out of sync with this module. An `@type` that woql2 does not know is
//! not WOQL we understand, and is therefore a barrier term or a bypass.
//!
//! Node shapes (verified against `Query::to_woql_json()`, not guessed):
//!
//! ```json
//! {"@type":"Triple",
//!  "subject":  {"@type":"NodeValue","node":"Person/alice"},
//!  "predicate":{"@type":"NodeValue","node":"name"},
//!  "object":   {"@type":"Value","variable":"n"},
//!  "graph":"instance"}
//! ```

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use serde_json::Value;

use crate::analyze::{is_read_method, Augment, CanonicalRequest, Plan, ReqView};
use crate::ids::{ClassName, DocId, Namespace, PropName};
use crate::schema_index::{PropKind, SchemaIndex};
use crate::term::Term;

use super::AnalyzeConfig;

/// Maximum JSON nesting we will walk.
const MAX_DEPTH: usize = 128;

/// Query classes that modify the database.
///
/// Note what is *not* here: `AddedTriple`, `AddedLink`, `AddedData`,
/// `DeletedTriple` and `DeletedLink`. Despite their names those are **reads** —
/// they ask "what did this commit add/remove" — and treating them as writes
/// would forward every commit-introspection query uncached and tell capture to
/// expect a commit that never comes. The spec's "any `Add*`/`Delete*`" rule is
/// too blunt; this list is the precise one.
const WRITE_TYPES: &[&str] = &[
    "AddTriple",
    "AddLink",
    "AddData",
    "DeleteTriple",
    "DeleteLink",
    "InsertDocument",
    "UpdateDocument",
    "DeleteDocument",
];

/// Query classes that read the commit *history* rather than the current state.
///
/// Their answer depends on which commit is current, which no document-level term
/// expresses, so they get a barrier.
const HISTORY_TYPES: &[&str] = &[
    "AddedTriple",
    "AddedLink",
    "AddedData",
    "DeletedTriple",
    "DeletedLink",
];

/// Query classes that read something outside the database entirely.
///
/// A CSV or remote resource can change with no commit at all, so there is no
/// term that could ever invalidate the entry. The only honest answer is to
/// forward it.
const EXTERNAL_TYPES: &[&str] = &["Get", "QueryResource", "Source"];

/// Every class name woql2 knows, derived from woql2 itself.
///
/// Computed once. The point is drift: if woql2 renames a query class, this set
/// changes with it, and the only consequence is that the *classification* lists
/// above have to keep up — which a test asserts.
pub fn woql_type_names() -> &'static HashSet<String> {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        use terminusdb_schema::ToTDBSchema;
        terminusdb_woql2::query::Query::to_schema_tree()
            .iter()
            .map(|schema| schema.class_name().clone())
            .collect()
    })
}

/// What analyzing a WOQL body concluded.
#[derive(Clone, Debug, PartialEq)]
pub struct WoqlAnalysis {
    /// The query modifies the database.
    pub is_write: bool,
    /// Every resource it reads is pinned at a commit, so the answer is fixed.
    pub immutable: bool,
    /// The dependencies we could prove.
    pub terms: Vec<Term>,
    /// A value-free canonical rendering, for the shape hash.
    pub shape: String,
    /// Set when the query cannot be cached at all.
    pub bypass: Option<&'static str>,
}

/// Plan a `/api/woql` request.
pub(super) fn plan(
    req: &ReqView,
    ns: &Namespace,
    schema: &SchemaIndex,
    cfg: &AnalyzeConfig,
) -> Plan {
    // A WOQL read is a POST with a JSON-LD body; a GET has nothing to analyze.
    if is_read_method(req.method) {
        return Plan::bypass("woql without a body");
    }
    let body: Value = match serde_json::from_slice(req.body) {
        Ok(body) => body,
        Err(_) => return Plan::bypass("woql body is not JSON"),
    };
    // The client wraps the query as `{"query": {...}, "commit_info": …}` on some
    // paths. Unwrap that envelope — but ONLY when the outer object is not itself
    // a WOQL node: `Using`, `Select`, `Limit`, `Not` and friends all have a
    // `query` field of their own, and unwrapping one of those would silently
    // discard the node that carries the resource scope.
    let query = if body.get("@type").is_none() {
        body.get("query").filter(|q| q.is_object()).unwrap_or(&body)
    } else {
        &body
    };

    let analysis = analyze_woql(query, ns, schema, cfg);
    if let Some(reason) = analysis.bypass {
        return Plan::bypass(reason);
    }
    if analysis.is_write {
        return Plan::Write;
    }

    let canonical = CanonicalRequest::with_params(
        "woql",
        ns,
        Vec::new(),
        req.body,
        Some(analysis.shape.clone()),
    );
    Plan::Read {
        key: canonical.request_key(ns, None),
        terms: if analysis.immutable {
            Vec::new()
        } else {
            analysis.terms
        },
        immutable: analysis.immutable,
        rewrite: None,
        // WOQL bindings are variable values, not documents; there is no
        // reliable id field to harvest, so the read set stands as analyzed.
        augment: Augment::None,
    }
}

/// Analyze a WOQL JSON-LD tree.
pub fn analyze_woql(
    query: &Value,
    ns: &Namespace,
    schema: &SchemaIndex,
    cfg: &AnalyzeConfig,
) -> WoqlAnalysis {
    let mut walk = Walk {
        ns,
        schema,
        cfg,
        terms: Vec::new(),
        is_write: false,
        immutable: false,
        saw_unpinned_read: false,
        bypass: None,
        var_classes: HashMap::new(),
    };
    walk.collect_bindings(query, 0);
    walk.visit(query, 0, false);

    let mut terms = std::mem::take(&mut walk.terms);
    terms.sort();
    terms.dedup();

    WoqlAnalysis {
        is_write: walk.is_write,
        // Only immutable if a commit pin was seen *and* nothing read the live
        // branch alongside it.
        immutable: walk.immutable && !walk.saw_unpinned_read,
        terms,
        shape: shape_of(query),
        bypass: walk.bypass,
    }
}

struct Walk<'a> {
    ns: &'a Namespace,
    schema: &'a SchemaIndex,
    cfg: &'a AnalyzeConfig,
    terms: Vec<Term>,
    is_write: bool,
    immutable: bool,
    saw_unpinned_read: bool,
    bypass: Option<&'static str>,
    /// Variable name → the class an `IsA`/`TypeOf`/`rdf:type` binding gave it.
    ///
    /// This is what turns "some unknown subject's `name` changed" from an
    /// `Everything` barrier into a `ClassField(Person, name)`, which is the
    /// difference between a WOQL entry that survives commits and one that does
    /// not.
    var_classes: HashMap<String, ClassName>,
}

impl Walk<'_> {
    /// Pass one: find out which classes the variables are constrained to.
    fn collect_bindings(&mut self, value: &Value, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        if let Some(obj) = value.as_object() {
            match obj.get("@type").and_then(Value::as_str) {
                Some("IsA") => self.bind(obj.get("element"), obj.get("type")),
                Some("TypeOf") => self.bind(obj.get("value"), obj.get("type")),
                Some(_) | None => {
                    // `triple(V, "rdf:type", node(C))` is the other spelling of
                    // a type constraint, and the one the `type_!` macro emits.
                    if let (Some(subject), Some(predicate), Some(object)) =
                        (obj.get("subject"), obj.get("predicate"), obj.get("object"))
                    {
                        if is_rdf_type(node_of(predicate)) {
                            self.bind(Some(subject), Some(object));
                        }
                    }
                }
            }
        }
        for child in children(value) {
            self.collect_bindings(child, depth + 1);
        }
    }

    fn bind(&mut self, element: Option<&Value>, class: Option<&Value>) {
        let (Some(element), Some(class)) = (element, class) else {
            return;
        };
        let (Some(var), Some(class)) = (var_of(element), node_of(class)) else {
            return;
        };
        let class = ClassName::new(strip_schema_prefix(class));
        self.var_classes.insert(var.to_owned(), class);
    }

    /// Pass two: derive terms.
    ///
    /// `pinned` is the lexical scope of a commit pin: a read nested inside
    /// `Using(".../local/commit/<id>", …)` reads a fixed commit, so it does not
    /// make the answer mutable. Tracking this as a flat flag instead of a scope
    /// would misclassify every pinned query that contains a triple — which is
    /// all of them.
    fn visit(&mut self, value: &Value, depth: usize, pinned: bool) {
        if depth > MAX_DEPTH {
            self.bypass = Some("woql query too deeply nested to analyze");
            return;
        }
        let mut child_pinned = pinned;
        if let Some(obj) = value.as_object() {
            if let Some(ty) = obj.get("@type").and_then(Value::as_str) {
                child_pinned = self.visit_typed(ty, obj, pinned);
            }
        }
        for child in children(value) {
            self.visit(child, depth + 1, child_pinned);
        }
    }

    /// Returns the pin scope that applies to this node's children.
    fn visit_typed(
        &mut self,
        ty: &str,
        obj: &serde_json::Map<String, Value>,
        pinned: bool,
    ) -> bool {
        // An `@type` woql2 does not know is not WOQL we can reason about.
        if !woql_type_names().contains(ty) {
            self.coarse("unrecognized woql node type");
            return pinned;
        }
        if WRITE_TYPES.contains(&ty) {
            self.is_write = true;
            return pinned;
        }
        if EXTERNAL_TYPES.contains(&ty) {
            // Can change with no commit: no term can ever invalidate it.
            self.bypass = Some("woql reads a resource outside the database");
            return pinned;
        }
        if HISTORY_TYPES.contains(&ty) {
            self.coarse("woql reads commit history");
            return pinned;
        }

        match ty {
            // Resource selection. `Using`/`From`/`Into` can point the query at
            // another database or branch entirely, in which case terms derived
            // for *our* namespace would never be fired by the changes that
            // actually matter — a silent staleness bug. So anything but our own
            // namespace or a commit pin is refused.
            "Using" | "From" | "Into" => {
                let collection = obj.get("collection").and_then(Value::as_str).unwrap_or("");
                match classify_collection(collection, self.ns) {
                    Collection::Pinned => {
                        self.immutable = true;
                        // Everything below this node reads a fixed commit.
                        return true;
                    }
                    Collection::SameNamespace => {}
                    Collection::Elsewhere => {
                        self.bypass = Some("woql reads another resource");
                    }
                }
            }
            "Pin" => {
                // `Pin` fixes the query at *the current* commit, which we cannot
                // name from the body alone, so it is not an immutability claim
                // we are able to make here.
                self.note_live_read(pinned);
            }
            "IsA" => self.class_terms(obj.get("type"), pinned),
            "TypeOf" => self.class_terms(obj.get("type"), pinned),
            "Subsumption" => {
                // Pure class reasoning: it reads the hierarchy, so it depends on
                // the schema, plus the classes themselves for good measure.
                self.terms.push(Term::Schema);
                self.class_terms(obj.get("parent"), pinned);
            }
            "ReadDocument" => match obj.get("identifier").and_then(node_of) {
                Some(id) => {
                    self.note_live_read(pinned);
                    self.terms.push(Term::Doc(DocId::parse(id)));
                }
                None => self.coarse("woql reads an unbound document"),
            },
            "Path" => {
                self.note_live_read(pinned);
                let from = obj.get("subject").and_then(node_of).map(DocId::parse);
                // We cannot prove which classes a path pattern traverses, and a
                // `Path` with no classes is a barrier by construction. Narrowing
                // it would mean resolving the pattern's predicates against the
                // schema, which is a precision win left for later.
                self.terms.push(Term::Path {
                    from,
                    classes: Vec::new(),
                });
            }
            _ => {
                // A triple-shaped node: Triple, Link, Data and their relatives
                // all carry subject/predicate/object, so one rule covers them.
                if obj.contains_key("subject") && obj.contains_key("predicate") {
                    self.triple_terms(obj, pinned);
                }
            }
        }
        pinned
    }

    /// Record that something read the live branch rather than a pinned commit.
    fn note_live_read(&mut self, pinned: bool) {
        if !pinned {
            self.saw_unpinned_read = true;
        }
    }

    /// `IsA`/`TypeOf`/`Subsumption` against a bound class.
    fn class_terms(&mut self, class: Option<&Value>, pinned: bool) {
        self.note_live_read(pinned);
        match class.and_then(node_of) {
            Some(class) => {
                let class = ClassName::new(strip_schema_prefix(class));
                self.terms.push(Term::Members(class.clone()));
                self.terms.push(Term::Class(class));
            }
            None => self.coarse("woql type constraint on an unbound class"),
        }
    }

    /// The read set of one triple pattern.
    fn triple_terms(&mut self, obj: &serde_json::Map<String, Value>, pinned: bool) {
        self.note_live_read(pinned);
        let subject = obj.get("subject");
        let predicate = obj.get("predicate");
        let object = obj.get("object");

        let subject_node = subject.and_then(node_of);
        let subject_var = subject.and_then(var_of);
        let predicate_node = predicate.and_then(node_of);

        // A type triple is a class constraint, not a field read.
        if is_rdf_type(predicate_node) {
            if let Some(class) = object.and_then(node_of) {
                let class = ClassName::new(strip_schema_prefix(class));
                self.terms.push(Term::Members(class.clone()));
                self.terms.push(Term::Class(class));
                return;
            }
        }

        match (subject_node, predicate_node) {
            // Bound subject and predicate: the tightest case there is.
            (Some(id), Some(prop)) => {
                let id = DocId::parse(id);
                let prop = PropName::new(strip_schema_prefix(prop));
                let kind = id
                    .class()
                    .and_then(|class| self.schema.prop(&class, &prop).cloned());
                match kind {
                    Some(PropKind::Value { .. }) => self.terms.push(Term::DocField(id, prop)),
                    // A link, or a property we cannot classify. `Edge` is the
                    // safe choice for the unknown case: a fired `DocField` also
                    // dirties an `Edge` dependency, so it covers both readings
                    // without being as coarse as `Doc`.
                    _ => self.terms.push(Term::Edge(id, prop)),
                }
            }
            // Bound subject only: anything about that document.
            (Some(id), None) => self.terms.push(Term::Doc(DocId::parse(id))),
            // Unbound subject with a bound predicate. If the subject variable
            // was type-constrained we can name the class; otherwise there is no
            // term for "this property on anything".
            (None, Some(prop)) => {
                let prop = PropName::new(strip_schema_prefix(prop));
                let class = subject_var
                    .and_then(|var| self.var_classes.get(var))
                    .cloned();
                match class {
                    Some(class) => {
                        self.terms
                            .push(Term::ClassField(class.clone(), prop.clone()));
                        // A bound object with a constrained subject class is a
                        // reverse-edge read: "which C points at this target".
                        //
                        // NOTE: a `Backlink` term is a promise about
                        // before-state — see the same note in `graphql.rs` and
                        // `ChangeSet::complete`. If the resolver cannot see the
                        // old targets of a changed link, the commit must fire
                        // `Everything` instead.
                        if let Some(target) = object.and_then(node_of) {
                            if matches!(
                                self.schema.prop(&class, &prop),
                                Some(PropKind::Link { .. })
                            ) {
                                self.terms.push(Term::Backlink {
                                    target: DocId::parse(target),
                                    prop,
                                    from_class: class,
                                });
                            }
                        }
                    }
                    None => self.coarse("woql triple with an unconstrained subject"),
                }
            }
            (None, None) => self.coarse("woql triple with no bound subject or predicate"),
        }
    }

    /// The "cannot prove it" fallback.
    fn coarse(&mut self, reason: &'static str) {
        if self.cfg.strict {
            self.bypass = Some(reason);
        } else {
            self.terms.push(Term::Everything);
        }
    }
}

/// Which resource a `Using`/`From`/`Into` collection string names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Collection {
    /// Pinned at a commit: immutable, and safe whichever database it is.
    Pinned,
    /// Our own namespace.
    SameNamespace,
    /// Another database, another branch, or a graph descriptor we do not model.
    Elsewhere,
}

fn classify_collection(collection: &str, ns: &Namespace) -> Collection {
    let segments: Vec<&str> = collection.split('/').filter(|s| !s.is_empty()).collect();
    match segments.as_slice() {
        [org, db] => {
            if *org == ns.org && *db == ns.db && ns.branch == "main" {
                Collection::SameNamespace
            } else {
                Collection::Elsewhere
            }
        }
        [org, db, "local", "branch", branch] => {
            if *org == ns.org && *db == ns.db && *branch == ns.branch {
                Collection::SameNamespace
            } else {
                Collection::Elsewhere
            }
        }
        [_, _, "local", "commit", _] => Collection::Pinned,
        _ => Collection::Elsewhere,
    }
}

/// The `node` of a `NodeValue`/`Value`, if it is bound to one.
fn node_of(value: &Value) -> Option<&str> {
    value.get("node").and_then(Value::as_str)
}

/// The `variable` of a `NodeValue`/`Value`, if it is a variable.
fn var_of(value: &Value) -> Option<&str> {
    value.get("variable").and_then(Value::as_str)
}

/// True for the several spellings of `rdf:type`.
fn is_rdf_type(predicate: Option<&str>) -> bool {
    matches!(
        predicate,
        Some("rdf:type") | Some("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
    )
}

/// Drop a `@schema:` prefix from a class or property name so it matches the
/// unprefixed names the schema index uses.
fn strip_schema_prefix(name: &str) -> &str {
    name.strip_prefix("@schema:").unwrap_or(name)
}

/// Every nested JSON value, so a walk does not have to know each node's fields.
fn children(value: &Value) -> Box<dyn Iterator<Item = &Value> + '_> {
    match value {
        Value::Object(map) => Box::new(map.values()),
        Value::Array(items) => Box::new(items.iter()),
        _ => Box::new(std::iter::empty()),
    }
}

/// A canonical, value-free rendering of a WOQL tree, for the shape hash.
///
/// Object keys are sorted (so key order cannot split one shape into two) and
/// the leaves that merely *select* content — a `data` literal, a bound `node` —
/// are replaced by a placeholder, so the same query over a different id is the
/// same projection. Variable names are kept: which variable appears where is
/// structural.
fn shape_of(query: &Value) -> String {
    let mut out = String::new();
    write_shape(query, 0, &mut out);
    out
}

fn write_shape(value: &Value, depth: usize, out: &mut String) {
    if depth > MAX_DEPTH {
        out.push('?');
        return;
    }
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(key);
                out.push(':');
                match (key.as_str(), map.get(key)) {
                    // Content-selecting leaves.
                    ("data", _) | ("node", _) => out.push('?'),
                    (_, Some(child)) => write_shape(child, depth + 1, out),
                    (_, None) => out.push('?'),
                }
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_shape(item, depth + 1, out);
            }
            out.push(']');
        }
        Value::String(s) => {
            out.push('"');
            out.push_str(s);
            out.push('"');
        }
        other => out.push_str(&other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::{plan as analyze, Plan, ReqView};
    use crate::testutil::{cls, doc, prop, schema_fixture};
    use bytes::Bytes;
    use http::{HeaderMap, Method};
    use terminusdb_woql2::prelude::*;
    use terminusdb_woql2::query::Query;
    use terminusdb_woql2::value::{NodeValue, Value as WValue};

    fn ns() -> Namespace {
        Namespace::new("admin", "mydb", "main")
    }

    /// Analyze a query built with the woql2 DSL, so the fixture is the real
    /// wire shape rather than a hand-written guess at it.
    fn analyze_query(query: Query) -> WoqlAnalysis {
        analyze_woql(
            &query.to_woql_json(),
            &ns(),
            &schema_fixture(),
            &AnalyzeConfig::default(),
        )
    }

    fn strict(query: Query) -> WoqlAnalysis {
        analyze_woql(
            &query.to_woql_json(),
            &ns(),
            &schema_fixture(),
            &AnalyzeConfig {
                strict: true,
                ..AnalyzeConfig::default()
            },
        )
    }

    fn plan_query(query: Query) -> Plan {
        let body = Bytes::from(query.to_woql_json().to_string());
        let headers = HeaderMap::new();
        let req = ReqView::new(&Method::POST, "/api/woql/admin/mydb", "", &headers, &body);
        analyze(&req, &schema_fixture(), &AnalyzeConfig::default())
    }

    fn triple_of(subject: WValue, predicate: &str, object: WValue) -> Query {
        Query::Triple(terminusdb_woql2::triple::Triple {
            subject: to_node_value(subject),
            predicate: NodeValue::Node(predicate.to_string()),
            object,
            graph: None,
        })
    }

    fn to_node_value(value: WValue) -> NodeValue {
        match value {
            WValue::Node(n) => NodeValue::Node(n),
            WValue::Variable(v) => NodeValue::Variable(v),
            other => panic!("not a node value: {other:?}"),
        }
    }

    // ------------------------------------------------------ the wire shape

    #[test]
    fn the_wire_shape_is_what_we_parse() {
        // Guards the assumption the whole module rests on. If woql2's
        // serialization changes, this fails loudly instead of silently
        // producing empty read sets.
        let json = triple_of(
            WValue::Node("Person/alice".into()),
            "name",
            WValue::Variable("n".into()),
        )
        .to_woql_json();
        assert_eq!(json["@type"], serde_json::json!("Triple"));
        assert_eq!(json["subject"]["@type"], serde_json::json!("NodeValue"));
        assert_eq!(json["subject"]["node"], serde_json::json!("Person/alice"));
        assert_eq!(json["predicate"]["node"], serde_json::json!("name"));
        assert_eq!(json["object"]["variable"], serde_json::json!("n"));
    }

    #[test]
    fn the_type_whitelist_comes_from_woql2() {
        let names = woql_type_names();
        assert!(names.contains("Triple"), "{} names", names.len());
        assert!(names.contains("Select"));
        assert!(names.contains("UuidV7"), "12.1 additions are included");
        // The classification lists must name real woql2 classes, or a rename
        // would quietly stop classifying.
        for list in [WRITE_TYPES, HISTORY_TYPES, EXTERNAL_TYPES] {
            for ty in list {
                assert!(names.contains(*ty), "{ty} is not a woql2 class any more");
            }
        }
    }

    // ------------------------------------------------------------ read sets

    #[test]
    fn a_bound_subject_depends_on_that_document() {
        let analysis = analyze_query(triple_of(
            WValue::Node("Person/alice".into()),
            "rdf:label",
            WValue::Variable("n".into()),
        ));
        // `rdf:label` is not in the schema, so the property is unclassified and
        // we take the safe `Edge` reading.
        assert!(analysis
            .terms
            .contains(&Term::Edge(doc("Person/alice"), prop("rdf:label"))));
    }

    #[test]
    fn a_bound_subject_and_a_value_property_is_a_doc_field() {
        let analysis = analyze_query(triple_of(
            WValue::Node("Person/alice".into()),
            "name",
            WValue::Variable("n".into()),
        ));
        assert_eq!(
            analysis.terms,
            vec![Term::DocField(doc("Person/alice"), prop("name"))]
        );
    }

    #[test]
    fn a_bound_subject_and_a_link_property_is_an_edge() {
        let analysis = analyze_query(triple_of(
            WValue::Node("Person/alice".into()),
            "employer",
            WValue::Variable("c".into()),
        ));
        assert_eq!(
            analysis.terms,
            vec![Term::Edge(doc("Person/alice"), prop("employer"))]
        );
    }

    #[test]
    fn a_schema_prefixed_predicate_is_unprefixed_first() {
        let analysis = analyze_query(triple_of(
            WValue::Node("Person/alice".into()),
            "@schema:name",
            WValue::Variable("n".into()),
        ));
        assert_eq!(
            analysis.terms,
            vec![Term::DocField(doc("Person/alice"), prop("name"))]
        );
    }

    #[test]
    fn isa_depends_on_membership_and_the_class() {
        let analysis = analyze_query(terminusdb_woql2::isa!(
            terminusdb_woql2::var!("p"),
            "Person"
        ));
        assert!(analysis.terms.contains(&Term::Members(cls("Person"))));
        assert!(analysis.terms.contains(&Term::Class(cls("Person"))));
    }

    #[test]
    fn typeof_depends_on_membership_and_the_class() {
        let analysis = analyze_query(Query::TypeOf(TypeOf {
            value: WValue::Variable("p".into()),
            type_uri: NodeValue::Node("Person".into()),
        }));
        assert!(analysis.terms.contains(&Term::Members(cls("Person"))));
        assert!(analysis.terms.contains(&Term::Class(cls("Person"))));
    }

    #[test]
    fn subsumption_depends_on_the_schema() {
        let analysis = analyze_query(Query::Subsumption(Subsumption {
            child: NodeValue::Variable("c".into()),
            parent: NodeValue::Node("Agent".into()),
        }));
        assert!(analysis.terms.contains(&Term::Schema));
        assert!(analysis.terms.contains(&Term::Class(cls("Agent"))));
    }

    #[test]
    fn read_document_depends_on_that_document() {
        let analysis = analyze_query(terminusdb_woql2::read_doc!(
            terminusdb_woql2::node!("Person/alice"),
            terminusdb_woql2::var!("doc")
        ));
        assert_eq!(analysis.terms, vec![Term::Doc(doc("Person/alice"))]);
    }

    #[test]
    fn an_unbound_read_document_is_a_barrier() {
        let analysis = analyze_query(Query::ReadDocument(ReadDocument {
            identifier: NodeValue::Variable("id".into()),
            document: WValue::Variable("doc".into()),
        }));
        assert!(analysis.terms.contains(&Term::Everything));
    }

    #[test]
    fn a_type_constrained_variable_narrows_an_unbound_subject() {
        // select(p) where p isa Person, p.employer = ?c
        let query = terminusdb_woql2::select!(
            [terminusdb_woql2::var!("p")],
            terminusdb_woql2::and!(
                terminusdb_woql2::isa!(terminusdb_woql2::var!("p"), "Person"),
                triple_of(
                    WValue::Variable("p".into()),
                    "employer",
                    WValue::Variable("c".into())
                )
            )
        );
        let analysis = analyze_query(query);
        assert!(
            analysis
                .terms
                .contains(&Term::ClassField(cls("Person"), prop("employer"))),
            "the IsA binding is what makes this better than a barrier: {:?}",
            analysis.terms
        );
        assert!(!analysis.terms.contains(&Term::Everything));
    }

    #[test]
    fn an_rdf_type_triple_also_binds_the_variable() {
        let query = terminusdb_woql2::and!(
            triple_of(
                WValue::Variable("p".into()),
                "rdf:type",
                WValue::Node("Person".into())
            ),
            triple_of(
                WValue::Variable("p".into()),
                "name",
                WValue::Variable("n".into())
            )
        );
        let analysis = analyze_query(query);
        assert!(analysis.terms.contains(&Term::Members(cls("Person"))));
        assert!(analysis
            .terms
            .contains(&Term::ClassField(cls("Person"), prop("name"))));
    }

    #[test]
    fn a_reverse_edge_read_becomes_a_backlink() {
        // Who works at acme? `p isa Person, p.employer = Company/acme`.
        let query = terminusdb_woql2::and!(
            terminusdb_woql2::isa!(terminusdb_woql2::var!("p"), "Person"),
            triple_of(
                WValue::Variable("p".into()),
                "employer",
                WValue::Node("Company/acme".into())
            )
        );
        let analysis = analyze_query(query);
        assert!(
            analysis.terms.contains(&Term::Backlink {
                target: doc("Company/acme"),
                prop: prop("employer"),
                from_class: cls("Person")
            }),
            "got {:?}",
            analysis.terms
        );
    }

    #[test]
    fn an_unconstrained_subject_is_a_barrier() {
        let analysis = analyze_query(triple_of(
            WValue::Variable("p".into()),
            "name",
            WValue::Variable("n".into()),
        ));
        assert!(analysis.terms.contains(&Term::Everything));
    }

    #[test]
    fn a_path_is_a_barrier_carrying_its_origin() {
        let analysis = analyze_query(Query::Path(terminusdb_woql2::query::Path {
            subject: WValue::Node("Person/alice".into()),
            pattern: terminusdb_woql2::path::PathPattern::Predicate(
                terminusdb_woql2::path::PathPredicate {
                    predicate: Some("employer".into()),
                },
            ),
            object: WValue::Variable("b".into()),
            path: None,
        }));
        assert!(analysis.terms.contains(&Term::Path {
            from: Some(doc("Person/alice")),
            classes: vec![]
        }));
    }

    // -------------------------------------------------------------- writes

    #[test]
    fn a_nested_write_anywhere_makes_the_whole_query_a_write() {
        let write = Query::AddTriple(AddTriple {
            subject: NodeValue::Node("Person/alice".into()),
            predicate: NodeValue::Node("name".into()),
            object: WValue::Data(terminusdb_schema::XSDAnySimpleType::String("bob".into())),
            graph: None,
        });
        // Buried under Select -> Limit -> And.
        let nested = terminusdb_woql2::select!(
            [terminusdb_woql2::var!("x")],
            terminusdb_woql2::limit!(
                10,
                terminusdb_woql2::and!(
                    triple_of(
                        WValue::Node("Person/alice".into()),
                        "name",
                        WValue::Variable("n".into())
                    ),
                    write
                )
            )
        );
        assert!(analyze_query(nested.clone()).is_write);
        assert_eq!(plan_query(nested), Plan::Write);
    }

    #[test]
    fn every_document_write_op_is_a_write() {
        let queries = vec![
            Query::InsertDocument(InsertDocument {
                document: WValue::Variable("d".into()),
                identifier: None,
            }),
            Query::UpdateDocument(UpdateDocument {
                document: WValue::Variable("d".into()),
                identifier: None,
            }),
            Query::DeleteDocument(DeleteDocument {
                identifier: NodeValue::Node("Person/alice".into()),
            }),
        ];
        for query in queries {
            assert!(analyze_query(query).is_write);
        }
    }

    #[test]
    fn commit_introspection_is_a_read_not_a_write() {
        // `AddedTriple` asks what a commit added. Classifying it as a write
        // would be wrong in a way that is easy to miss.
        let analysis = analyze_query(Query::AddedTriple(AddedTriple {
            subject: NodeValue::Variable("s".into()),
            predicate: NodeValue::Variable("p".into()),
            object: WValue::Variable("o".into()),
            graph: None,
        }));
        assert!(!analysis.is_write);
        assert!(analysis.terms.contains(&Term::Everything));
    }

    // ------------------------------------------------------- resource scope

    #[test]
    fn using_a_commit_makes_the_answer_immutable() {
        let query = Query::Using(Using {
            collection: "admin/mydb/local/commit/abc123".into(),
            query: Box::new(triple_of(
                WValue::Node("Person/alice".into()),
                "name",
                WValue::Variable("n".into()),
            )),
        });
        let analysis = analyze_query(query.clone());
        assert!(analysis.immutable);
        match plan_query(query) {
            Plan::Read {
                immutable, terms, ..
            } => {
                assert!(immutable);
                assert!(terms.is_empty(), "nothing can invalidate a pinned read");
            }
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn using_our_own_branch_is_fine() {
        for collection in ["admin/mydb", "admin/mydb/local/branch/main"] {
            let query = Query::Using(Using {
                collection: collection.into(),
                query: Box::new(triple_of(
                    WValue::Node("Person/alice".into()),
                    "name",
                    WValue::Variable("n".into()),
                )),
            });
            let analysis = analyze_query(query);
            assert_eq!(analysis.bypass, None, "{collection} is our namespace");
            assert!(!analysis.immutable);
        }
    }

    #[test]
    fn using_another_resource_is_refused() {
        // The subtle one: terms derived here would be filed under *our*
        // namespace and never fired by the other database's commits.
        for collection in [
            "admin/otherdb",
            "other/mydb",
            "admin/mydb/local/branch/feature",
            "_system",
            "admin/mydb/local/_commits",
        ] {
            let query = Query::Using(Using {
                collection: collection.into(),
                query: Box::new(triple_of(
                    WValue::Node("Person/alice".into()),
                    "name",
                    WValue::Variable("n".into()),
                )),
            });
            assert!(
                analyze_query(query).bypass.is_some(),
                "{collection} must not be cached under our namespace"
            );
        }
    }

    #[test]
    fn an_external_resource_read_is_refused() {
        let analysis = analyze_woql(
            &serde_json::json!({
                "@type": "Get",
                "columns": [],
                "resource": {"@type": "QueryResource", "source": {"@type": "Source", "url": "file:///tmp/x.csv"}, "format": "csv"},
                "has_header": true
            }),
            &ns(),
            &schema_fixture(),
            &AnalyzeConfig::default(),
        );
        assert!(
            analysis.bypass.is_some(),
            "a CSV can change with no commit, so no term could invalidate it"
        );
    }

    // ------------------------------------------------------------- bypasses

    #[test]
    fn an_unrecognized_node_type_degrades_to_a_barrier() {
        let analysis = analyze_woql(
            &serde_json::json!({"@type": "FutureOperator", "query": {"@type": "True"}}),
            &ns(),
            &schema_fixture(),
            &AnalyzeConfig::default(),
        );
        assert!(analysis.terms.contains(&Term::Everything));
    }

    #[test]
    fn an_unrecognized_node_type_is_refused_under_strict() {
        let analysis = analyze_woql(
            &serde_json::json!({"@type": "FutureOperator"}),
            &ns(),
            &schema_fixture(),
            &AnalyzeConfig {
                strict: true,
                ..AnalyzeConfig::default()
            },
        );
        assert!(analysis.bypass.is_some());
    }

    #[test]
    fn strict_mode_refuses_an_unconstrained_triple() {
        let analysis = strict(triple_of(
            WValue::Variable("p".into()),
            "name",
            WValue::Variable("n".into()),
        ));
        assert!(analysis.bypass.is_some());
    }

    #[test]
    fn malformed_json_bypasses() {
        let headers = HeaderMap::new();
        let body = Bytes::from_static(b"{not json");
        let req = ReqView::new(&Method::POST, "/api/woql/admin/mydb", "", &headers, &body);
        assert!(matches!(
            analyze(&req, &schema_fixture(), &AnalyzeConfig::default()),
            Plan::Bypass { .. }
        ));
    }

    #[test]
    fn a_get_with_no_body_bypasses() {
        let headers = HeaderMap::new();
        let body = Bytes::new();
        let req = ReqView::new(&Method::GET, "/api/woql/admin/mydb", "", &headers, &body);
        assert!(matches!(
            analyze(&req, &schema_fixture(), &AnalyzeConfig::default()),
            Plan::Bypass { .. }
        ));
    }

    #[test]
    fn a_woql_node_with_its_own_query_field_is_not_mistaken_for_an_envelope() {
        // Regression: `Using`, `Select`, `Limit` and `Not` all have a `query`
        // field. Unwrapping the client envelope by looking for `query` alone
        // discarded the outer node — which, for `Using`, threw away the whole
        // resource scope and turned an out-of-namespace read into a cacheable
        // one. The `@type` check is what prevents that.
        let outer = Query::Using(Using {
            collection: "admin/otherdb".into(),
            query: Box::new(triple_of(
                WValue::Node("Person/alice".into()),
                "name",
                WValue::Variable("n".into()),
            )),
        });
        let body = Bytes::from(outer.to_woql_json().to_string());
        let headers = HeaderMap::new();
        let req = ReqView::new(&Method::POST, "/api/woql/admin/mydb", "", &headers, &body);
        assert!(
            matches!(
                analyze(&req, &schema_fixture(), &AnalyzeConfig::default()),
                Plan::Bypass { .. }
            ),
            "the Using wrapper must still be seen"
        );

        // And a Select is still analyzed as a Select, not as its subquery.
        let select = terminusdb_woql2::select!(
            [terminusdb_woql2::var!("n")],
            triple_of(
                WValue::Node("Person/alice".into()),
                "name",
                WValue::Variable("n".into())
            )
        );
        let body = Bytes::from(select.to_woql_json().to_string());
        let req = ReqView::new(&Method::POST, "/api/woql/admin/mydb", "", &headers, &body);
        match analyze(&req, &schema_fixture(), &AnalyzeConfig::default()) {
            Plan::Read { terms, .. } => assert_eq!(
                terms,
                vec![Term::DocField(doc("Person/alice"), prop("name"))]
            ),
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn a_query_wrapped_in_a_query_envelope_is_unwrapped() {
        let inner = triple_of(
            WValue::Node("Person/alice".into()),
            "name",
            WValue::Variable("n".into()),
        );
        let body = Bytes::from(serde_json::json!({"query": inner.to_woql_json()}).to_string());
        let headers = HeaderMap::new();
        let req = ReqView::new(&Method::POST, "/api/woql/admin/mydb", "", &headers, &body);
        match analyze(&req, &schema_fixture(), &AnalyzeConfig::default()) {
            Plan::Read { terms, .. } => assert_eq!(
                terms,
                vec![Term::DocField(doc("Person/alice"), prop("name"))]
            ),
            other => panic!("expected a read, got {other:?}"),
        }
    }

    // ----------------------------------------------------------------- shape

    #[test]
    fn the_shape_ignores_literal_values_and_ids() {
        let shape_of_query = |id: &str, value: &str| {
            analyze_query(triple_of(
                WValue::Node(id.into()),
                "name",
                WValue::Data(terminusdb_schema::XSDAnySimpleType::String(value.into())),
            ))
            .shape
        };
        assert_eq!(
            shape_of_query("Person/alice", "Ann"),
            shape_of_query("Person/bob", "Bob"),
            "same projection, different slice"
        );
    }

    #[test]
    fn the_shape_distinguishes_different_structures() {
        let one = analyze_query(triple_of(
            WValue::Node("Person/alice".into()),
            "name",
            WValue::Variable("n".into()),
        ))
        .shape;
        let two = analyze_query(terminusdb_woql2::and!(
            triple_of(
                WValue::Node("Person/alice".into()),
                "name",
                WValue::Variable("n".into())
            ),
            terminusdb_woql2::isa!(terminusdb_woql2::var!("p"), "Person")
        ))
        .shape;
        assert_ne!(one, two);
    }

    #[test]
    fn the_shape_is_stable_under_json_key_order() {
        let a: serde_json::Value = serde_json::from_str(
            r#"{"@type":"Triple","subject":{"@type":"NodeValue","node":"Person/alice"},"predicate":{"@type":"NodeValue","node":"name"},"object":{"@type":"Value","variable":"n"}}"#,
        )
        .unwrap();
        let b: serde_json::Value = serde_json::from_str(
            r#"{"object":{"variable":"n","@type":"Value"},"predicate":{"node":"name","@type":"NodeValue"},"subject":{"node":"Person/alice","@type":"NodeValue"},"@type":"Triple"}"#,
        )
        .unwrap();
        assert_eq!(shape_of(&a), shape_of(&b));
    }

    #[test]
    fn two_woql_reads_differing_only_in_an_id_share_a_shape() {
        let key_of = |id: &str| match plan_query(triple_of(
            WValue::Node(id.into()),
            "name",
            WValue::Variable("n".into()),
        )) {
            Plan::Read { key, .. } => key,
            other => panic!("expected a read, got {other:?}"),
        };
        let alice = key_of("Person/alice");
        let bob = key_of("Person/bob");
        assert_eq!(alice.shape, bob.shape);
        assert_ne!(alice.root, bob.root);
    }
}
