//! RDF Turtle serialization for any model implementing TerminusDB's instance traits.
//!
//! This crate deliberately serializes [`terminusdb_schema::Instance`] rather than
//! ordinary Serde data. `#[derive(TerminusDBModel)]` models may have a wire shape
//! that differs from their Serde representation; the instance tree is the canonical
//! representation used by the TerminusDB client itself.

use std::collections::{BTreeMap, BTreeSet};

use terminusdb_schema::{
    Instance, InstanceProperty, Key, PrimitiveValue, RelationValue, ToTDBInstances,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

/// Controls the RDF namespaces emitted by the serializer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TurtleOptions {
    /// Namespace used for relative TerminusDB instance IDs.
    pub base: String,
    /// Namespace used for classes and properties without a model-specific base.
    pub schema: String,
    /// Additional prefixes written in deterministic name order.
    pub prefixes: BTreeMap<String, String>,
}

impl Default for TurtleOptions {
    fn default() -> Self {
        Self {
            // These are the context IRIs installed by a new TerminusDB database.
            // They intentionally do not use terminusdb-schema's legacy Parture
            // constants, which do not match the live server's graph context.
            base: "terminusdb:///data/".to_owned(),
            schema: "terminusdb:///schema#".to_owned(),
            prefixes: BTreeMap::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TurtleError {
    #[error("invalid IRI contains a control character: {0:?}")]
    InvalidIri(String),
    #[error(
        "JSON object/array values are not representable as RDF literals at {subject}.{property}"
    )]
    JsonValue { subject: String, property: String },
}

/// Serialize any one model or model collection supported by the TerminusDB client.
pub fn to_turtle<T: ToTDBInstances + ?Sized>(value: &T) -> Result<String, TurtleError> {
    to_turtle_with(value, &TurtleOptions::default())
}

/// Serialize using explicit data and schema namespaces.
pub fn to_turtle_with<T: ToTDBInstances + ?Sized>(
    value: &T,
    options: &TurtleOptions,
) -> Result<String, TurtleError> {
    let instances = value.to_instance_tree();
    let mut writer = Writer::new(options);
    for (index, instance) in instances.iter().enumerate() {
        writer.write_instance(instance, format!("root-{index}"))?;
    }
    writer.finish()
}

struct Writer<'a> {
    options: &'a TurtleOptions,
    triples: BTreeSet<String>,
    expanded: BTreeSet<String>,
}

impl<'a> Writer<'a> {
    fn new(options: &'a TurtleOptions) -> Self {
        Self {
            options,
            triples: BTreeSet::new(),
            expanded: BTreeSet::new(),
        }
    }

    fn finish(self) -> Result<String, TurtleError> {
        validate_iri(&self.options.base)?;
        validate_iri(&self.options.schema)?;

        let mut output = format!(
            "@base <{}> .\n@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n@prefix xsd: <{XSD}> .\n@prefix doc: <{}> .\n@prefix scm: <{}> .\n",
            self.options.base, self.options.base, self.options.schema
        );
        for (prefix, iri) in &self.options.prefixes {
            validate_iri(iri)?;
            output.push_str(&format!("@prefix {prefix}: <{iri}> .\n"));
        }
        output.push('\n');
        for triple in self.triples {
            output.push_str(&triple);
            output.push('\n');
        }
        Ok(output)
    }

    fn write_instance(&mut self, instance: &Instance, path: String) -> Result<String, TurtleError> {
        let subject = self.subject(instance, &path)?;
        if !self.expanded.insert(subject.clone()) || instance.is_reference() {
            return Ok(subject);
        }

        let class = instance
            .schema
            .namespace_base()
            .map(|base| format!("{base}{}", instance.schema.class_name()))
            .unwrap_or_else(|| format!("{}{}", self.options.schema, instance.schema.class_name()));
        self.triple(&subject, RDF_TYPE, &iri_term(&class)?);

        for (name, property) in &instance.properties {
            if property.is_null() {
                continue;
            }
            let predicate_base = instance
                .schema
                .namespace_base()
                .unwrap_or(&self.options.schema);
            let predicate = format!("{predicate_base}{name}");
            let datatype = instance
                .schema
                .own_properties()
                .into_iter()
                .find(|candidate| candidate.name == *name)
                .map(|candidate| candidate.class.as_str());
            self.write_property(&subject, &predicate, name, property, datatype, &path)?;
        }
        Ok(subject)
    }

