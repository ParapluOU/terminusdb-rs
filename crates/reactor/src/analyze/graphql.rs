//! `/api/graphql` analysis: normalize, inject, extract.
//!
//! GraphQL is the hardest route to cache and the most valuable, because a single
//! query touches many documents and a naive cache has to throw the whole
//! response away when any of them changes. Three things make it tractable:
//!
//! 1. **Normalization.** Two queries that differ only in whitespace, field
//!    order, aliases or fragment structure are the same projection and must
//!    share a [`crate::ids::ShapeId`].
//! 2. **Injection.** A GraphQL response is a tree of anonymous objects: without
//!    `_id` there is no way to say which *document* a node came from, so the
//!    best dependency available would be "any instance of this class". We
//!    therefore add `_id`/`_type` to every object selection ourselves, keep them
//!    in the stored fragment, and strip them from the client's copy. This is
//!    Stellate's hardest-won lesson, solved on our side of the wire instead of
//!    asking clients to change their queries.
//! 3. **Extraction against the schema.** Root fields name classes, nested
//!    fields name properties, and the generated `_p_of_C` / `_path_to_C` fields
//!    name reverse edges and traversals. Each maps to a term — but only where
//!    [`SchemaIndex`] can confirm it. An unknown field means an unknown
//!    dependency, which means `Everything` or a bypass.
//!
//! Printing is done by one traversal with two modes rather than by two
//! printers, so the executed query and the shape text cannot drift apart.

use std::collections::BTreeMap;

use graphql_parser::query::{
    Definition, Mutation, OperationDefinition, Query as GqlQuery, Selection, SelectionSet,
    Subscription, Value as GqlValue,
};

use crate::analyze::{
    AnalyzeConfig, Augment, CanonicalRequest, InjectedField, Plan, ReqView, Rewrite,
};
use crate::ids::{ClassName, DocId, Namespace, PropName};
use crate::schema_index::{PropKind, SchemaIndex};
use crate::term::{Term, ValueKey};

/// Maximum nesting we will walk in a GraphQL document or a response body.
///
/// Both walks are recursive and both inputs are attacker-supplied, so they need
/// a bound. Exceeding it is treated as unanalyzable, never as "fine".
const MAX_DEPTH: usize = 64;

/// Which kind of operation a document contains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationKind {
    /// A read.
    Query,
    /// A write: forwarded, never cached.
    Mutation,
    /// A live stream: forwarded, never cached.
    Subscription,
}

/// The result of analyzing one GraphQL request.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphQlAnalysis {
    /// What kind of operation it is.
    pub kind: OperationKind,
    /// Canonical, value-free rendering, for the shape hash.
    pub shape: String,
    /// The query to execute upstream, with `_id`/`_type` injected. `None` when
    /// nothing needed injecting, in which case the client's bytes are forwarded
    /// unchanged — the byte-faithful path, and the one to prefer.
    pub executed: Option<String>,
    /// Where fields were injected, so the response can be stripped.
    pub injected: Vec<InjectedField>,
    /// The dependencies we could prove.
    pub terms: Vec<Term>,
    /// True when the selection asks only for `_id`/`_type` and link
    /// traversals, so the response contains no field value.
    ///
    /// Such an answer depends on membership and on where the edges point, never
    /// on what the documents contain — which is what lets the ORM's phase-one id
    /// list survive a field update that its phase-two bodies do not. The serve
    /// path reads it to decide whether the response has to be walked for ids at
    /// all; see [`crate::analyze::Augment`].
    pub id_only: bool,
}

/// An argument value, kept in GraphQL's own value domain.
///
/// Deliberately *not* `serde_json::Value`: GraphQL distinguishes an enum value
/// (`ASC`) from a string (`"ASC"`) and an `Int` from a `Float`, and the executed
/// query has to be reprinted faithfully. Collapsing them into JSON would change
/// the meaning of the query we forward.
#[derive(Clone, Debug, PartialEq)]
enum ArgValue {
    Var(String),
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Null,
    Enum(String),
    List(Vec<ArgValue>),
    Object(Vec<(String, ArgValue)>),
}

impl ArgValue {
    fn from_gql(value: &GqlValue<'_, String>, depth: usize) -> Option<ArgValue> {
        if depth > MAX_DEPTH {
            return None;
        }
        Some(match value {
            GqlValue::Variable(name) => ArgValue::Var(name.clone()),
            GqlValue::Int(n) => ArgValue::Int(n.as_i64()?),
            GqlValue::Float(f) => ArgValue::Float(*f),
            GqlValue::String(s) => ArgValue::Str(s.clone()),
            GqlValue::Boolean(b) => ArgValue::Bool(*b),
            GqlValue::Null => ArgValue::Null,
            GqlValue::Enum(e) => ArgValue::Enum(e.clone()),
            GqlValue::List(items) => ArgValue::List(
                items
                    .iter()
                    .map(|i| ArgValue::from_gql(i, depth + 1))
                    .collect::<Option<Vec<_>>>()?,
            ),
            GqlValue::Object(map) => {
                let mut out: Vec<(String, ArgValue)> = map
                    .iter()
                    .map(|(k, v)| ArgValue::from_gql(v, depth + 1).map(|v| (k.clone(), v)))
                    .collect::<Option<Vec<_>>>()?;
                out.sort_by(|a, b| a.0.cmp(&b.0));
                ArgValue::Object(out)
            }
        })
    }

    /// Reprint as GraphQL source.
    fn print(&self, out: &mut String) {
        match self {
            ArgValue::Var(name) => {
                out.push('$');
                out.push_str(name);
            }
            ArgValue::Int(n) => out.push_str(&n.to_string()),
            // `{:?}` rather than `{}` so a whole-numbered float keeps its point
            // (`1.0`, not `1`) and stays a Float in the forwarded query.
            ArgValue::Float(f) => out.push_str(&format!("{f:?}")),
            ArgValue::Str(s) => {
                out.push_str(&serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\"")))
            }
            ArgValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            ArgValue::Null => out.push_str("null"),
            ArgValue::Enum(e) => out.push_str(e),
            ArgValue::List(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.print(out);
                }
                out.push(']');
            }
            ArgValue::Object(fields) => {
                out.push('{');
                for (i, (name, value)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(name);
                    out.push(':');
                    value.print(out);
                }
                out.push('}');
            }
        }
    }

    /// The scalar as JSON, for a value-indexed term. `None` for anything that
    /// is not a literal scalar — a variable's value is not known here, and a
    /// list or object is not an index key.
    fn scalar_json(&self) -> Option<serde_json::Value> {
        Some(match self {
            ArgValue::Int(n) => serde_json::json!(n),
            ArgValue::Float(f) => serde_json::json!(f),
            ArgValue::Str(s) => serde_json::json!(s),
            ArgValue::Bool(b) => serde_json::json!(b),
            // An enum member's identity is its name; indexing on it is the same
            // comparison the server performs.
            ArgValue::Enum(e) => serde_json::json!(e),
            _ => return None,
        })
    }
}

/// A directive, preserved verbatim so the forwarded query keeps its semantics.
#[derive(Clone, Debug, PartialEq)]
struct Directive {
    name: String,
    args: Vec<(String, ArgValue)>,
}

/// Our own selection IR.
///
/// Built from the parsed AST so that fragment spreads are already inlined and so
/// that printing, injection and extraction all walk one simple tree instead of
/// three different AST shapes.
#[derive(Clone, Debug, PartialEq)]
enum Node {
    Field {
        alias: Option<String>,
        name: String,
        args: Vec<(String, ArgValue)>,
        directives: Vec<Directive>,
        children: Vec<Node>,
        /// True when we added this field and the client did not ask for it.
        injected: bool,
    },
    /// An inline fragment. Transparent in the response — its fields appear at
    /// the parent's level — so it adds no path segment when stripping.
    Inline {
        type_condition: Option<String>,
        directives: Vec<Directive>,
        children: Vec<Node>,
    },
}

impl Node {
    fn children(&self) -> &[Node] {
        match self {
            Node::Field { children, .. } | Node::Inline { children, .. } => children,
        }
    }

    fn children_mut(&mut self) -> &mut Vec<Node> {
        match self {
            Node::Field { children, .. } | Node::Inline { children, .. } => children,
        }
    }

