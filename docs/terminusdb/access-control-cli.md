---
tags:
  - how-to
  - access-control
  - curl
  - cli
  - intermediate
title: Manage Access Control with the CLI
nextjs:
  metadata:
    title: Manage Access Control with the CLI — TerminusDB
    description: Create users, list roles, grant and revoke capabilities using the TerminusDB CLI. Includes a worked example for anonymous public cloning.
    keywords: terminusdb cli, access control, capability grant, anonymous clone, public database, terminusdb user, terminusdb role, rbac cli
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/access-control-cli/
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have configured access control (users, organisations, capabilities) using the CLI.
{% /callout %}

TerminusDB's CLI provides direct access to the role-based access control system without HTTP requests. Use it to create users, inspect roles, and grant or revoke capabilities from the command line.

{% callout type="note" %}
**Prerequisites**
- TerminusDB installed and accessible via the `terminusdb` CLI command. If you are running TerminusDB in Docker, prefix commands with `docker exec terminusdb` (e.g. `docker exec terminusdb terminusdb user get`).
- A database to grant access to. This guide uses the canonical `admin/MyDatabase`.
{% /callout %}

{% callout type="note" %}
For the HTTP API equivalent of these operations, see [Access Control Tutorial (HTTP)](/docs/access-control-tutorial). For the full access control model (roles, capabilities, inheritance), see the [Access Control Reference](/docs/access-control).
{% /callout %}

## List built-in roles

TerminusDB ships with two built-in roles. Inspect them before granting:

```bash
terminusdb role get
```

Expected output:

```json
[
  {
    "@id": "Role/admin",
    "name": "admin",
    "action": ["branch", "class_frame", "clone", "commit_read_access", "commit_write_access", "create_database", "delete_database", "fetch", "instance_read_access", "instance_write_access", "manage_capabilities", "meta_read_access", "meta_write_access", "push", "rebase", "schema_read_access", "schema_write_access"]
  },
  {
    "@id": "Role/consumer",
    "name": "consumer",
    "action": ["class_frame", "clone", "instance_read_access", "meta_read_access", "schema_read_access"]
  }
]
```

| Role | Purpose |
|------|---------|
| `admin` | Full read/write access — all actions |
| `consumer` | Read-only access — read instance data, schema, metadata, and clone |

To inspect a single role by name:

```bash
terminusdb role get consumer --json
```

## Create a custom role

The built-in roles cover common cases, but you can create roles with exactly the actions you need:

```bash
terminusdb role create ROLE_NAME ACTION_1 ACTION_2 ... ACTION_N
```

Available actions: `create_database`, `delete_database`, `class_frame`, `clone`, `fetch`, `push`, `branch`, `rebase`, `instance_read_access`, `instance_write_access`, `schema_read_access`, `schema_write_access`, `meta_read_access`, `meta_write_access`, `commit_read_access`, `commit_write_access`, `manage_capabilities`

### Example: a clone-only role

```bash
terminusdb role create cloner clone commit_read_access
```

