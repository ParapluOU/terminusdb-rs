use std::marker::PhantomData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NestedRef<T> {
    #[serde(rename = "@type")]
    pub type_name: String,
    #[serde(rename = "@ref")]
    pub reference: String,

    _phantom: PhantomData<T>

}