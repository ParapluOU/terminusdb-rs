//! Option structs for all TerminusDB CLI commands.
//!
//! Each command has a corresponding options struct with defaults matching the CLI.

use super::spec::GraphType;
use super::types::{Author, Message, RdfFormat, ScopeType};

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

// ============================================================================
// Role Commands
// ============================================================================

/// Options for `role create` command.
#[derive(Debug, Clone)]
pub struct RoleCreateOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
}

impl Default for RoleCreateOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
        }
    }
}

/// Options for `role delete` command.
#[derive(Debug, Clone)]
pub struct RoleDeleteOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Interpret argument as role ID instead of name.
    pub id: bool,
}

impl Default for RoleDeleteOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            id: false,
        }
    }
}

/// Options for `role update` command.
#[derive(Debug, Clone)]
pub struct RoleUpdateOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Interpret argument as role ID instead of name.
    pub id: bool,
}

impl Default for RoleUpdateOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            id: false,
        }
    }
}

/// Options for `role get` command.
#[derive(Debug, Clone)]
pub struct RoleGetOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Interpret argument as role ID instead of name.
    pub id: bool,
    /// Return as JSON document.
    pub json: bool,
}

impl Default for RoleGetOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            id: false,
            json: false,
        }
    }
}

// ============================================================================
// User Commands
// ============================================================================

/// Options for `user create` command.
#[derive(Debug, Clone)]
pub struct UserCreateOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Specify user password.
    pub password: Option<String>,
}

impl Default for UserCreateOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            password: None,
        }
    }
}

/// Options for `user delete` command.
#[derive(Debug, Clone)]
pub struct UserDeleteOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Interpret argument as user ID instead of name.
    pub id: bool,
}

impl Default for UserDeleteOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            id: false,
        }
    }
}

/// Options for `user get` command.
#[derive(Debug, Clone)]
pub struct UserGetOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Interpret argument as user ID instead of name.
    pub id: bool,
    /// Report all capabilities.
    pub capability: bool,
    /// Return as JSON document.
    pub json: bool,
}

impl Default for UserGetOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            id: false,
            capability: false,
            json: false,
        }
    }
}

/// Options for `user password` command.
#[derive(Debug, Clone)]
pub struct UserPasswordOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Specify user password.
    pub password: Option<String>,
}

impl Default for UserPasswordOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            password: None,
        }
    }
}

// ============================================================================
// Organization Commands
// ============================================================================

/// Options for `organization create` command.
#[derive(Debug, Clone)]
pub struct OrganizationCreateOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
}

impl Default for OrganizationCreateOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
        }
    }
}

/// Options for `organization delete` command.
#[derive(Debug, Clone)]
pub struct OrganizationDeleteOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Interpret argument as organization ID instead of name.
    pub id: bool,
}

impl Default for OrganizationDeleteOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            id: false,
        }
    }
}

/// Options for `organization get` command.
#[derive(Debug, Clone)]
pub struct OrganizationGetOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Interpret argument as organization ID instead of name.
    pub id: bool,
    /// Return as JSON document.
    pub json: bool,
}

impl Default for OrganizationGetOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            id: false,
            json: false,
        }
    }
}

// ============================================================================
// Capability Commands
// ============================================================================

/// Options for `capability grant` command.
#[derive(Debug, Clone)]
pub struct CapabilityGrantOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Scope type (database or organization).
    pub scope_type: ScopeType,
}

impl Default for CapabilityGrantOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            scope_type: ScopeType::Database,
        }
    }
}

/// Options for `capability revoke` command.
#[derive(Debug, Clone)]
pub struct CapabilityRevokeOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Scope type (database or organization).
    pub scope_type: ScopeType,
}

impl Default for CapabilityRevokeOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            scope_type: ScopeType::Database,
        }
    }
}

// ============================================================================
// Store Commands
// ============================================================================

/// Options for `store init` command.
#[derive(Debug, Clone)]
pub struct StoreInitOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Key for admin login.
    pub key: String,
    /// Force creation even when store exists.
    pub force: bool,
}

