use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PathError {
    #[error("Invalid database name: {0}")]
    InvalidDatabaseName(String),

    #[error("Invalid database path: {0}")]
    InvalidDatabasePath(String),

    #[error("Invalid resource path: {0}")]
    InvalidResourcePath(String),

    #[error("System database cannot be used in this context: {0}")]
    SystemDatabase(String),

    #[error("Empty path component")]
    EmptyComponent,
}

/// A database name without organization prefix (e.g., "mydb")
/// System databases (starting with underscore) are allowed but marked
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DatabaseName(String);

impl DatabaseName {
    /// Create a new DatabaseName with validation
    pub fn new(name: impl Into<String>) -> Result<Self, PathError> {
        let name = name.into();

        if name.is_empty() {
            return Err(PathError::EmptyComponent);
        }

        // Additional validation could go here (e.g., character restrictions)
        Ok(Self(name))
    }

    /// Create a new DatabaseName without validation (use with caution)
    pub fn new_unchecked(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Check if this is a system database (starts with underscore)
    pub fn is_system_database(&self) -> bool {
        self.0.starts_with('_')
    }

    /// Get the inner string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume and return the inner string
    pub fn into_string(self) -> String {
        self.0
    }
}

impl FromStr for DatabaseName {
    type Err = PathError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl fmt::Display for DatabaseName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for DatabaseName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A database path with organization prefix (e.g., "admin/mydb")
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DatabasePath {
    organization: String,
    database: DatabaseName,
}

impl DatabasePath {
    /// Create a new DatabasePath
    pub fn new(organization: impl Into<String>, database: DatabaseName) -> Result<Self, PathError> {
        let organization = organization.into();

        if organization.is_empty() {
            return Err(PathError::EmptyComponent);
        }

        Ok(Self {
            organization,
            database,
        })
    }

    /// Parse from a string like "admin/mydb"
    ///
    /// Only accepts paths with exactly 2 components separated by '/'.
    /// Paths with more or fewer components are rejected.
    pub fn parse(path: &str) -> Result<Self, PathError> {
        let parts: Vec<&str> = path.split('/').collect();

        if parts.len() != 2 {
            return Err(PathError::InvalidDatabasePath(format!(
                "Expected exactly 2 components 'organization/database', got {} components in '{}'",
                parts.len(),
                path
            )));
        }

        let organization = parts[0];
        let database = parts[1];

        if organization.is_empty() || database.is_empty() {
            return Err(PathError::InvalidDatabasePath(format!(
                "Organization and database name cannot be empty: '{}'",
                path
            )));
        }

        Ok(Self {
            organization: organization.to_string(),
            database: DatabaseName::new(database)?,
        })
    }

    /// Get the organization
    pub fn organization(&self) -> &str {
        &self.organization
    }

    /// Get the database name
    pub fn database(&self) -> &DatabaseName {
        &self.database
    }

    /// Get just the database name as a string (without organization prefix)
    pub fn database_name(&self) -> &str {
        self.database.as_str()
    }

    /// Check if this is a system database
    pub fn is_system_database(&self) -> bool {
        self.database.is_system_database()
    }

    /// Convert to a full path string
    pub fn to_path_string(&self) -> String {
        format!("{}/{}", self.organization, self.database)
    }
}

impl FromStr for DatabasePath {
    type Err = PathError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for DatabasePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.organization, self.database)
    }
}

/// Location type for resource paths
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Location {
    Local,
    Remote,
}

impl FromStr for Location {
    type Err = PathError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "local" => Ok(Location::Local),
            "remote" => Ok(Location::Remote),
            _ => Err(PathError::InvalidResourcePath(format!(
                "Invalid location '{}', expected 'local' or 'remote'",
                s
            ))),
        }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Location::Local => write!(f, "local"),
            Location::Remote => write!(f, "remote"),
        }
    }
}

/// Resource type for resource paths
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Branch(String),
    Commit(String),
    Meta,
    Commits,
    Remote(String),
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceType::Branch(name) => write!(f, "branch/{}", name),
            ResourceType::Commit(id) => write!(f, "commit/{}", id),
            ResourceType::Meta => write!(f, "_meta"),
            ResourceType::Commits => write!(f, "_commits"),
            ResourceType::Remote(name) => write!(f, "remote/{}", name),
        }
    }
}

/// A full resource path (e.g., "admin/mydb/local/branch/main" or "admin/mydb/local/_meta")
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourcePath {
    database_path: DatabasePath,
    location: Location,
    resource: ResourceType,
}

