//! Remote repository management and collaboration commands
//! (add/list/get/update/delete remotes, clone/fetch/pull/push).

use anyhow::Result;
use terminusdb_client::TerminusDBHttpClient;
use url::Url;

/// Helper function to parse remote authentication string
fn parse_remote_auth(auth_str: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = auth_str.splitn(2, ':').collect();
    if parts.len() != 2 {
        anyhow::bail!("Remote auth must be in format 'username:password'");
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

pub(crate) async fn run_remote_add(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    name: String,
    url: String,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let path = format!("{}/{}", org, database);
    let result = client.add_remote(&path, &name, &url).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn run_remote_list(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let path = format!("{}/{}", org, database);
    let remotes = client.list_remotes(&path).await?;

    println!("{}", serde_json::to_string_pretty(&remotes)?);
    Ok(())
}

pub(crate) async fn run_remote_get(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    name: String,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let path = format!("{}/{}", org, database);
    let remote = client.get_remote(&path, &name).await?;

    println!("{}", serde_json::to_string_pretty(&remote)?);
    Ok(())
}

pub(crate) async fn run_remote_update(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    name: String,
    url: String,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let path = format!("{}/{}", org, database);
    let result = client.update_remote(&path, &name, &url).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn run_remote_delete(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    name: String,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let path = format!("{}/{}", org, database);
    let result = client.delete_remote(&path, &name).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn run_clone(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    remote_url: String,
    label: Option<String>,
    comment: Option<String>,
    remote_auth: Option<String>,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let auth_creds = remote_auth
        .as_ref()
        .map(|s| parse_remote_auth(s))
        .transpose()?;
    let auth = auth_creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str()));

    let result = client
        .clone_repository(
            &org,
            &database,
            &remote_url,
            label.as_deref(),
            comment.as_deref(),
            auth,
            None, // timeout
        )
        .await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn run_fetch(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    branch: String,
    remote_url: String,
    remote_branch: String,
    remote_auth: Option<String>,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let auth_creds = remote_auth
        .as_ref()
        .map(|s| parse_remote_auth(s))
        .transpose()?;
    let auth = auth_creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str()));

    let path = format!("{}/{}/local/branch/{}", org, database, branch);
    let result = client
        .fetch(&path, &remote_url, Some(&remote_branch), auth, None)
        .await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn run_pull(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    branch: String,
    remote_url: String,
    remote_branch: Option<String>,
    author: String,
    message: String,
    remote_auth: Option<String>,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let auth_creds = remote_auth
        .as_ref()
        .map(|s| parse_remote_auth(s))
        .transpose()?;
    let auth = auth_creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str()));

    let path = format!("{}/{}/local/branch/{}", org, database, branch);
    let result = client
        .pull(
            &path,
            &remote_url,
            remote_branch.as_deref(),
            &author,
            &message,
            auth,
            None, // timeout
        )
        .await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn run_push(
    host: String,
    user: String,
    password: String,
    org: String,
    database: String,
    branch: String,
    remote_url: String,
    remote_branch: Option<String>,
    remote_auth: Option<String>,
) -> Result<()> {
    let parsed_url = Url::parse(&host)?;
    let client = TerminusDBHttpClient::new(parsed_url, &user, &password, &org).await?;

    let auth_creds = remote_auth
        .as_ref()
        .map(|s| parse_remote_auth(s))
        .transpose()?;
    let auth = auth_creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str()));

    let path = format!("{}/{}/local/branch/{}", org, database, branch);
    let result = client
        .push(&path, &remote_url, remote_branch.as_deref(), auth, None)
        .await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
