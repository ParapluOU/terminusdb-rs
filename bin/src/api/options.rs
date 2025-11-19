//! Option structs for all TerminusDB CLI commands.
//!
//! Each command has a corresponding options struct with defaults matching the CLI.

use super::types::{Author, Message, RoleAction, RdfFormat, ScopeType, CommitType};
use super::spec::GraphType;

// ============================================================================
// Common Options
// ============================================================================

/// Common impersonate option for all commands.
pub const DEFAULT_IMPERSONATE: &str = "admin";

// ============================================================================
// Help Command
// ============================================================================

/// Options for the `help` command.
#[derive(Debug, Clone, Default)]
pub struct HelpOptions {
    /// Generate help as markdown.
    pub markdown: bool,
}

// ============================================================================
// Test Command
// ============================================================================

/// Options for the `test` command.
#[derive(Debug, Clone)]
pub struct TestOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Run a specific test.
    pub test: Option<String>,
}

impl Default for TestOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            test: None,
        }
    }
}

// ============================================================================
// Serve Command
// ============================================================================

/// Options for the `serve` command.
#[derive(Debug, Clone, Default)]
pub struct ServeOptions {
    /// Run server in interactive mode.
    pub interactive: bool,
    /// Run server in-memory without persistent store.
    /// Takes optional password (if None, uses "root").
    pub memory: Option<String>,
}

// ============================================================================
// Query Command
// ============================================================================

/// Options for the `query` command.
#[derive(Debug, Clone)]
pub struct QueryOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Commit message.
    pub message: Message,
    /// Commit author.
    pub author: Author,
    /// Allow query reordering for optimization.
    pub optimize: bool,
    /// Add WOQL library for defined predicates.
    pub library: Option<String>,
    /// Return results as JSON object.
    pub json: bool,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            message: "cli query".into(),
            author: Author::default(),
            optimize: true,
            library: None,
            json: false,
        }
    }
}

// ============================================================================
// Database Commands
// ============================================================================

/// Options for `db create` command.
#[derive(Debug, Clone)]
pub struct DbCreateOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Organizational owner.
    pub organization: String,
    /// Database label.
    pub label: Option<String>,
    /// Long description/comment.
    pub comment: Option<String>,
    /// Whether database is public.
    pub public: bool,
    /// Whether to use a schema.
    pub schema: bool,
    /// URI prefix for data.
    pub data_prefix: String,
    /// URI prefix for schema.
    pub schema_prefix: String,
    /// Additional defined prefixes in JSON.
    pub prefixes: Option<String>,
}

impl Default for DbCreateOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            organization: "admin".to_string(),
            label: None,
            comment: None,
            public: false,
            schema: true,
            data_prefix: "terminusdb:///data/".to_string(),
            schema_prefix: "terminusdb:///schema#".to_string(),
            prefixes: None,
        }
    }
}

/// Options for `db delete` command.
#[derive(Debug, Clone)]
pub struct DbDeleteOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Organizational owner.
    pub organization: String,
    /// Force deletion (unsafe).
    pub force: bool,
}

impl Default for DbDeleteOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            organization: "admin".to_string(),
            force: false,
        }
    }
}

/// Options for `db list` command.
#[derive(Debug, Clone)]
pub struct DbListOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Also describe available branches.
    pub branches: bool,
    /// Return lots of metadata.
    pub verbose: bool,
    /// Return JSON result.
    pub json: bool,
}

impl Default for DbListOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            branches: false,
            verbose: false,
            json: false,
        }
    }
}

/// Options for `db update` command.
#[derive(Debug, Clone)]
pub struct DbUpdateOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Database label.
    pub label: Option<String>,
    /// Long description/comment.
    pub comment: Option<String>,
    /// Whether database is public.
    pub public: Option<bool>,
    /// Whether to use a schema.
    pub schema: Option<bool>,
    /// Explicitly defined prefix set (JSON).
    pub prefixes: Option<String>,
}

impl Default for DbUpdateOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            label: None,
            comment: None,
            public: None,
            schema: None,
            prefixes: None,
        }
    }
}

// ============================================================================
// Document Commands
// ============================================================================

/// Options for `doc insert` command.
#[derive(Debug, Clone)]
pub struct DocInsertOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Commit message.
    pub message: Message,
    /// Commit author.
    pub author: Author,
    /// Graph type (instance or schema).
    pub graph_type: GraphType,
    /// Require inferred migration.
    pub require_migration: bool,
    /// Allow destructive migration.
    pub allow_destructive_migration: bool,
    /// Document data (JSON).
    pub data: Option<String>,
    /// Insert as raw JSON.
    pub raw_json: bool,
    /// Merge repeated documents.
    pub merge_repeats: bool,
    /// Delete all previous and substitute.
    pub full_replace: bool,
}

impl Default for DocInsertOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            message: "cli: document insert".into(),
            author: Author::default(),
            graph_type: GraphType::Instance,
            require_migration: false,
            allow_destructive_migration: false,
            data: None,
            raw_json: false,
            merge_repeats: false,
            full_replace: false,
        }
    }
}

/// Options for `doc delete` command.
#[derive(Debug, Clone)]
pub struct DocDeleteOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Commit message.
    pub message: Message,
    /// Commit author.
    pub author: Author,
    /// Graph type.
    pub graph_type: GraphType,
    /// Require inferred migration.
    pub require_migration: bool,
    /// Allow destructive migration.
    pub allow_destructive_migration: bool,
    /// Document ID to delete.
    pub id: Option<String>,
    /// Document type to delete.
    pub doc_type: Option<String>,
    /// Document data.
    pub data: Option<String>,
    /// Nuke all documents.
    pub nuke: bool,
}