This creates a role with the `clone` and `commit_read_access` actions — the minimum required for anonymous public access (see [worked example below](#worked-example-enable-anonymous-cloning)).

### Example: a read-write role without admin powers

```bash
terminusdb role create writer instance_read_access instance_write_access schema_read_access commit_read_access commit_write_access branch rebase
```

To delete a custom role:

```bash
terminusdb role delete cloner
```

## Create a user

Create a new user with a password:

```bash
terminusdb user create alice --password alice-secret
```

To create a user without a password (for token-based or anonymous access):

```bash
terminusdb user create anonymous
```

Verify the user exists:

```bash
terminusdb user get alice --json
```

To list all users:

```bash
terminusdb user get
```

## Grant a capability

Grant one or more roles to a user on a specific scope (database or organisation):

```bash
terminusdb capability grant USER SCOPE ROLE
```

**Parameters:**

| Parameter | Description |
|-----------|-------------|
| `USER` | The user name (e.g. `alice`, `anonymous`) |
| `SCOPE` | The target database (e.g. `admin/MyDatabase`) or organisation (e.g. `admin`) |
| `ROLE` | One or more role names (e.g. `consumer`, `admin`) |

**Options:**

| Flag | Description |
|------|-------------|
| `--scope-type database` | Interpret SCOPE as a database path (default) |
| `--scope-type organization` | Interpret SCOPE as an organisation name |

### Example: grant read-only access to a database

```bash
terminusdb capability grant alice admin/MyDatabase consumer
```

Alice can now read documents and schema from `admin/MyDatabase`, but cannot write, branch, or delete.

### Example: grant admin access to an organisation

```bash
terminusdb capability grant alice admin admin --scope-type organization
```

Alice now has full admin access to the `admin` organisation and all databases within it.

## Revoke a capability

Remove one or more roles from a user on a scope:

```bash
terminusdb capability revoke USER SCOPE ROLE
```

The parameters and options are identical to `capability grant`.

### Example: revoke read-only access

```bash
terminusdb capability revoke alice admin/MyDatabase consumer
```

Alice can no longer access `admin/MyDatabase`.

## Verify a user's capabilities

Check what capabilities a user has:

```bash
terminusdb user get alice --capability --json
```

This returns the user record including all granted capabilities, their scopes, and associated roles.

---

## Worked example: enable anonymous cloning

The most common use case for CLI access control is making a database publicly cloneable without authentication. TerminusDB has a built-in `anonymous` user — granting it clone permission on a database allows anyone to clone that database without credentials.

### Why this matters

Public datasets, documentation examples, and template databases all need to be cloneable by anyone. Without anonymous access, every `clone` request requires valid credentials — which blocks use cases like:

- Documentation quickstarts that clone a pre-built dataset
- Public API servers that serve read-only data
- Template servers that distribute starter databases

### Step 1 — Create a minimal `cloner` role

The built-in `consumer` role grants broader read access (fetch, schema read, instance read) than anonymous users need. Follow the principle of least privilege — create a dedicated role with only the actions required for cloning:

```bash
terminusdb role create cloner clone commit_read_access
```

This creates a role with exactly two actions: `clone` (to clone the database) and `commit_read_access` (required by the clone operation). Nothing else.

Verify it was created:

```bash
terminusdb role get cloner --json
```

Expected output:

```json
{
  "@id": "Role/cloner",
  "name": "cloner",
  "action": ["clone", "commit_read_access"]
}
```

{% callout type="note" title="Why not use the consumer role?" %}
`consumer` grants `clone`, `fetch`, `instance_read_access`, `schema_read_access`, `meta_read_access`, and `class_frame`. For anonymous public access, that is more than needed. If an anonymous user only needs to clone the database (not browse its documents via the API), `cloner` is the correct, minimal role.

Use `consumer` instead if you want anonymous users to also read documents and schema via the HTTP API without cloning.
{% /callout %}

### Step 2 — Confirm the database exists

```bash
terminusdb db list
```

You should see `admin/MyDatabase` (or whichever database you want to make public) in the output.

### Step 3 — Grant clone access to anonymous

```bash
terminusdb capability grant anonymous admin/MyDatabase cloner
```

This grants the `anonymous` user the `cloner` role on `admin/MyDatabase`. Anonymous requests can now clone that database — and nothing else.

### Step 4 — Verify the grant

```bash
terminusdb user get anonymous --capability --json
```

Expected output includes a capability entry linking `anonymous` to the `cloner` role on your database:

```json
{
  "@id": "User/anonymous",
  "name": "anonymous",
  "capability": [
    {
      "@id": "Capability/...",
      "role": ["Role/cloner"],
      "scope": "admin/MyDatabase"
    }
  ]
}
```

### Step 5 — Test anonymous cloning

From a different machine (or without credentials), clone the database:

```bash
curl -X POST http://localhost:6363/api/clone/admin/my-local-copy \
  -H "Content-Type: application/json" \
  -d '{
    "remote_url": "http://localhost:6363/admin/MyDatabase",
    "label": "My Local Copy",
    "comment": "Cloned without authentication"
  }'
```

No `-u admin:root` — the request succeeds because `anonymous` has `clone` permission via the `cloner` role.

### Step 6 — Revoke access (if needed)

To make the database private again:

```bash
terminusdb capability revoke anonymous admin/MyDatabase cloner
```

Anonymous clone requests will now return `401 Unauthorized`.

To remove the role entirely (if no longer needed by any user):

```bash
terminusdb role delete cloner
```

---

## Quick reference

| Task | Command |
|------|---------|
| List all roles | `terminusdb role get` |
| Create a custom role | `terminusdb role create ROLE_NAME ACTION1 ACTION2 ...` |
| Delete a custom role | `terminusdb role delete ROLE_NAME` |
| List all users | `terminusdb user get` |
| Create a user | `terminusdb user create USER --password PASS` |
| Grant a role on a database | `terminusdb capability grant USER DB ROLE` |
| Grant a role on an organisation | `terminusdb capability grant USER ORG ROLE --scope-type organization` |
| Revoke a role | `terminusdb capability revoke USER SCOPE ROLE` |
| Check user capabilities | `terminusdb user get USER --capability --json` |
| Enable anonymous cloning | `terminusdb role create cloner clone` then `terminusdb capability grant anonymous admin/DB cloner` |
| Disable anonymous cloning | `terminusdb capability revoke anonymous admin/DB cloner` |

## See also

- [Access Control Tutorial (HTTP API)](/docs/access-control-tutorial) — same operations using curl
- [Access Control Reference](/docs/access-control) — full model: roles, actions, capability inheritance
- [CLI Commands Reference](/docs/terminusdb-cli-commands) — complete CLI documentation
