//! Main client for TerminusDB CLI operations.

// Allow hidden lifetime parameters in return types - this is intentional for ergonomic API design
#![allow(mismatched_lifetime_syntaxes)]

use super::commands::{add_flag, add_option, add_required, execute};
use super::options::*;
use super::spec::{BranchSpec, DbSpec, GraphSpec};
use std::process::ExitStatus;

/// Main client for TerminusDB operations.
///
/// This struct provides strongly-typed methods for all TerminusDB CLI commands.
///
/// # Example
///
/// ```no_run
/// use terminusdb_bin::api::{TerminusDB, DbSpec, ServeOptions};
///
/// let client = TerminusDB::new();
///
/// // Start server
/// client.serve(ServeOptions::default())?;
///
/// // Create database
/// let spec = DbSpec::new("admin", "mydb");
/// client.db().create(spec, Default::default())?;
/// # Ok::<(), std::io::Error>(())
/// ```
#[derive(Debug, Default)]
pub struct TerminusDB;

impl TerminusDB {
    /// Create a new TerminusDB client.
    pub fn new() -> Self {
        Self
    }

    // ========================================================================
    // Basic Commands
    // ========================================================================

    /// Display help information.
    pub fn help(&self, options: HelpOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["help".to_string()];
        add_flag(&mut args, "--markdown", options.markdown);
        execute(args)
    }

