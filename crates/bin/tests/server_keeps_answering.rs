//! The embedded server must keep answering however much it logs.
//!
//! It writes a line to standard error for every request. Standard error is a pipe
//! to this process; when nobody read it, the pipe filled after 64 KiB (about six
//! hundred requests) and the server blocked in `write(2)` for good, idle, with
//! every request timing out.

use std::time::Duration;

use terminusdb_bin::TerminusDBServer;

#[tokio::test]
async fn the_server_answers_after_logging_more_than_a_pipe_holds() -> anyhow::Result<()> {
    let server = TerminusDBServer::test().await?;
    let client = server.client().await?;
    // Each request logs a line of about 60 bytes: 2000 of them are nearly twice
    // what a pipe holds.
    for request in 0..2000 {
        tokio::time::timeout(Duration::from_secs(20), client.info())
            .await
            .map_err(|_| anyhow::anyhow!("request {request} was not answered: the server has stopped"))??;
    }
    let log = server.recent_log();
    assert_eq!(log.len(), 500, "the last lines of the server's log are kept");
    assert!(
        log.iter().all(|line| line.contains("/api/info")),
        "and they are what it logged: {:?}",
        log.last()
    );
    Ok(())
}