    /// The key this node's value appears under in the response.
    fn response_key(&self) -> Option<&str> {
        match self {
            Node::Field { alias, name, .. } => Some(alias.as_deref().unwrap_or(name)),
            Node::Inline { .. } => None,
        }
    }
}

/// Plan a `/api/graphql` request.
pub(super) fn plan(
    req: &ReqView,
    ns: &Namespace,
    schema: &SchemaIndex,
    cfg: &AnalyzeConfig,
) -> Plan {
    // The body is `{"query": "...", "variables": {...}, "operationName": "..."}`.
    let body: serde_json::Value = match serde_json::from_slice(req.body) {
        Ok(body) => body,
        Err(_) => return Plan::bypass("graphql body is not JSON"),
    };
    let query_text = match body.get("query").and_then(|q| q.as_str()) {
        Some(text) => text,
        None => return Plan::bypass("graphql body has no query"),
    };
    let operation_name = body.get("operationName").and_then(|n| n.as_str());

    let analysis = match analyze_graphql(query_text, operation_name, schema, cfg) {
        Ok(analysis) => analysis,
        Err(reason) => return Plan::bypass(reason),
    };

    match analysis.kind {
        OperationKind::Mutation => return Plan::Write,
        // A subscription is a stream, not a response; there is nothing to cache
        // and holding one open through the cache path would be wrong.
        OperationKind::Subscription => return Plan::bypass("graphql subscription"),
        OperationKind::Query => {}
    }

    let canonical = CanonicalRequest::with_params(
        "graphql",
        ns,
        // Variable *values* belong to the identity of the request, not to its
        // shape, so they ride in the body hash (which covers them) while the
        // shape text carries placeholders.
        Vec::new(),
        req.body,
        Some(analysis.shape.clone()),
    );

    let rewrite = analysis.executed.map(|body_query| Rewrite {
        body: rewritten_body(&body, &body_query),
        injected: analysis.injected,
    });

    Plan::Read {
        key: canonical.request_key(ns, None),
        terms: analysis.terms,
        immutable: false,
        rewrite,
        // A selection says "some Person documents", not which. The ids in the
        // response are what make the entry entity-precise, and they are the
        // whole reason `_id` is injected.
        //
        // Except when the response is *made of* ids. Walking it would add a
        // `Doc` term per id returned, which fires on any field change to any of
        // them — so the ORM's phase-one id list would be discarded by an edit it
        // does not show. The dependencies the analyzer derived (membership, the
        // value index, and the traversed properties) already cover everything
        // such an answer can turn on.
        augment: if analysis.id_only {
            Augment::None
        } else {
            Augment::DocIds
        },
        // GraphQL body splicing is deliberately deferred (design doc R4): a
        // GraphQL response is a nested selection, not a sequence of documents,
        // so there is no framing to learn and composition would mean rewriting
        // the body — which invariant 1 forbids.
        decompose: crate::analyze::Decompose::No,
    }
}

/// Rebuild the request body with the executed query substituted in, preserving
/// `variables` and `operationName`.
fn rewritten_body(original: &serde_json::Value, query: &str) -> String {
    let mut map = match original.as_object() {
        Some(map) => map.clone(),
        None => serde_json::Map::new(),
    };
    map.insert(
        "query".to_owned(),
        serde_json::Value::String(query.to_owned()),
    );
    serde_json::Value::Object(map).to_string()
}

/// Normalize a GraphQL query for the shape hash only.
///
/// Exposed for testing and for `/_reactor/explain`.
pub fn normalize_graphql(query_text: &str, schema: &SchemaIndex) -> Result<String, &'static str> {
    analyze_graphql(query_text, None, schema, &AnalyzeConfig::default()).map(|a| a.shape)
}

/// Parse, normalize, inject and extract.
fn analyze_graphql(
    query_text: &str,
    operation_name: Option<&str>,
    schema: &SchemaIndex,
    cfg: &AnalyzeConfig,
) -> Result<GraphQlAnalysis, &'static str> {
    let doc = graphql_parser::query::parse_query::<String>(query_text)
        .map_err(|_| "graphql query does not parse")?;

    // Fragment definitions, for inlining.
    let mut fragments: BTreeMap<String, (String, &SelectionSet<'_, String>)> = BTreeMap::new();
    for definition in &doc.definitions {
        if let Definition::Fragment(fragment) = definition {
            let graphql_parser::query::TypeCondition::On(on) = &fragment.type_condition;
            fragments.insert(fragment.name.clone(), (on.clone(), &fragment.selection_set));
        }
    }

    let operations: Vec<&OperationDefinition<'_, String>> = doc
        .definitions
        .iter()
        .filter_map(|d| match d {
            Definition::Operation(op) => Some(op),
            Definition::Fragment(_) => None,
        })
        .collect();

    let operation = match (operations.len(), operation_name) {
        (0, _) => return Err("graphql document has no operation"),
        (1, _) => operations[0],
        (_, Some(name)) => operations
            .into_iter()
            .find(|op| operation_definition_name(op).as_deref() == Some(name))
            .ok_or("named graphql operation not found")?,
        // Several operations and no name: the server would reject it, and we
        // must not guess which one runs.
        (_, None) => return Err("ambiguous graphql operation"),
    };

    let (kind, selection_set, variable_defs) = match operation {
        OperationDefinition::Query(GqlQuery {
            selection_set,
            variable_definitions,
            ..
        }) => (
            OperationKind::Query,
            selection_set,
            variable_definitions.len(),
        ),
        OperationDefinition::SelectionSet(set) => (OperationKind::Query, set, 0),
        OperationDefinition::Mutation(Mutation { selection_set, .. }) => {
            (OperationKind::Mutation, selection_set, 0)
        }
        OperationDefinition::Subscription(Subscription { selection_set, .. }) => {
            (OperationKind::Subscription, selection_set, 0)
        }
    };

    // A mutation or subscription is never cached, so there is no point
    // normalizing it; bail out with just the classification.
    if kind != OperationKind::Query {
        return Ok(GraphQlAnalysis {
            kind,
            shape: String::new(),
            executed: None,
            injected: Vec::new(),
            terms: Vec::new(),
            id_only: false,
        });
    }

    let mut roots = convert_selection_set(selection_set, &fragments, 0)
        .ok_or("graphql query is too deeply nested to analyze")?;

    // Does this selection reveal any field value? Decided before injection,
    // because injection is only there to make a *value-bearing* response
    // mappable to documents: an answer made of ids and edges needs no ids added
    // to it, so an id-only query is forwarded verbatim and nothing has to be
    // stripped on the way back.
    let id_only = selection_is_id_only(&roots, schema);

    // Inject `_id`/`_type` and record where, so the response can be stripped.
    let mut injected = Vec::new();
    let mut any_injected = false;
    if !id_only {
        let mut path = Vec::new();
        for node in &mut roots {
            inject(node, &mut path, &mut injected, &mut any_injected);
        }
    }

    let shape = print_nodes(&roots, Mode::Shape);
    let executed = if any_injected {
        Some(print_operation(&roots, operation, variable_defs))
    } else {
        None
    };

    let reads = if id_only {
        Reads::IdsOnly
    } else {
        Reads::Values
    };
    let terms = extract_terms(&roots, schema, cfg, reads)?;

    Ok(GraphQlAnalysis {
        kind,
        shape,
        executed,
        injected,
        terms,
        id_only,
    })
}

/// True when every root selection asks only for ids, `_type` and link
/// traversals — no scalar field anywhere.
///
/// Deliberately a proof, not a guess. Anything we cannot classify — an unknown
/// field, an inline fragment on an unknown type, a `_path_to_` traversal, a
/// backlink, an object selection with no fields — answers `false` and the query
/// keeps the value-bearing behaviour. Under-invalidating an id list serves
/// *stale ids*, which is worse than any hit rate it could buy.
fn selection_is_id_only(roots: &[Node], schema: &SchemaIndex) -> bool {
    !roots.is_empty() && roots.iter().all(|node| root_is_id_only(node, schema, 0))
}

