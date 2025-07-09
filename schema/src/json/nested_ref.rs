use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NestedRef<T> {
    #[serde(rename = "@type")]
    pub type_name: String,
    #[serde(rename = "@ref")]
    pub reference: String,

    _phantom: PhantomData<T>,
}
