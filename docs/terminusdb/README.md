# TerminusDB Documentation (local mirror)

This directory is a local mirror of the **official TerminusDB documentation**,
kept in-repo so LLM agents and developers can reference it offline.

- **Upstream**: https://github.com/dfrnt-labs/terminusdb-docs-static (source of https://terminusdb.org/docs/)
- **Mirrored commit**: `0f36c9a074d112f4325886bc4a5f4030b9c82b8c` (2026-05-19, "Added public sandbox")
- **Covers**: TerminusDB 12 (DFRNT-maintained releases)
- **Last refreshed**: 2026-07-08

## Layout

- Every upstream page `src/app/docs/<slug>/page.md` is copied here as `<slug>.md` (flat).
- `index.md` is the docs landing page; `INDEX.md` mirrors the site navigation and links every local file — start there.
- Pages are Markdoc-flavored markdown with YAML frontmatter (`tags`, `title`, SEO metadata). `{% callout %}`-style tags are Markdoc directives; read through them.
- A handful of pages are React-only upstream (interactive OpenAPI/Swagger viewers, topic browser) and have no markdown; `INDEX.md` links those to the live site.

## How to refresh

```bash
git clone --depth 1 https://github.com/dfrnt-labs/terminusdb-docs-static /tmp/terminusdb-docs-static
git rm -rq docs/terminusdb && mkdir -p docs/terminusdb
for f in /tmp/terminusdb-docs-static/src/app/docs/*/page.md; do
  cp "$f" "docs/terminusdb/$(basename $(dirname "$f")).md"
done
cp /tmp/terminusdb-docs-static/src/app/docs/page.md docs/terminusdb/index.md
# regenerate INDEX.md from src/lib/navigation.ts, restore this README,
# and record the new upstream commit hash + date above
```

## Historical note

Before 2026-07-08 this directory held a copy of the old
`terminusdb/terminusdb-docs` GitBook repo, which had been unmaintained since the
docs moved to terminusdb.org under DFRNT stewardship (2025). That copy predated
TerminusDB 12 and is superseded by this mirror.
