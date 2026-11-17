---
name: terminusdb-version-upgrade
description: Upgrade this workspace's support to a newer TerminusDB server release. Use when asked to "upgrade TerminusDB support", bump the embedded server version, refresh the docs mirror, or find and implement new TerminusDB functionality (WOQL constructs, API endpoints/params, schema annotations, GraphQL features) that our crates do not cover yet.
---

# TerminusDB version upgrade

Upgrading support is a research task first and a coding task second. The
authoritative sources are the **server source tree**, not the docs — the docs
lag and sometimes describe enterprise-only or unreleased features. Work in this
order.

## 1. Refresh the documentation mirror

`docs/terminusdb/` mirrors https://github.com/dfrnt-labs/terminusdb-docs-static
(the source of terminusdb.org/docs). Its `README.md` holds the refresh recipe
and the mirrored commit hash — update both.

```bash
git clone --depth 1 https://github.com/dfrnt-labs/terminusdb-docs-static /tmp/tdbdocs/docs-static
cp docs/terminusdb/README.md /tmp/README.md.bak
git rm -rq docs/terminusdb && mkdir -p docs/terminusdb/releases
S=/tmp/tdbdocs/docs-static/src/app/docs
for f in $S/*/page.md;          do cp "$f" "docs/terminusdb/$(basename $(dirname "$f")).md"; done
for f in $S/releases/*/page.md; do cp "$f" "docs/terminusdb/releases/$(basename $(dirname "$f")).md"; done
cp $S/page.md docs/terminusdb/index.md
cp /tmp/README.md.bak docs/terminusdb/README.md
```

Gotchas: pages nest one level deeper under `releases/`, so the flat `*/page.md`
glob misses them. Regenerate `INDEX.md` from `$S/../../lib/navigation.ts`, and
record the new upstream commit + date in `README.md`.

Diff the page list with `LC_ALL=C` sorting (plain `sort` is locale-ordered and
`comm` will reject it). New pages are the fastest signal of new features:

```bash
comm -23 <(ls $S/*/page.md | sed 's|.*/docs/||; s|/page.md||' | LC_ALL=C sort) \
         <(ls docs/terminusdb/*.md | xargs -n1 basename | sed 's/\.md$//' | LC_ALL=C sort)
```

Then read `docs/terminusdb/releases/*.md` for every release newer than our base
(see step 2) — each has a "what shipped" summary with the PR numbers.

## 2. Locate our base and the upstream target

The embedded server builds the `ParapluOU/terminusdb` fork, pinned in
`crates/bin/build.rs` (`TERMINUSDB_VERSION`, overridable by env var).

```bash
git clone https://github.com/ParapluOU/terminusdb.git /tmp/tdbsrc/fork
cd /tmp/tdbsrc/fork && git remote add up https://github.com/terminusdb/terminusdb.git
git fetch up main:refs/heads/up-main && git fetch --tags up
git merge-base <our-pinned-tag> up-main      # our base commit
git show <base>:VERSION                      # the upstream version we branched from
git log --oneline up-main --not <our-pinned-tag> | wc -l
```

Our fork carries a handful of local patches (graphql module visibility for
`crates/gql`, `_path` GraphQL ordering, GHCR publish workflow, a MAKEFLAGS
build fix). Everything else must come from upstream.

## 3. Diff the real server surface (do not trust the docs alone)

```bash
B=<base-commit>
# WOQL AST: the canonical class list and every field
diff <(git show $B:src/terminus-schema/woql.json      | grep -oP '"@id": *"\K[A-Za-z]+') \
     <(git show up-main:src/terminus-schema/woql.json | grep -oP '"@id": *"\K[A-Za-z]+')
# HTTP routes
diff <(git show $B:src/server/routes.pl      | grep -oP "http_handler\(\K[^,]+" | sort -u) \
     <(git show up-main:src/server/routes.pl | grep -oP "http_handler\(\K[^,]+" | sort -u)
# query params (per handler; params are read with param_value_search*_optional)
git show up-main:src/server/routes.pl | sed -n '/^document_handler(get/,/^document_handler(post/p' | grep -oP 'JSON, \K[a-z_]+'
git diff --stat $B up-main -- src/terminus-schema/ src/core/api/
```

