//! The schema facts the engine needs, precomputed.
//!
//! Two questions dominate invalidation, and both are schema questions:
//!
//! 1. **Is this property a value or a link?** A value property's change fires
//!    [`crate::term::Term::DocField`] plus value-indexed
//!    [`crate::term::Term::Index`] terms; a link property's change fires
//!    [`crate::term::Term::Edge`] plus [`crate::term::Term::Backlink`] terms on
//!    *both* the old and the new target. Getting this backwards does not fail
//!    loudly — it silently stops invalidating traversals, which is exactly the
//!    failure mode `CLAUDE.md`'s VALUE-vs-LINK section warns about.
//! 2. **What is this class a subclass of?** Coarse terms (`Class`, `Members`,
//!    `ClassField`) must be dirtied by changes to *descendant* classes, so
//!    subsumption needs the hierarchy in both directions.
//!
//! Both answers are computed once, at construction, into flat maps. `subsumes`
//! runs per candidate fragment per commit, so it cannot afford to walk an
//! inheritance graph per call.
//!
//! The parsing itself is delegated to [`terminusdb_format::schema_doc`], which
//! already models TerminusDB's authored schema JSON (families, `@oneOf`,
//! `@inherits`, `@subdocument`, tagged unions) and is tested there.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use terminusdb_format::schema_doc::{parse_schema, Family, RawClass, RawSchema};
use terminusdb_format::PropertyKind;

use crate::ids::{ClassName, PropName};

/// Whether a property's value is stored data or a traversable edge.
///
/// `family` is carried on both variants because `fire` needs to know whether to
/// expect a list on the wire: a `Set`/`List`/`Array` property's before/after
/// values are JSON arrays, and the value-index firing rule operates on the
/// *symmetric difference* of those two lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropKind {
    /// Datatype property: a stored scalar. `range` is an xsd/sys type name, or
    /// the name of a declared enum — an enum value is stored, not traversed, so
    /// it is a value for our purposes even though TerminusDB models it as an
    /// IRI.
    Value {
        /// The declared range, verbatim (`xsd:string`, `sys:JSON`, `Colour`).
        range: String,
        /// The type family, when the property was declared as a container.
        family: Option<Family>,
    },
    /// Object property: a graph edge.
    Link {
        /// The target class.
        range: ClassName,
        /// True when the target class is a `@subdocument`, i.e. the edge is an
        /// ownership edge whose target has no independent identity.
        subdocument: bool,
        /// The type family, when the property was declared as a container.
        family: Option<Family>,
    },
}

impl PropKind {
    /// True for a graph edge. The one predicate `fire` branches on.
    pub fn is_link(&self) -> bool {
        matches!(self, PropKind::Link { .. })
    }

    /// True when the property holds a collection, so its wire value is an array.
    pub fn is_multivalued(&self) -> bool {
        self.family().map(Family::is_multivalued).unwrap_or(false)
    }

    /// The declared type family, if any.
    pub fn family(&self) -> Option<Family> {
        match self {
            PropKind::Value { family, .. } | PropKind::Link { family, .. } => *family,
        }
    }