fn root_is_id_only(node: &Node, schema: &SchemaIndex, depth: usize) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    match node {
        Node::Inline { children, .. } => children
            .iter()
            .all(|child| root_is_id_only(child, schema, depth + 1)),
        Node::Field {
            name,
            args,
            children,
            ..
        } => {
            // `_count` is a number derived from membership, not a field value,
            // and it puts no ids in the response either way.
            if name == "_count" {
                return args
                    .iter()
                    .any(|(arg, _)| known_class(schema, &ClassName::new(arg.as_str())));
            }
            let class = ClassName::new(name.as_str());
            known_class(schema, &class) && object_is_id_only(children, &class, schema, depth + 1)
        }
    }
}

fn object_is_id_only(
    children: &[Node],
    class: &ClassName,
    schema: &SchemaIndex,
    depth: usize,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    // An object with no selected fields is not a query the server would answer,
    // so we have nothing to prove about it.
    if children.is_empty() {
        return false;
    }
    children.iter().all(|child| match child {
        Node::Inline {
            type_condition,
            children,
            ..
        } => match type_condition {
            Some(on) => {
                let narrowed = ClassName::new(on.as_str());
                known_class(schema, &narrowed)
                    && object_is_id_only(children, &narrowed, schema, depth + 1)
            }
            None => object_is_id_only(children, class, schema, depth + 1),
        },
        Node::Field { name, children, .. } => {
            if INJECTED_FIELDS.contains(&name.as_str()) {
                return true;
            }
            match schema.prop(class, &PropName::new(name.as_str())) {
                // A link: recurse, and the traversal is only id-only if what it
                // leads to is.
                Some(PropKind::Link { range, .. }) => {
                    object_is_id_only(children, range, schema, depth + 1)
                }
                // A scalar property, a nested `_count`, a `_path_to_`, a
                // backlink, or a field the schema does not declare.
                _ => false,
            }
        }
    })
}

/// The operation's name, owned — `OperationDefinition` carries two lifetimes
/// and borrowing through it buys nothing here.
fn operation_definition_name(op: &OperationDefinition<'_, String>) -> Option<String> {
    match op {
        OperationDefinition::Query(q) => q.name.clone(),
        OperationDefinition::Mutation(m) => m.name.clone(),
        OperationDefinition::Subscription(s) => s.name.clone(),
        OperationDefinition::SelectionSet(_) => None,
    }
}

/// Convert a parsed selection set into the IR, inlining named fragments.
///
/// A spread is replaced by an inline fragment carrying the fragment's own type
/// condition, which is semantically identical and removes the need to resolve
/// names again later (notably when recording injection paths, where a spread
/// would make the path ambiguous).
fn convert_selection_set(
    set: &SelectionSet<'_, String>,
    fragments: &BTreeMap<String, (String, &SelectionSet<'_, String>)>,
    depth: usize,
) -> Option<Vec<Node>> {
    if depth > MAX_DEPTH {
        return None;
    }
    let mut out = Vec::new();
    for selection in &set.items {
        match selection {
            Selection::Field(field) => {
                let args = field
                    .arguments
                    .iter()
                    .map(|(name, value)| {
                        ArgValue::from_gql(value, depth).map(|v| (name.clone(), v))
                    })
                    .collect::<Option<Vec<_>>>()?;
                out.push(Node::Field {
                    alias: field.alias.clone(),
                    name: field.name.clone(),
                    args,
                    directives: convert_directives(&field.directives, depth)?,
                    children: convert_selection_set(&field.selection_set, fragments, depth + 1)?,
                    injected: false,
                });
            }
            Selection::InlineFragment(inline) => {
                let type_condition = inline.type_condition.as_ref().map(|tc| {
                    let graphql_parser::query::TypeCondition::On(on) = tc;
                    on.clone()
                });
                out.push(Node::Inline {
                    type_condition,
                    directives: convert_directives(&inline.directives, depth)?,
                    children: convert_selection_set(&inline.selection_set, fragments, depth + 1)?,
                });
            }
            Selection::FragmentSpread(spread) => {
                let (on, set) = fragments.get(&spread.fragment_name)?;
                out.push(Node::Inline {
                    type_condition: Some(on.clone()),
                    directives: convert_directives(&spread.directives, depth)?,
                    children: convert_selection_set(set, fragments, depth + 1)?,
                });
            }
        }
    }
    Some(out)
}

fn convert_directives(
    directives: &[graphql_parser::query::Directive<'_, String>],
    depth: usize,
) -> Option<Vec<Directive>> {
    directives
        .iter()
        .map(|d| {
            let args = d
                .arguments
                .iter()
                .map(|(name, value)| ArgValue::from_gql(value, depth).map(|v| (name.clone(), v)))
                .collect::<Option<Vec<_>>>()?;
            Some(Directive {
                name: d.name.clone(),
                args,
            })
        })
        .collect()
}

/// Fields we add to every object selection.
const INJECTED_FIELDS: [&str; 2] = ["_id", "_type"];

/// Add `_id`/`_type` to every object selection set that lacks them, recording
/// the response path of each addition.
fn inject(
    node: &mut Node,
    path: &mut Vec<String>,
    injected: &mut Vec<InjectedField>,
    any: &mut bool,
) {
    // An inline fragment is transparent in the response, so it contributes no
    // path segment; its children are addressed from the enclosing object.
    let pushed = match node.response_key() {
        Some(key) => {
            path.push(key.to_owned());
            true
        }
        None => false,
    };

    // A selection set with sub-selections is an object (or a list of objects),
    // which means it maps to documents and can carry an id. A leaf is a scalar.
    if !node.children().is_empty() {
        for field in INJECTED_FIELDS {
            let present = node.children().iter().any(|child| {
                matches!(child, Node::Field { name, alias, .. }
                    if name == field && alias.is_none())
            });
            if !present {
                node.children_mut().push(Node::Field {
                    alias: None,
                    name: field.to_owned(),
                    args: Vec::new(),
                    directives: Vec::new(),
                    children: Vec::new(),
                    injected: true,
                });
                injected.push(InjectedField {
                    path: path.clone(),
                    field: field.to_owned(),
                });
                *any = true;
            }
        }
        // Recurse into the client's children only: the two we just appended are
        // leaves, and re-walking them would be harmless but pointless.
        let count = node.children().len();
        let original = count.saturating_sub(
            node.children()
                .iter()
                .filter(|c| matches!(c, Node::Field { injected: true, .. }))
                .count(),
        );
        for i in 0..original {
            let child = &mut node.children_mut()[i];
            inject(child, path, injected, any);
        }
    }

    if pushed {
        path.pop();
    }
}

/// Printing mode.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Faithful: aliases, argument values and directives preserved. This text
    /// is executed, so it must mean exactly what the client asked plus the
    /// injected ids.
    Executed,
    /// Canonical: fields and arguments sorted, aliases dropped, literal values
    /// replaced by a placeholder. This text only identifies a projection.
    Shape,
}

/// Print the whole operation, including its variable definitions, for execution.
fn print_operation(
    roots: &[Node],
    operation: &OperationDefinition<'_, String>,
    variable_defs: usize,
) -> String {
    let mut out = String::from("query");
    if let Some(name) = operation_definition_name(operation) {
        out.push(' ');
        out.push_str(&name);
    }
    if variable_defs > 0 {
        if let OperationDefinition::Query(q) = operation {
            out.push('(');
            for (i, var) in q.variable_definitions.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push('$');
                out.push_str(&var.name);
                out.push(':');
                out.push_str(&var.var_type.to_string());
                if let Some(default) = &var.default_value {
                    out.push('=');
                    if let Some(value) = ArgValue::from_gql(default, 0) {
                        value.print(&mut out);
                    }
                }
            }
            out.push(')');
        }
    }
    out.push_str(&print_nodes(roots, Mode::Executed));
    out
}

fn print_nodes(nodes: &[Node], mode: Mode) -> String {
    let mut out = String::new();
    print_selection_set(nodes, mode, &mut out);
    out
}

fn print_selection_set(nodes: &[Node], mode: Mode, out: &mut String) {
    let mut order: Vec<&Node> = nodes.iter().collect();
    if mode == Mode::Shape {
        // Field order does not affect a GraphQL response's content, only its
        // key order, and we key on the projection — so sort.
        order.sort_by_key(|node| sort_key(node));
    }
    out.push('{');
    for (i, node) in order.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        print_node(node, mode, out);
    }
    out.push('}');
}