    /// Run TerminusDB tests.
    pub fn test(&self, options: TestOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["test".to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_option(&mut args, "--test", &options.test);
        execute(args)
    }

    /// Start the TerminusDB server.
    pub fn serve(&self, options: ServeOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["serve".to_string()];
        add_flag(&mut args, "--interactive", options.interactive);
        if let Some(password) = &options.memory {
            args.push("--memory".to_string());
            args.push(password.clone());
        }
        execute(args)
    }

    /// Execute a WOQL query.
    pub fn query(
        &self,
        db_spec: DbSpec,
        query: &str,
        options: QueryOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["query".to_string(), db_spec.to_string(), query.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--message", options.message.as_ref());
        add_required(&mut args, "--author", options.author.as_ref());
        add_flag(&mut args, "--optimize", options.optimize);
        add_option(&mut args, "--library", &options.library);
        add_flag(&mut args, "--json", options.json);
        execute(args)
    }

    // ========================================================================
    // Command Builders
    // ========================================================================

    /// Access database commands.
    pub fn db(&self) -> DbCommands {
        DbCommands { client: self }
    }

    /// Access document commands.
    pub fn doc(&self) -> DocCommands {
        DocCommands { client: self }
    }

    /// Access branch commands.
    pub fn branch(&self) -> BranchCommands {
        BranchCommands { client: self }
    }

    /// Access git-like commands.
    pub fn git(&self) -> GitCommands {
        GitCommands { client: self }
    }
}

// ============================================================================
// Database Commands
// ============================================================================

/// Database commands (create, delete, list, update).
pub struct DbCommands<'a> {
    client: &'a TerminusDB,
}

impl<'a> DbCommands<'a> {
    /// Create a new database.
    pub fn create(&self, spec: DbSpec, options: DbCreateOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["db".to_string(), "create".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--organization", &options.organization);
        add_option(&mut args, "--label", &options.label);
        add_option(&mut args, "--comment", &options.comment);
        add_flag(&mut args, "--public", options.public);
        add_flag(&mut args, "--schema", options.schema);
        add_required(&mut args, "--data-prefix", &options.data_prefix);
        add_required(&mut args, "--schema-prefix", &options.schema_prefix);
        add_option(&mut args, "--prefixes", &options.prefixes);
        execute(args)
    }

    /// Delete a database.
    pub fn delete(&self, spec: DbSpec, options: DbDeleteOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["db".to_string(), "delete".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--organization", &options.organization);
        add_flag(&mut args, "--force", options.force);
        execute(args)
    }

    /// List databases.
    pub fn list(&self, specs: Vec<DbSpec>, options: DbListOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["db".to_string(), "list".to_string()];
        for spec in specs {
            args.push(spec.to_string());
        }
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_flag(&mut args, "--branches", options.branches);
        add_flag(&mut args, "--verbose", options.verbose);
        add_flag(&mut args, "--json", options.json);
        execute(args)
    }

    /// Update database metadata.
    pub fn update(&self, spec: DbSpec, options: DbUpdateOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["db".to_string(), "update".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_option(&mut args, "--label", &options.label);
        add_option(&mut args, "--comment", &options.comment);
        if let Some(public) = options.public {
            args.push("--public".to_string());
            args.push(public.to_string());
        }
        if let Some(schema) = options.schema {
            args.push("--schema".to_string());
            args.push(schema.to_string());
        }
        add_option(&mut args, "--prefixes", &options.prefixes);
        execute(args)
    }
}

// ============================================================================
// Document Commands
// ============================================================================

/// Document commands (insert, delete, replace, get).
pub struct DocCommands<'a> {
    client: &'a TerminusDB,
}

impl<'a> DocCommands<'a> {
    /// Insert documents.
    pub fn insert(&self, spec: DbSpec, options: DocInsertOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["doc".to_string(), "insert".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--message", options.message.as_ref());
        add_required(&mut args, "--author", options.author.as_ref());
        add_required(&mut args, "--graph-type", options.graph_type.as_str());
        add_flag(&mut args, "--require-migration", options.require_migration);
        add_flag(
            &mut args,
            "--allow-destructive-migration",
            options.allow_destructive_migration,
        );
        add_option(&mut args, "--data", &options.data);
        add_flag(&mut args, "--raw-json", options.raw_json);
        add_flag(&mut args, "--merge-repeats", options.merge_repeats);
        add_flag(&mut args, "--full-replace", options.full_replace);
        execute(args)
    }

    /// Delete documents.
    pub fn delete(&self, spec: DbSpec, options: DocDeleteOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["doc".to_string(), "delete".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--message", options.message.as_ref());
        add_required(&mut args, "--author", options.author.as_ref());
        add_required(&mut args, "--graph-type", options.graph_type.as_str());
        add_flag(&mut args, "--require-migration", options.require_migration);
        add_flag(
            &mut args,
            "--allow-destructive-migration",
            options.allow_destructive_migration,
        );
        add_option(&mut args, "--id", &options.id);
        add_option(&mut args, "--type", &options.doc_type);
        add_option(&mut args, "--data", &options.data);
        add_flag(&mut args, "--nuke", options.nuke);
        execute(args)
    }

    /// Replace documents.
    pub fn replace(&self, spec: DbSpec, options: DocReplaceOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["doc".to_string(), "replace".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--message", options.message.as_ref());
        add_required(&mut args, "--author", options.author.as_ref());
        add_required(&mut args, "--graph-type", options.graph_type.as_str());
        add_flag(&mut args, "--require-migration", options.require_migration);
        add_flag(
            &mut args,
            "--allow-destructive-migration",
            options.allow_destructive_migration,
        );
        add_option(&mut args, "--data", &options.data);
        add_flag(&mut args, "--raw-json", options.raw_json);
        add_flag(&mut args, "--create", options.create);
        execute(args)
    }

    /// Get documents.
    pub fn get(&self, spec: DbSpec, options: DocGetOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["doc".to_string(), "get".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--graph-type", options.graph_type.as_str());
        add_required(&mut args, "--skip", options.skip.to_string());
        if let Some(count) = options.count {
            add_required(&mut args, "--count", count.to_string());
        } else {
            add_required(&mut args, "--count", "unlimited");
        }
        add_flag(&mut args, "--minimized", options.minimized);
        add_flag(&mut args, "--as-list", options.as_list);
        add_flag(&mut args, "--unfold", options.unfold);
        add_option(&mut args, "--id", &options.id);
        if !options.ids.is_empty() {
            args.push("--ids".to_string());
            args.push(format!("[{}]", options.ids.join(",")));
        }
        add_option(&mut args, "--type", &options.doc_type);
        add_flag(&mut args, "--compress-ids", options.compress_ids);
        add_option(&mut args, "--query", &options.query);
        execute(args)
    }
}

// ============================================================================
// Branch Commands
// ============================================================================

/// Branch commands (create, delete).
pub struct BranchCommands<'a> {
    client: &'a TerminusDB,
}

impl<'a> BranchCommands<'a> {
    /// Create a new branch.
    pub fn create(
        &self,
        spec: BranchSpec,
        options: BranchCreateOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["branch".to_string(), "create".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        if let Some(origin) = &options.origin {
            add_required(&mut args, "--origin", origin);
        } else {
            add_required(&mut args, "--origin", "false");
        }
        execute(args)
    }

    /// Delete a branch.
    pub fn delete(
        &self,
        spec: BranchSpec,
        options: BranchDeleteOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["branch".to_string(), "delete".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        execute(args)
    }
}

// ============================================================================
// Git-like Commands
// ============================================================================

/// Git-like commands (push, pull, clone, fetch, rebase).
pub struct GitCommands<'a> {
    client: &'a TerminusDB,
}

impl<'a> GitCommands<'a> {
    /// Push to remote.
    pub fn push(&self, spec: DbSpec, options: PushOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["push".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--branch", &options.branch);
        add_option(&mut args, "--remote-branch", &options.remote_branch);
        add_required(&mut args, "--remote", &options.remote);
        add_flag(&mut args, "--prefixes", options.prefixes);
        add_option(&mut args, "--token", &options.token);
        add_option(&mut args, "--user", &options.user);
        add_option(&mut args, "--password", &options.password);
        execute(args)
    }

    /// Clone from remote.
    pub fn clone(
        &self,
        uri: &str,
        db_spec: Option<DbSpec>,
        options: CloneOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["clone".to_string(), uri.to_string()];
        if let Some(spec) = db_spec {
            args.push(spec.to_string());
        }
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_option(&mut args, "--token", &options.token);
        add_option(&mut args, "--user", &options.user);
        add_option(&mut args, "--password", &options.password);
        add_required(&mut args, "--organization", &options.organization);
        add_option(&mut args, "--label", &options.label);
        add_required(&mut args, "--remote", &options.remote);
        add_required(&mut args, "--comment", &options.comment);
        add_flag(&mut args, "--public", options.public);
        execute(args)
    }

    /// Pull from remote.
    pub fn pull(&self, spec: BranchSpec, options: PullOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["pull".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_option(&mut args, "--remote-branch", &options.remote_branch);
        add_required(&mut args, "--remote", &options.remote);
        add_option(&mut args, "--token", &options.token);
        add_option(&mut args, "--user", &options.user);
        add_option(&mut args, "--password", &options.password);
        execute(args)
    }

    /// Fetch from remote.
    pub fn fetch(&self, spec: DbSpec, options: FetchOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["fetch".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--remote", &options.remote);
        add_option(&mut args, "--token", &options.token);
        add_option(&mut args, "--user", &options.user);
        add_option(&mut args, "--password", &options.password);
        execute(args)
    }

    /// Rebase branches.
    pub fn rebase(
        &self,
        to: DbSpec,
        from: DbSpec,
        options: RebaseOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["rebase".to_string(), to.to_string(), from.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--author", options.author.as_ref());
        execute(args)
    }
}

// ============================================================================
// Role Commands
// ============================================================================

/// Role commands (create, delete, update, get).
pub struct RoleCommands<'a> {
    client: &'a TerminusDB,
}

impl<'a> RoleCommands<'a> {
    /// Create a new role.
    pub fn create(
        &self,
        name: &str,
        actions: Vec<super::types::RoleAction>,
        options: super::options::RoleCreateOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["role".to_string(), "create".to_string(), name.to_string()];
        for action in actions {
            args.push(action.as_str().to_string());
        }
        add_required(&mut args, "--impersonate", &options.impersonate);
        execute(args)
    }

    /// Delete a role.
    pub fn delete(
        &self,
        role_id_or_name: &str,
        options: super::options::RoleDeleteOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "role".to_string(),
            "delete".to_string(),
            role_id_or_name.to_string(),
        ];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_flag(&mut args, "--id", options.id);
        execute(args)
    }

    /// Update a role.
    pub fn update(
        &self,
        role_id_or_name: &str,
        actions: Vec<super::types::RoleAction>,
        options: super::options::RoleUpdateOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "role".to_string(),
            "update".to_string(),
            role_id_or_name.to_string(),
        ];
        for action in actions {
            args.push(action.as_str().to_string());
        }
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_flag(&mut args, "--id", options.id);
        execute(args)
    }

    /// Get role information.
    pub fn get(
        &self,
        role_id_or_name: Option<&str>,
        options: super::options::RoleGetOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["role".to_string(), "get".to_string()];
        if let Some(name) = role_id_or_name {
            args.push(name.to_string());
        }
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_flag(&mut args, "--id", options.id);
        add_flag(&mut args, "--json", options.json);
        execute(args)
    }
}

// ============================================================================
// User Commands
// ============================================================================

/// User commands (create, delete, get, password).
pub struct UserCommands<'a> {
    client: &'a TerminusDB,
}

impl<'a> UserCommands<'a> {
    /// Create a new user.
    pub fn create(
        &self,
        username: &str,
        options: super::options::UserCreateOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "user".to_string(),
            "create".to_string(),
            username.to_string(),
        ];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_option(&mut args, "--password", &options.password);
        execute(args)
    }

    /// Delete a user.
    pub fn delete(
        &self,
        user_id_or_name: &str,
        options: super::options::UserDeleteOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "user".to_string(),
            "delete".to_string(),
            user_id_or_name.to_string(),
        ];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_flag(&mut args, "--id", options.id);
        execute(args)
    }

    /// Get user information.
    pub fn get(
        &self,
        user_id_or_name: Option<&str>,
        options: super::options::UserGetOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["user".to_string(), "get".to_string()];
        if let Some(name) = user_id_or_name {
            args.push(name.to_string());
        }
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_flag(&mut args, "--id", options.id);
        add_flag(&mut args, "--capability", options.capability);
        add_flag(&mut args, "--json", options.json);
        execute(args)
    }

    /// Update user password.
    pub fn password(
        &self,
        username: &str,
        options: super::options::UserPasswordOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "user".to_string(),
            "password".to_string(),
            username.to_string(),
        ];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_option(&mut args, "--password", &options.password);
        execute(args)
    }
}

// ============================================================================
// Organization Commands
// ============================================================================

/// Organization commands (create, delete, get).
pub struct OrganizationCommands<'a> {
    client: &'a TerminusDB,
}

impl<'a> OrganizationCommands<'a> {
    /// Create a new organization.
    pub fn create(
        &self,
        name: &str,
        options: super::options::OrganizationCreateOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "organization".to_string(),
            "create".to_string(),
            name.to_string(),
        ];
        add_required(&mut args, "--impersonate", &options.impersonate);
        execute(args)
    }

    /// Delete an organization.
    pub fn delete(
        &self,
        org_id_or_name: &str,
        options: super::options::OrganizationDeleteOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "organization".to_string(),
            "delete".to_string(),
            org_id_or_name.to_string(),
        ];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_flag(&mut args, "--id", options.id);
        execute(args)
    }

