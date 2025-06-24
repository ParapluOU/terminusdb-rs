mod commit;
mod entity;
mod entry;
mod iter;
mod migration;
mod opts;

use crate::http::TerminusDBHttpClient;
use crate::spec::BranchSpec;

pub use {commit::*, entity::*, entry::*, iter::*, migration::*, opts::*};

#[tokio::test]
async fn test_log() {
    let client = TerminusDBHttpClient::local_node_test().await.unwrap();

    // Create a BranchSpec using the From trait
    let branch_spec = BranchSpec::from("test");

    let res = client.log(
        &branch_spec,
        LogOpts {
            offset: None,
            count: None,
            verbose: true,
        },
    );

    let result = res.await;
    dbg!(&result);

    assert!(result.is_ok());
}