fn sort_key(node: &Node) -> String {
    match node {
        Node::Field { name, args, .. } => {
            // Two fields with the same name but different arguments are
            // different selections, so the arguments are part of the key.
            let mut key = format!("0{name}(");
            for (arg, _) in args {
                key.push_str(arg);
                key.push(',');
            }
            key
        }
        Node::Inline { type_condition, .. } => {
            format!("1{}", type_condition.as_deref().unwrap_or(""))
        }
    }
}

fn print_node(node: &Node, mode: Mode, out: &mut String) {
    match node {
        Node::Field {
            alias,
            name,
            args,
            directives,
            children,
            ..
        } => {
            if mode == Mode::Executed {
                if let Some(alias) = alias {
                    out.push_str(alias);
                    out.push(':');
                }
            }
            out.push_str(name);
            if !args.is_empty() {
                let mut args: Vec<&(String, ArgValue)> = args.iter().collect();
                if mode == Mode::Shape {
                    args.sort_by(|a, b| a.0.cmp(&b.0));
                }
                out.push('(');
                for (i, (arg, value)) in args.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(arg);
                    out.push(':');
                    print_arg_value(value, mode, out);
                }
                out.push(')');
            }
            if mode == Mode::Executed {
                for directive in directives {
                    print_directive(directive, out);
                }
            }
            if !children.is_empty() {
                print_selection_set(children, mode, out);
            }
        }
        Node::Inline {
            type_condition,
            directives,
            children,
        } => {
            out.push_str("...");
            if let Some(on) = type_condition {
                out.push_str(" on ");
                out.push_str(on);
            }
            if mode == Mode::Executed {
                for directive in directives {
                    print_directive(directive, out);
                }
            }
            print_selection_set(children, mode, out);
        }
    }
}

fn print_directive(directive: &Directive, out: &mut String) {
    out.push('@');
    out.push_str(&directive.name);
    if !directive.args.is_empty() {
        out.push('(');
        for (i, (name, value)) in directive.args.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(name);
            out.push(':');
            value.print(out);
        }
        out.push(')');
    }
}

/// In shape mode, a literal's *value* is replaced by a placeholder: the same
/// query filtered on two different names is one projection of two different
/// slices. The values still reach the cache key through the body hash.
fn print_arg_value(value: &ArgValue, mode: Mode, out: &mut String) {
    match mode {
        Mode::Executed => value.print(out),
        Mode::Shape => match value {
            // A variable reference is part of the shape (which variable is used
            // where), its value is not.
            ArgValue::Var(name) => {
                out.push('$');
                out.push_str(name);
            }
            ArgValue::List(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    print_arg_value(item, mode, out);
                }
                out.push(']');
            }
            // An object's *keys* shape the query (`filter: {name: {eq: …}}`
            // selects on `name`), so they stay; only the leaves are placeheld.
            ArgValue::Object(fields) => {
                out.push('{');
                for (i, (name, value)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(name);
                    out.push(':');
                    print_arg_value(value, mode, out);
                }
                out.push('}');
            }
            _ => out.push('?'),
        },
    }
}

// --------------------------------------------------------------- extraction

/// How much of a response the dependency set has to cover.
///
/// The two modes exist because the ORM asks two different questions of one
/// endpoint. Phase one asks for *structure* — ids and the edges between them —
/// and phase two batch-loads the bodies through `/api/document`. A structural
/// answer contains no field value, so it cannot depend on one; conflating the
/// two layers is what made an id list die on every field edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reads {
    /// Field values appear in the response. Collections depend on their
    /// members' contents, and the response has to be walked for ids.
    Values,
    /// Only `_id`/`_type` and link traversals appear. The answer depends on
    /// membership and on which documents the edges point at, never on what
    /// those documents contain.
    IdsOnly,
}

/// Walk the selection tree and derive the dependency set.
fn extract_terms(
    roots: &[Node],
    schema: &SchemaIndex,
    cfg: &AnalyzeConfig,
    reads: Reads,
) -> Result<Vec<Term>, &'static str> {
    let mut terms = Vec::new();
    for node in roots {
        extract_root(node, schema, cfg, reads, &mut terms)?;
    }
    terms.sort();
    terms.dedup();
    Ok(terms)
}

fn extract_root(
    node: &Node,
    schema: &SchemaIndex,
    cfg: &AnalyzeConfig,
    reads: Reads,
    out: &mut Vec<Term>,
) -> Result<(), &'static str> {
    match node {
        Node::Inline { children, .. } => {
            for child in children {
                extract_root(child, schema, cfg, reads, out)?;
            }
            Ok(())
        }
        Node::Field {
            name,
            args,
            children,
            ..
        } => {
            // `_count(Person: {})` — the *argument name* is the class, which is
            // why this is handled before the class lookup below. Documented in
            // `docs/terminusdb/graphql-query-reference.md`; an empty filter is
            // answered from the store indexes.
            if name == "_count" {
                let mut matched = false;
                for (arg, value) in args {
                    let class = ClassName::new(arg.as_str());
                    if known_class(schema, &class) {
                        count_terms(&class, Some(value), out);
                        matched = true;
                    }
                }
                if !matched {
                    return coarse(cfg, out, "graphql _count on an unknown class");
                }
                return Ok(());
            }

            let class = ClassName::new(name.as_str());
            if !known_class(schema, &class) {
                return coarse(cfg, out, "unknown graphql root field");
            }

            // `id: "Person/alice"` is the precise case: one document.
            match single_id_arg(args) {
                Some(id) => match reads {
                    Reads::Values => out.push(Term::Doc(id)),
                    // Only the id came back, so what the answer turns on is
                    // whether that document still exists.
                    Reads::IdsOnly => out.push(existence_term(id)),
                },
                None => {
                    let filter = args
                        .iter()
                        .find(|(name, _)| name == "filter")
                        .map(|(_, value)| value);
                    match reads {
                        Reads::Values => collection_terms(&class, filter, out),
                        Reads::IdsOnly => membership_terms(&class, filter, out),
                    }
                }
            }
            extract_object(children, &class, schema, cfg, reads, out, 0)
        }
    }
}

/// The dependency of "a collection of `class`, optionally filtered", **whose
/// members' field values are in the response**.
///
/// Compare [`membership_terms`] and [`count_terms`], which are for answers that
/// do *not* contain field values. The distinction is the whole point and it is
/// easy to erase by accident: a listing that returns documents genuinely
/// depends on their contents, so [`Term::Class`] belongs here and nowhere else.
/// Removing it from a value-bearing collection would serve stale bodies; adding
/// it to a count or an id list makes both die on every unrelated field edit,
/// which is the bug this split fixes. Do not "simplify" the three into one.
fn collection_terms(class: &ClassName, filter: Option<&ArgValue>, out: &mut Vec<Term>) {
    match filter_shape(filter) {
        // `filter: {p: {eq: v}}` is exactly a value index. The members' own
        // field values are covered by the response augmentation, which adds a
        // `Doc` term per id actually returned.
        FilterShape::Equality(prop, value) => out.push(Term::Index {
            class: class.clone(),
            prop,
            value,
        }),
        // Some other filter shape (ranges, `and`, substring, …). We cannot name
        // the bucket, but every member of the result is an instance of the
        // class, so the class is a sound dependency.
        FilterShape::Opaque | FilterShape::Unfiltered => {
            out.push(Term::Members(class.clone()));
            out.push(Term::Class(class.clone()));
        }
    }
}

/// The dependency of "*which* `class` documents match this filter", for an
/// answer that reveals nothing about their contents: an id list or a count.
///
/// The unfiltered case is the one that matters. A total count is a statement
/// about how many instances exist, so only a create or a delete can change it —
/// which is exactly [`Term::Members`]. Adding [`Term::Class`] there (as the
/// collection form must) would discard the entry on every field change to any
/// instance, so paging metadata would thrash on every write, which is precisely
/// the moment it most needs to survive.
fn membership_terms(class: &ClassName, filter: Option<&ArgValue>, out: &mut Vec<Term>) {
    match filter_shape(filter) {
        // Membership of the bucket, and nothing else. A document moving in or
        // out fires this on both the old and the new value, so a count cannot
        // miss a crossing.
        FilterShape::Equality(prop, value) => out.push(Term::Index {
            class: class.clone(),
            prop,
            value,
        }),
        FilterShape::Unfiltered => out.push(Term::Members(class.clone())),
        // We cannot name the bucket, so we cannot say which field values move a
        // document into or out of it. Stay conservative.
        FilterShape::Opaque => {
            out.push(Term::Members(class.clone()));
            out.push(Term::Class(class.clone()));
        }
    }
}

