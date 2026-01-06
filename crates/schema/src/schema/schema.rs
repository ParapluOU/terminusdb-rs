// https://terminusdb.com/docs/index/terminusx-db/reference-guides/schema

use crate::*;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashSet};
use std::cmp::Ordering;
use std::fs::File;
use std::io::Write;

/// Helper function to namespace-qualify class references in property values.
/// This prefixes class references with the namespace when they're not already
/// fully qualified (xsd:*, sys:*, or already start with http).
fn namespace_property_value(namespace: &str, value: &Value) -> Value {
    match value {
        // Simple string class reference like "PersonType" -> "http://ns#PersonType"
        Value::String(s) => {
            if s.starts_with("xsd:") || s.starts_with("sys:") || s.starts_with("http") {
                value.clone()
            } else {
                Value::String(format!("{}{}", namespace, s))
            }
        }
        // Complex property like {"@type": "Optional", "@class": "PersonType"}
        Value::Object(obj) => {
            let mut new_obj = obj.clone();
            if let Some(class_val) = obj.get("@class") {
                if let Value::String(class_str) = class_val {
                    if !class_str.starts_with("xsd:")
                        && !class_str.starts_with("sys:")
                        && !class_str.starts_with("http")
                    {
                        new_obj.insert(
                            "@class".to_string(),
                            Value::String(format!("{}{}", namespace, class_str)),
                        );
                    }
                }
            }
            Value::Object(new_obj)
        }
        _ => value.clone(),
    }
}

// todo: the derived serialize and deserialize do not comply with the TerminusDB schema and are only used for RPC calls!
#[derive(Eq, Debug, Clone, Hash)]
pub enum Schema {
    Class {
        id: ID,
        /// should be settable by derive attribute
        base: Option<String>,
        /// should be settable by derive attribute
        key: Key,

        /// Use @documentation to add documentation to the class and the property fields or values of the class.
        documentation: Option<ClassDocumentation>,

        /// The @subdocument key is present with the value [] or it is not present.
        /// A class designated as a sub-document is considered to be completely owned by its containing document.
        /// It is not possible to directly update or delete a subdocument,
        /// but it must be done through the containing document.
        /// Currently, subdocuments must have a key that is Random or ValueHash (this restriction may be relaxed in the future.)
        ///
        /// **example subdocument decl**:
        /// {
        ///     "@type"        : "@context",
        ///     "@base"        : "terminusdb://i/",
        ///     "@schema"      : "terminusdb://s#"
        /// }
        /// {
        ///     "@type"        : "Class",
        ///     "@id"          : "Person",
        ///     "age"          : "xsd:integer",
        ///     "name"         : "xsd:string",
        ///     "address"      : "Address"
        /// }
        /// {
        ///     "@type"        : "Class",
        ///     "@id"          : "Address",
        ///     "@key"         :
        ///     {
        ///         "@type"    : "Random"
        ///     },
        ///     "@subdocument" : [],
        ///     "country"      : "xsd:string",
        ///     "postal_code"  : "xsd:string",
        ///     "street"       : "xsd:string"
        /// }
        ///
        /// **example subdocument instance**:
        /// {
        ///     "@type"           : "Person",
        ///     "@id"             : "doug",
        ///     "name"            : "Doug A. Trench",
        ///     "address"         :
        ///     {
        ///         "@type"       : "Address",
        ///         "country"     : "Neverlandistan",
        ///         "postal_code" : "3",
        ///         "street"      : "Cool Harbour lane"
        ///     }
        /// }
        /// should be settable by derive attribute
        subdocument: bool,

        /// The @abstract key is present with the value [] or it is not present.
        /// An abstract class has no concrete referents. It provides a common superclass and potentially several properties shared by all of its descendants. Create useful concrete members using the @inherits keyword.
        /// An example of the abstract keyword in a schema, and a concrete instance of the Person class, but not of the NamedEntity class:
        ///
        /// Code: An example of the abstract keyword
        /// {
        //     "@type"     : "@context",
        //     "@base"     : "terminusdb://i/",
        //     "@schema"   : "terminusdb://s#"
        // }
        // {
        //     "@type"     : "Class",
        //     "@abstract" : [],
        //     "@id"       : "NamedEntity",
        //     "name"      : "xsd:string"
        // }
        // {
        //     "@type"     : "Person",
        //     "@id"       : "Person",
        //     "@inherits" : ["NamedEntity"]
        // }
        ///
        /// {
        //     "@type" : "Person",
        //     "@id"   : "doug",
        //     "name"  : "Doug A. Trench"
        // }
        /// should be settable by derive attribute
        r#abstract: bool,

        /// @inherits enables classes to inherit properties (and the @subdocument designation) from parent classes. It does not inherit key strategies.
        /// This inheritance tree is also available as a subsumption relation in the WOQL query language and provides semantics for frames in the schema API.
        /// The range of @inherits can be a class or a list of classes. For example:
        /// {
        //     ...,
        //
        //     "@inherits" : "MyClass",
        //
        //     ...
        // }
        /// {
        //     ...,
        //
        //     "@inherits" :
        //     [
        //         "MyFirstClass", "MySecondClass"
        //     ]
        //
        //     ...
        // }
        /// Multiple inheritance is allowed as long as all inherited properties of the same name have the same range class. If range classes conflict, the schema check fails.
        /// An example of inheritance of properties and an object meeting this specification:
        /// {
        //     "@type"      : "@context",
        //     "@base"      : "http://i/",
        //     "@schema"    : "http://s/"
        // }
        // {
        //     "@id"        : "RightHanded",
        //     "@type"      : "Class",
        //     "right_hand" : "xsd:string"
        // }
        // {
        //     "@id"        : "LeftHanded",
        //     "@type"      : "Class",
        //     "left_hand"  : "xsd:string"
        // }
        // {
        //     "@id"        : "TwoHanded",
        //     "@type"      : "Class",
        //     "@inherits"  :
        //     [
        //         "RightHanded", "LeftHanded"
        //     ]
        // }
        ///
        /// {
        //     "@type"      : "TwoHanded",
        //     "@id"        : "a two-hander",
        //     "left_hand"  : "Pretty sinister",
        //     "right_hand" : "But this one is dexterous"
        // }
        inherits: Vec<String>,

        /// The @unfoldable key is present with the value [] or it is not present.
        ///
        /// In the document API, when retrieving documents, the default behavior is for any linked document to be returned as an IRI, while subdocuments are fully unfolded and returned as a nested document. With the @unfoldable option set, linked documents will behave just like subdocuments, and will also be unfolded on retrieval.
        ///
        /// The @unfoldable option can only be set on a class which does not directly or indirectly link to itself. This prevents a self-referencing document from being unfolded infinitely.
        ///
        /// The purpose of @unfoldable is to be able to treat linked (top-level) documents as subdocuments in representation. Subdocuments can only be linked by one document, its owner, whereas normal documents can be linked by any number of other documents. If the desired result is to have a document linked by several other documents, but still have it fully unfolded on retrieval like a subdocument, use this option.
        /// should be settable by derive attribute
        unfoldable: bool,

        // all user-defined properties
        properties: Vec<Property>,
    },

