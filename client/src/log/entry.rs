use crate::log::Migration;
use serde::{Deserialize, Serialize};

/// {
//     "@id":"ValidCommit/iviupfwgw9mlond3ax3ky728zzcdv06",
//     "@type":"ValidCommit",
//     "author":"system",
//     "identifier":"iviupfwgw9mlond3ax3ky728zzcdv06",
//     "message":"initial schema registration",
//     "migration": [
//       {
//         "@type":"CreateClass",
//         "class_document": {
//           "@id":"Tempo",
//           "@key": {"@type":"Random"},
//           "@type":"Class",
//           "@unfoldable": [],
//           "name": {"@class":"xsd:string", "@type":"Optional"},
//           "value":"xsd:unsignedInt"
//         }
//       }
//     ],
//     "schema":"layer_data:Layer_1c62ac127cdf23607a145076dc5ee060bffa004cfa8d8760f65f44b659ad05ae",
//     "timestamp":1719865624.9757895
//   }
#[derive(Deserialize, Debug, Clone)]
pub struct LogEntry {
    /// should alwas be "ValidCommit/...
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub ty: String,
    pub author: String,
    /// the commit identifier that comes after the ValidCommit prefix
    pub identifier: String,
    // parent commit
    pub parent: Option<String>,
    // link to document instance if created?
    pub instance: Option<String>,
    pub message: String,
    #[serde(default)]
    pub migration: Vec<Migration>,
    pub schema: String,
    pub timestamp: f64,
}