/// The dependency of a `_count`. See [`membership_terms`] — a count is the
/// purest membership question there is.
fn count_terms(class: &ClassName, filter: Option<&ArgValue>, out: &mut Vec<Term>) {
    membership_terms(class, filter, out)
}

/// "Does this document still exist", for an answer that carries its id and
/// nothing else. An id with no class prefix has no membership we could name, so
/// it falls back to the coarser whole-document term.
fn existence_term(id: DocId) -> Term {
    match id.class() {
        Some(class) => Term::Members(class),
        None => Term::Doc(id),
    }
}

/// What a filter argument lets us say about the set it selects.
enum FilterShape {
    /// Absent, or `{}`: the whole class. An empty filter object is what the
    /// `_count(Person: {})` form sends, so treating it as "some filter we do
    /// not understand" would make every unfiltered count conservative.
    Unfiltered,
    /// `{prop: {eq: value}}`: one nameable value-index bucket.
    Equality(PropName, ValueKey),
    /// Anything else: we cannot name the bucket.
    Opaque,
}

fn filter_shape(filter: Option<&ArgValue>) -> FilterShape {
    let filter = match filter {
        Some(filter) => filter,
        None => return FilterShape::Unfiltered,
    };
    if matches!(filter, ArgValue::Object(fields) if fields.is_empty()) {
        return FilterShape::Unfiltered;
    }
    match equality_filter(filter) {
        Some((prop, value)) => FilterShape::Equality(prop, value),
        None => FilterShape::Opaque,
    }
}

/// Recognize `{prop: {eq: scalar}}` and nothing else.
fn equality_filter(filter: &ArgValue) -> Option<(PropName, ValueKey)> {
    let fields = match filter {
        ArgValue::Object(fields) => fields,
        _ => return None,
    };
    let [(prop, inner)] = &fields[..] else {
        return None;
    };
    let inner = match inner {
        ArgValue::Object(inner) => inner,
        _ => return None,
    };
    let [(op, value)] = &inner[..] else {
        return None;
    };
    if op != "eq" {
        return None;
    }
    let json = value.scalar_json()?;
    Some((PropName::new(prop.as_str()), ValueKey::of(&json)))
}

/// `id: "Person/alice"` on a root field.
fn single_id_arg(args: &[(String, ArgValue)]) -> Option<DocId> {
    args.iter().find_map(|(name, value)| {
        if name != "id" {
            return None;
        }
        match value {
            ArgValue::Str(s) => Some(DocId::parse(s)),
            _ => None,
        }
    })
}

/// Walk the fields selected on an object of class `class`.
#[allow(clippy::too_many_arguments)]
fn extract_object(
    children: &[Node],
    class: &ClassName,
    schema: &SchemaIndex,
    cfg: &AnalyzeConfig,
    reads: Reads,
    out: &mut Vec<Term>,
    depth: usize,
) -> Result<(), &'static str> {
    if depth > MAX_DEPTH {
        return Err("graphql selection too deep");
    }
    for child in children {
        match child {
            Node::Inline {
                type_condition,
                children,
                ..
            } => {
                // `... on Employee { … }` narrows the class for its own fields.
                let narrowed = type_condition
                    .as_ref()
                    .map(|on| ClassName::new(on.as_str()))
                    .filter(|c| known_class(schema, c))
                    .unwrap_or_else(|| class.clone());
                extract_object(children, &narrowed, schema, cfg, reads, out, depth + 1)?;
            }
            Node::Field {
                name,
                args,
                children,
                ..
            } => {
                if INJECTED_FIELDS.contains(&name.as_str()) {
                    // Our own ids, and `_type`, depend on the document itself,
                    // which the response augmentation already covers.
                    continue;
                }
                if name == "_count" {
                    // A nested count is still a count: membership only. See
                    // `count_terms` — the class term that used to be here made
                    // every field edit to any instance discard it.
                    let filter = args
                        .iter()
                        .find(|(name, _)| name == "filter")
                        .map(|(_, value)| value);
                    count_terms(class, filter, out);
                    continue;
                }
                // `_path_to_C(path: "film,<film")`: a traversal. We cannot prove
                // which hops it takes, so the dependency is every class it could
                // reach, which is the target class and its subclasses.
                if let Some(target) = name.strip_prefix("_path_to_") {
                    let target = ClassName::new(target);
                    if !known_class(schema, &target) {
                        coarse(cfg, out, "graphql path to an unknown class")?;
                        continue;
                    }
                    out.push(Term::Path {
                        from: None,
                        classes: schema.descendants(&target),
                    });
                    extract_object(children, &target, schema, cfg, reads, out, depth + 1)?;
                    continue;
                }
                // `_employer_of_Person`: the reverse edge.
                //
                // NOTE: emitting a `Backlink` term is a *promise about
                // before-state*. `Term::subsumes` matches `Backlink` only
                // against a fired `Backlink`, which the capture pipeline can
                // only produce when it knows the old targets of a changed link.
                // A resolver that cannot see the before state MUST set
                // `ChangeSet::complete = false` (see its docs) so the commit
                // fires `Everything` instead. If that obligation is ever
                // broken, these entries go stale silently.
                if let Some((prop, from_class)) = SchemaIndex::parse_backlink_field(name) {
                    if !known_class(schema, &from_class)
                        || schema.prop(&from_class, &prop).is_none()
                    {
                        coarse(cfg, out, "graphql backlink field not in the schema")?;
                        continue;
                    }
                    out.push(Term::ClassField(from_class.clone(), prop.clone()));
                    // The per-target `Backlink{target, prop, from_class}` terms
                    // need the parent ids, which only the response has; the
                    // augmented `Doc` terms cover the pointing documents, and
                    // `ClassField` covers the property on the pointing class.
                    extract_object(children, &from_class, schema, cfg, reads, out, depth + 1)?;
                    continue;
                }

                let prop = PropName::new(name.as_str());
                match schema.prop(class, &prop) {
                    Some(PropKind::Link { range, .. }) => {
                        // The traversal itself: where this property's edges
                        // point. `ClassField` rather than a response-derived
                        // `Edge(parent_id, prop)` because an id-only selection
                        // need not select `_id` at every level, so a walk of the
                        // response cannot always name the parent — and a
                        // traversal term we failed to emit serves stale ids.
                        out.push(Term::ClassField(class.clone(), prop));
                        let range = range.clone();
                        if reads == Reads::IdsOnly {
                            // The nested ids are this edge's targets, so a
                            // target that is deleted (or created with a new
                            // identity) changes the answer without the parent's
                            // property moving.
                            out.push(Term::Members(range.clone()));
                        }
                        // A nested link may carry its own filter.
                        if let Some(filter) = args
                            .iter()
                            .find(|(name, _)| name == "filter")
                            .map(|(_, v)| v)
                        {
                            match reads {
                                Reads::Values => collection_terms(&range, Some(filter), out),
                                Reads::IdsOnly => membership_terms(&range, Some(filter), out),
                            }
                        }
                        extract_object(children, &range, schema, cfg, reads, out, depth + 1)?;
                    }
                    Some(PropKind::Value { .. }) => {
                        out.push(Term::ClassField(class.clone(), prop));
                    }
                    // A field the schema does not declare: either our index is
                    // stale or this is a generated field we do not model.
                    None => coarse(cfg, out, "unknown graphql field")?,
                }
            }
        }
    }
    Ok(())
}

/// True when the schema knows this class. With an *empty* index nothing is
/// known, which is what forces the coarse path.
fn known_class(schema: &SchemaIndex, class: &ClassName) -> bool {
    schema.classes().any(|c| c == class)
}

