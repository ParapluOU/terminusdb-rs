//! Command-line interface definitions (clap `Parser`/`Subcommand` types).

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tdb")]
#[command(about = "TerminusDB CLI - Command line interface for TerminusDB operations", long_about = None)]
#[command(version)]
pub(crate) struct Cli {
    /// Profile name to use for credentials (uses active profile if not specified)
    #[arg(long, global = true)]
    profile: Option<String>,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Stream changeset events from TerminusDB SSE endpoint
    Changestream {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG", default_value = "admin")]
        org: String,

        /// Database name to monitor
        #[arg(long, env = "TERMINUSDB_DB")]
        database: Option<String>,

        /// Branch name to monitor
        #[arg(long, env = "TERMINUSDB_BRANCH", default_value = "main")]
        branch: String,

        /// Output format: json, compact, or pretty (default)
        #[arg(long, default_value = "pretty")]
        format: String,

        /// Color output: auto (default), always, or never
        #[arg(long, default_value = "auto")]
        color: String,
    },

    /// Remote repository management commands
    Remote {
        #[command(subcommand)]
        command: RemoteCommands,
    },

    /// Clone a remote repository to create a new database
    Clone {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication (for local TerminusDB)
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization to create database in
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Name for the new database
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// URL of the remote repository to clone
        #[arg(long)]
        remote_url: String,

        /// Optional label for the database
        #[arg(long)]
        label: Option<String>,

        /// Optional comment for the database
        #[arg(long)]
        comment: Option<String>,

        /// Remote authentication in format "username:password" (for private repos)
        #[arg(long)]
        remote_auth: Option<String>,
    },

    /// Fetch changes from a remote repository
    Fetch {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication (for local TerminusDB)
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Branch name
        #[arg(long, env = "TERMINUSDB_BRANCH", default_value = "main")]
        branch: String,

        /// Name of the remote repository
        #[arg(long)]
        remote_url: String,

        /// Remote branch name
        #[arg(long, default_value = "main")]
        remote_branch: String,

        /// Remote authentication in format "username:password" (for private repos)
        #[arg(long)]
        remote_auth: Option<String>,
    },

    /// Pull changes from a remote repository (fetch + merge)
    Pull {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication (for local TerminusDB)
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Branch name
        #[arg(long, env = "TERMINUSDB_BRANCH", default_value = "main")]
        branch: String,

        /// URL of the remote repository
        #[arg(long)]
        remote_url: String,

        /// Optional remote branch name
        #[arg(long)]
        remote_branch: Option<String>,

        /// Author for the merge commit
        #[arg(long)]
        author: String,

        /// Message for the merge commit
        #[arg(long)]
        message: String,

        /// Remote authentication in format "username:password" (for private repos)
        #[arg(long)]
        remote_auth: Option<String>,
    },

    /// Push changes to a remote repository
    Push {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication (for local TerminusDB)
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Branch name
        #[arg(long, env = "TERMINUSDB_BRANCH", default_value = "main")]
        branch: String,

        /// URL of the remote repository
        #[arg(long)]
        remote_url: String,

        /// Optional remote branch name
        #[arg(long)]
        remote_branch: Option<String>,

        /// Remote authentication in format "username:password" (for private repos)
        #[arg(long)]
        remote_auth: Option<String>,
    },

    /// Optimize a database graph (branch or metadata)
    Optimize {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Branch name (ignored if --meta is used)
        #[arg(long, env = "TERMINUSDB_BRANCH", default_value = "main")]
        branch: String,

        /// Optimize the metadata graph instead of a branch
        #[arg(long)]
        meta: bool,
    },

    /// Squash commit history into a single commit
    Squash {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Branch name
        #[arg(long, env = "TERMINUSDB_BRANCH", default_value = "main")]
        branch: String,

        /// Commit author
        #[arg(long, default_value = "admin")]
        author: String,

        /// Commit message
        #[arg(long, default_value = "Squash commits")]
        message: String,
    },

    /// Squash commit history and immediately apply it to the branch
    SquashAndReset {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Branch name
        #[arg(long, env = "TERMINUSDB_BRANCH", default_value = "main")]
        branch: String,

        /// Commit author
        #[arg(long, default_value = "admin")]
        author: String,

        /// Commit message
        #[arg(long, default_value = "Squash commits")]
        message: String,
    },