impl Default for DocDeleteOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            message: "cli: document delete".into(),
            author: Author::default(),
            graph_type: GraphType::Instance,
            require_migration: false,
            allow_destructive_migration: false,
            id: None,
            doc_type: None,
            data: None,
            nuke: false,
        }
    }
}

/// Options for `doc replace` command.
#[derive(Debug, Clone)]
pub struct DocReplaceOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Commit message.
    pub message: Message,
    /// Commit author.
    pub author: Author,
    /// Graph type.
    pub graph_type: GraphType,
    /// Require inferred migration.
    pub require_migration: bool,
    /// Allow destructive migration.
    pub allow_destructive_migration: bool,
    /// Document data.
    pub data: Option<String>,
    /// Replace as raw JSON.
    pub raw_json: bool,
    /// Create document if doesn't exist.
    pub create: bool,
}

impl Default for DocReplaceOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            message: "cli: document replace".into(),
            author: Author::default(),
            graph_type: GraphType::Instance,
            require_migration: false,
            allow_destructive_migration: false,
            data: None,
            raw_json: false,
            create: false,
        }
    }
}

/// Options for `doc get` command.
#[derive(Debug, Clone)]
pub struct DocGetOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Graph type.
    pub graph_type: GraphType,
    /// Number of documents to skip.
    pub skip: usize,
    /// Number of documents to return (None = unlimited).
    pub count: Option<usize>,
    /// Return minimized prefixes.
    pub minimized: bool,
    /// Return as JSON list vs JSON-lines.
    pub as_list: bool,
    /// Include subdocuments or only IDs.
    pub unfold: bool,
    /// ID of document to retrieve.
    pub id: Option<String>,
    /// List of document IDs to retrieve.
    pub ids: Vec<String>,
    /// Type of document to retrieve.
    pub doc_type: Option<String>,
    /// Return compressed/minimized IDs.
    pub compress_ids: bool,
    /// Document query search template.
    pub query: Option<String>,
}

impl Default for DocGetOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            graph_type: GraphType::Instance,
            skip: 0,
            count: None,
            minimized: true,
            as_list: false,
            unfold: true,
            id: None,
            ids: Vec::new(),
            doc_type: None,
            compress_ids: true,
            query: None,
        }
    }
}

// ============================================================================
// Git-like Commands
// ============================================================================

/// Options for `push` command.
#[derive(Debug, Clone)]
pub struct PushOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Set origin branch for push.
    pub branch: String,
    /// Set branch on remote for push.
    pub remote_branch: Option<String>,
    /// Name of remote to use.
    pub remote: String,
    /// Send prefixes for database.
    pub prefixes: bool,
    /// Machine access token.
    pub token: Option<String>,
    /// User on the remote.
    pub user: Option<String>,
    /// Password on the remote.
    pub password: Option<String>,
}

impl Default for PushOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            branch: "main".to_string(),
            remote_branch: None,
            remote: "origin".to_string(),
            prefixes: false,
            token: None,
            user: None,
            password: None,
        }
    }
}

/// Options for `clone` command.
#[derive(Debug, Clone)]
pub struct CloneOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Machine access token.
    pub token: Option<String>,
    /// User on remote.
    pub user: Option<String>,
    /// Password on remote.
    pub password: Option<String>,
    /// Organizational owner.
    pub organization: String,
    /// Database label.
    pub label: Option<String>,
    /// Remote to use.
    pub remote: String,
    /// Long description.
    pub comment: String,
    /// Whether database is public.
    pub public: bool,
}

impl Default for CloneOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            token: None,
            user: None,
            password: None,
            organization: "admin".to_string(),
            label: None,
            remote: "origin".to_string(),
            comment: String::new(),
            public: false,
        }
    }
}

/// Options for `pull` command.
#[derive(Debug, Clone)]
pub struct PullOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Set branch on remote for pull.
    pub remote_branch: Option<String>,
    /// Name of remote to use.
    pub remote: String,
    /// Machine access token.
    pub token: Option<String>,
    /// User on remote.
    pub user: Option<String>,
    /// Password on remote.
    pub password: Option<String>,
}

impl Default for PullOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            remote_branch: None,
            remote: "origin".to_string(),
            token: None,
            user: None,
            password: None,
        }
    }
}

/// Options for `fetch` command.
#[derive(Debug, Clone)]
pub struct FetchOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Name of remote to use.
    pub remote: String,
    /// Machine access token.
    pub token: Option<String>,
    /// User on remote.
    pub user: Option<String>,
    /// Password on remote.
    pub password: Option<String>,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            remote: "origin".to_string(),
            token: None,
            user: None,
            password: None,
        }
    }
}

/// Options for `rebase` command.
#[derive(Debug, Clone)]
pub struct RebaseOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// The author of the rebase.
    pub author: Author,
}

impl Default for RebaseOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            author: Author::default(),
        }
    }
}

// ============================================================================
// Branch Commands
// ============================================================================

/// Options for `branch create` command.
#[derive(Debug, Clone)]
pub struct BranchCreateOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Origin branch to use (None for no origin).
    pub origin: Option<String>,
}

impl Default for BranchCreateOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            origin: Some("main".to_string()),
        }
    }
}

/// Options for `branch delete` command.
#[derive(Debug, Clone)]
pub struct BranchDeleteOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
}

impl Default for BranchDeleteOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
        }
    }
}

// Continued in next message due to length...
