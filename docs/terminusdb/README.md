# TerminusDB Documentation (local mirror)

This directory is a local mirror of the **official TerminusDB documentation**,
kept in-repo so LLM agents and developers can reference it offline.

- **Upstream**: https://github.com/dfrnt-labs/terminusdb-docs-static (source of https://terminusdb.org/docs/)
- **Mirrored commit**: `f6f1b66caee4c4e506a3957e4b28ac667a92d11a` (2026-11-15, "Restore blogposts")
- **Covers**: TerminusDB 12.0.x and the 12.1 release candidates
- **Last refreshed**: 2026-11-17

## Layout

- Every upstream page `src/app/docs/<slug>/page.md` is copied here as `<slug>.md` (flat).
- Release notes nest one level deeper upstream (`src/app/docs/releases/<slug>/page.md`)
  and are mirrored under `releases/<slug>.md`. A flat `*/page.md` glob misses them.
- `index.md` is the docs landing page; `INDEX.md` mirrors the site navigation and links every local file — start there.
- Pages are Markdoc-flavored markdown with YAML frontmatter (`tags`, `title`, SEO metadata). `{% callout %}`-style tags are Markdoc directives; read through them.
- A handful of pages are React-only upstream (interactive OpenAPI/Swagger viewers, topic browser) and have no markdown; `INDEX.md` links those to the live site.

## How to refresh

```bash
git clone --depth 1 https://github.com/dfrnt-labs/terminusdb-docs-static /tmp/terminusdb-docs-static
git rm -rq docs/terminusdb && mkdir -p docs/terminusdb
mkdir -p docs/terminusdb/releases
S=/tmp/terminusdb-docs-static/src/app/docs
for f in $S/*/page.md;          do cp "$f" "docs/terminusdb/$(basename $(dirname "$f")).md"; done
for f in $S/releases/*/page.md; do cp "$f" "docs/terminusdb/releases/$(basename $(dirname "$f")).md"; done
cp $S/page.md docs/terminusdb/index.md
# regenerate INDEX.md from src/lib/navigation.ts, restore this README,
# and record the new upstream commit hash + date above
```

## Historical note

Before 2026-07-08 this directory held a copy of the old
`terminusdb/terminusdb-docs` GitBook repo, which had been unmaintained since the
docs moved to terminusdb.org under DFRNT stewardship (2025). That copy predated
TerminusDB 12 and is superseded by this mirror.
