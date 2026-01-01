use crate::{json::ToJson, SetCardinality};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Use type families to construct optionality or collections of values. Type families are List, Set, Array, and Optional.
#[derive(Eq, PartialEq, PartialOrd, Ord, Debug, Clone, Copy, Hash)]
pub enum TypeFamily {
    /// Use List to specify an ordered collection, with multiplicity, of values of a class or datatype.
    /// Code: An example of type family List
    /// {
    //     "@type"      : "@context",
    //     "@base"      : "http://i/",
    //     "@schema"    : "http://s/"
    // }
    // {
    //     "@id"        : "TaskList",
    //     "@type"      : "Class",
    //     "tasks"      :
    //     {
    //         "@type"  : "List",
    //         "@class" : "Task"
    //     }
    // }
    // {
    //     "@id"        : "Task",
    //     "@type"      : "Class",
    //     "@key"       : "ValueHash",
    //     "name"       : "xsd:string"
    // }
    /// An example of an object Task contained in a List of elements known as a TaskList. This list is retrieved in the same order that it is inserted. It is also capable of storing duplicates.
    /// {
    //     "@id"   : "my_task_list",
    //     "@type" : "TaskList",
    //     "tasks" :
    //     [
    //         {
    //             "@type" : "Task",
    //             "name"  : "Laundry"
    //         },
    //         {
    //             "@type" : "Task",
    //             "name"  : "Take_Garage_Out"
    //         }
    //     ]
    // }
    List,

    /// Use Set to specify an unordered set of values of a class or datatype.
    /// Code: An example of type family Set:
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
    //     }
    // }
    /// An example of an object Person that can have 0 to any number of friends. This list has no order and is retrieved from the database in a potentially different order. Inserted duplicates do not create additional linkages and only a single of the multiple supplied results are returned.
    /// {
    //     "@id"           : "Me",
    //     "@type"         : "Person",
    //     "friends"       :
    //     [
    //         {
    //             "@type" : "Person",
    //             "@id"   : "you",
    //             "name"  : "You"
    //         },
    //         {
    //             "@type" : "Person",
    //             "@id"   : "someone_else",
    //             "name"  : "Someone Else"
    //         }
    //     ]
    // }
    Set(SetCardinality),

    /// Use Array to specify an ordered collection, with multiplicity, of values of a class or datatype in which you may want random access to the data and which may be multi-dimensional. Array is implemented with intermediate indexed objects, with a sys:value and indexes placed at sys:index, sys:index2, ... sys:indexN for each of the array indices of the multi-dimensional array. However when extracted as JSON they will appear merely as lists (possibly of lists), with possible null values representing gaps in the array.
    /// {
    //     "@type"      : "@context",
    //     "@base"      : "http://i/",
    //     "@schema"    : "http://s/"
    // }
    // {
    //     "@id"        : "GeoPolygon",
    //     "@type"      : "Class",
    //     "name"       : "xsd:string",
    //     "coordinates"    :
    //     {
    //         "@type"  : "Array",
    //         "@dimensions" : 2,
    //         "@class" : "xsd:decimal"
    //     }
    // }
    /// An example of a polygon object GeoPolygon points to a 2D array of coordinates which specify a polygon encompassing the Phoneix Park.
    /// {
    //     "@id"           : "PhoenixPark",
    //     "@type"         : "GeoPolygon",
    //     "name"          : "The Pheonix Park",
    //     "coordinates"   :
    //     [
    //       [
    //         -6.3491535,
    //         53.3700669
    //       ],
    //       [
    //         -6.3364506,
    //         53.3717056
    //       ],
    //       [
    //         -6.349411,
    //         53.3699645
    //       ]
    //     ]
    // }
    Array(usize),

    /// Use Optional as a type family where a property is not required.
    /// {
    //     "@type"      : "@context",
    //     "@schema"    : "http://example.com/people#",
    //     "@base"      : "http://example.com/people/" }
    //
    // {
    //     "@type"      : "Class",
    //     "@id"        : "CodeBlock",
    //     "code"       : "xsd:string",
    //     "comment"    :
    //     {
    //         "@type"  : "Optional",
    //         "@class" : "xsd:string"
    //     }
    // }
    /// Supply an optional comment field in CodeBlock. Both of the following documents are valid:
    /// {
    //     "@type"   : "CodeBlock",
    //     "@id"     : "my_code_block",
    //     "code"    : "print('hello world')",
    //     "comment" : "This is a silly bit of code"
    // }
    ///
    /// OR:
    /// {
    //     "@type" : "CodeBlock",
    //     "@id"   : "my_code_block",
    //     "code"  : "print('hello world')"
    // }
    Optional,
}

impl TypeFamily {
    pub fn to_string(&self) -> &str {
        match self {
            TypeFamily::List => "List",
            TypeFamily::Set(maybeCardinality) => "Set",
            TypeFamily::Array(dimensions) => "Array",
            TypeFamily::Optional => "Optional",
        }
    }

    pub fn is_array(&self) -> bool {
        matches!(self, TypeFamily::Array(_))
    }

    pub fn is_set(&self) -> bool {
        matches!(self, TypeFamily::Set(_))
    }

    pub fn is_optional(&self) -> bool {
        matches!(self, TypeFamily::Optional)
    }

    pub fn is_list(&self) -> bool {
        matches!(self, TypeFamily::List)
    }
}

// todo: refactor into serde-compatible
impl ToJson for TypeFamily {
    fn to_map(&self) -> Map<String, Value> {
        let mut map = serde_json::Map::new();

        match self {
            TypeFamily::Set(cardinality) => return cardinality.to_map(),
            TypeFamily::Array(dimensions) => {
                map.insert("@dimensions".to_string(), (*dimensions).into());
            }
            _ => {}
        }

        map
    }
}