impl Default for StoreInitOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            key: "root".to_string(),
            force: false,
        }
    }
}

// ============================================================================
// Triples Commands
// ============================================================================

/// Options for `triples dump` command.
#[derive(Debug, Clone)]
pub struct TriplesDumpOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// RDF format.
    pub format: RdfFormat,
}

impl Default for TriplesDumpOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            format: RdfFormat::Turtle,
        }
    }
}

/// Options for `triples update` command.
#[derive(Debug, Clone)]
pub struct TriplesUpdateOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Commit message.
    pub message: Message,
    /// Commit author.
    pub author: Author,
    /// RDF format.
    pub format: RdfFormat,
}

impl Default for TriplesUpdateOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            message: "cli: triples update".into(),
            author: Author::default(),
            format: RdfFormat::Turtle,
        }
    }
}

/// Options for `triples load` command.
#[derive(Debug, Clone)]
pub struct TriplesLoadOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Commit message.
    pub message: Message,
    /// Commit author.
    pub author: Author,
    /// RDF format.
    pub format: RdfFormat,
}

impl Default for TriplesLoadOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            message: "cli: triples load".into(),
            author: Author::default(),
            format: RdfFormat::Turtle,
        }
    }
}

// ============================================================================
// Remote Commands
// ============================================================================

/// Options for `remote add` command.
#[derive(Debug, Clone)]
pub struct RemoteAddOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
}

impl Default for RemoteAddOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
        }
    }
}

/// Options for `remote remove` command.
#[derive(Debug, Clone)]
pub struct RemoteRemoveOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
}

impl Default for RemoteRemoveOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
        }
    }
}

/// Options for `remote set-url` command.
#[derive(Debug, Clone)]
pub struct RemoteSetUrlOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
}

impl Default for RemoteSetUrlOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
        }
    }
}

/// Options for `remote get-url` command.
#[derive(Debug, Clone)]
pub struct RemoteGetUrlOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Name of remote to use.
    pub remote: String,
}

impl Default for RemoteGetUrlOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            remote: "origin".to_string(),
        }
    }
}

/// Options for `remote list` command.
#[derive(Debug, Clone)]
pub struct RemoteListOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
}

impl Default for RemoteListOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
        }
    }
}

// ============================================================================
// Utility Commands
// ============================================================================

/// Options for `optimize` command.
#[derive(Debug, Clone)]
pub struct OptimizeOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
}

impl Default for OptimizeOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
        }
    }
}

/// Options for `squash` command.
#[derive(Debug, Clone)]
pub struct SquashOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Output result as JSON.
    pub json: bool,
    /// Commit message.
    pub message: Message,
    /// Commit author.
    pub author: Author,
}

impl Default for SquashOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            json: false,
            message: "cli: squash".into(),
            author: Author::default(),
        }
    }
}

/// Options for `rollup` command.
#[derive(Debug, Clone)]
pub struct RollupOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
}

impl Default for RollupOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
        }
    }
}

/// Options for `bundle` command.
#[derive(Debug, Clone)]
pub struct BundleOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// File name for pack output.
    pub output: Option<String>,
}

impl Default for BundleOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            output: None,
        }
    }
}

/// Options for `unbundle` command.
#[derive(Debug, Clone)]
pub struct UnbundleOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
}

impl Default for UnbundleOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
        }
    }
}

/// Options for `log` command.
#[derive(Debug, Clone)]
pub struct LogOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
    /// Return log as JSON.
    pub json: bool,
    /// How far back in commit log to start.
    pub start: i32,
    /// Number of results to return (-1 = all).
    pub count: i32,
    /// Give back additional information on commits.
    pub verbose: bool,
}

impl Default for LogOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
            json: false,
            start: 0,
            count: -1,
            verbose: false,
        }
    }
}

/// Options for `reset` command.
#[derive(Debug, Clone)]
pub struct ResetOptions {
    /// Impersonate a particular user.
    pub impersonate: String,
}

impl Default for ResetOptions {
    fn default() -> Self {
        Self {
            impersonate: DEFAULT_IMPERSONATE.to_string(),
        }
    }
}