    fn write_property(
        &mut self,
        subject: &str,
        predicate: &str,
        property_name: &str,
        property: &InstanceProperty,
        datatype: Option<&str>,
        path: &str,
    ) -> Result<(), TurtleError> {
        match property {
            InstanceProperty::Primitive(value) => {
                if let Some(object) = primitive_term(value, datatype, subject, property_name)? {
                    self.triple(subject, predicate, &object);
                }
            }
            InstanceProperty::Primitives(values) => {
                for value in values {
                    if let Some(object) = primitive_term(value, datatype, subject, property_name)? {
                        self.triple(subject, predicate, &object);
                    }
                }
            }
            InstanceProperty::Relation(value) => {
                self.write_relation(subject, predicate, value, format!("{path}-{property_name}"))?;
            }
            InstanceProperty::Relations(values) => {
                for (index, value) in values.iter().enumerate() {
                    self.write_relation(
                        subject,
                        predicate,
                        value,
                        format!("{path}-{property_name}-{index}"),
                    )?;
                }
            }
            InstanceProperty::Any(values) => {
                for (index, value) in values.iter().enumerate() {
                    self.write_property(
                        subject,
                        predicate,
                        property_name,
                        value,
                        datatype,
                        &format!("{path}-{property_name}-{index}"),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn write_relation(
        &mut self,
        subject: &str,
        predicate: &str,
        relation: &RelationValue,
        path: String,
    ) -> Result<(), TurtleError> {
        match relation {
            RelationValue::ExternalReference(id) | RelationValue::TransactionRef(id) => {
                let object = self.id_subject(id)?;
                self.triple(subject, predicate, &object);
            }
            RelationValue::ExternalReferences(ids) | RelationValue::TransactionRefs(ids) => {
                for id in ids {
                    let object = self.id_subject(id)?;
                    self.triple(subject, predicate, &object);
                }
            }
            RelationValue::One(instance) => {
                let object = self.write_instance(instance, path)?;
                self.triple(subject, predicate, &object);
            }
            RelationValue::More(instances) => {
                for (index, instance) in instances.iter().enumerate() {
                    let object = self.write_instance(instance, format!("{path}-{index}"))?;
                    self.triple(subject, predicate, &object);
                }
            }
        }
        Ok(())
    }

    fn subject(&self, instance: &Instance, path: &str) -> Result<String, TurtleError> {
        match instance.id() {
            Some(id) => self.id_subject(id),
            None => match instance.schema.key() {
                Some(Key::Lexical(fields)) => {
                    let values = fields
                        .iter()
                        .filter_map(|field| instance.properties.get(field))
                        .filter_map(lexical_value)
                        .collect::<Vec<_>>();
                    if values.len() == fields.len() {
                        let encoded = values
                            .iter()
                            .map(|value| urlencoding::encode(value).into_owned())
                            .collect::<Vec<_>>()
                            .join("+");
                        self.id_subject(&format!("{}/{encoded}", instance.schema.class_name()))
                    } else {
                        Ok(format!("_:{}", blank_label(path)))
                    }
                }
                _ => Ok(format!("_:{}", blank_label(path))),
            },
        }
    }

    fn id_subject(&self, id: &str) -> Result<String, TurtleError> {
        if is_absolute_iri(id) {
            iri_term(id)
        } else {
            iri_term(&format!("{}{}", self.options.base, id))
        }
    }

    fn triple(&mut self, subject: &str, predicate: &str, object: &str) {
        self.triples
            .insert(format!("{subject} <{predicate}> {object} ."));
    }
}

fn primitive_term(
    value: &PrimitiveValue,
    datatype: Option<&str>,
    subject: &str,
    property: &str,
) -> Result<Option<String>, TurtleError> {
    let term = match value {
        PrimitiveValue::Null | PrimitiveValue::Unit => return Ok(None),
        PrimitiveValue::Bool(value) => format!("\"{value}\"^^xsd:boolean"),
        PrimitiveValue::Number(value) => {
            let dtype = expand_datatype(datatype.unwrap_or(if value.is_f64() {
                "xsd:double"
            } else {
                "xsd:integer"
            }));
            format!("\"{value}\"^^<{dtype}>")
        }
        PrimitiveValue::String(value) => {
            let literal = format!("\"{}\"", escape_literal(value));
            match datatype.filter(|d| d.starts_with("xsd:") && *d != "xsd:string") {
                Some(dtype) => format!("{literal}^^<{}>", expand_datatype(dtype)),
                None => literal,
            }
        }
        PrimitiveValue::Object(serde_json::Value::String(value)) => {
            format!("\"{}\"", escape_literal(value))
        }
        PrimitiveValue::Object(serde_json::Value::Bool(value)) => {
            format!("\"{value}\"^^xsd:boolean")
        }
        PrimitiveValue::Object(serde_json::Value::Number(value)) => value.to_string(),
        PrimitiveValue::Object(serde_json::Value::Null) => return Ok(None),
        PrimitiveValue::Object(_) => {
            return Err(TurtleError::JsonValue {
                subject: subject.to_owned(),
                property: property.to_owned(),
            })
        }
    };
    Ok(Some(term))
}

fn expand_datatype(value: &str) -> String {
    value
        .strip_prefix("xsd:")
        .map(|local| format!("{XSD}{local}"))
        .unwrap_or_else(|| value.to_owned())
}

fn lexical_value(property: &InstanceProperty) -> Option<String> {
    match property {
        InstanceProperty::Primitive(PrimitiveValue::String(value)) => Some(value.clone()),
        InstanceProperty::Primitive(PrimitiveValue::Number(value)) => Some(value.to_string()),
        InstanceProperty::Primitive(PrimitiveValue::Bool(value)) => Some(value.to_string()),
        InstanceProperty::Relation(RelationValue::ExternalReference(value))
        | InstanceProperty::Relation(RelationValue::TransactionRef(value)) => Some(value.clone()),
        InstanceProperty::Relation(RelationValue::One(instance)) => instance.id().cloned(),
        _ => None,
    }
}

fn escape_literal(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            '\t' => "\\t".chars().collect(),
            ch => vec![ch],
        })
        .collect()
}

fn iri_term(value: &str) -> Result<String, TurtleError> {
    validate_iri(value)?;
    Ok(format!("<{value}>"))
}

fn validate_iri(value: &str) -> Result<(), TurtleError> {
    if value.chars().any(char::is_control) {
        Err(TurtleError::InvalidIri(value.to_owned()))
    } else {
        Ok(())
    }
}

fn is_absolute_iri(value: &str) -> bool {
    value.find(':').is_some_and(|colon| {
        colon > 0
            && value[..colon]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

fn blank_label(path: &str) -> String {
    path.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminusdb_schema::{Ref, ToTDBInstance};
    use terminusdb_schema_derive::TerminusDBModel;

    #[derive(Clone, Debug, TerminusDBModel)]
    #[tdb(key = "lexical", key_fields = "id")]
    struct Artist {
        id: String,
        name: String,
    }

    #[derive(Clone, Debug, TerminusDBModel)]
    #[tdb(key = "lexical", key_fields = "id")]
    struct Recording {
        id: String,
        title: String,
        popularity: Option<i32>,
        artists: Vec<Ref<Artist>>,
    }

    #[test]
    fn serializes_a_generic_model_graph_deterministically() {
        let artist = Artist {
            id: "artist-1".into(),
            name: "Miles \"Dewey\" Davis".into(),
        };
        let recording = Recording {
            id: "recording-1".into(),
            title: "So What".into(),
            popularity: Some(76),
            artists: vec![artist.into()],
        };

        let first = to_turtle(&recording).unwrap();
        let second = to_turtle(&recording).unwrap();
        assert_eq!(first, second);
        assert!(first.contains("<terminusdb:///data/Recording/recording-1>"));
        assert!(first.contains("<terminusdb:///schema#Recording>"));
        assert!(first.contains("\"Miles \\\"Dewey\\\" Davis\""));
        assert!(first.contains("\"76\"^^<http://www.w3.org/2001/XMLSchema#integer>"));
        assert!(first.contains("<terminusdb:///data/Artist/artist-1>"));
    }

    #[test]
    fn serializes_collections_of_models() {
        let artists = vec![
            Artist {
                id: "a".into(),
                name: "A".into(),
            },
            Artist {
                id: "b".into(),
                name: "B".into(),
            },
        ];
        let turtle = to_turtle(&artists).unwrap();
        assert_eq!(turtle.matches("rdf-syntax-ns#type> <").count(), 2);
    }
}