    /// Get organization information.
    pub fn get(
        &self,
        org_id_or_name: Option<&str>,
        options: super::options::OrganizationGetOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["organization".to_string(), "get".to_string()];
        if let Some(name) = org_id_or_name {
            args.push(name.to_string());
        }
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_flag(&mut args, "--id", options.id);
        add_flag(&mut args, "--json", options.json);
        execute(args)
    }
}

// ============================================================================
// Capability Commands
// ============================================================================

/// Capability commands (grant, revoke).
pub struct CapabilityCommands<'a> {
    client: &'a TerminusDB,
}

impl<'a> CapabilityCommands<'a> {
    /// Grant capabilities to a user.
    pub fn grant(
        &self,
        user: &str,
        scope: &str,
        roles: Vec<&str>,
        options: super::options::CapabilityGrantOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "capability".to_string(),
            "grant".to_string(),
            user.to_string(),
            scope.to_string(),
        ];
        for role in roles {
            args.push(role.to_string());
        }
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--scope-type", options.scope_type.as_str());
        execute(args)
    }

    /// Revoke capabilities from a user.
    pub fn revoke(
        &self,
        user: &str,
        scope: &str,
        roles: Vec<&str>,
        options: super::options::CapabilityRevokeOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "capability".to_string(),
            "revoke".to_string(),
            user.to_string(),
            scope.to_string(),
        ];
        for role in roles {
            args.push(role.to_string());
        }
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--scope-type", options.scope_type.as_str());
        execute(args)
    }
}

