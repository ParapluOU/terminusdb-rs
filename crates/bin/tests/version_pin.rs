//! Guards against the supported-TerminusDB-version claim drifting apart.
//!
//! The pin lives in exactly one place — `TERMINUSDB_VERSION`'s default in
//! `crates/bin/build.rs` — but it is *stated* in several: the README table,
//! the CLAUDE.md version section, and the compat plan. Those are what a reader
//! or an agent trusts, and nothing stops a bump from updating the build and
//! leaving the prose behind. This test fails when they disagree.
//!
//! Tag convention (see the README's "Versioning and release tags"):
//! `v<semver>` for our releases, `tdb/<server version>` for the commit where
//! support for a server release landed.

use std::fs;
use std::path::{Path, PathBuf};

/// Repo root, from this crate's manifest dir (`<root>/crates/bin`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/bin should sit two levels below the repo root")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The fork tag `build.rs` falls back to when `TERMINUSDB_VERSION` is unset.
/// This is the single source of truth every other mention has to match.
fn pinned_fork_tag() -> String {
    let build_rs = read("crates/bin/build.rs");
    let marker = "env::var(\"TERMINUSDB_VERSION\")";
    let at = build_rs
        .find(marker)
        .expect("build.rs should read TERMINUSDB_VERSION");
    // The default follows in `.unwrap_or_else(|_| "<tag>".to_string())`.
    let tail = &build_rs[at..];
    let open = tail.find('"').expect("no string literal after the env lookup");
    let rest = &tail[open + 1..];
    let next = rest.find('"').expect("unterminated default tag literal");
    let after_marker = &rest[..next];
    // Skip the marker's own closing quote: find the literal that looks like a tag.
    let tag = if after_marker.starts_with('v') {
        after_marker.to_string()
    } else {
        let rest2 = &rest[next + 1..];
        let open2 = rest2.find('"').expect("no default tag literal");
        let rest3 = &rest2[open2 + 1..];
        let close2 = rest3.find('"').expect("unterminated default tag literal");
        rest3[..close2].to_string()
    };
    assert!(
        tag.starts_with('v'),
        "expected a v-prefixed fork tag in build.rs, got {tag:?}"
    );
    tag
}

/// The upstream TerminusDB version the fork tag is built from, e.g. `12.1.0-rc3`
/// out of `v12.1.0-rc3-paraplu.1`.
fn upstream_version(fork_tag: &str) -> String {
    let without_v = fork_tag.strip_prefix('v').unwrap_or(fork_tag);
    match without_v.find("-paraplu") {
        Some(cut) => without_v[..cut].to_string(),
        // A tag that is not a paraplu fork tag is the upstream version itself.
        None => without_v.to_string(),
    }
}

#[test]
fn readme_states_the_pinned_server_version() {
    let fork_tag = pinned_fork_tag();
    let upstream = upstream_version(&fork_tag);
    let readme = read("README.md");

    let heading = "## Supported TerminusDB version";
    assert!(
        readme.contains(heading),
        "README lost its {heading:?} section — the supported version must stay \
         discoverable from the front page"
    );

    // Check inside the section only, so an unrelated mention elsewhere cannot
    // make this pass.
    let start = readme.find(heading).unwrap();
    let section_end = readme[start + heading.len()..]
        .find("\n## ")
        .map(|o| start + heading.len() + o)
        .unwrap_or(readme.len());
    let section = &readme[start..section_end];

    assert!(
        section.contains(&upstream),
        "README's supported-version section does not mention the pinned server \
         version {upstream:?} (from build.rs's {fork_tag:?})"
    );
    assert!(
        section.contains(&fork_tag),
        "README's supported-version section does not mention the pinned fork \
         tag {fork_tag:?}"
    );
}

#[test]
fn claude_md_states_the_pinned_server_version() {
    let fork_tag = pinned_fork_tag();
    let claude_md = read("CLAUDE.md");
    assert!(
        claude_md.contains(&fork_tag),
        "CLAUDE.md does not mention the pinned fork tag {fork_tag:?}; agents \
         read that file for the target server version"
    );
}

#[test]
fn compat_plan_states_the_pinned_server_version() {
    let fork_tag = pinned_fork_tag();
    let plan = read("docs/terminusdb12-compat-plan.md");
    assert!(
        plan.contains(&fork_tag),
        "docs/terminusdb12-compat-plan.md does not mention the pinned fork tag \
         {fork_tag:?}"
    );
}

/// `upstream_version` has to survive both tag shapes it will meet.
#[test]
fn upstream_version_is_derived_from_the_fork_tag() {
    assert_eq!(upstream_version("v12.1.0-rc3-paraplu.1"), "12.1.0-rc3");
    assert_eq!(upstream_version("v12.0.6-paraplu.1"), "12.0.6");
    assert_eq!(upstream_version("v12.1.0-rc3"), "12.1.0-rc3");
}
