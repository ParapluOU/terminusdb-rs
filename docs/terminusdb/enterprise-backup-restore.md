---
tags:
  - how-to
  - documents
  - curl
  - enterprise
title: Backup & Restore
nextjs:
  metadata:
    title: Backup & Restore
    description: How to use the TwinfoxDB Enterprise bundle and unbundle API endpoints for database backup, restore, migration, and disaster recovery.
    keywords: backup, restore, bundle, unbundle, migration, disaster recovery, enterprise, api
    alternates:
      canonical: https://terminusdb.org/docs/enterprise-backup-restore/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB Enterprise edition running
- Access to the backup/restore CLI or API
- Sufficient storage for backup files
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to back up and restore TerminusDB Enterprise databases.
{% /callout %}

Enterprise provides binary bundle and unbundle API endpoints for database backup, migration between servers, and disaster recovery. A bundle captures the complete state of a database — schema, data, and commit history — in a single binary payload.

## Bundle (export)

Create a bundle of an entire database:

```bash
curl -s -X POST \
  -H "$AUTH" \
  "$SERVER/api/bundle/admin/MyDatabase" \
  --output MyDatabase-backup.bundle
```

The response is a binary payload containing the full database state. Save it to a file for later restoration or transfer to another server.

### What a bundle contains

A bundle is a self-contained snapshot that includes:

- All documents in the instance graph
- The complete schema
- The full commit history (all branches, all commits)
- Layer data from the underlying immutable store

Bundles are deterministic — bundling the same database state twice produces the same binary output.

## Unbundle (import)

Restore a bundle into a new data product. First create the target database, then unbundle into it:

```bash
# Create a new data product for the restore
curl -s -X POST \
  -H "Content-Type: application/json" \
  -H "$AUTH" \
  -d '{"label": "My Database (restored)", "comment": "Restored from backup"}' \
  "$SERVER/api/db/admin/MyDatabase-restored"

# Restore the bundle into the new data product
curl -s -X POST \
  -H "Content-Type: application/octets" \
  -H "$AUTH" \
  --data-binary @MyDatabase-backup.bundle \
  "$SERVER/api/unbundle/admin/MyDatabase-restored"
```

The target database must already exist before unbundling. The unbundle operation replaces the database contents with the bundle's state, including all history. Restoring to a separate data product lets you verify the backup before replacing the original.

## Use cases

### Scheduled backups

Run a cron job to create nightly backups:

```bash
#!/bin/bash
DATE=$(date +%Y%m%d)
curl -s -X POST -H "Authorization: Basic $AUTH_TOKEN" \
  "http://localhost:6363/api/bundle/admin/production" \
  --output "/backups/production-${DATE}.bundle"
```

### Server migration

Move a database between servers:

```bash
# On the source server
curl -s -X POST -H "$AUTH" \
  "$SOURCE_SERVER/api/bundle/admin/MyDatabase" \
  --output MyDatabase.bundle

# On the target server
curl -s -X POST -H "Content-Type: application/json" -H "$AUTH" \
  -d '{"label": "My Database"}' \
  "$TARGET_SERVER/api/db/admin/MyDatabase"

curl -s -X POST -H "Content-Type: application/octets" -H "$AUTH" \
  --data-binary @MyDatabase.bundle \
  "$TARGET_SERVER/api/unbundle/admin/MyDatabase"
```

### Disaster recovery

Store bundles in an object store (S3, GCS, Azure Blob) for off-site recovery. Bundles are binary files that use succinct data structure which makes them very compact.

## API reference

### POST /api/bundle/{path}

Creates a binary bundle of the database at `{path}`.

- **Authentication:** Required
- **Response Content-Type:** `application/octets`
- **Response:** Binary bundle payload

### POST /api/unbundle/{path}

Restores a binary bundle into the database at `{path}`.

- **Authentication:** Required
- **Request Content-Type:** `application/octets`
- **Request body:** Binary bundle payload
- **Response:** JSON success confirmation

## Further reading

- [Enterprise Overview](/docs/enterprise/) — all enterprise features
- [Configuration Reference](/docs/enterprise-configuration/) — environment variables
