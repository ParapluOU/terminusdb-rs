---
tags:
  - reference
  - documents
  - curl
  - intermediate
title: Database Path Identifiers Reference
nextjs:
  metadata:
    title: Database Path Identifiers - GRAPH_SPEC and DB_SPEC Reference
    description: Reference guide for GRAPH_SPEC, DB_SPEC, and path identifiers used in TerminusDB CLI, REST API, and client libraries to address databases, branches, commits, and graphs
    keywords: terminusdb, api, branch, commit, database path identifiers reference, document database, documents, graph traversal
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/graph-spec-db-spec-database-path-identifiers/
media: []
---

Understanding how to address specific databases, branches, commits, and graphs is fundamental to working with TerminusDB. Whether you're using the CLI, REST API, or client libraries, you'll encounter **DB_SPEC** and **GRAPH_SPEC** identifiers. This reference guide explains these identifiers in plain language with practical examples.

## What is a DB_SPEC?

A **DB_SPEC** (Database Specification) is a path-based identifier that precisely locates a database, branch, or commit within TerminusDB. Think of it as an address that tells TerminusDB exactly which version of which database you want to access.

### Basic DB_SPEC Structure

The general format follows this pattern:

```text
<organization>/<database>/<repository>/<ref_type>/<ref_name>
```

### DB_SPEC Components Explained

1. **Organization** - Your team or organization (e.g., `admin`, `my_team`)
2. **Database** - The name of your data product or database
3. **Repository** - Usually `local` (or a remote name if configured)
4. **Reference Type** - Either `branch` or `commit`
5. **Reference Name** - The branch name (e.g., `main`) or commit ID

### Common DB_SPEC Patterns

Here are the most frequently used DB_SPEC formats, and special ones:

#### Simple Database Reference (Default Branch)
```text
<organization>/<database>
```
**Example:** `admin/MyDatabase`

This is shorthand for `admin/MyDatabase/local/branch/main` - it automatically points to the main branch of the local repository.

#### Specific Branch
```text
<organization>/<database>/local/branch/<branch_name>
```
**Examples:**
- `admin/MyDatabase/local/branch/main`
- `my_team/products/local/branch/development`
- `admin/customers/local/branch/feature-updates`

#### Specific Commit
```text
<organization>/<database>/local/commit/<commit_id>
```
**Example:** `admin/MyDatabase/local/commit/9w8hk3y6rb8tjdy961ed3i536ntkqd8`

Use this for time-travel queries or to reference a specific point in history.

#### Repository Metadata
```text
<organization>/<database>/_meta
```
**Example:** `admin/MyDatabase/_meta`

Access the repository graph containing information about the local repository and all known remotes.

#### Commit Graph
```text
<organization>/<database>/<repository>/_commits
```
**Example:** `admin/MyDatabase/local/_commits`

Access the commit graph containing branch histories, commit objects, authorship, and timestamps.

#### Remote Repository
```text
<organization>/<database>/<remote_name>/branch/<branch_name>
```
**Example:** `admin/MyDatabase/origin/branch/main`

Reference a branch on a configured remote repository.

#### System Database
```text
_system
```
Access the system metadata containing user information, organization data, and database records (requires system administrator permissions).

## What is a GRAPH_SPEC?

A **GRAPH_SPEC** (Graph Specification) extends a DB_SPEC to point to a specific graph within a database. TerminusDB stores data in multiple graphs - primarily **instance** data and **schema** definitions.

### GRAPH_SPEC Structure

```text
<DB_SPEC>/<graph_type>
```

Where `<graph_type>` is one of:
- `instance` - Your actual data documents
- `schema` - Your data model and type definitions

### GRAPH_SPEC Examples

#### Access Schema Graph
```text
admin/MyDatabase/local/branch/main/schema
```
Read or modify the schema (data model) for the employees database.

#### Access Instance Graph
```text
admin/MyDatabase/local/branch/main/instance
```
Query or update the actual data documents in the employees database.

#### Schema on Specific Commit
```text
admin/products/local/commit/abc123def456/schema
```
View the schema as it existed at a specific commit.

## Practical Usage Examples

### CLI Usage

#### Query a Database
```bash
terminusdb query admin/MyDatabase "select([X], t(X, rdf:type, Y))"
```

#### Optimize a Specific Branch
```bash
terminusdb optimize admin/MyDatabase/local/branch/main
```

#### Dump Triples from Schema
```bash
terminusdb triples dump admin/people/local/branch/main/schema
```

#### Load Triples into Instance Graph
```bash
terminusdb triples load admin/people/local/branch/development/instance data.ttl
```