    OneOfClass {
        id: ID,
        base: Option<String>,
        // key: Key,
        /// Use @documentation to add documentation to the class and the property fields or values of the class.
        documentation: Option<ClassDocumentation>,

        /// The @subdocument key is present with the value [] or it is not present.
        /// A class designated as a sub-document is considered to be completely owned by its containing document.
        /// It is not possible to directly update or delete a subdocument,
        /// but it must be done through the containing document.
        /// Currently, subdocuments must have a key that is Random or ValueHash (this restriction may be relaxed in the future.)
        ///
        /// **example subdocument decl**:
        /// {
        ///     "@type"        : "@context",
        ///     "@base"        : "terminusdb://i/",
        ///     "@schema"      : "terminusdb://s#"
        /// }
        /// {
        ///     "@type"        : "Class",
        ///     "@id"          : "Person",
        ///     "age"          : "xsd:integer",
        ///     "name"         : "xsd:string",
        ///     "address"      : "Address"
        /// }
        /// {
        ///     "@type"        : "Class",
        ///     "@id"          : "Address",
        ///     "@key"         :
        ///     {
        ///         "@type"    : "Random"
        ///     },
        ///     "@subdocument" : [],
        ///     "country"      : "xsd:string",
        ///     "postal_code"  : "xsd:string",
        ///     "street"       : "xsd:string"
        /// }
        ///
        /// **example subdocument instance**:
        /// {
        ///     "@type"           : "Person",
        ///     "@id"             : "doug",
        ///     "name"            : "Doug A. Trench",
        ///     "address"         :
        ///     {
        ///         "@type"       : "Address",
        ///         "country"     : "Neverlandistan",
        ///         "postal_code" : "3",
        ///         "street"      : "Cool Harbour lane"
        ///     }
        /// }
        subdocument: bool,

        /// The @abstract key is present with the value [] or it is not present.
        /// An abstract class has no concrete referents. It provides a common superclass and potentially several properties shared by all of its descendants. Create useful concrete members using the @inherits keyword.
        /// An example of the abstract keyword in a schema, and a concrete instance of the Person class, but not of the NamedEntity class:
        ///
        /// Code: An example of the abstract keyword
        /// {
        //     "@type"     : "@context",
        //     "@base"     : "terminusdb://i/",
        //     "@schema"   : "terminusdb://s#"
        // }
        // {
        //     "@type"     : "Class",
        //     "@abstract" : [],
        //     "@id"       : "NamedEntity",
        //     "name"      : "xsd:string"
        // }
        // {
        //     "@type"     : "Person",
        //     "@id"       : "Person",
        //     "@inherits" : ["NamedEntity"]
        // }
        ///
        /// {
        //     "@type" : "Person",
        //     "@id"   : "doug",
        //     "name"  : "Doug A. Trench"
        // }
        r#abstract: bool,

        /// @inherits enables classes to inherit properties (and the @subdocument designation) from parent classes. It does not inherit key strategies.
        /// This inheritance tree is also available as a subsumption relation in the WOQL query language and provides semantics for frames in the schema API.
        /// The range of @inherits can be a class or a list of classes. For example:
        /// {
        //     ...,
        //
        //     "@inherits" : "MyClass",
        //
        //     ...
        // }
        /// {
        //     ...,
        //
        //     "@inherits" :
        //     [
        //         "MyFirstClass", "MySecondClass"
        //     ]
        //
        //     ...
        // }
        /// Multiple inheritance is allowed as long as all inherited properties of the same name have the same range class. If range classes conflict, the schema check fails.
        /// An example of inheritance of properties and an object meeting this specification:
        /// {
        //     "@type"      : "@context",
        //     "@base"      : "http://i/",
        //     "@schema"    : "http://s/"
        // }
        // {
        //     "@id"        : "RightHanded",
        //     "@type"      : "Class",
        //     "right_hand" : "xsd:string"
        // }
        // {
        //     "@id"        : "LeftHanded",
        //     "@type"      : "Class",
        //     "left_hand"  : "xsd:string"
        // }
        // {
        //     "@id"        : "TwoHanded",
        //     "@type"      : "Class",
        //     "@inherits"  :
        //     [
        //         "RightHanded", "LeftHanded"
        //     ]
        // }
        ///
        /// {
        //     "@type"      : "TwoHanded",
        //     "@id"        : "a two-hander",
        //     "left_hand"  : "Pretty sinister",
        //     "right_hand" : "But this one is dexterous"
        // }
        inherits: Vec<String>,

        // possible subclasses that the composition is made of
        classes: Vec<BTreeSet<Property>>,

        // all user-defined properties
        properties: Vec<Property>,
    },

