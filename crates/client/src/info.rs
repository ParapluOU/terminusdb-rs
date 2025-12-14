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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Database {
    #[serde(rename = "@id")]
    pub id: Option<String>,

    #[serde(rename = "@type")]
    pub database_type: Option<String>,

    pub comment: Option<String>,
    
    pub creation_date: Option<String>,
    
    pub label: Option<String>,
    
    pub name: Option<String>,
    
    pub state: Option<String>,
    
    // The actual response seems to use "path" as the main identifier
    pub path: Option<String>,
    
    // When branches=true, we get a branches array
    pub branches: Option<Vec<String>>,
}

impl Database {
    /// Extracts the database name from the path.
    /// For example, "admin/mydb" returns "mydb"
    pub fn database_name(&self) -> Option<String> {
        self.path.as_ref().and_then(|p| {
            p.split('/').last().map(|s| s.to_string())
        })
    }
    
    /// Extracts the organization from the path.
    /// For example, "admin/mydb" returns "admin"
    pub fn organization(&self) -> Option<String> {
        self.path.as_ref().and_then(|p| {
            let parts: Vec<&str> = p.split('/').collect();
            if parts.len() >= 2 {
                Some(parts[0].to_string())
            } else {
                None
            }
        })
    }
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