/// The "we cannot prove it" fallback: a barrier term, or a bypass under strict.
fn coarse(
    cfg: &AnalyzeConfig,
    out: &mut Vec<Term>,
    reason: &'static str,
) -> Result<(), &'static str> {
    if cfg.strict {
        return Err(reason);
    }
    out.push(Term::Everything);
    Ok(())
}

// -------------------------------------------------------------- augmentation

/// Add a `Doc` term for every id the response mentions.
///
/// This is what turns a class-coarse entry into an entity-precise one, and it is
/// the reason `_id` is injected in the first place. Both spellings are
/// recognized: `_id` is the GraphQL field, `@id` is the document API's.
///
/// If the body cannot be parsed, a [`Term::Everything`] is pushed instead of
/// nothing. Returning silently would leave the caller holding an *incomplete*
/// read set, which is the one failure this module exists to prevent.
pub fn augment_terms_from_response(body: &[u8], terms: &mut Vec<Term>) {
    let value: serde_json::Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => {
            terms.push(Term::Everything);
            terms.sort();
            terms.dedup();
            return;
        }
    };
    collect_ids(&value, 0, terms);
    terms.sort();
    terms.dedup();
}

fn collect_ids(value: &serde_json::Value, depth: usize, out: &mut Vec<Term>) {
    if depth > MAX_DEPTH {
        // Deeper than we will walk: we cannot claim to have seen every id.
        out.push(Term::Everything);
        return;
    }
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                if (key == "_id" || key == "@id") && child.is_string() {
                    if let Some(id) = child.as_str() {
                        let id = DocId::parse(id);
                        if !id.as_str().is_empty() {
                            out.push(Term::Doc(id));
                        }
                    }
                }
                collect_ids(child, depth + 1, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_ids(item, depth + 1, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::{plan as analyze, Plan, ReqView};
    use crate::testutil::{cls, doc, prop, schema_fixture};
    use bytes::Bytes;
    use http::{HeaderMap, Method};
    use serde_json::json;

    fn analysis(query: &str) -> GraphQlAnalysis {
        analyze_graphql(query, None, &schema_fixture(), &AnalyzeConfig::default())
            .expect("query should analyze")
    }

    fn shape(query: &str) -> String {
        analysis(query).shape
    }

    fn terms(query: &str) -> Vec<Term> {
        analysis(query).terms
    }

    fn plan_body(body: serde_json::Value) -> Plan {
        let bytes = Bytes::from(body.to_string());
        let headers = HeaderMap::new();
        let req = ReqView::new(
            &Method::POST,
            "/api/graphql/admin/mydb",
            "",
            &headers,
            &bytes,
        );
        analyze(&req, &schema_fixture(), &AnalyzeConfig::default())
    }

    // ------------------------------------------------------- normalization

    #[test]
    fn normalization_is_stable_under_whitespace_and_comments() {
        let a = shape("query { Person { name } }");
        let b = shape("# a comment\nquery{\n  Person {\n    name   # trailing\n  }\n}\n");
        assert_eq!(a, b);
    }

    #[test]
    fn normalization_is_stable_under_field_reordering() {
        assert_eq!(
            shape("query { Person { name favourite } }"),
            shape("query { Person { favourite name } }")
        );
    }

    #[test]
    fn normalization_is_stable_under_argument_reordering() {
        assert_eq!(
            shape(r#"query { Person(offset: 1, limit: 2) { name } }"#),
            shape(r#"query { Person(limit: 2, offset: 1) { name } }"#)
        );
    }

    #[test]
    fn normalization_drops_aliases() {
        assert_eq!(
            shape("query { Person { nickname: name } }"),
            shape("query { Person { name } }")
        );
    }

    #[test]
    fn normalization_inlines_fragment_definitions() {
        let with_fragment =
            shape("query { Person { ...bits } } fragment bits on Person { name favourite }");
        let inlined = shape("query { Person { ... on Person { name favourite } } }");
        assert_eq!(with_fragment, inlined);
    }

    #[test]
    fn normalization_placeholds_literal_values_but_keeps_filter_keys() {
        let alice = shape(r#"query { Person(filter: {name: {eq: "Ann"}}) { name } }"#);
        let bob = shape(r#"query { Person(filter: {name: {eq: "Bob"}}) { name } }"#);
        assert_eq!(alice, bob, "same projection, different slice");

        let by_favourite = shape(r#"query { Person(filter: {favourite: {eq: "Red"}}) { name } }"#);
        assert_ne!(
            alice, by_favourite,
            "filtering on a different property is a different shape"
        );
        assert!(alice.contains("name:{eq:?}"), "got {alice}");
    }

    #[test]
    fn normalization_keeps_variable_names_in_the_shape() {
        let one = shape(r#"query Q($n: String) { Person(filter: {name: {eq: $n}}) { name } }"#);
        let two = shape(r#"query Q($m: String) { Person(filter: {name: {eq: $m}}) { name } }"#);
        assert_ne!(
            one, two,
            "which variable feeds which argument is structural"
        );
    }

    #[test]
    fn a_different_selection_is_a_different_shape() {
        assert_ne!(
            shape("query { Person { name } }"),
            shape("query { Person { name favourite } }")
        );
    }

    #[test]
    fn an_unparseable_query_is_rejected() {
        assert!(analyze_graphql(
            "query { Person { name ",
            None,
            &schema_fixture(),
            &AnalyzeConfig::default()
        )
        .is_err());
    }

    // ------------------------------------------------------------ injection

    #[test]
    fn injection_adds_ids_to_every_object_selection_and_records_the_paths() {
        let a = analysis("query { Person { name address { city } } }");
        let executed = a.executed.expect("injection must rewrite the query");
        assert_eq!(
            executed,
            "query{Person{name address{city _id _type} _id _type}}"
        );
        let mut paths: Vec<(Vec<String>, String)> = a
            .injected
            .iter()
            .map(|f| (f.path.clone(), f.field.clone()))
            .collect();
        paths.sort();
        assert_eq!(
            paths,
            vec![
                (vec!["Person".to_owned()], "_id".to_owned()),
                (vec!["Person".to_owned()], "_type".to_owned()),
                (
                    vec!["Person".to_owned(), "address".to_owned()],
                    "_id".to_owned()
                ),
                (
                    vec!["Person".to_owned(), "address".to_owned()],
                    "_type".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn injection_uses_aliases_in_the_recorded_path() {
        // The response key is the alias, so that is what has to be stripped.
        let a = analysis("query { folk: Person { home: address { city } } }");
        let paths: Vec<Vec<String>> = a.injected.iter().map(|f| f.path.clone()).collect();
        assert!(paths.contains(&vec!["folk".to_owned()]));
        assert!(paths.contains(&vec!["folk".to_owned(), "home".to_owned()]));
    }

    #[test]
    fn injection_is_skipped_where_the_client_already_asked() {
        let a = analysis("query { Person { _id _type name } }");
        assert!(
            a.executed.is_none(),
            "nothing to inject means the client's bytes are forwarded unchanged"
        );
        assert!(a.injected.is_empty());
    }

    #[test]
    fn injection_adds_only_the_missing_field() {
        let a = analysis("query { Person { _id name } }");
        assert_eq!(a.injected.len(), 1);
        assert_eq!(a.injected[0].field, "_type");
        assert_eq!(a.executed.as_deref(), Some("query{Person{_id name _type}}"));
    }

    #[test]
    fn injection_leaves_scalar_leaves_alone() {
        let a = analysis("query { _count(Person: {}) }");
        assert!(
            a.executed.is_none(),
            "a scalar field has no selection set to inject into"
        );
    }

    #[test]
    fn the_executed_query_preserves_aliases_arguments_and_directives() {
        let a = analysis(
            r#"query Q($n: String!) { folk: Person(filter: {name: {eq: $n}}) @include(if: true) { name } }"#,
        );
        let executed = a.executed.expect("ids are injected");
        assert!(
            executed.starts_with("query Q($n:String!)"),
            "got {executed}"
        );
        assert!(executed.contains("folk:Person(filter:{name:{eq:$n}})"));
        assert!(executed.contains("@include(if:true)"));
        assert!(executed.contains("_id"));
    }

    #[test]
    fn the_executed_query_keeps_float_literals_floating() {
        let a = analysis(r#"query { Person(filter: {salary: {eq: 1.0}}) { name } }"#);
        let executed = a.executed.expect("ids are injected");
        assert!(
            executed.contains("1.0"),
            "a Float must not be reprinted as an Int: {executed}"
        );
    }

    #[test]
    fn the_rewritten_body_keeps_the_variables() {
        let body = json!({
            "query": "query Q($n: String) { Person(filter: {name: {eq: $n}}) { name } }",
            "variables": {"n": "Ann"},
            "operationName": "Q"
        });
        match plan_body(body) {
            Plan::Read { rewrite, .. } => {
                let rewrite = rewrite.expect("injection rewrites the request");
                let parsed: serde_json::Value = serde_json::from_str(&rewrite.body).unwrap();
                assert_eq!(parsed["variables"]["n"], json!("Ann"));
                assert_eq!(parsed["operationName"], json!("Q"));
                assert!(parsed["query"].as_str().unwrap().contains("_id"));
            }
            other => panic!("expected a read, got {other:?}"),
        }
    }

    // ----------------------------------------------------------- extraction

    #[test]
    fn an_equality_filter_is_a_value_index() {
        assert!(
            terms(r#"query { Person(filter: {name: {eq: "Ann"}}) { name } }"#).contains(
                &Term::Index {
                    class: cls("Person"),
                    prop: prop("name"),
                    value: ValueKey::of(&json!("Ann"))
                }
            )
        );
    }

    #[test]
    fn another_filter_shape_falls_back_to_the_class() {
        let terms = terms(r#"query { Person(filter: {name: {regex: "A.*"}}) { name } }"#);
        assert!(terms.contains(&Term::Class(cls("Person"))));
        assert!(!terms.iter().any(|t| matches!(t, Term::Index { .. })));
    }

    #[test]
    fn no_filter_depends_on_membership_and_the_class() {
        let terms = terms("query { Person { name } }");
        assert!(terms.contains(&Term::Members(cls("Person"))));
        assert!(terms.contains(&Term::Class(cls("Person"))));
    }

    #[test]
    fn an_id_argument_is_the_precise_single_document_case() {
        let terms = terms(r#"query { Person(id: "Person/alice") { name } }"#);
        assert!(terms.contains(&Term::Doc(doc("Person/alice"))));
        assert!(
            !terms.contains(&Term::Members(cls("Person"))),
            "one named document does not depend on class membership"
        );
    }

    #[test]
    fn nested_value_and_link_fields_become_class_fields() {
        let terms = terms("query { Person { name employer { name } } }");
        // The term names the class that was *selected*, not the ancestor that
        // declares the property. That is both tighter and still sound: `fire`
        // emits `DocField(Person/x, name)` for the change, and a
        // `ClassField(Person, name)` dependency is dirtied by any `DocField`
        // whose document is a Person or a subclass of one.
        assert!(terms.contains(&Term::ClassField(cls("Person"), prop("name"))));
        assert!(terms.contains(&Term::ClassField(cls("Person"), prop("employer"))));
        // Descending into the link moves the class context to Company.
        assert!(terms.contains(&Term::ClassField(cls("Company"), prop("name"))));
    }

    #[test]
    fn a_nested_filter_constrains_the_target_class() {
        let terms =
            terms(r#"query { Company { departments(filter: {head: {eq: "x"}}) { _id } } }"#);
        assert!(terms.contains(&Term::Index {
            class: cls("Department"),
            prop: prop("head"),
            value: ValueKey::of(&json!("x"))
        }));
    }

    #[test]
    fn a_backlink_field_depends_on_the_pointing_property() {
        let terms = terms("query { Company { _employer_of_Person { name } } }");
        assert!(terms.contains(&Term::ClassField(cls("Person"), prop("employer"))));
    }

    #[test]
    fn an_unknown_backlink_field_degrades() {
        let terms = terms("query { Company { _nonsense_of_Person { name } } }");
        assert!(terms.contains(&Term::Everything));
    }

    #[test]
    fn a_path_field_becomes_a_path_term_over_the_target_and_its_subclasses() {
        let terms = terms(r#"query { Person { _path_to_Agent(path: "employer") { name } } }"#);
        let path = terms
            .iter()
            .find_map(|t| match t {
                Term::Path { classes, .. } => Some(classes.clone()),
                _ => None,
            })
            .expect("a path term");
        assert!(path.contains(&cls("Agent")));
        assert!(
            path.contains(&cls("Employee")),
            "a path to Agent can land on any subclass of Agent"
        );
    }

    #[test]
    fn an_unfiltered_count_depends_on_membership_and_nothing_else() {
        // The regression this test exists for: a total count is a statement
        // about how many instances exist, so a field edit to one of them must
        // not discard it. `Class(Person)` here made paging metadata die on
        // every write.
        assert_eq!(
            terms("query { _count(Person: {}) }"),
            vec![Term::Members(cls("Person"))]
        );
    }

    #[test]
    fn a_filtered_count_is_a_value_index_alone() {
        let terms = terms(r#"query { _count(Person: {name: {eq: "Ann"}}) }"#);
        assert_eq!(
            terms,
            vec![Term::Index {
                class: cls("Person"),
                prop: prop("name"),
                value: ValueKey::of(&json!("Ann"))
            }],
            "a document moving in or out of the bucket fires the index on both \
             the old and the new value, so nothing coarser is needed"
        );
    }

    #[test]
    fn a_count_whose_bucket_we_cannot_name_stays_conservative() {
        // A filter shape we do not model: we cannot say which field values move
        // a document in or out, so the class dependency has to stay.
        let terms = terms(r#"query { _count(Person: {name: {regex: "A.*"}}) }"#);
        assert!(terms.contains(&Term::Members(cls("Person"))));
        assert!(terms.contains(&Term::Class(cls("Person"))), "got {terms:?}");
    }

    #[test]
    fn a_nested_count_depends_on_membership_and_nothing_else() {
        // Rooted at one document so the root contributes no class term, which
        // leaves the nested `_count`'s own dependencies observable.
        let terms = terms(r#"query { Person(id: "Person/alice") { friends { _count } } }"#);
        assert!(terms.contains(&Term::Members(cls("Person"))));
        assert!(
            !terms.contains(&Term::Class(cls("Person"))),
            "a nested count must not depend on every instance's fields: {terms:?}"
        );
    }

    #[test]
    fn a_collection_that_returns_documents_still_depends_on_field_values() {
        // The other half of the count fix: a listing whose *bodies* are in the
        // response genuinely depends on their contents.
        let terms = terms("query { Person { name } }");
        assert!(terms.contains(&Term::Members(cls("Person"))));
        assert!(terms.contains(&Term::Class(cls("Person"))), "got {terms:?}");
    }

    // ------------------------------------------------------------- id-only

    #[test]
    fn an_id_only_selection_does_not_depend_on_field_values() {
        let a = analysis("query { Person { _id friends { _id } } }");
        assert!(a.id_only);
        assert!(
            !a.terms.contains(&Term::Class(cls("Person"))),
            "an answer made of ids cannot turn on a field value: {:?}",
            a.terms
        );
        assert!(a.terms.contains(&Term::Members(cls("Person"))));
        assert!(
            a.terms
                .contains(&Term::ClassField(cls("Person"), prop("friends"))),
            "the traversal is still a dependency: {:?}",
            a.terms
        );
        assert!(
            a.executed.is_none(),
            "an id-only query needs no injection, so it is forwarded verbatim"
        );
        assert!(a.injected.is_empty());
    }

    #[test]
    fn an_id_only_selection_still_dies_on_a_deleted_edge_target() {
        // The nested ids are the edge's targets, so a target that is deleted (or
        // recreated with a new identity) changes the answer even though the
        // parent's property did not move.
        let terms = terms("query { Person { _id address { _id } } }");
        assert!(
            terms.contains(&Term::Members(cls("Address"))),
            "got {terms:?}"
        );
    }

    #[test]
    fn one_scalar_field_anywhere_restores_value_dependence() {
        for query in [
            "query { Person { _id name } }",
            "query { Person { _id friends { _id name } } }",
            "query { Person { _id nonexistent } }",
            "query { Person { _id _path_to_Company(path: \"employer\") { _id } } }",
        ] {
            let a = analyze_graphql(query, None, &schema_fixture(), &AnalyzeConfig::default())
                .expect("analyzable");
            assert!(
                !a.id_only,
                "{query} must not be treated as id-only ({:?})",
                a.terms
            );
        }
    }

    #[test]
    fn an_id_only_filtered_selection_is_a_value_index() {
        let a = analysis(r#"query { Person(filter: {name: {eq: "Ann"}}) { _id } }"#);
        assert!(a.id_only);
        assert_eq!(
            a.terms,
            vec![Term::Index {
                class: cls("Person"),
                prop: prop("name"),
                value: ValueKey::of(&json!("Ann"))
            }]
        );
    }

    #[test]
    fn an_id_only_by_id_selection_is_an_existence_question() {
        let a = analysis(r#"query { Person(id: "Person/alice") { _id } }"#);
        assert!(a.id_only);
        assert_eq!(a.terms, vec![Term::Members(cls("Person"))]);
    }

    #[test]
    fn an_id_only_selection_skips_the_response_augmentation() {
        match plan_body(json!({"query": "query { Person { _id friends { _id } } }"})) {
            Plan::Read { augment, terms, .. } => {
                assert_eq!(
                    augment,
                    Augment::None,
                    "walking the response would add a Doc term per id, which is \
                     exactly the field dependence we just removed"
                );
                assert!(terms.contains(&Term::Members(cls("Person"))));
            }
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn an_inline_fragment_narrows_the_class() {
        let terms = terms("query { Person { ... on Employee { salary } } }");
        assert!(terms.contains(&Term::ClassField(cls("Employee"), prop("salary"))));
    }

    #[test]
    fn an_unknown_root_field_degrades_to_everything() {
        assert!(terms("query { Nonexistent { name } }").contains(&Term::Everything));
    }

    #[test]
    fn an_unknown_nested_field_degrades_to_everything() {
        assert!(terms("query { Person { nonexistent } }").contains(&Term::Everything));
    }

    #[test]
    fn strict_mode_bypasses_instead_of_degrading() {
        let cfg = AnalyzeConfig {
            strict: true,
            ..AnalyzeConfig::default()
        };
        for query in [
            "query { Nonexistent { name } }",
            "query { Person { nonexistent } }",
            "query { _count(Nonexistent: {}) }",
        ] {
            assert!(
                analyze_graphql(query, None, &schema_fixture(), &cfg).is_err(),
                "{query} must be refused under strict"
            );
        }
    }

    #[test]
    fn with_no_schema_every_field_is_unknown() {
        let a = analyze_graphql(
            "query { Person { name } }",
            None,
            &SchemaIndex::default(),
            &AnalyzeConfig::default(),
        )
        .unwrap();
        assert_eq!(a.terms, vec![Term::Everything]);
    }

    // ---------------------------------------------------------------- plans

    #[test]
    fn a_mutation_is_a_write() {
        assert_eq!(
            plan_body(json!({"query": "mutation { insertPerson(name: \"x\") { _id } }"})),
            Plan::Write
        );
    }

    #[test]
    fn a_subscription_is_bypassed() {
        assert!(matches!(
            plan_body(json!({"query": "subscription { _ChangeSet { _id } }"})),
            Plan::Bypass { .. }
        ));
    }

    #[test]
    fn a_query_is_a_read_that_needs_augmentation() {
        match plan_body(json!({"query": "query { Person { name } }"})) {
            Plan::Read {
                augment,
                decompose: crate::analyze::Decompose::No,
                immutable,
                terms,
                ..
            } => {
                assert_eq!(augment, Augment::DocIds);
                assert!(!immutable);
                assert!(terms.contains(&Term::Members(cls("Person"))));
            }
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn a_non_json_or_queryless_body_is_bypassed() {
        for body in [json!({"notquery": 1}), json!("just a string"), json!([])] {
            assert!(matches!(plan_body(body), Plan::Bypass { .. }));
        }
        let headers = HeaderMap::new();
        let bytes = Bytes::from_static(b"not json at all");
        let req = ReqView::new(
            &Method::POST,
            "/api/graphql/admin/mydb",
            "",
            &headers,
            &bytes,
        );
        assert!(matches!(
            analyze(&req, &schema_fixture(), &AnalyzeConfig::default()),
            Plan::Bypass { .. }
        ));
    }

    #[test]
    fn two_operations_without_a_name_are_ambiguous() {
        assert!(analyze_graphql(
            "query A { Person { name } } query B { Company { name } }",
            None,
            &schema_fixture(),
            &AnalyzeConfig::default()
        )
        .is_err());
    }

    #[test]
    fn a_named_operation_is_selected() {
        let a = analyze_graphql(
            "query A { Person { name } } query B { Company { name } }",
            Some("B"),
            &schema_fixture(),
            &AnalyzeConfig::default(),
        )
        .unwrap();
        assert!(a.terms.contains(&Term::Members(cls("Company"))));
    }

    #[test]
    fn two_graphql_requests_differing_only_in_a_filter_value_share_a_shape() {
        let key_of = |value: &str| {
            let query =
                format!(r#"query {{ Person(filter: {{name: {{eq: "{value}"}}}}) {{ name }} }}"#);
            match plan_body(json!({"query": query})) {
                Plan::Read { key, .. } => key,
                other => panic!("expected a read, got {other:?}"),
            }
        };
        let ann = key_of("Ann");
        let bob = key_of("Bob");
        assert_eq!(ann.shape, bob.shape);
        assert_ne!(ann.root, bob.root, "but they are different entries");
    }

    // --------------------------------------------------------- augmentation

    #[test]
    fn augmentation_collects_graphql_ids() {
        let body = json!({
            "data": {
                "Person": [
                    {"_id": "Person/alice", "name": "Ann", "employer": {"_id": "Company/acme"}},
                    {"_id": "Person/bob", "name": "Bob", "employer": null}
                ]
            }
        });
        let mut terms = vec![Term::Members(cls("Person"))];
        augment_terms_from_response(body.to_string().as_bytes(), &mut terms);
        assert!(terms.contains(&Term::Doc(doc("Person/alice"))));
        assert!(terms.contains(&Term::Doc(doc("Person/bob"))));
        assert!(terms.contains(&Term::Doc(doc("Company/acme"))));
        assert!(terms.contains(&Term::Members(cls("Person"))));
    }

    #[test]
    fn augmentation_collects_document_api_ids_and_normalizes_them() {
        let body = json!([
            {"@id": "terminusdb:///data/Person/alice", "@type": "Person"},
            {"@id": "Person/bob"}
        ]);
        let mut terms = Vec::new();
        augment_terms_from_response(body.to_string().as_bytes(), &mut terms);
        assert_eq!(
            terms,
            vec![Term::Doc(doc("Person/alice")), Term::Doc(doc("Person/bob"))]
        );
    }

    #[test]
    fn augmentation_tolerates_deep_nesting_and_lists_of_lists() {
        let body = json!({"a": [[{"_id": "Person/deep"}]], "b": {"c": {"d": {"_id": "Person/x"}}}});
        let mut terms = Vec::new();
        augment_terms_from_response(body.to_string().as_bytes(), &mut terms);
        assert!(terms.contains(&Term::Doc(doc("Person/deep"))));
        assert!(terms.contains(&Term::Doc(doc("Person/x"))));
    }

    #[test]
    fn augmentation_dedupes() {
        let body = json!([{"_id": "Person/alice"}, {"_id": "Person/alice"}]);
        let mut terms = Vec::new();
        augment_terms_from_response(body.to_string().as_bytes(), &mut terms);
        assert_eq!(terms, vec![Term::Doc(doc("Person/alice"))]);
    }

    #[test]
    fn augmentation_of_an_unparseable_body_is_a_barrier_not_a_silent_pass() {
        let mut terms = vec![Term::Members(cls("Person"))];
        augment_terms_from_response(b"<html>error</html>", &mut terms);
        assert!(
            terms.contains(&Term::Everything),
            "an incomplete read set must not look complete"
        );
    }

    #[test]
    fn augmentation_ignores_non_string_ids() {
        let body = json!({"_id": 7, "nested": {"_id": {"not": "a string"}}});
        let mut terms = Vec::new();
        augment_terms_from_response(body.to_string().as_bytes(), &mut terms);
        assert!(terms.is_empty());
    }
}