    /// An Enum is a non-standard class in which each instance is a simple URI with no additional structure.
    /// To be a member of the class, you must be one of the referent URIs. An Enum example with an extension Blue is s shown below.
    /// In the database, the actual URI for an Enum is expanded with the preceding type name,
    /// so the Blue extension becomes http://s#PrimaryColour/Blue
    /// {
    //     "@type"   : "Enum",
    //     "@id"     : "PrimaryColour",
    //     "@value" :
    //     [
    //         "Red",
    //         "Blue",
    //         "Yellow"
    //     ]
    // }
    Enum {
        id: ID,
        /// Namespace base for multi-namespace support (e.g., XSD-derived schemas)
        base: Option<String>,
        values: Vec<URI>,

        documentation: Option<ClassDocumentation>,
    },

    /// A TaggedUnion specifies mutually exclusive properties. This is useful when there is a disjoint choice between options.
    /// Examples below of a schema with a TaggedUnion and a concrete TaggedUnion class extension.
    ///  In these examples, the BinaryTree class specifies a TaggedUnion enabling a choice between a leaf (with no value), or a node class with a value and branches.
    ///
    /// Code: An example schema with a TaggedUnion
    /// {
    //     "@type"     : "@context",
    //     "@base"     : "http://i/",
    //     "@schema"   : "http://s#"
    // }
    // {
    //     "@id"       : "BinaryTree",
    //     "@type"     : "TaggedUnion",
    //     "@base"     : "binary_tree_",
    //     "@key"      :
    //     {
    //         "@type" : "ValueHash"
    //     },
    //     "leaf"      : "sys:Unit",
    //     "node"      : "Node"
    // }
    // {
    //     "@id"       : "Node",
    //     "@type"     : "Class",
    //     "@key"      :
    //     {
    //         "@type" : "ValueHash"
    //     },
    //     "value"     : "xsd:integer",
    //     "left"      : "BinaryTree",
    //     "right"     : "BinaryTree"
    // }
    ///
    /// Code: An example TaggedUnion class extension
    /// {
    //     "@type"     : "Node",
    //     "value"     : 0,
    //     "left"      :
    //     {
    //         "@type" : "BinaryTree",
    //         "leaf"  : []
    //     },
    //     "right":
    //     {
    //         "@type" : "BinaryTree",
    //         "leaf"  : []
    //     }
    // }
    TaggedUnion {
        id: ID,
        /// should be settable by derive attribute
        base: Option<String>,
        /// should be settable by derive attribute
        key: Key,

        /// some TerminusDB constructs such as Query are abstract,
        /// meaning they are only a semantic superclass and provide base
        /// properties for concrete classes.
        /// when an abstract is serialized to JSON-LD, the abstract
        /// part should not be rendered; only the inheritor
        r#abstract: bool,

        /// Use @documentation to add documentation to the class and the property fields or values of the class.
        documentation: Option<ClassDocumentation>,

        /// The @subdocument key is present with the value [] or it is not present.
        /// A class designated as a sub-document is considered to be completely owned by its containing document.
        /// It is not possible to directly update or delete a subdocument,
        /// but it must be done through the containing document.
        /// Currently, subdocuments must have a key that is Random or ValueHash (this restriction may be relaxed in the future.)
        /// should be settable by derive attribute
        subdocument: bool,

        // all user-defined mutually-exclusive properties
        properties: Vec<Property>,

        /// should be settable by derive attribute
        unfoldable: bool,
    },
}

impl Schema {
    fn write_to_file(&self, path: impl AsRef<str>, schemas: Vec<Self>) -> std::io::Result<()> {
        let mut output = File::create(path.as_ref())?;
        let line = schemas.iter().map(|s| s.to_string()).join(", ");
        write!(output, "[{}]", line);
        Ok(())
    }

    pub fn key(&self) -> Option<Key> {
        match self {
            Schema::Class { key, .. } => Some(key.clone()),
            Schema::OneOfClass { .. } => None,
            Schema::Enum { .. } => None,
            Schema::TaggedUnion { key, .. } => Some(key.clone()),
        }
    }

    pub fn base(&self) -> Option<&String> {
        match self {
            Schema::Class { base, .. } => base.as_ref(),
            Schema::OneOfClass { base, .. } => base.as_ref(),
            Schema::Enum { base, .. } => base.as_ref(),
            Schema::TaggedUnion { base, .. } => base.as_ref(),
        }
    }

