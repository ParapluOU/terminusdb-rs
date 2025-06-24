use crate::json::ToJson;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Use Cardinality to specify an unordered set of values of a class or datatype in which the property has a limited number of elements as specified by the cardinality constraint properties.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Serialize, Deserialize)]
pub enum SetCardinality {
    /// @cardinality
    /// When specified, the number of elements for the given property must be exactly the cardinality specified. This is equivalent to specifying both @min_cardinality and @max_cardinality as the same cardinality.
    ///
    /// Code: An example of type family Cardinality with @cardinality
    /// {
    //     "@type"      : "@context",
    //     "@base"      : "http://i/",
    //     "@schema"    : "http://s/"
    // }
    // {
    //     "@id"        : "Person",
    //     "@type"      : "Class",
    //     "name"       : "xsd:string",
    //     "friends"    :
    //     {
    //         "@type"  : "Set",
    //         "@class" : "Person"
    //         "@cardinality" : 3
    //     }
    // }
    /// An example of an object Person that can have exactly threefriends. As with Set This list has no order and is retrieved from the database in a potentially different order.
    /// {
    //     "@id"           : "Person/Me",
    //     "@type"         : "Person",
    //     "friends"       :
    //     [
    //         {
    //             "@type" : "Person",
    //             "@id"   : "Person/you",
    //             "name"  : "You"
    //         },
    //         {
    //             "@type" : "Person",
    //             "@id"   : "Person/someone_else",
    //             "name"  : "Someone Else"
    //         },
    //         {
    //             "@type" : "Person",
    //             "@id"   : "Person/Another",
    //             "name"  : "Another"
    //         }
    //     ]
    // }
    Exact(usize),
    /// @min_cardinality
    /// When specified, the number of elements for the given property must be at least the cardinality specified.
    /// {
    //     "@type"      : "@context",
    //     "@base"      : "http://i/",
    //     "@schema"    : "http://s/"
    // }
    // {
    //     "@id"        : "Person",
    //     "@type"      : "Class",
    //     "name"       : "xsd:string",
    //     "friends"    :
    //     {
    //         "@type"  : "Set",
    //         "@class" : "Person"
    //         "@min_cardinality" : 1
    //     }
    // }
    ///
    Min(usize),
    /// @max_cardinality
    /// When specified, the number of elements for the given property must be no more than the cardinality specified.
    ///
    /// {
    //     "@type"      : "@context",
    //     "@base"      : "http://i/",
    //     "@schema"    : "http://s/"
    // }
    // {
    //     "@id"        : "Person",
    //     "@type"      : "Class",
    //     "name"       : "xsd:string",
    //     "friends"    :
    //     {
    //         "@type"  : "Set",
    //         "@class" : "Person"
    //         "@min_cardinality" : 1
    //     }
    // }
    ///
    /// When set to 1, this is functionally equivalent to the Optional constraint.
    Max(usize),
    Range {
        min: usize,
        max: usize,
    },
    None,
}

// todo: refactor into serde-compatible
impl ToJson for SetCardinality {
    fn to_map(&self) -> Map<String, Value> {
        let mut map = serde_json::Map::new();

        match self {
            SetCardinality::Exact(num) => {
                map.insert("@cardinality".to_string(), (*num).into());
            }
            SetCardinality::Min(num) => {
                map.insert("@min_cardinality".to_string(), (*num).into());
            }
            SetCardinality::Max(num) => {
                map.insert("@max_cardinality".to_string(), (*num).into());
            }
            SetCardinality::Range { min, max } => {
                map.insert("@min_cardinality".to_string(), (*min).into());
                map.insert("@max_cardinality".to_string(), (*max).into());
            }
            SetCardinality::None => {}
        }

        map
    }
}
