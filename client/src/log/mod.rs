mod commit;
mod entity;
mod entry;
mod iter;
mod migration;
mod opts;

pub use {commit::*, entity::*, entry::*, iter::*, migration::*, opts::*};

// NOTE: Log functionality is tested in client/tests/http_client_tests.rs
// (test_commit_added_entities_query uses client.log() internally).
// Unit tests that use terminusdb_bin cause diamond dependency type conflicts
// when defined inside the terminusdb_client crate.