### REST API Usage

In REST API endpoints, the DB_SPEC appears directly in the URL path structure:

#### Insert Document
```bash
POST http://localhost:6363/api/document/admin/MyDatabase/local/branch/main
```

The path structure breaks down as:
- `admin` - organization
- `MyDatabase` - database
- `local` - repository
- `branch/main` - branch reference

#### Query Schema Graph
```bash
GET http://localhost:6363/api/document/admin/MyDatabase/local/branch/main?graph_type=schema
```

### Client Library Usage

#### JavaScript Client
```javascript
const client = new TerminusClient.Client('http://localhost:6363', {
  organization: 'admin',
  db: 'MyDatabase',
  branch: 'main'  // Optional, defaults to 'main'
});

// The client constructs DB_SPEC internally as: admin/MyDatabase/local/branch/main
```

#### Python Client
```python
client = Client("http://localhost:6363")
client.connect(team="admin", db="MyDatabase", branch="main")

# The client uses: admin/MyDatabase/local/branch/main
```

## Special Cases and Edge Scenarios

### Default Values

When components are omitted, TerminusDB applies these defaults:

- **No repository specified:** Defaults to `local`
- **No branch specified:** Defaults to `branch/main`
- **No graph_type specified:** Typically defaults to `instance` (context-dependent)

### Examples with Defaults

```text
admin/MyDatabase
↓ Expands to ↓
admin/MyDatabase/local/branch/main
```

### Working with Remotes

After adding a remote:

```bash
terminusdb remote add admin/MyDatabase origin https://example.terminusdb.com/MyTeam/MyDatabase
```

You can reference the remote:

```bash
terminusdb pull admin/MyDatabase/origin/branch/main
```

## Common Patterns and Use Cases

### Time Travel Queries

Access historical data by referencing a specific commit:

```bash
terminusdb query admin/MyDatabase/local/commit/abc123 "select([X], t(X, rdf:type, Y))"
```

### Multi-Branch Development

Work with feature branches:

```bash
# Create feature branch
terminusdb branch create admin/products/local/branch/feature-new-fields

# Query feature branch
terminusdb query admin/products/local/branch/feature-new-fields "select([X], t(X, rdf:type, Y))"

# Push to remote
terminusdb push admin/products/local/branch/feature-new-fields
```

### Schema Evolution

View schema changes across commits:

```bash
# Current schema
terminusdb triples dump admin/inventory/local/branch/main/schema

# Schema at previous commit
terminusdb triples dump admin/inventory/local/commit/previous123/schema
```

## Use _system Carefully
The `_system` database contains critical metadata:
```bash
# ⚠️ Be extremely careful with write operations
terminusdb query _system "..."
```

## Quick Reference Table

| Format | Example | Use Case |
|--------|---------|----------|
| `_system` | `_system` | Access system metadata |
| `<org>/<db>` | `admin/MyDatabase` | Quick access to main branch |
| `<org>/<db>/local/branch/<branch>` | `admin/emp/local/branch/dev` | Specific branch |
| `<org>/<db>/local/commit/<hash>` | `admin/emp/local/commit/abc123` | Specific commit (time travel) |
| `<org>/<db>/_meta` | `admin/MyDatabase/_meta` | Repository metadata |
| `<org>/<db>/local/_commits` | `admin/emp/local/_commits` | Commit history graph |
| `<DB_SPEC>/schema` | `admin/emp/local/branch/main/schema` | Schema graph |
| `<DB_SPEC>/instance` | `admin/emp/local/branch/main/instance` | Instance data graph |

## Related Documentation

- [CLI Commands Reference](/docs/terminusdb-cli-commands/) - Complete CLI command documentation
- [CLI Query Interface](/docs/terminusdb-db-cli-querying/) - WOQL syntax for CLI
- [HTTP Documents API](/docs/http-documents-api/) - REST API examples with path identifiers
- [JavaScript Client](/docs/javascript/) - Client library documentation
- [Python Client](/docs/python/) - Python client documentation

## Getting Help

If you're struggling to construct the right DB_SPEC or GRAPH_SPEC:

1. **List your databases:** `terminusdb db list`
2. **Check branches:** `terminusdb db list <org>/<db> --branches`
3. **View commit history:** `terminusdb log <org>/<db>`
4. **Test with simple queries:** Start with `_system` to verify syntax

For community support, visit the [TerminusDB Discord](https://discord.gg/terminusdb) or [GitHub Discussions](https://github.com/terminusdb/terminusdb/discussions).
