# TerminusDB CLI (`tdb`)

Command-line interface for TerminusDB operations.

## Installation

```bash
cargo install --path .
```

Or run directly from the workspace:

```bash
cargo run -p terminusdb-cli -- [command]
```

## Commands

### `changestream` - Stream database changesets

Stream real-time changeset events from TerminusDB's SSE endpoint to stdout.

#### Usage

```bash
# Using environment variables
export TERMINUSDB_HOST="http://localhost:6363"
export TERMINUSDB_USER="admin"
export TERMINUSDB_PASS="root"
export TERMINUSDB_ORG="admin"
export TERMINUSDB_DB="mydb"
export TERMINUSDB_BRANCH="main"

tdb changestream

# Using CLI arguments
tdb changestream \
  --host http://localhost:6363 \
  --user admin \
  --password root \
  --org admin \
  --database mydb \
  --branch main

# With custom output format
tdb changestream --database mydb --format compact
```

#### Arguments

- `--host` - TerminusDB server URL (env: `TERMINUSDB_HOST`, default: `http://localhost:6363`)
- `--user` - Username for authentication (env: `TERMINUSDB_USER`, default: `admin`)
- `--password` - Password for authentication (env: `TERMINUSDB_PASS`, default: `root`)
- `--org` - Organization name (env: `TERMINUSDB_ORG`, default: `admin`)
- `--database` - Database name to monitor (env: `TERMINUSDB_DB`, **required**)
- `--branch` - Branch name to monitor (env: `TERMINUSDB_BRANCH`, default: `main`)
- `--format` - Output format: `pretty`, `json`, or `compact` (default: `pretty`)
- `--color` - Color output: `auto`, `always`, or `never` (default: `auto`)

#### Output Formats

**Pretty format (default):**

Stylized, colorful output showing structured document changes:

```
Commit abc123... by User/admin
Message: Updated user profiles
Changes: +1 ~2 -0

  ~ User/123 changed
    User {
      email: "new@example.com"
    }

  + Person/456 added

```

Colors:
- **Green (+)** - Added documents
- **Yellow (~)** - Updated documents
- **Red (-)** - Deleted documents
- **Cyan** - Commit IDs
- **Blue** - Authors
- **Dim** - Document IDs

**JSON format:**

Each changeset event is printed as a JSON line (use `--format json`):

```json
{
  "timestamp": "2025-10-27T12:34:56.789Z",
  "resource": "admin/mydb/local/branch/main",
  "branch": "main",
  "commit": {
    "id": "abc123...",
    "author": "User/admin",
    "message": "Updated documents",
    "timestamp": 1730000000.0
  },
  "metadata": {
    "inserts_count": 5,
    "deletes_count": 2,
    "documents_added": 1,
    "documents_deleted": 0,
    "documents_updated": 2
  },
  "changes": [
    {
      "id": "Person/123",
      "action": "updated"
    },
    {
      "id": "Person/456",
      "action": "added"
    }
  ]
}
```

**Compact format:**

One-line summary per changeset:

```
abc123... | User/admin | Updated documents | + 1 ~ 2 - 0
```

Format: `commit_id | author | message | changes_summary`

With colors enabled, the `+`, `~`, and `-` symbols are colored green, yellow, and red respectively.

#### Examples

**Pipe to jq for filtering:**

```bash
tdb changestream --database mydb | jq -r '.changes[] | select(.action == "added") | .id'
```

**Monitor changes and log to file:**

```bash
tdb changestream --database mydb --format compact | tee changes.log
```

**Filter for specific document types:**

```bash
tdb changestream --database mydb --format json | jq -r '.changes[] | select(.id | startswith("Person/"))'
```

**Disable colors for piping:**

```bash
tdb changestream --database mydb --color never | grep "User"
```

**Force colors even when piping:**

```bash
tdb changestream --database mydb --color always | less -R
```

## Environment Variables

The CLI supports the following environment variables:

- `TERMINUSDB_HOST` - TerminusDB server URL
- `TERMINUSDB_USER` - Username for authentication
- `TERMINUSDB_PASS` - Password for authentication (also checks `TERMINUSDB_ADMIN_PASS`)
- `TERMINUSDB_ORG` - Organization name
- `TERMINUSDB_DB` - Database name
- `TERMINUSDB_BRANCH` - Branch name

## Logging

The CLI uses `tracing` for logging. Logs are written to stderr (keeping stdout clean for data output).

Set the `RUST_LOG` environment variable to control log levels:

```bash
# Show debug logs
RUST_LOG=debug tdb changestream --database mydb

# Show only errors
RUST_LOG=error tdb changestream --database mydb
```

## Notes

- The changestream command connects to the TerminusDB SSE endpoint at `/changesets/stream`
- Events are filtered by resource path (org/database/local/branch/branch_name)
- Press Ctrl+C to stop streaming
- Connection errors will cause the command to exit with an error code
