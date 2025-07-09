use crate::LogEntry;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use terminusdb_schema::ToTDBSchema;

#[derive(Debug, Clone)]
pub struct CommitLogEntry {
    pub log_entry: LogEntry,
    pub commit_state: CommitState,
}

impl CommitLogEntry {
    pub fn id(&self) -> &String {
        &self.log_entry.id
    }
}

impl Deref for CommitLogEntry {
    type Target = CommitState;

    fn deref(&self) -> &Self::Target {
        &self.commit_state
    }
}

///     {
//       "action": "updated",
//       "id": "Note/m90-9qUEO4bRV4Fx"
//     }
#[derive(Deserialize, Debug, Clone)]
pub struct ObjectState {
    // todo: make enum
    pub action: String,
    pub id: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CommitState {
    #[serde(rename = "api:variable_names")]
    pub variable_names: Vec<String>,
    pub bindings: Vec<ObjectState>,
    pub deletes: usize,
    pub inserts: usize,
}

impl CommitState {
    pub fn has_added_entity<T: ToTDBSchema>(&self) -> bool {
        self.first_added_entity::<T>().is_some()
    }

    pub fn first_added_entity<T: ToTDBSchema>(&self) -> Option<&ObjectState> {
        self.bindings
            .iter()
            .find(|b| b.action == "added" && b.id.starts_with(&format!("{}/", T::schema_name())))
    }

    pub fn all_added_entity_ids(&self) -> Vec<&String> {
        self.bindings
            .iter()
            .filter(|b| b.action == "added")
            .map(|b| &b.id)
            .collect()
    }

    pub fn all_added_entities<T: ToTDBSchema>(&self) -> Vec<&ObjectState> {
        self.bindings
            .iter()
            .filter(|b| b.action == "added" && b.id.starts_with(&format!("{}/", T::schema_name())))
            .collect()
    }
}
