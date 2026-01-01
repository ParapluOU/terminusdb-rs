use serde::{Deserialize, Serialize};
// Removed incorrect imports for TdbDataType, TdbDebug, TdbDisplay
use terminusdb_schema::FromTDBInstance;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

// Represents TaggedUnion "Indicator"
/// Tagged union for specifying a column by index or name.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub enum Indicator {
    /// Column index (non-negative integer).
    Index(u64),
    /// Column name (string).
    Name(String),
}

/// Specifies a column for data retrieval, mapping an indicator to a variable.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Column {
    /// Indicator (index or name) for the column.
    pub indicator: Indicator,
    /// Variable name to bind the column data to.
    pub variable: String,
    /// Optional data type hint for the column.
    #[tdb(name = "type")]
    pub type_of: Option<String>,
}

// Represents TaggedUnion "Source"
/// Tagged union specifying the source of data (POST body or URL).
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub enum Source {
    /// Data source is a POST request body.
    Post(String),
    /// Data source is a URL.
    Url(String),
}

// Represents Enum "FormatType"
/// Enum specifying the format of the input data.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub enum FormatType {
    /// Comma-Separated Values format.
    Csv,
}

// Represents TaggedUnion "QueryResource"
/// Specifies the resource for the Get query, including source, format, and options.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct QueryResource {
    // Modeled as struct based on Get usage
    /// The source of the data (URL or POST).
    pub source: Source,
    /// The format of the data (e.g., CSV).
    pub format: FormatType,
    /// Optional format-specific options.
    pub options: Option<serde_json::Value>,
}

/// Retrieves data from an external resource (CSV via URL or POST) and binds it to variables.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Get {
    /// List of columns to extract and bind to variables.
    pub columns: Vec<Column>,
    /// The resource (source, format, options) to get data from.
    pub resource: QueryResource, // Corrected based on usage in Get
    /// Optional flag indicating if the resource has a header row.
    pub has_header: Option<bool>,
}