    pub fn is_abstract(&self) -> bool {
        match self {
            Schema::Class { r#abstract, .. } => *r#abstract,
            Schema::OneOfClass { .. } => false,
            Schema::Enum { .. } => false,
            Schema::TaggedUnion { r#abstract, .. } => *r#abstract,
        }
    }

    pub fn is_enum(&self) -> bool {
        match self {
            Schema::Enum { .. } => true,
            _ => false,
        }
    }

    pub fn is_tagged_union(&self) -> bool {
        matches!(self, Schema::TaggedUnion { .. })
    }

    pub fn is_subdocument(&self) -> bool {
        match self {
            Schema::Class { subdocument, .. } => *subdocument,
            Schema::TaggedUnion { subdocument, .. } => *subdocument,
            _ => false,
        }
    }

    pub fn is_key_random(&self) -> bool {
        match self {
            Schema::Class { key, .. } => key == &Key::Random,
            Schema::OneOfClass { .. } => false,
            Schema::Enum { .. } => false,
            Schema::TaggedUnion { key, .. } => key == &Key::Random,
        }
    }

    pub fn should_unfold(&self) -> bool {
        match self {
            Schema::Class { unfoldable, .. } => *unfoldable,
            Schema::TaggedUnion { unfoldable, .. } => *unfoldable,
            _ => false,
        }
    }

    pub fn own_properties(&self) -> Vec<&Property> {
        match self {
            Schema::Class { properties, .. } => properties.iter().collect(),
            Schema::OneOfClass { properties, .. } => properties.iter().collect(),
            Schema::Enum { .. } | Schema::TaggedUnion { .. } => {
                vec![]
            }
        }
    }

    pub fn is_relation_property(&self, field_name: &str) -> bool {
        self.own_properties()
            .iter()
            .any(|prop| prop.field_name() == field_name && prop.is_relation())
    }

    // todo: make ID field strongly typed
    pub fn format_id(&self, id: &str) -> String {
        if id.starts_with(&format!("{}/", self.class_name())) {
            id.to_string()
        } else {
            format!("{}/{}", self.class_name(), id)
        }
    }

    #[pseudonym::alias(id)]
    pub fn class_name(&self) -> &ID {
        match self {
            Schema::Class { id, .. } => id,
            Schema::OneOfClass { id, .. } => id,
            Schema::Enum { id, .. } => id,
            Schema::TaggedUnion { id, .. } => id,
        }
    }

    /// Returns the namespace base URL if one is set and is a valid URL.
    /// This is used for XSD-derived schemas where the base contains the namespace like
    /// "http://example.com/book#".
    pub fn namespace_base(&self) -> Option<&str> {
        match self {
            Schema::Class { base: Some(b), .. } if b.contains("://") => Some(b.as_str()),
            Schema::OneOfClass { base: Some(b), .. } if b.contains("://") => Some(b.as_str()),
            Schema::Enum { base: Some(b), .. } if b.contains("://") => Some(b.as_str()),
            Schema::TaggedUnion { base: Some(b), .. } if b.contains("://") => Some(b.as_str()),
            _ => None,
        }
    }

    /// Returns the fully qualified class URI if a namespace base is present,
    /// otherwise returns just the class name.
    /// Example: "http://example.com/book#DocumentType" or just "DocumentType"
    pub fn full_class_uri(&self) -> String {
        if let Some(ns) = self.namespace_base() {
            format!("{}{}", ns, self.class_name())
        } else {
            self.class_name().clone()
        }
    }

    pub fn is_of_type<T: ToTDBInstance>(&self) -> bool {
        &T::schema_name() == self.id()
    }

    pub fn empty_class(id: &str) -> Self {
        Schema::Class {
            id: id.to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            properties: vec![],
            unfoldable: true,
        }
    }

    /// Convert schema to JSON with fully-qualified URIs for multi-namespace support.
    ///
    /// Unlike `to_json()` which uses short names, this method produces fully-qualified
    /// class URIs when a namespace base is present. Use this when inserting multiple
    /// XSD schemas with the same class names but different namespaces into the same
    /// TerminusDB database.
    ///
    /// Example output:
    /// ```json
    /// {
    ///   "@type": "Class",
    ///   "@id": "http://example.com/book#DocumentType",
    ///   ...
    /// }
    /// ```
    pub fn to_namespaced_json(&self) -> serde_json::Value {
        let namespace = self.namespace_base();

        match self {
            Schema::Class {
                id,
                base,
                key,
                subdocument,
                inherits,
                unfoldable,
                properties,
                documentation,
                r#abstract,
            } => {
                let mut map = serde_json::Map::new();

                map.insert("@type".to_string(), "Class".to_string().into());

                // Use fully-qualified ID if namespace is present
                if let Some(ns) = namespace {
                    map.insert("@id".to_string(), format!("{}{}", ns, id).into());
                } else {
                    map.insert("@id".to_string(), id.clone().into());
                }

                // Only output @base for ID generation prefixes, not for namespace URLs
                if let Some(base2) = base {
                    if !base2.contains("://") {
                        map.insert("@base".to_string(), base2.clone().into());
                    }
                }

                map.insert("@key".to_string(), key.to_map().into());

                if *subdocument {
                    map.insert("@subdocument".to_string(), Value::Array(vec![]));
                }
                if *r#abstract {
                    map.insert("@abstract".to_string(), Value::Array(vec![]));
                }
                if *unfoldable {
                    map.insert("@unfoldable".to_string(), Value::Array(vec![]));
                }
                if let Some(doc) = documentation {
                    map.insert("@documentation".to_string(), doc.to_map().into());
                }

                // Namespace-qualify inherits
                if !inherits.is_empty() {
                    let inherits_vals: Vec<Value> = inherits
                        .iter()
                        .map(|s| {
                            if let Some(ns) = namespace {
                                if !s.starts_with("http") {
                                    Value::String(format!("{}{}", ns, s))
                                } else {
                                    Value::String(s.clone())
                                }
                            } else {
                                Value::String(s.clone())
                            }
                        })
                        .collect();
                    map.insert("@inherits".to_string(), inherits_vals.into());
                }

                // Namespace-qualify property type references
                for prop in properties {
                    let prop_value = prop.to_property_value();
                    if let Some(ns) = namespace {
                        map.insert(prop.name.clone(), namespace_property_value(ns, &prop_value));
                    } else {
                        map.insert(prop.name.clone(), prop_value);
                    }
                }

                serde_json::Value::Object(map)
            }
            Schema::TaggedUnion {
                id,
                base,
                key,
                subdocument,
                properties,
                documentation,
                r#abstract,
                unfoldable,
            } => {
                let mut map = serde_json::Map::new();

                map.insert("@type".to_string(), "TaggedUnion".to_string().into());

                // Use fully-qualified ID if namespace is present
                if let Some(ns) = namespace {
                    map.insert("@id".to_string(), format!("{}{}", ns, id).into());
                } else {
                    map.insert("@id".to_string(), id.clone().into());
                }

                if let Some(base2) = base {
                    if !base2.contains("://") {
                        map.insert("@base".to_string(), base2.clone().into());
                    }
                }

                map.insert("@key".to_string(), key.to_map().into());

                if *subdocument {
                    map.insert("@subdocument".to_string(), Value::Array(vec![]));
                }
                if *r#abstract {
                    map.insert("@abstract".to_string(), Value::Array(vec![]));
                }
                if *unfoldable {
                    map.insert("@unfoldable".to_string(), Value::Array(vec![]));
                }
                if let Some(doc) = documentation {
                    map.insert("@documentation".to_string(), doc.to_map().into());
                }

                // Namespace-qualify property type references
                for prop in properties {
                    let prop_value = prop.to_property_value();
                    if let Some(ns) = namespace {
                        map.insert(prop.name.clone(), namespace_property_value(ns, &prop_value));
                    } else {
                        map.insert(prop.name.clone(), prop_value);
                    }
                }

                serde_json::Value::Object(map)
            }
            Schema::Enum {
                id,
                base,
                values,
                documentation,
            } => {
                let mut map = serde_json::Map::new();

                map.insert("@type".to_string(), "Enum".to_string().into());

                // Use fully-qualified ID if namespace is present
                if let Some(ns) = namespace {
                    map.insert("@id".to_string(), format!("{}{}", ns, id).into());
                } else {
                    map.insert("@id".to_string(), id.clone().into());
                }

                map.insert(
                    "@value".to_string(),
                    values
                        .iter()
                        .map(|s| Value::from(s.clone()))
                        .collect::<Vec<_>>()
                        .into(),
                );

                // Add documentation if available
                if let Some(doc) = documentation {
                    map.insert("comment".to_string(), doc.comment.clone().into());
                }

                serde_json::Value::Object(map)
            }
            // For OneOfClass, use the regular to_json (rare case, not from XSD)
            Schema::OneOfClass { .. } => self.to_json(),
        }
    }
}