    /// The declared range as a string, for diagnostics.
    pub fn range_str(&self) -> &str {
        match self {
            PropKind::Value { range, .. } => range,
            PropKind::Link { range, .. } => range.as_str(),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct ClassInfo {
    /// Properties resolved *through inheritance*: a subclass's map contains its
    /// superclasses' properties too, because a document of that class really
    /// does carry them.
    props: BTreeMap<PropName, PropKind>,
    /// Direct superclasses, as authored.
    parents: Vec<ClassName>,
    subdocument: bool,
    is_abstract: bool,
    is_enum: bool,
}

/// What the engine needs from the schema: which properties are values vs links,
/// and the class hierarchy for subclass matching.
///
/// An *empty* index is a legitimate, important state: it means "we have not read
/// the schema yet". Analyzers and `fire` must degrade to coarser terms when the
/// index is empty rather than guess, because a guess here is an under-fire, and
/// an under-fire is a correctness bug.
#[derive(Clone, Debug, Default)]
pub struct SchemaIndex {
    classes: HashMap<ClassName, ClassInfo>,
    /// Insertion-ordered class list, so `classes()` and anything derived from
    /// it (e.g. the generated `_ChangeSet` subscription query) is stable.
    order: Vec<ClassName>,
    /// Transitive, inclusive superclasses.
    ancestors: HashMap<ClassName, Vec<ClassName>>,
    /// Transitive, inclusive subclasses.
    descendants: HashMap<ClassName, Vec<ClassName>>,
    /// For each class, the `(declaring class, property)` pairs that can point at
    /// an instance of it. Precomputed because the GraphQL analyzer needs it per
    /// backlink field.
    backlinks: HashMap<ClassName, Vec<(ClassName, PropName)>>,
}

impl SchemaIndex {
    /// Build from the schema graph as returned by
    /// `GET /api/document/<path>?graph_type=schema&as_list=true`, i.e. the
    /// authored schema documents.
    ///
    /// Unknown document kinds and non-objects are skipped rather than fatal (the
    /// `@context` document, future document types): a schema we only partly
    /// understand still yields useful hierarchy, and the unknown parts degrade
    /// to [`SchemaIndex::prop`] returning `None`, which `fire` handles by
    /// over-firing.
    pub fn from_schema_documents(docs: &[serde_json::Value]) -> anyhow::Result<Self> {
        let raw = parse_schema(docs)?;
        Ok(Self::from_raw(raw))
    }

    /// Build from an already-parsed [`RawSchema`]. Separate from
    /// [`SchemaIndex::from_schema_documents`] so tests and other producers can
    /// construct an index without going through JSON.
    pub fn from_raw(raw: RawSchema) -> Self {
        let enums: HashSet<ClassName> = raw
            .enums
            .iter()
            .map(|e| ClassName::new(e.id.as_str()))
            .collect();
        let declared: HashSet<ClassName> = raw
            .classes
            .iter()
            .map(|c| ClassName::new(c.id.as_str()))
            .collect();
        let subdocs: HashSet<ClassName> = raw
            .classes
            .iter()
            .filter(|c| c.is_subdocument)
            .map(|c| ClassName::new(c.id.as_str()))
            .collect();

        let mut index = SchemaIndex::default();

        // Enums are registered as (leaf, propertyless) classes so that a
        // `@type` reported as an enum still resolves in `ancestors`.
        for e in &raw.enums {
            let name = ClassName::new(e.id.as_str());
            index.insert_class(
                name,
                ClassInfo {
                    is_enum: true,
                    ..Default::default()
                },
            );
        }

        for class in &raw.classes {
            let name = ClassName::new(class.id.as_str());
            let info = ClassInfo {
                props: own_props(class, &enums, &declared, &subdocs),
                parents: class
                    .inherits
                    .iter()
                    .map(|p| ClassName::new(p.as_str()))
                    .collect(),
                subdocument: class.is_subdocument,
                is_abstract: class.is_abstract,
                is_enum: false,
            };
            index.insert_class(name, info);
        }

        index.close_hierarchy();
        index.inherit_props();
        index.build_backlinks();
        index
    }

    fn insert_class(&mut self, name: ClassName, info: ClassInfo) {
        if !self.classes.contains_key(&name) {
            self.order.push(name.clone());
        }
        self.classes.insert(name, info);
    }

    /// Compute the transitive closures in both directions, tolerating cycles and
    /// references to classes that were never declared (a partial schema).
    fn close_hierarchy(&mut self) {
        let names: Vec<ClassName> = self.order.clone();
        for name in &names {
            let mut seen: BTreeSet<ClassName> = BTreeSet::new();
            let mut queue: VecDeque<ClassName> = VecDeque::new();
            queue.push_back(name.clone());
            while let Some(cur) = queue.pop_front() {
                if !seen.insert(cur.clone()) {
                    continue;
                }
                if let Some(info) = self.classes.get(&cur) {
                    for parent in &info.parents {
                        queue.push_back(parent.clone());
                    }
                }
            }
            self.ancestors
                .insert(name.clone(), seen.into_iter().collect());
        }

        // Descendants are the reverse relation of the closure we just built.
        for (class, ancestors) in &self.ancestors {
            for ancestor in ancestors {
                self.descendants
                    .entry(ancestor.clone())
                    .or_default()
                    .push(class.clone());
            }
        }
        for list in self.descendants.values_mut() {
            list.sort();
            list.dedup();
        }
    }

    /// Fold each class's inherited properties into its own map, so `prop()` is a
    /// single hash lookup. Done in ancestor order with the most-derived
    /// declaration winning, which matches TerminusDB's override semantics.
    fn inherit_props(&mut self) {
        let mut resolved: HashMap<ClassName, BTreeMap<PropName, PropKind>> = HashMap::new();
        for name in &self.order {
            let mut props = BTreeMap::new();
            // Walk ancestors furthest-first so nearer declarations overwrite.
            // "Furthest" = fewest ancestors of its own, i.e. closest to the
            // root of the hierarchy.
            let mut chain = self.ancestors.get(name).cloned().unwrap_or_default();
            chain.sort_by_key(|c| self.ancestors.get(c).map_or(0, Vec::len));
            for ancestor in chain {
                if &ancestor == name {
                    continue;
                }
                if let Some(info) = self.classes.get(&ancestor) {
                    for (p, k) in &info.props {
                        props.insert(p.clone(), k.clone());
                    }
                }
            }
            if let Some(info) = self.classes.get(name) {
                for (p, k) in &info.props {
                    props.insert(p.clone(), k.clone());
                }
            }
            resolved.insert(name.clone(), props);
        }
        for (name, props) in resolved {
            if let Some(info) = self.classes.get_mut(&name) {
                info.props = props;
            }
        }
    }

    /// For every link property, record it against every class its range can
    /// hold — i.e. the range and all of the range's subclasses.
    fn build_backlinks(&mut self) {
        let mut out: HashMap<ClassName, Vec<(ClassName, PropName)>> = HashMap::new();
        for class in &self.order {
            let info = match self.classes.get(class) {
                Some(info) => info,
                None => continue,
            };
            for (prop, kind) in &info.props {
                if let PropKind::Link { range, .. } = kind {
                    let targets = self
                        .descendants
                        .get(range)
                        .cloned()
                        .unwrap_or_else(|| vec![range.clone()]);
                    for target in targets {
                        out.entry(target)
                            .or_default()
                            .push((class.clone(), prop.clone()));
                    }
                }
            }
        }
        for list in out.values_mut() {
            list.sort();
            list.dedup();
        }
        self.backlinks = out;
    }

    /// Empty index: every class unknown. [`SchemaIndex::prop`] returns `None`
    /// and [`SchemaIndex::ancestors`] returns just the class. Analyzers MUST
    /// degrade to coarse terms when the index is empty rather than guess.
    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }

    /// Every class and enum we know about, in declaration order.
    pub fn classes(&self) -> impl Iterator<Item = &ClassName> {
        self.order.iter()
    }

    /// The kind of `prop` on `class`, resolving inherited properties.
    ///
    /// `None` means *unknown*, not *absent*: an undeclared class, a partial
    /// schema, or a range we could not classify. Callers must treat `None` as
    /// "fire both the value and the link forms".
    pub fn prop(&self, class: &ClassName, prop: &PropName) -> Option<&PropKind> {
        self.classes.get(class)?.props.get(prop)
    }

    /// Every property of `class`, inherited ones included.
    pub fn props(&self, class: &ClassName) -> impl Iterator<Item = (&PropName, &PropKind)> {
        self.classes
            .get(class)
            .into_iter()
            .flat_map(|info| info.props.iter())
    }

    /// `class` plus all transitive superclasses (inclusive).
    ///
    /// An unknown class yields just itself, which keeps `fire` and `subsumes`
    /// total: with no schema, every class is its own island and only exact
    /// matches fire.
    pub fn ancestors(&self, class: &ClassName) -> Vec<ClassName> {
        match self.ancestors.get(class) {
            Some(list) => list.clone(),
            None => vec![class.clone()],
        }
    }

    /// `class` plus all transitive subclasses (inclusive).
    pub fn descendants(&self, class: &ClassName) -> Vec<ClassName> {
        match self.descendants.get(class) {
            Some(list) => list.clone(),
            None => vec![class.clone()],
        }
    }

    /// True when `sub` is `sup` or inherits from it, transitively.
    ///
    /// Reflexive on purpose: the subsumption rules are all phrased as "`C` or a
    /// descendant of `C`", and making the reflexive case part of this predicate
    /// keeps them from each needing a special case.
    pub fn is_subclass_of(&self, sub: &ClassName, sup: &ClassName) -> bool {
        if sub == sup {
            return true;
        }
        self.ancestors
            .get(sub)
            .map(|list| list.contains(sup))
            .unwrap_or(false)
    }

    /// True when documents of `class` are subdocuments, i.e. owned by a parent
    /// and not independently addressable.
    pub fn is_subdocument(&self, class: &ClassName) -> bool {
        self.classes
            .get(class)
            .map(|info| info.subdocument)
            .unwrap_or(false)
    }

    /// True when `class` cannot be instantiated directly. Abstract classes never
    /// appear as a document's `@type`, but they do appear in terms, because a
    /// coarse dependency on an abstract class is a perfectly good dependency on
    /// all of its concrete descendants.
    pub fn is_abstract(&self, class: &ClassName) -> bool {
        self.classes
            .get(class)
            .map(|info| info.is_abstract)
            .unwrap_or(false)
    }

    /// True when `class` is an enum rather than a document class.
    pub fn is_enum(&self, class: &ClassName) -> bool {
        self.classes
            .get(class)
            .map(|info| info.is_enum)
            .unwrap_or(false)
    }

    /// Every `(class, prop)` whose range is `class` or an ancestor of it: the
    /// reverse edges. Needed to map a GraphQL backlink field to a
    /// [`crate::term::Term::Backlink`].
    pub fn backlinks_to(&self, class: &ClassName) -> Vec<(ClassName, PropName)> {
        self.backlinks.get(class).cloned().unwrap_or_default()
    }

    /// The generated GraphQL backlink field name: `_{prop}_of_{class}`.
    pub fn backlink_field(prop: &PropName, class: &ClassName) -> String {
        format!("_{}_of_{}", prop, class)
    }

    /// Parse a generated backlink field name back into `(prop, class)`.
    ///
    /// Split on the *last* `_of_`: a property name may contain `_of_`
    /// (`list_of_things`) whereas a class name, being CamelCase by convention,
    /// usually does not. The ambiguity is unavoidable in the generated naming
    /// scheme; the caller should check the result against the schema.
    pub fn parse_backlink_field(field: &str) -> Option<(PropName, ClassName)> {
        let rest = field.strip_prefix('_')?;
        let idx = rest.rfind("_of_")?;
        let (prop, class) = (&rest[..idx], &rest[idx + "_of_".len()..]);
        if prop.is_empty() || class.is_empty() {
            return None;
        }
        Some((PropName::from(prop), ClassName::from(class)))
    }
}

/// Classify the properties a class declares itself (inheritance is folded in
/// later).
fn own_props(
    class: &RawClass,
    enums: &HashSet<ClassName>,
    declared: &HashSet<ClassName>,
    subdocs: &HashSet<ClassName>,
) -> BTreeMap<PropName, PropKind> {
    let mut out = BTreeMap::new();
    for prop in &class.properties {
        let range = prop.class.as_str();
        let name = ClassName::new(range);
        let kind = if PropertyKind::of(range).is_datatype() || enums.contains(&name) {
            // xsd:*/sys:* literals and enum members are stored values: there is
            // no document at the other end to traverse to.
            Some(PropKind::Value {
                range: range.to_string(),
                family: prop.family,
            })
        } else if declared.contains(&name) {
            Some(PropKind::Link {
                range: name.clone(),
                subdocument: subdocs.contains(&name),
                family: prop.family,
            })
        } else {
            // Unknown range: a forward reference, or a schema we only partly
            // loaded. Do NOT guess — leaving it out makes `prop()` return None,
            // which `fire` handles by firing both the value and the link forms.
            None
        };
        if let Some(kind) = kind {
            out.insert(PropName::new(prop.name.as_str()), kind);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{schema_fixture, schema_fixture_docs};

    #[test]
    fn empty_index_knows_nothing_but_stays_total() {
        let idx = SchemaIndex::default();
        assert!(idx.is_empty());
        let person = ClassName::from("Person");
        assert_eq!(idx.prop(&person, &PropName::from("name")), None);
        assert_eq!(idx.ancestors(&person), vec![person.clone()]);
        assert_eq!(idx.descendants(&person), vec![person.clone()]);
        assert!(idx.is_subclass_of(&person, &person));
        assert!(!idx.is_subclass_of(&person, &ClassName::from("Agent")));
        assert!(!idx.is_subdocument(&person));
        assert!(idx.backlinks_to(&person).is_empty());
    }

    #[test]
    fn parses_realistic_class_frames() {
        let idx = SchemaIndex::from_schema_documents(&schema_fixture_docs()).unwrap();
        assert!(!idx.is_empty());
        let names: Vec<&str> = idx.classes().map(|c| c.as_str()).collect();
        for expected in [
            "Agent",
            "Person",
            "Employee",
            "Company",
            "Address",
            "Department",
        ] {
            assert!(names.contains(&expected), "missing {expected} in {names:?}");
        }
        assert!(names.contains(&"Colour"), "enums are registered too");
    }

    #[test]
    fn classifies_value_properties() {
        let idx = schema_fixture();
        let person = ClassName::from("Person");
        assert_eq!(
            idx.prop(&person, &PropName::from("name")),
            Some(&PropKind::Value {
                range: "xsd:string".into(),
                family: None
            })
        );
        assert_eq!(
            idx.prop(&person, &PropName::from("nicknames")),
            Some(&PropKind::Value {
                range: "xsd:string".into(),
                family: Some(Family::Set)
            })
        );
        assert!(idx
            .prop(&person, &PropName::from("nicknames"))
            .unwrap()
            .is_multivalued());
    }

    #[test]
    fn enum_ranges_are_values_not_links() {
        // An enum is an IRI in the store but a value to us: there is no
        // document behind it to invalidate or traverse.
        let idx = schema_fixture();
        let kind = idx
            .prop(&ClassName::from("Person"), &PropName::from("favourite"))
            .unwrap();
        assert!(!kind.is_link(), "enum-ranged property must be a Value");
        assert_eq!(kind.range_str(), "Colour");
    }

    #[test]
    fn classifies_link_properties_and_subdocuments() {
        let idx = schema_fixture();
        let person = ClassName::from("Person");
        assert_eq!(
            idx.prop(&person, &PropName::from("employer")),
            Some(&PropKind::Link {
                range: ClassName::from("Company"),
                subdocument: false,
                family: Some(Family::Optional)
            })
        );
        assert_eq!(
            idx.prop(&person, &PropName::from("address")),
            Some(&PropKind::Link {
                range: ClassName::from("Address"),
                subdocument: true,
                family: None
            })
        );
        assert!(idx.is_subdocument(&ClassName::from("Address")));
        assert!(!idx.is_subdocument(&ClassName::from("Company")));
    }

    #[test]
    fn set_valued_link_keeps_its_family() {
        let idx = schema_fixture();
        let kind = idx
            .prop(&ClassName::from("Company"), &PropName::from("departments"))
            .unwrap();
        assert!(kind.is_link());
        assert!(kind.is_multivalued());
        assert_eq!(kind.family(), Some(Family::Set));
    }

    #[test]
    fn unknown_range_is_unknown_not_guessed() {
        // `Ghost` is referenced but never declared: we must not claim to know
        // whether it is a value or a link.
        let idx = schema_fixture();
        assert_eq!(
            idx.prop(&ClassName::from("Department"), &PropName::from("ghost")),
            None
        );
    }

    #[test]
    fn inheritance_resolves_properties_and_hierarchy() {
        let idx = schema_fixture();
        let employee = ClassName::from("Employee");
        // `name` is declared on Agent, two levels up.
        assert!(idx.prop(&employee, &PropName::from("name")).is_some());
        assert!(idx.prop(&employee, &PropName::from("salary")).is_some());

        assert_eq!(
            idx.ancestors(&employee),
            vec![
                ClassName::from("Agent"),
                ClassName::from("Employee"),
                ClassName::from("Person")
            ],
            "ancestors are inclusive and sorted"
        );
        assert!(idx.is_subclass_of(&employee, &ClassName::from("Agent")));
        assert!(!idx.is_subclass_of(&ClassName::from("Agent"), &employee));
        assert!(idx.is_abstract(&ClassName::from("Agent")));
        assert!(!idx.is_abstract(&employee));
    }

    #[test]
    fn a_subclass_declaration_overrides_its_superclass() {
        // Inheritance is folded in root-first, so the most-derived declaration
        // is the one that survives. Getting this backwards would classify a
        // property by the wrong family (or, worse, the wrong kind).
        let idx = schema_fixture();
        assert_eq!(
            idx.prop(&ClassName::from("Person"), &PropName::from("nicknames"))
                .unwrap()
                .family(),
            Some(Family::Set)
        );
        assert_eq!(
            idx.prop(&ClassName::from("Employee"), &PropName::from("nicknames"))
                .unwrap()
                .family(),
            Some(Family::Optional)
        );
    }

    #[test]
    fn descendants_are_transitive_and_inclusive() {
        let idx = schema_fixture();
        let mut d = idx.descendants(&ClassName::from("Agent"));
        d.sort();
        assert_eq!(
            d,
            vec![
                ClassName::from("Agent"),
                ClassName::from("Company"),
                ClassName::from("Employee"),
                ClassName::from("Person"),
            ]
        );
    }

    #[test]
    fn backlinks_include_subclass_targets() {
        let idx = schema_fixture();
        // Person.employer : Company, Department.head : Person.
        let to_company = idx.backlinks_to(&ClassName::from("Company"));
        assert!(to_company.contains(&(ClassName::from("Person"), PropName::from("employer"))));
        assert!(
            to_company.contains(&(ClassName::from("Employee"), PropName::from("employer"))),
            "Employee inherits employer, so it can point at a Company too"
        );

        // A link whose range is Person must also be reported for Employee,
        // because an Employee *is* a Person and can be the target.
        let to_employee = idx.backlinks_to(&ClassName::from("Employee"));
        assert!(to_employee.contains(&(ClassName::from("Department"), PropName::from("head"))));
    }

    #[test]
    fn backlink_field_round_trips() {
        let prop = PropName::from("employer");
        let class = ClassName::from("Person");
        let field = SchemaIndex::backlink_field(&prop, &class);
        assert_eq!(field, "_employer_of_Person");
        assert_eq!(
            SchemaIndex::parse_backlink_field(&field),
            Some((prop, class))
        );
    }

    #[test]
    fn backlink_field_with_of_in_the_prop_name_round_trips() {
        let prop = PropName::from("list_of_things");
        let class = ClassName::from("Catalogue");
        let field = SchemaIndex::backlink_field(&prop, &class);
        assert_eq!(field, "_list_of_things_of_Catalogue");
        assert_eq!(
            SchemaIndex::parse_backlink_field(&field),
            Some((prop, class))
        );
    }

    #[test]
    fn backlink_field_rejects_non_backlinks() {
        assert_eq!(SchemaIndex::parse_backlink_field("employer"), None);
        assert_eq!(SchemaIndex::parse_backlink_field("_employer"), None);
        assert_eq!(SchemaIndex::parse_backlink_field("__of_Person"), None);
        assert_eq!(SchemaIndex::parse_backlink_field("_employer_of_"), None);
    }

    #[test]
    fn unparseable_schema_document_is_an_error_not_a_panic() {
        let docs = vec![serde_json::json!({"@id": "Broken", "@type": "Class", "p": 7})];
        assert!(SchemaIndex::from_schema_documents(&docs).is_err());
    }
}