impl ResourcePath {
    /// Create a new ResourcePath
    pub fn new(database_path: DatabasePath, location: Location, resource: ResourceType) -> Self {
        Self {
            database_path,
            location,
            resource,
        }
    }

    /// Parse from a string like "admin/mydb/local/branch/main"
    pub fn parse(path: &str) -> Result<Self, PathError> {
        let parts: Vec<&str> = path.split('/').collect();

        if parts.len() < 4 {
            return Err(PathError::InvalidResourcePath(format!(
                "Expected format 'org/db/location/resource', got '{}'",
                path
            )));
        }

        // Parse org/db
        let database_path = DatabasePath::parse(&format!("{}/{}", parts[0], parts[1]))?;

        // Parse location
        let location = Location::from_str(parts[2])?;

        // Parse resource type
        let resource = if parts.len() == 4 {
            // Could be _meta, _commits, etc.
            match parts[3] {
                "_meta" => ResourceType::Meta,
                "_commits" => ResourceType::Commits,
                _ => {
                    return Err(PathError::InvalidResourcePath(format!(
                        "Invalid resource type '{}'",
                        parts[3]
                    )))
                }
            }
        } else if parts.len() >= 5 {
            // branch/name, commit/id, remote/name
            match parts[3] {
                "branch" => ResourceType::Branch(parts[4..].join("/")),
                "commit" => ResourceType::Commit(parts[4..].join("/")),
                "remote" => ResourceType::Remote(parts[4..].join("/")),
                _ => {
                    return Err(PathError::InvalidResourcePath(format!(
                        "Invalid resource type '{}'",
                        parts[3]
                    )))
                }
            }
        } else {
            return Err(PathError::InvalidResourcePath(format!(
                "Invalid resource path '{}'",
                path
            )));
        };

        Ok(Self {
            database_path,
            location,
            resource,
        })
    }

    /// Get the database path
    pub fn database_path(&self) -> &DatabasePath {
        &self.database_path
    }

    /// Get the location
    pub fn location(&self) -> Location {
        self.location
    }

    /// Get the resource type
    pub fn resource(&self) -> &ResourceType {
        &self.resource
    }

    /// Convert to a full path string
    pub fn to_path_string(&self) -> String {
        format!("{}/{}/{}", self.database_path, self.location, self.resource)
    }
}

impl FromStr for ResourcePath {
    type Err = PathError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for ResourcePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{}/{}",
            self.database_path, self.location, self.resource
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_name() {
        let name = DatabaseName::new("mydb").unwrap();
        assert_eq!(name.as_str(), "mydb");
        assert!(!name.is_system_database());

        let system_name = DatabaseName::new("_meta").unwrap();
        assert!(system_name.is_system_database());

        assert!(DatabaseName::new("").is_err());
    }

    #[test]
    fn test_database_path_parse() {
        let path = DatabasePath::parse("admin/mydb").unwrap();
        assert_eq!(path.organization(), "admin");
        assert_eq!(path.database_name(), "mydb");
        assert_eq!(path.to_path_string(), "admin/mydb");
        assert!(!path.is_system_database());

        let system_path = DatabasePath::parse("admin/_meta").unwrap();
        assert!(system_path.is_system_database());

        assert!(DatabasePath::parse("admin").is_err());
        assert!(DatabasePath::parse("admin/mydb/extra").is_err());
        assert!(DatabasePath::parse("admin/").is_err());
        assert!(DatabasePath::parse("/mydb").is_err());
    }

    #[test]
    fn test_resource_path_parse() {
        // Test branch path
        let path = ResourcePath::parse("admin/mydb/local/branch/main").unwrap();
        assert_eq!(path.database_path().database_name(), "mydb");
        assert_eq!(path.location(), Location::Local);
        match path.resource() {
            ResourceType::Branch(name) => assert_eq!(name, "main"),
            _ => panic!("Expected branch resource"),
        }
        assert_eq!(path.to_path_string(), "admin/mydb/local/branch/main");

        // Test meta path
        let meta_path = ResourcePath::parse("admin/mydb/local/_meta").unwrap();
        match meta_path.resource() {
            ResourceType::Meta => (),
            _ => panic!("Expected meta resource"),
        }

        // Test invalid paths
        assert!(ResourcePath::parse("admin/mydb").is_err());
        assert!(ResourcePath::parse("admin/mydb/invalid/branch/main").is_err());
    }

    #[test]
    fn test_location_parse() {
        assert_eq!(Location::from_str("local").unwrap(), Location::Local);
        assert_eq!(Location::from_str("remote").unwrap(), Location::Remote);
        assert!(Location::from_str("invalid").is_err());
    }
}