impl ToJson for Schema {
    fn to_map(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut map = serde_json::Map::new();

        match self {
            Schema::Class {
                id,
                base,
                key,
                subdocument,
                inherits,
                unfoldable,
                properties,
                documentation,
                r#abstract,
            } => {
                // Use SHORT names - Context @schema will expand them
                // This allows Context to handle namespace resolution

                map.insert("@type".to_string(), "Class".to_string().into());

                // Use short @id - Context will expand it
                map.insert("@id".to_string(), id.clone().into());

                // Only output @base for ID generation prefixes, not for namespace URLs
                if let Some(base2) = base {
                    if !base2.contains("://") {
                        map.insert("@base".to_string(), base2.clone().into());
                    }
                }
                map.insert("@key".to_string(), key.to_map().into());
                if *subdocument {
                    map.insert("@subdocument".to_string(), Value::Array(vec![]));
                }
                if *r#abstract {
                    map.insert("@abstract".to_string(), Value::Array(vec![]));
                }
                if *unfoldable {
                    map.insert("@unfoldable".to_string(), Value::Array(vec![]));
                }
                if let Some(doc) = documentation {
                    map.insert("@documentation".to_string(), doc.to_map().into());
                }
                if !inherits.is_empty() {
                    // Use short names for inherits - Context will expand
                    let inherits_vals: Vec<Value> = inherits
                        .iter()
                        .map(|s| Value::String(s.clone()))
                        .collect();
                    map.insert("@inherits".to_string(), inherits_vals.into());
                }
                // Use SHORT property names - Context @schema will expand them
                for prop in properties {
                    map.insert(prop.name.clone(), prop.to_property_value());
                }
            }
            Schema::Enum {
                id,
                values: value,
                documentation,
                ..
            } => {
                // Note: base is intentionally unused here - to_map() produces short names,
                // relying on Context for expansion. Use to_namespaced_json() for fully-qualified names.
                map.insert("@type".to_string(), "Enum".to_string().into());
                map.insert("@id".to_string(), id.clone().into());
                map.insert(
                    "@value".to_string(),
                    value
                        .into_iter()
                        .map(|s| Value::from(s.clone()))
                        .collect::<Vec<_>>()
                        .into(),
                );

                // Add documentation if available
                if let Some(doc) = documentation {
                    map.insert("comment".to_string(), doc.comment.clone().into());
                }
            }
            Schema::TaggedUnion {
                id,
                base,
                key,
                documentation,
                subdocument,
                properties,
                unfoldable,
                r#abstract,
            } => {
                // Use SHORT names - Context @schema will expand them

                map.insert("@type".to_string(), "TaggedUnion".to_string().into());

                // Use short @id - Context will expand it
                map.insert("@id".to_string(), id.clone().into());

                // Only output @base for ID generation prefixes, not for namespace URLs
                // (namespace is handled by Context.schema)
                if let Some(base2) = base {
                    if !base2.contains("://") {
                        map.insert("@base".to_string(), base2.clone().into());
                    }
                }
                map.insert("@key".to_string(), key.to_map().into());
                if *subdocument {
                    map.insert("@subdocument".to_string(), Value::Array(vec![]));
                }
                if *r#abstract {
                    map.insert("@abstract".to_string(), Value::Array(vec![]));
                }
                if *unfoldable {
                    map.insert("@unfoldable".to_string(), Value::Array(vec![]));
                }
                if let Some(doc) = documentation {
                    map.insert("@documentation".to_string(), doc.to_map().into());
                }
                // Use SHORT property names - Context will expand them
                for prop in properties {
                    map.insert(prop.name.clone(), prop.to_property_value());
                }
            }
            Schema::OneOfClass {
                id,
                base,
                // key,
                subdocument,
                inherits,
                classes,
                properties,
                documentation,
                r#abstract,
            } => {
                // Use SHORT names - Context @schema will expand them

                map.insert("@type".to_string(), "Class".to_string().into());

                // Use short @id - Context will expand it
                map.insert("@id".to_string(), id.clone().into());

                // Only output @base for ID generation prefixes, not for namespace URLs
                // (namespace is handled by Context.schema)
                if let Some(base2) = base {
                    if !base2.contains("://") {
                        map.insert("@base".to_string(), base2.clone().into());
                    }
                }
                // map.insert("@key".to_string(), key.to_map().into());
                if *subdocument {
                    map.insert("@subdocument".to_string(), Value::Array(vec![]));
                }
                if *r#abstract {
                    map.insert("@abstract".to_string(), Value::Array(vec![]));
                }
                if let Some(doc) = documentation {
                    map.insert("@documentation".to_string(), doc.to_map().into());
                }
                if !inherits.is_empty() {
                    // Use short names for inherits - Context will expand
                    let inherits_vals: Vec<Value> = inherits
                        .iter()
                        .map(|s| Value::String(s.clone()))
                        .collect();
                    map.insert("@inherits".to_string(), inherits_vals.into());
                }
                // Use SHORT property names - Context will expand them
                for prop in properties {
                    map.insert(prop.name.clone(), prop.to_property_value());
                }
                // For @oneOf classes, also use short property names
                let oneof_classes: Vec<Value> = classes
                    .iter()
                    .map(|prop_set| {
                        let mut class_map = serde_json::Map::new();
                        for prop in prop_set {
                            class_map.insert(prop.name.clone(), prop.to_property_value());
                        }
                        serde_json::Value::Object(class_map)
                    })
                    .collect();
                map.insert("@oneOf".to_string(), serde_json::Value::Array(oneof_classes));
            }
        }
        map
    }
}