// ============================================================================
// Store Commands
// ============================================================================

/// Store commands (init).
pub struct StoreCommands<'a> {
    client: &'a TerminusDB,
}

impl<'a> StoreCommands<'a> {
    /// Initialize the store.
    pub fn init(&self, options: super::options::StoreInitOptions) -> std::io::Result<ExitStatus> {
        let mut args = vec!["store".to_string(), "init".to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--key", &options.key);
        add_flag(&mut args, "--force", options.force);
        execute(args)
    }
}

// ============================================================================
// Triples Commands
// ============================================================================

/// Triples commands (dump, update, load).
pub struct TriplesCommands<'a> {
    client: &'a TerminusDB,
}

impl<'a> TriplesCommands<'a> {
    /// Dump triples from a graph.
    pub fn dump(
        &self,
        graph_spec: GraphSpec,
        options: super::options::TriplesDumpOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "triples".to_string(),
            "dump".to_string(),
            graph_spec.to_string(),
        ];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--format", options.format.as_str());
        execute(args)
    }

    /// Update triples in a graph from a file.
    pub fn update(
        &self,
        graph_spec: GraphSpec,
        file: &str,
        options: super::options::TriplesUpdateOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "triples".to_string(),
            "update".to_string(),
            graph_spec.to_string(),
            file.to_string(),
        ];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--message", options.message.as_ref());
        add_required(&mut args, "--author", options.author.as_ref());
        add_required(&mut args, "--format", options.format.as_str());
        execute(args)
    }

    /// Load triples into a graph from a file.
    pub fn load(
        &self,
        graph_spec: GraphSpec,
        file: &str,
        options: super::options::TriplesLoadOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "triples".to_string(),
            "load".to_string(),
            graph_spec.to_string(),
            file.to_string(),
        ];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--message", options.message.as_ref());
        add_required(&mut args, "--author", options.author.as_ref());
        add_required(&mut args, "--format", options.format.as_str());
        execute(args)
    }
}

