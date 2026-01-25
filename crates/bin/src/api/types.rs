//! Common types and enums used across the API.

use std::fmt;

/// Commit author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author(pub String);

impl Default for Author {
    fn default() -> Self {
        Self("admin".to_string())
    }
}

impl From<String> for Author {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Author {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for Author {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Commit message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message(pub String);

impl From<String> for Message {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Message {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for Message {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Role actions for capability management.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleAction {
    CreateDatabase,
    DeleteDatabase,
    ClassFrame,
    Clone,
    Fetch,
    Push,
    Branch,
    Rebase,
    InstanceReadAccess,
    InstanceWriteAccess,
    SchemaReadAccess,
    SchemaWriteAccess,
    MetaReadAccess,
    MetaWriteAccess,
    CommitReadAccess,
    CommitWriteAccess,
    ManageCapabilities,
}

impl RoleAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            RoleAction::CreateDatabase => "create_database",
            RoleAction::DeleteDatabase => "delete_database",
            RoleAction::ClassFrame => "class_frame",
            RoleAction::Clone => "clone",
            RoleAction::Fetch => "fetch",
            RoleAction::Push => "push",
            RoleAction::Branch => "branch",
            RoleAction::Rebase => "rebase",
            RoleAction::InstanceReadAccess => "instance_read_access",
            RoleAction::InstanceWriteAccess => "instance_write_access",
            RoleAction::SchemaReadAccess => "schema_read_access",
            RoleAction::SchemaWriteAccess => "schema_write_access",
            RoleAction::MetaReadAccess => "meta_read_access",
            RoleAction::MetaWriteAccess => "meta_write_access",
            RoleAction::CommitReadAccess => "commit_read_access",
            RoleAction::CommitWriteAccess => "commit_write_access",
            RoleAction::ManageCapabilities => "manage_capabilities",
        }
    }
}

impl fmt::Display for RoleAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// RDF serialization format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RdfFormat {
    #[default]
    Turtle,
}

impl RdfFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            RdfFormat::Turtle => "turtle",
        }
    }
}

impl fmt::Display for RdfFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Scope type for capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScopeType {
    #[default]
    Database,
    Organization,
}

impl ScopeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScopeType::Database => "database",
            ScopeType::Organization => "organization",
        }
    }
}

impl fmt::Display for ScopeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Commit type for apply command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommitType {
    #[default]
    Squash,
}

impl CommitType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CommitType::Squash => "squash",
        }
    }
}

impl fmt::Display for CommitType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