impl serde::Serialize for Schema {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_json().serialize(serializer)
    }
}

impl ToString for Schema {
    fn to_string(&self) -> String {
        self.to_json_string()
    }
}

/*
   terminusdb_schema::Schema::Class {
       id: <Self as terminusdb_schema::ToSchemaClass>::to_class().to_string(),
       // base: Some(terminusdb_schema::DEFAULT_BASE_STRING.to_string()),
       base: None,
       // todo: make configurable
       key: terminusdb_schema::Key::ValueHash,
       documentation: None,
       // todo: we might want to use this, needs experimentation
       subdocument: false,
       r#abstract: false,
       inherits: vec![],
       properties: vec![
           #(
               <#property_field_idents as terminusdb_schema::ToSchemaProperty>::to_property( stringify!(#property_field_name_idents).to_string() )
           ),*
       ]
   }
*/
pub trait ToTDBSchema {
    type Type: SchemaTypeI = SchemaTypeClass;
    type Predicates: PredicateSpec = DefaultPredicateSpecs;

    fn to_schema() -> Schema {
        let ty: SchemaType = Self::Type::default().into();
        match ty {
            SchemaType::SchemaTypeClass => Schema::Class {
                id: Self::id().expect(&format!(
                    "id for Class not defined in ToTDBSchema for entity {}",
                    std::any::type_name::<Self>()
                )),
                base: Self::base(),
                key: Self::key(),
                documentation: Self::documentation(),
                subdocument: Self::subdocument().unwrap_or_default(),
                r#abstract: Self::abstractdocument().unwrap_or_default(),
                inherits: Self::inherits().unwrap_or_default(),
                unfoldable: Self::unfoldable(),
                properties: Self::properties().unwrap_or_default(),
            },
            SchemaType::SchemaTypeOneOfClass => {
                todo!()
            }
            SchemaType::SchemaTypeEnum => Schema::Enum {
                id: Self::id().expect("id for Enum not defined in ToTDBSchema"),
                base: Self::base(),
                values: Self::values().unwrap(),
                documentation: Self::documentation(),
            },
            SchemaType::SchemaTypeTaggedUnion => Schema::TaggedUnion {
                id: Self::id().expect("id for Enum not defined in ToTDBSchema"),
                base: Self::base(),
                key: Self::key(),
                documentation: Self::documentation(),
                subdocument: Self::subdocument().unwrap_or_default(),
                properties: Self::properties().unwrap_or_default(),
                unfoldable: Self::unfoldable(),
                r#abstract: Self::abstractdocument().unwrap_or_default(),
            },
        }
    }

    fn schema_name() -> ID {
        Self::to_schema().class_name().clone()
    }

    fn assert_schema_tree_includes<T: ToTDBSchema>() {
        let schema_tree = Self::to_schema_tree();
        let class_name = T::schema_name();
        assert!(
            schema_tree.iter().any(|s| s.class_name() == &class_name),
            "expected schema tree of {} to include {}, but was: {:#?}",
            std::any::type_name::<Self>(),
            class_name,
            schema_tree
                .iter()
                .map(|s| s.class_name())
                .collect::<Vec<_>>()
        );
    }

    fn to_schema_tree() -> Vec<Schema>;

    // Change to_schema_tree_mut to be a static method
    fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        let schema = Self::to_schema();
        let class_name = schema.class_name().clone();