Field-level parity for WOQL is worth scripting: parse `woql.json` for each
`@inherits: Query` class's properties and compare with the `pub struct` fields
in `crates/woql2/src/*.rs`. Expect false positives — our structs rename
reserved/clashing words via `#[tdb(name = "...")]` (`type`→`type_uri`,
`then`→`then_query`, `result`→`result_value`, `Optional`→`WoqlOptional`,
`Value`→`WoqlValue`).

Check gating before committing to a feature: `grep` the handler for
`enterprise`, `indexer_backend(...)`, or a `document_output_format/1` clause.
Enterprise-only or external-engine features (e.g. versioned search fronts a
separate `tdb-search` engine; `jsonld`/`rdfxml`/`turtle` document output) cannot
be tested against the embedded community server — defer them explicitly rather
than shipping untested code.

## 4. Rebase the fork and push a new tag

```bash
cd /tmp/tdbsrc/fork
git checkout -B v<version>-paraplu <our-pinned-tag>
git rebase up-main            # expect context conflicts in the graphql crate
```

Conflicts are usually context drift around our visibility patch; keep
upstream's lines and re-apply only our `pub` changes. For the `_path` ordering
patch, merge both sides (upstream tests + our helper fn) and fix renamed
imports. Then verify before pushing:

```bash
source ~/.claude/tdb-build-env.sh
cd src/rust && cargo check -p terminusdb-community
```

Push the branch and an annotated tag, then bump `TERMINUSDB_VERSION` in
`crates/bin/build.rs` and rebuild:

```bash
source ~/.claude/tdb-build-env.sh
cargo build -p terminusdb-bin        # long: SWI-Prolog + the Rust dylib
cargo test -p terminusdb-bin
```

The build env script is mandatory on this host (nix gcc15/gmp `-std=gnu17`,
libclang shim, bindgen headers, bundled `protoc`). See the
`tdb-build-environment` memory.

## 5. Implement, one feature at a time, each with a test

Where new functionality lands:

| Server change | Our crate |
|---|---|
| New WOQL class / field | `crates/woql2/src/*.rs` (AST + `Query` enum + `dsl.rs`/`macros.rs`). Never add to `crates/woql-builder` — deprecated. |
| New endpoint or query param | `crates/client/src/http/` (+ `url_builder.rs` for params) |
| Document API behaviour | `crates/client/src/document/` |
| `@key` / class / field annotation | `crates/schema/src/schema/` + `crates/schema/derive/` (`tdb()` attrs) |
| GraphQL | `crates/client/src/http/graphql.rs`, `crates/gql` |

Rules that bite otherwise:

- **A WOQL feature is not working until it has run against a real server.**
  Unit tests that only assert JSON shape prove nothing. Use
  `terminusdb_bin::TerminusDBServer::test_instance()` with `with_tmp_db` /
  `with_db_schema` / `with_db_seed` (see CLAUDE.md) — no `#[ignore]`, runs in
  parallel.
- Serialization problems are fixed with `tdb()` attributes or
  `ToInstanceProperty`, never with hand-written `Serialize`/`Deserialize`.
- Never hand-format an IRI; use `EntityIDFor<T>`/`TdbIRI`.
- Prefer wrapping an existing tested client method over a bespoke
  WOQL/Document-API path.
- After a model's shape changes, an existing database needs
  `client.migrate_schema(...)`, or drop it if it is empty/dev.

## 6. Record the outcome

Update `docs/terminusdb12-compat-plan.md` (milestones + status + explicit
deferrals), the TerminusDB version section of `CLAUDE.md`, and the
`terminusdb-fork-v12-state` memory with the new pin and what was deferred.
Commit directly to `main` — this repo does not branch.