    /// Deploy a database from source to target (reverse branch cloning)
    Deploy {
        /// Source TerminusDB server URL
        #[arg(long)]
        source_host: String,

        /// Source username
        #[arg(long, default_value = "admin")]
        source_user: String,

        /// Source password
        #[arg(long, default_value = "root")]
        source_password: String,

        /// Source organization
        #[arg(long)]
        source_org: String,

        /// Source database
        #[arg(long)]
        source_db: String,

        /// Source branch (default: main)
        #[arg(long, default_value = "main")]
        source_branch: String,

        /// Target TerminusDB server URL
        #[arg(long)]
        target_host: String,

        /// Target username
        #[arg(long, default_value = "admin")]
        target_user: String,

        /// Target password
        #[arg(long, default_value = "root")]
        target_password: String,

        /// Target organization
        #[arg(long)]
        target_org: String,

        /// Target database
        #[arg(long)]
        target_db: String,

        /// Optional label for target database
        #[arg(long)]
        target_label: Option<String>,

        /// Optional comment for target database
        #[arg(long)]
        target_comment: Option<String>,

        /// Skip creating target database (use if it already exists)
        #[arg(long)]
        skip_create: bool,
    },

    /// Database management commands
    Database {
        #[command(subcommand)]
        command: DatabaseCommands,
    },

    /// Login and store credentials for a profile
    Login {
        /// Profile name (default: "default")
        #[arg(long, default_value = "default")]
        profile: String,
    },

    /// Logout and remove stored credentials
    Logout {
        /// Profile name (default: active profile)
        #[arg(long)]
        profile: Option<String>,
    },

    /// Manage profiles
    Profile {
        #[command(subcommand)]
        command: ProfileCommands,
    },
}

#[derive(Subcommand)]
pub(crate) enum DatabaseCommands {
    /// Create a new database
    Create {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Optional label for the database
        #[arg(long)]
        label: Option<String>,

        /// Optional comment/description for the database
        #[arg(long)]
        comment: Option<String>,

        /// Create with schema graph (default: true)
        #[arg(long, default_value = "true")]
        schema: bool,
    },

    /// Get information about a database
    Info {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,
    },

    /// List all databases in an organization
    List {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,
    },

    /// Delete a database
    Delete {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Get commit log for a database
    Log {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Limit number of commits to show
        #[arg(long, default_value = "10")]
        limit: usize,
    },
}

#[derive(Subcommand)]
pub(crate) enum ProfileCommands {
    /// List all profiles
    List,

    /// Set the active profile
    Set {
        /// Profile name to make active
        name: String,
    },

    /// Show profile configuration
    Show {
        /// Profile name (default: active profile)
        name: Option<String>,
    },

    /// Delete a profile
    Delete {
        /// Profile name to delete
        name: String,

        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum RemoteCommands {
    /// Add a new remote repository
    Add {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Name for the remote (e.g., "origin")
        #[arg(long)]
        name: String,

        /// URL of the remote repository
        #[arg(long)]
        url: String,
    },

    /// List all remotes for a database
    List {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,
    },

    /// Get information about a specific remote
    Get {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Name of the remote
        #[arg(long)]
        name: String,
    },

    /// Update a remote repository URL
    Update {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Name of the remote
        #[arg(long)]
        name: String,

        /// New URL for the remote repository
        #[arg(long)]
        url: String,
    },

    /// Delete a remote repository
    Delete {
        /// TerminusDB server URL
        #[arg(long, env = "TERMINUSDB_HOST", default_value = "http://localhost:6363")]
        host: String,

        /// Username for authentication
        #[arg(long, env = "TERMINUSDB_USER", default_value = "admin")]
        user: String,

        /// Password for authentication
        #[arg(long, env = "TERMINUSDB_PASS", default_value = "root")]
        password: String,

        /// Organization name
        #[arg(long, env = "TERMINUSDB_ORG")]
        org: String,

        /// Database name
        #[arg(long, env = "TERMINUSDB_DB")]
        database: String,

        /// Name of the remote to delete
        #[arg(long)]
        name: String,
    },
}
