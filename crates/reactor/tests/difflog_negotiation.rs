//! Does the driver actually negotiate against a live server?
//!
//! The unit tests cover the parsing and the unreachable case; this covers the
//! one thing they cannot, which is that the field names agree with what the
//! server really sends. A mismatch there would read as "no extensions" and
//! silently downgrade every namespace to head polling.

use std::sync::Arc;
use std::time::Duration;

use terminusdb_reactor::capture::difflog::{Availability, DiffLog};

fn upstream() -> Option<url::Url> {
    std::env::var("REACTOR_TEST_UPSTREAM")
        .ok()
        .and_then(|url| url::Url::parse(&url).ok())
}

#[tokio::test]
async fn a_paraplu_server_reports_the_difflog_extension() {
    let Some(upstream) = upstream() else {
        eprintln!("REACTOR_TEST_UPSTREAM unset; skipping");
        return;
    };
    let feed = DiffLog::new(
        reqwest::Client::new(),
        upstream,
        "admin",
        "root",
        Arc::new(|| Duration::from_millis(1)),
    );
    assert_eq!(Availability::Present, feed.availability().await);
}