// ============================================================================
// Remote Commands
// ============================================================================

/// Remote commands (add, remove, set-url, get-url, list).
pub struct RemoteCommands<'a> {
    client: &'a TerminusDB,
}

impl<'a> RemoteCommands<'a> {
    /// Add a remote.
    pub fn add(
        &self,
        spec: DbSpec,
        remote_name: &str,
        remote_location: &str,
        options: super::options::RemoteAddOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "remote".to_string(),
            "add".to_string(),
            spec.to_string(),
            remote_name.to_string(),
            remote_location.to_string(),
        ];
        add_required(&mut args, "--impersonate", &options.impersonate);
        execute(args)
    }

    /// Remove a remote.
    pub fn remove(
        &self,
        spec: DbSpec,
        remote_name: &str,
        options: super::options::RemoteRemoveOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "remote".to_string(),
            "remove".to_string(),
            spec.to_string(),
            remote_name.to_string(),
        ];
        add_required(&mut args, "--impersonate", &options.impersonate);
        execute(args)
    }

    /// Set the URL of a remote.
    pub fn set_url(
        &self,
        spec: DbSpec,
        remote_name: &str,
        remote_location: &str,
        options: super::options::RemoteSetUrlOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "remote".to_string(),
            "set-url".to_string(),
            spec.to_string(),
            remote_name.to_string(),
            remote_location.to_string(),
        ];
        add_required(&mut args, "--impersonate", &options.impersonate);
        execute(args)
    }

    /// Get the URL of a remote.
    pub fn get_url(
        &self,
        spec: DbSpec,
        remote_name: &str,
        options: super::options::RemoteGetUrlOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "remote".to_string(),
            "get-url".to_string(),
            spec.to_string(),
            remote_name.to_string(),
        ];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_required(&mut args, "--remote", &options.remote);
        execute(args)
    }

    /// List all remotes.
    pub fn list(
        &self,
        spec: DbSpec,
        options: super::options::RemoteListOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["remote".to_string(), "list".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        execute(args)
    }
}

