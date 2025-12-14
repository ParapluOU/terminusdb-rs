use crate::{json::ToJson, Field};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

// TDB uses "Random" as default
#[derive(Eq, PartialEq, PartialOrd, Ord, Debug, Clone, Hash, Serialize, Deserialize)]
pub enum Key {
    /// A Lexical key specifies a URI name formed from a URI encoded combination of all @fields arguments provided,
    /// in the order provided. An example is shown below.
    /// With this key type (or key strategy) a URI is formed from the combination of first_name and last_name.
    /// If @base is specified in the class, this is prepended.
    Lexical(Vec<Field>),

    /// Hash is generated in the same way as Lexical except that values are first hashed using the SHA-256 hash algorithm.
    /// Use this where there:
    /// - Are numerous items that form the key making the URI unwieldy.
    /// - Is no need for the URI to inform the user of the content of the object.
    /// - Is a requirement that data about the object is not be revealed by the key.
    /// Define a Hash in the same way as the Lexical key strategy example in the previous section.
    /// replacing the @key @type value from Lexical to Hash.
    /// Example: Person_5dd7004081e437b3e684075fa3132542f5cd06c1
    Hash(Vec<Field>),

    /// The ValueHash key generates a key defined as the downward transitive closure of the directed acyclic graph from the root of the document.
    /// This means you can produce a key that is entirely based on the entire data object. Note ValueHash:
    /// - Takes no additional keywords.
    /// - Objects must be directed acyclic graphs, they cannot be cyclic.
    ValueHash,

    /// Use Random as a convenient key type when an object has no important characteristics that
    /// inform a key or does not need to be constructed such that it is reproducible.
    /// In the example below, the @key @type is defined as Random, meaning each new database that
    /// is added is unique regardless of label.
    Random,
}

// todo: replace with derive attr
impl Default for Key {
    fn default() -> Self {
        Self::ValueHash
    }
}

// todo: refactor to serde-compatible
impl ToJson for Key {
    fn to_map(&self) -> Map<String, Value> {
        let mut map = serde_json::Map::new();

        match self {
            Key::Lexical(fields) => {
                map.insert("@type".to_string(), "Lexical".into());
                map.insert("@fields".to_string(), fields.clone().into());
            }
            Key::Hash(fields) => {
                map.insert("@type".to_string(), "Hash".into());
                map.insert("@fields".to_string(), fields.clone().into());
            }
            Key::ValueHash => {
                map.insert("@type".to_string(), "ValueHash".into());
            }
            Key::Random => {
                map.insert("@type".to_string(), "Random".into());
            }
        }

        map
    }
}
