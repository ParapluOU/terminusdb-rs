---
tags:
  - reference
  - schema
  - curl
  - enterprise
title: Context Cache
nextjs:
  metadata:
    title: Context Cache
    description: How TwinfoxDB Enterprise caches remote JSON-LD contexts locally using pre-seeded files and a disk-backed HTTP cache.
    keywords: context cache, json-ld, schema.org, seed files, enterprise, terminusdb
    alternates:
      canonical: https://terminusdb.org/docs/enterprise-context-cache/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
---

Remote JSON-LD contexts like `https://schema.org/` are large files that change infrequently. Fetching them on every request would add latency and create a network dependency in production. Enterprise solves this with a two-layer disk cache that serves pre-seeded contexts instantly and caches any new remote contexts for future use.

## Architecture

The context cache uses two storage locations under a single configurable path:

```
<cache_path>/
├── catalog/                  # Catalog directory (plain files)
│   ├── catalog               # SWI-Prolog SGML Open catalog
│   ├── index.json            # URL → filename mapping
│   └── 63403d...01.jsonld    # Cached context files (SHA-1 hash of URL)
└── http-cache/               # HTTP cache (managed by cacache)
    └── ...                   # Automatic HTTP cache entries
```

**Catalog directory** — contains plain, inspectable JSON-LD files keyed by the SHA-1 hash of the source URL. A human-readable `index.json` maps URLs to filenames. This is the primary lookup layer.

**HTTP cache** — This provides RFC-compliant HTTP caching with proper `Cache-Control` and `ETag` handling for network fetches.

## Pre-seeded contexts

Enterprise ships with pre-seeded context files for commonly used vocabularies. These are plain files in the repository at `terminusdb-enterprise/data/context-seeds/` and are copied into the catalog directory on first use.

Currently pre-seeded:

{% table %}

- URL
- Description

---

- `https://schema.org/`
- Schema.org vocabulary (~210 KB)

{% /table %}

Pre-seeded contexts resolve instantly from disk without any network access. This means schema.org-based JSON-LD processing works out of the box, even in air-gapped environments.

### Adding custom seed files

To pre-seed additional contexts:

1. Download the context file
2. Compute the SHA-1 hash of the URL (not the file content)
3. Save as `<hash>.jsonld` in the seed directory
4. Add the mapping to `index.json`

For example, to pre-seed `https://example.org/context.jsonld`:

```bash
# Compute the SHA-1 hash of the URL string
echo -n "https://example.org/context.jsonld" | shasum -a 1
# Output: a1b2c3d4...  (40 hex characters)

# Download and save with the hash as filename
curl -sL "https://example.org/context.jsonld" \
  -o terminusdb-enterprise/data/context-seeds/a1b2c3d4....jsonld
```

Then add to `index.json`:

```json
{
  "https://schema.org/": "63403d88865650fff3d3159efe7ec3b01e9d3401.jsonld",
  "https://example.org/context.jsonld": "a1b2c3d4....jsonld"
}
```

## Cache lookup order

When the JSON-LD processor encounters a remote URL context, the cache resolves it in this order:

1. **Catalog directory** — check if a file exists for this URL's SHA-1 hash. If found, read and return it immediately.
2. **HTTP cache** — if remote fetching is allowed, send an HTTP request through the caching middleware. The middleware checks its disk cache first and only makes a network request if needed.
3. **Dual-write** — if a network fetch succeeds, the response is written to both the HTTP cache (automatic) and the catalog directory (explicit), so future lookups hit the fast catalog path.

If remote fetching is disabled and the context is not in the catalog, the request fails with an error.

## Cache path configuration

The cache directory defaults to `<storage_dir>/context_cache`, where `<storage_dir>` is the parent of the database path (typically `./storage/`).

Override it with the `TERMINUSDB_CONTEXT_CACHE_PATH` environment variable:

```bash
export TERMINUSDB_CONTEXT_CACHE_PATH=/var/cache/terminusdb/contexts
```

## Remote fetching control

By default, the cache only serves pre-seeded and previously cached contexts. To allow fetching new remote contexts over the network:

```bash
export TERMINUSDB_ALLOW_REMOTE_CONTEXT=true
```

When set to `true`, cache misses trigger an HTTP fetch. When `false` (the default), only locally available contexts are served.

For production deployments, the recommended approach is to pre-seed all required contexts and leave remote fetching disabled. This eliminates network dependencies and ensures deterministic behavior.

## Docker considerations

In Docker enterprise images, the seed files are copied into the image at build time. The cache directory is created under the `storage/` volume, so cached contexts persist across container restarts.

If you mount a custom storage volume, the cache will be populated on first use from the seed files bundled in the image.

## Further reading

- [JSON-LD Context Processing](/docs/enterprise-jsonld-context/) — how `@context` works in the processing pipeline
- [Configuration Reference](/docs/enterprise-configuration/) — all cache-related environment variables
