mod auth;
mod changestream;
mod cli;
mod database;
mod formatter;
mod profile_cmds;
mod remote;

use anyhow::Result;
use clap::Parser;

use changestream::run_changestream;
use cli::{Cli, Commands, DatabaseCommands, ProfileCommands, RemoteCommands};
use database::*;
use profile_cmds::*;
use remote::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing (logs to stderr, keeping stdout clean for data)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Changestream {
            host,
            user,
            password,
            org,
            database,
            branch,
            format,
            color,
        } => run_changestream(host, user, password, org, database, branch, format, color).await,
        Commands::Remote { command } => match command {
            RemoteCommands::Add {
                host,
                user,
                password,
                org,
                database,
                name,
                url,
            } => run_remote_add(host, user, password, org, database, name, url).await,
            RemoteCommands::List {
                host,
                user,
                password,
                org,
                database,
            } => run_remote_list(host, user, password, org, database).await,
            RemoteCommands::Get {
                host,
                user,
                password,
                org,
                database,
                name,
            } => run_remote_get(host, user, password, org, database, name).await,
            RemoteCommands::Update {
                host,
                user,
                password,
                org,
                database,
                name,
                url,
            } => run_remote_update(host, user, password, org, database, name, url).await,
            RemoteCommands::Delete {
                host,
                user,
                password,
                org,
                database,
                name,
            } => run_remote_delete(host, user, password, org, database, name).await,
        },
        Commands::Clone {
            host,
            user,
            password,
            org,
            database,
            remote_url,
            label,
            comment,
            remote_auth,
        } => {
            run_clone(
                host,
                user,
                password,
                org,
                database,
                remote_url,
                label,
                comment,
                remote_auth,
            )
            .await
        }
        Commands::Fetch {
            host,
            user,
            password,
            org,
            database,
            branch,
            remote_url,
            remote_branch,
            remote_auth,
        } => {
            run_fetch(
                host,
                user,
                password,
                org,
                database,
                branch,
                remote_url,
                remote_branch,
                remote_auth,
            )
            .await
        }
        Commands::Pull {
            host,
            user,
            password,
            org,
            database,
            branch,
            remote_url,
            remote_branch,
            author,
            message,
            remote_auth,
        } => {
            run_pull(
                host,
                user,
                password,
                org,
                database,
                branch,
                remote_url,
                remote_branch,
                author,
                message,
                remote_auth,
            )
            .await
        }
        Commands::Push {
            host,
            user,
            password,
            org,
            database,
            branch,
            remote_url,
            remote_branch,
            remote_auth,
        } => {
            run_push(
                host,
                user,
                password,
                org,
                database,
                branch,
                remote_url,
                remote_branch,
                remote_auth,
            )
            .await
        }
        Commands::Optimize {
            host,
            user,
            password,
            org,
            database,
            branch,
            meta,
        } => run_optimize(host, user, password, org, database, branch, meta).await,
        Commands::Squash {
            host,
            user,
            password,
            org,
            database,
            branch,
            author,
            message,
        } => run_squash(host, user, password, org, database, branch, author, message).await,
        Commands::SquashAndReset {
            host,
            user,
            password,
            org,
            database,
            branch,
            author,
            message,
        } => {
            run_squash_and_reset(host, user, password, org, database, branch, author, message).await
        }
        Commands::Deploy {
            source_host,
            source_user,
            source_password,
            source_org,
            source_db,
            source_branch,
            target_host,
            target_user,
            target_password,
            target_org,
            target_db,
            target_label,
            target_comment,
            skip_create,
        } => {
            run_deploy(
                source_host,
                source_user,
                source_password,
                source_org,
                source_db,
                source_branch,
                target_host,
                target_user,
                target_password,
                target_org,
                target_db,
                target_label,
                target_comment,
                skip_create,
            )
            .await
        }
        Commands::Database { command } => match command {
            DatabaseCommands::Create {
                host,
                user,
                password,
                org,
                database,
                label,
                comment,
                schema,
            } => {
                run_database_create(host, user, password, org, database, label, comment, schema)
                    .await
            }
            DatabaseCommands::Info {
                host,
                user,
                password,
                org,
                database,
            } => run_database_info(host, user, password, org, database).await,
            DatabaseCommands::List {
                host,
                user,
                password,
                org,
            } => run_database_list(host, user, password, org).await,
            DatabaseCommands::Delete {
                host,
                user,
                password,
                org,
                database,
                force,
            } => run_database_delete(host, user, password, org, database, force).await,
            DatabaseCommands::Log {
                host,
                user,
                password,
                org,
                database,
                limit,
            } => run_database_log(host, user, password, org, database, limit).await,
        },
        Commands::Login { profile } => run_login(&profile).await,
        Commands::Logout { profile } => run_logout(profile.as_deref()).await,
        Commands::Profile { command } => match command {
            ProfileCommands::List => run_profile_list().await,
            ProfileCommands::Set { name } => run_profile_set(&name).await,
            ProfileCommands::Show { name } => run_profile_show(name.as_deref()).await,
            ProfileCommands::Delete { name, force } => run_profile_delete(&name, force).await,
        },
    }
}