        // Check if we already have a schema with this class name
        if !collection
            .iter()
            .any(|s: &Schema| s.class_name() == &class_name)
        {
            collection.insert(schema);

            // Process any inner schemas if needed, but this would be type-specific
            // Implementations for container types like Option<T>, Vec<T>, etc.
            // will need to call T::to_schema_tree_mut(collection)
        }
    }

    fn to_schema_json() -> serde_json::Value {
        Self::to_schema().to_json()
    }

    fn find_schema_by_name(class_name: &String) -> Option<Schema> {
        for schema in Self::to_schema_tree() {
            if schema.class_name() == class_name {
                return Some(schema);
            }
        }
        None
    }

    // fn get_context() -> Context {
    //     Context {
    //         schema: "http://parture.org/schema/woql".to_string(),
    //         base: "<parture://".to_string(),
    //         xsd: None,
    //         documentation: None
    //     }
    // }

    fn id() -> Option<String> {
        None
        // <Self as ToSchemaClass>::to_class().to_string().into()
    }

    fn base() -> Option<String> {
        None
    }

    fn key() -> Key {
        Key::Random
    }

    fn documentation() -> Option<ClassDocumentation> {
        None
    }

    fn subdocument() -> Option<bool> {
        None
    }

    fn abstractdocument() -> Option<bool> {
        None
    }

    fn inherits() -> Option<Vec<String>> {
        None
    }

    /// whether linked documents that are not subdocuments should be "unfoldable" on retrieval.
    /// in other contexts this is known as tree querying, or resolving.
    /// only supported on data structures that are not self-referencing, directly or indirectly
    fn unfoldable() -> bool {
        true
    }

    fn properties() -> Option<Vec<Property>> {
        None
    }

    fn values() -> Option<Vec<URI>> {
        None
    }
}

impl<T: ToTDBSchema> From<T> for Schema {
    fn from(to: T) -> Self {
        T::to_schema()
    }
}

impl PartialOrd for Schema {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Schema {
    fn eq(&self, other: &Self) -> bool {
        // Compare by schema name (ID) which should be unique
        self.class_name() == other.class_name()
    }
}

impl Ord for Schema {
    fn cmp(&self, other: &Self) -> Ordering {
        // Simply compare by schema name (ID) which should be unique
        self.class_name().cmp(other.class_name())
    }
}

/// trait to be used when in a derive or proc macro
/// a field needs to call .to_schema() on nit but we can't be sure the field
/// actually implements ToTDBSchema. By using this, we can have a valid function
/// call and jut receive an empty array
pub trait ToMaybeTDBSchema {
    fn to_schema() -> Option<Schema> {
        None
    }

    fn to_schema_tree() -> Vec<Schema> {
        vec![]
    }

    fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        // Do nothing by default - this type might not have a schema
    }

    // fn to_schema_tree_documents() -> Documents {
    //     Self::to_schema_tree().into()
    // }

    // fn get_context() -> Context {
    //     Context {
    //         schema: "http://parture.org/schema/woql".to_string(),
    //         base: "<parture://".to_string(),
    //         xsd: None,
    //         documentation: None
    //     }
    // }
}

#[test]
fn test_schema_enum_json() {
    // example from https://terminusdb.com/docs/index/terminusx-db/reference-guides/schema#code-an-example-of-an-enum-class
    let schema = Schema::Enum {
        id: "PrimaryColour".to_string(),
        base: None,
        values: vec!["Red".to_string(), "Blue".to_string(), "Yellow".to_string()],
        documentation: None,
    };

    assert_eq!(
        schema.to_json(),
        json!({
            "@type": "Enum",
            "@id": "PrimaryColour",
            "@value": ["Red", "Blue", "Yellow"]
        })
    )
}

#[test]
fn test_schema_taggedunion_json() {
    // example from https://terminusdb.com/docs/index/terminusx-db/reference-guides/schema#code-an-example-of-an-enum-class
    let schema = Schema::TaggedUnion {
        id: "BinaryTree".to_string(),
        base: Some("binary_tree_".to_string()),
        key: Key::ValueHash,
        r#abstract: false,
        documentation: None,
        subdocument: false,
        properties: vec![
            Property {
                name: "leaf".to_string(),
                class: "sys:Unit".to_string(),
                r#type: None,
            }
            .into(),
            Property {
                name: "node".to_string(),
                class: "Node".to_string().to_string(),
                r#type: None,
            }
            .into(),
        ],
        unfoldable: true,
    };

    assert_eq!(
        schema.to_json(),
        json!({
            "@type"     : "TaggedUnion",
            "@id"       : "BinaryTree",
            "@base"     : "binary_tree_",
            "@key": {
                "@type": "ValueHash"
            },
            "@unfoldable": [],
            "leaf": "sys:Unit",
            "node": "Node"
        })
    )
}

#[test]
fn test_schema_class_exact_json() {
    // example from https://terminusdb.com/docs/index/terminusx-db/reference-guides/schema#code-an-example-of-an-enum-class
    let schema = Schema::Class {
        id: "Dog".to_string(),
        base: Some("Dog_".to_string()),
        key: Key::Lexical(vec!["name".to_string()]),
        documentation: None,
        subdocument: false,
        r#abstract: false,
        inherits: vec![],
        properties: vec![
            Property {
                name: "name".to_string(),
                class: "xsd:string".to_string(),
                r#type: None,
            }
            .into(),
            Property {
                name: "hair_colour".to_string(),
                class: "Colour".to_string(),
                r#type: None,
            }
            .into(),
        ],
        unfoldable: true,
    };

    assert_eq!(
        schema.to_json(),
        json!({
            "@type"       : "Class",
            "@id"         : "Dog",
            "@base"       : "Dog_",
            "@key"        :
            {
                "@type"   : "Lexical",
                "@fields" : [ "name" ]
            },
            "@unfoldable": [],
            "name"        : "xsd:string",
            "hair_colour" : "Colour"
        })
    )
}