// ============================================================================
// Utility Commands
// ============================================================================

impl TerminusDB {
    /// Access role commands.
    pub fn role(&self) -> RoleCommands {
        RoleCommands { client: self }
    }

    /// Access user commands.
    pub fn user(&self) -> UserCommands {
        UserCommands { client: self }
    }

    /// Access organization commands.
    pub fn organization(&self) -> OrganizationCommands {
        OrganizationCommands { client: self }
    }

    /// Access capability commands.
    pub fn capability(&self) -> CapabilityCommands {
        CapabilityCommands { client: self }
    }

    /// Access store commands.
    pub fn store(&self) -> StoreCommands {
        StoreCommands { client: self }
    }

    /// Access triples commands.
    pub fn triples(&self) -> TriplesCommands {
        TriplesCommands { client: self }
    }

    /// Access remote commands.
    pub fn remote(&self) -> RemoteCommands {
        RemoteCommands { client: self }
    }

    /// Optimize a database.
    pub fn optimize(
        &self,
        spec: DbSpec,
        options: super::options::OptimizeOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["optimize".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        execute(args)
    }

    /// Squash commits.
    pub fn squash(
        &self,
        spec: DbSpec,
        options: super::options::SquashOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["squash".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_flag(&mut args, "--json", options.json);
        add_required(&mut args, "--message", options.message.as_ref());
        add_required(&mut args, "--author", options.author.as_ref());
        execute(args)
    }

    /// Rollup commits.
    pub fn rollup(
        &self,
        spec: DbSpec,
        options: super::options::RollupOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["rollup".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        execute(args)
    }

    /// Create a bundle.
    pub fn bundle(
        &self,
        spec: DbSpec,
        options: super::options::BundleOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["bundle".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_option(&mut args, "--output", &options.output);
        execute(args)
    }

    /// Apply a bundle.
    pub fn unbundle(
        &self,
        spec: DbSpec,
        file: &str,
        options: super::options::UnbundleOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["unbundle".to_string(), spec.to_string(), file.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        execute(args)
    }

    /// View commit log.
    pub fn log(
        &self,
        spec: DbSpec,
        options: super::options::LogOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec!["log".to_string(), spec.to_string()];
        add_required(&mut args, "--impersonate", &options.impersonate);
        add_flag(&mut args, "--json", options.json);
        add_required(&mut args, "--start", options.start.to_string());
        add_required(&mut args, "--count", options.count.to_string());
        add_flag(&mut args, "--verbose", options.verbose);
        execute(args)
    }

    /// Reset a branch to a specific commit.
    pub fn reset(
        &self,
        branch_spec: BranchSpec,
        commit_spec: &str,
        options: super::options::ResetOptions,
    ) -> std::io::Result<ExitStatus> {
        let mut args = vec![
            "reset".to_string(),
            branch_spec.to_string(),
            commit_spec.to_string(),
        ];
        add_required(&mut args, "--impersonate", &options.impersonate);
        execute(args)
    }
}
