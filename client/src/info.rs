use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Info {
    #[serde(rename = "@type")]
    pub response_type: String,

    #[serde(rename = "api:info")]
    pub info: ApiInfo,

    #[serde(rename = "api:status")]
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiInfo {
    pub authority: String,

    pub storage: StorageInfo,

    pub terminusdb: TerminusDbInfo,

    #[serde(rename = "terminusdb_store")]
    pub terminusdb_store: StoreInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageInfo {
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TerminusDbInfo {
    pub git_hash: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoreInfo {
    pub version: String,
}