#[test]
fn test_schema_class_oneof_json() {
    // example from https://terminusdb.com/docs/index/terminusx-db/reference-guides/schema#code-an-example-of-an-enum-class
    let schema = Schema::OneOfClass {
        id: "Pet".to_string(),
        base: None,
        documentation: None,
        subdocument: false,
        r#abstract: false,
        inherits: vec![],
        properties: vec![Property {
            name: "name".to_string(),
            class: "xsd:string".to_string(),
            r#type: None,
        }
        .into()],
        classes: vec![
            vec![
                Property {
                    name: "cat".to_string(),
                    class: "Toy".to_string(),
                    r#type: None,
                },
                Property {
                    name: "dog".to_string(),
                    class: "Friend".to_string(),
                    r#type: None,
                },
            ]
            .into_iter()
            .collect::<BTreeSet<_>>(),
            vec![
                Property {
                    name: "employers".to_string(),
                    class: "xsd:positiveInteger".to_string(),
                    r#type: None,
                },
                Property {
                    name: "unemployed".to_string(),
                    class: "xsd:string".to_string(),
                    r#type: None,
                },
            ]
            .into_iter()
            .collect::<BTreeSet<_>>(),
        ],
    };

    assert_eq!(
        schema.to_json(),
        json!({
            "@type"     : "Class",
            "@id"       : "Pet",
            "name"      : "xsd:string",
            "@oneOf"    : [
                {
                    "cat" : "Toy",
                    "dog" : "Friend"
                },
                {
                    "employers" : "xsd:positiveInteger",
                    "unemployed": "xsd:string"
                },
            ]
        })
    )
}

#[test]
fn test_schema_relation_opt_json() {
    // example from https://terminusdb.com/docs/index/terminusx-db/reference-guides/schema#code-an-example-of-an-enum-class
    let schema = Schema::Class {
        id: "CodeBlock".to_string(),
        base: None,
        key: Key::ValueHash,
        documentation: None,
        subdocument: false,
        r#abstract: false,
        inherits: vec![],
        properties: vec![
            Property {
                name: "code".to_string(),
                class: "xsd:string".to_string(),
                r#type: None,
            }
            .into(),
            Property {
                name: "comment".to_string(),
                r#type: Some(TypeFamily::Optional),
                class: "xsd:string".to_string(),
            }
            .into(),
        ],
        unfoldable: true,
    };

    assert_eq!(
        schema.to_json(),
        json!({
            "@type"      : "Class",
            "@id"        : "CodeBlock",
            "@key"        :
            {
                "@type"   : "ValueHash",
            },
            "@unfoldable": [],
            "code"       : "xsd:string",
            "comment"    :
            {
                "@type"  : "Optional",
                "@class" : "xsd:string"
            }
        })
    )
}

#[test]
fn test_schema_relation_list_json() {
    // example from https://terminusdb.com/docs/index/terminusx-db/reference-guides/schema#code-an-example-of-an-enum-class
    let schema = Schema::Class {
        id: "TaskList".to_string(),
        base: None,
        key: Key::ValueHash,
        documentation: None,
        subdocument: false,
        r#abstract: false,
        inherits: vec![],
        properties: vec![Property {
            name: "tasks".to_string(),
            r#type: Some(TypeFamily::List),
            class: "Task".to_string(),
        }
        .into()],
        unfoldable: true,
    };

    assert_eq!(
        schema.to_json(),
        json!({
            "@type"      : "Class",
            "@id"        : "TaskList",
            "@unfoldable": [],
            "@key"        :
            {
                "@type"   : "ValueHash",
            },
            "tasks"    :
            {
                "@type"  : "List",
                "@class" : "Task"
            }
        })
    )
}

#[test]
fn test_schema_relation_set_json() {
    // example from https://terminusdb.com/docs/index/terminusx-db/reference-guides/schema#code-an-example-of-an-enum-class
    let schema = Schema::Class {
        id: "Person".to_string(),
        base: None,
        key: Key::ValueHash,
        documentation: None,
        subdocument: false,
        r#abstract: false,
        inherits: vec![],
        properties: vec![
            Property {
                name: "name".to_string(),
                class: "xsd:string".to_string(),
                r#type: None,
            }
            .into(),
            Property {
                name: "friends".to_string(),
                r#type: Some(TypeFamily::Set(SetCardinality::None)),
                class: "Person".to_string(),
            }
            .into(),
        ],
        unfoldable: true,
    };

    assert_eq!(
        schema.to_json(),
        json!({
            "@id"        : "Person",
            "@type"      : "Class",
            "@unfoldable": [],
            "@key"        :
            {
                "@type"   : "ValueHash",
            },
            "name"       : "xsd:string",
            "friends"    :
            {
                "@type"  : "Set",
                "@class" : "Person"
            }
        })
    )
}

#[test]
fn test_schema_relation_set_cardinality_json() {
    // example from https://terminusdb.com/docs/index/terminusx-db/reference-guides/schema#code-an-example-of-an-enum-class
    let schema = Schema::Class {
        id: "Person".to_string(),
        base: None,
        key: Key::ValueHash,
        documentation: None,
        subdocument: false,
        r#abstract: false,
        inherits: vec![],
        properties: vec![
            Property {
                name: "name".to_string(),
                class: "xsd:string".to_string(),
                r#type: None,
            }
            .into(),
            Property {
                name: "friends".to_string(),
                r#type: Some(TypeFamily::Set(SetCardinality::Exact(3))),
                class: "Person".to_string(),
            }
            .into(),
            Property {
                name: "friends2".to_string(),
                r#type: Some(TypeFamily::Set(SetCardinality::Min(5))),
                class: "Person".to_string(),
            }
            .into(),
            Property {
                name: "friends3".to_string(),
                r#type: Some(TypeFamily::Set(SetCardinality::Max(10))),
                class: "Person".to_string(),
            }
            .into(),
        ],
        unfoldable: true,
    };

    assert_eq!(
        schema.to_json(),
        json!({
            "@id"        : "Person",
            "@type"      : "Class",
            "@unfoldable": [],
            "@key"        :
            {
                "@type"   : "ValueHash",
            },
            "name"       : "xsd:string",
            "friends"    :
            {
                "@type"  : "Set",
                "@cardinality": 3,
                "@class" : "Person"
            },
            "friends2"    :
            {
                "@type"  : "Set",
                "@min_cardinality": 5,
                "@class" : "Person"
            },
            "friends3"    :
            {
                "@type"  : "Set",
                "@max_cardinality": 10,
                "@class" : "Person"
            },
        })
    )
}
