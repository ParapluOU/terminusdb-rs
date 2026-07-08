---
tags:
  - reference
  - access-control
  - curl
  - intermediate
title: Access Control Reference
nextjs:
  metadata:
    title: TerminusDB Access Control Reference — Users, Roles, and Capabilities
    description: Complete reference for TerminusDB's role-based access control system. Manage users, roles, capabilities, and database permissions via the HTTP API.
    keywords: terminusdb access control, terminusdb rbac, terminusdb user permissions, access control, RBAC, roles, capabilities, permissions, users, authorization
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/access-control/
---

TerminusDB provides role-based access control (RBAC) for managing who can access databases and what operations they can perform. This page is a complete reference for the access control model and its HTTP API.

{% callout type="note" %}
For a step-by-step walkthrough, see the [Access Control Tutorial](/docs/access-control-tutorial). For the JavaScript client API, see the [JavaScript Access Control Reference](/docs/access-control-with-javascript/).
{% /callout %}

## Access Control Model

TerminusDB's access control consists of four concepts:

| Concept | Description |
|---------|-------------|
| **User** | An identity that can authenticate and perform operations |
| **Role** | A named set of permitted actions (e.g. "admin", "consumer") |
| **Capability** | A grant that links a user to a role on a specific resource (scope) |
| **Resource** | The target of a capability — either an organisation or a database |

A user can perform an action on a resource only if they have a capability that includes a role containing that action for that resource.

```text
User ──→ Capability ──→ Role (contains Actions)
                   └──→ Resource (Organisation or Database)
```

## Built-in Roles

TerminusDB includes two built-in roles:

### Admin Role

The admin role grants all available actions. The default `admin` user has this role on the `admin` organisation.

**Actions:** `branch`, `class_frame`, `clone`, `commit_read_access`, `commit_write_access`, `create_database`, `delete_database`, `fetch`, `instance_read_access`, `instance_write_access`, `manage_capabilities`, `meta_read_access`, `meta_write_access`, `push`, `rebase`, `schema_read_access`, `schema_write_access`

### Consumer Role

The consumer role grants read-only access to instance data and schema.

**Actions:** `class_frame`, `instance_read_access`, `schema_read_access`

## All Available Actions

| Action | Description |
|--------|-------------|
| `branch` | Create and manage branches |
| `class_frame` | Read class frame information |
| `clone` | Clone a database |
| `commit_read_access` | Read commit history |
| `commit_write_access` | Write commits (required for any data mutation) |
| `create_database` | Create new databases in an organisation |
| `delete_database` | Delete databases |
| `fetch` | Fetch from a remote |
| `instance_read_access` | Read instance (document) data |
| `instance_write_access` | Write instance (document) data |
| `manage_capabilities` | Grant or revoke capabilities for other users over the resource |
| `meta_read_access` | Read repository metadata for the resource |
| `meta_write_access` | Write repository metadata for the resource |
| `push` | Push to a remote |
| `rebase` | Rebase branches |
| `schema_read_access` | Read schema definitions |
| `schema_write_access` | Write schema definitions |

## Capability Inheritance

Capabilities granted at the **organisation level** are inherited by all databases within that organisation. You can also grant capabilities at the **database level** to give a user additional permissions on a specific database without affecting their access to other databases in the organisation.

Database-level capabilities **add to** (never reduce) organisation-level capabilities.

## HTTP API Reference

All access control endpoints require authentication as the `admin` user (or a user with `manage_capabilities` permission). Use Basic Auth with `admin:root` for local installations.

### Users

#### List all users

```bash
curl -s -u admin:root http://localhost:6363/api/users
```

**Response:**
```json
[
  {
    "@id": "User/admin",
    "@type": "User",
    "capability": ["Capability/server_access"],
    "name": "admin"
  }
]
```

#### Get a user (with capabilities)

```bash
curl -s -u admin:root "http://localhost:6363/api/users/admin?capability=true"
```

#### Create a user

```bash
curl -s -u admin:root -X POST http://localhost:6363/api/users \
  -H "Content-Type: application/json" \
  -d '{"name": "alice", "password": "secure-password"}'
```

**Response:**
```json
"terminusdb://system/data/User/alice"
```

#### Change a user's password

```bash
curl -s -u admin:root -X PUT http://localhost:6363/api/users \
  -H "Content-Type: application/json" \
  -d '{"name": "alice", "password": "new-secure-password"}'
```

#### Delete a user

```bash
curl -s -u admin:root -X DELETE http://localhost:6363/api/users/alice
```

### Roles

#### List all roles

```bash
curl -s -u admin:root http://localhost:6363/api/roles
```

**Response:**
```json
[
  {
    "@id": "Role/admin",
    "@type": "Role",
    "action": ["branch", "class_frame", "clone", "commit_read_access", "..."],
    "name": "Admin Role"
  },
  {
    "@id": "Role/consumer",
    "@type": "Role",
    "action": ["class_frame", "instance_read_access", "schema_read_access"],
    "name": "Consumer Role"
  }
]
```

#### Create a custom role

```bash
curl -s -u admin:root -X POST http://localhost:6363/api/roles \
  -H "Content-Type: application/json" \
  -d '{
    "name": "writer",
    "action": [
      "commit_write_access",
      "instance_read_access",
      "instance_write_access",
      "schema_read_access",
      "class_frame"
    ]
  }'
```

**Response:**
```json
"terminusdb://system/data/Role/writer"
```

#### Update a role

```bash
curl -s -u admin:root -X PUT http://localhost:6363/api/roles \
  -H "Content-Type: application/json" \
  -d '{
    "name": "writer",
    "action": [
      "commit_write_access",
      "instance_read_access",
      "instance_write_access",
      "schema_read_access",
      "schema_write_access",
      "class_frame"
    ]
  }'
```

#### Delete a role

```bash
curl -s -u admin:root -X DELETE http://localhost:6363/api/roles/writer
```

### Organisations

#### List all organisations

```bash
curl -s -u admin:root http://localhost:6363/api/organizations
```

#### Create an organisation

```bash
curl -s -u admin:root -X POST http://localhost:6363/api/organizations/MyTeam \
  -H "Content-Type: application/json" -d '{}'
```

#### Delete an organisation

```bash
curl -s -u admin:root -X DELETE http://localhost:6363/api/organizations/MyTeam
```

#### List users in an organisation

```bash
curl -s -u admin:root http://localhost:6363/api/organizations/admin/users
```

#### Get a user's databases in an organisation

```bash
curl -s -u admin:root http://localhost:6363/api/organizations/admin/users/alice/databases
```

### Capabilities (Grant and Revoke Access)

The `/api/capabilities` endpoint manages the relationship between users, roles, and resources.

#### Grant a role to a user on an organisation

```bash
curl -s -u admin:root -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "grant",
    "scope_type": "organization",
    "scope": "admin",
    "user": "alice",
    "roles": ["Consumer Role"]
  }'
```

This gives `alice` read-only access to all databases in the `admin` organisation.

#### Grant a role to a user on a specific database

```bash
curl -s -u admin:root -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "grant",
    "scope_type": "database",
    "scope": "admin/MyDatabase",
    "user": "alice",
    "roles": ["writer"]
  }'
```

This gives `alice` write access specifically to `MyDatabase`, in addition to any organisation-level permissions.

#### Revoke a role from a user

```bash
curl -s -u admin:root -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "revoke",
    "scope_type": "organization",
    "scope": "admin",
    "user": "alice",
    "roles": ["Consumer Role"]
  }'
```

### Capabilities Request Body

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `operation` | `"grant"` or `"revoke"` | Yes | Whether to add or remove the capability |
| `scope_type` | `"organization"` or `"database"` | No | Enables name-based lookups (recommended) |
| `scope` | string | Yes | Target resource — org name, `"org/db"` format, or full document ID |
| `user` | string | Yes | Target user — username (with `scope_type`) or full document ID |
| `roles` | string[] | Yes | Roles to grant/revoke — role names (with `scope_type`) or full document IDs |

{% callout type="warning" title="scope_type is recommended" %}
When `scope_type` is provided, you can use human-readable names for `scope`, `user`, and `roles`. Without it, you must use full system document IDs (e.g. `"Organization/abc123..."`, `"User/alice"`, `"Role/consumer"`).
{% /callout %}

## Common Patterns

### Read-only user for a single database

```bash
# 1. Create the user
curl -s -u admin:root -X POST http://localhost:6363/api/users \
  -H "Content-Type: application/json" \
  -d '{"name": "readonly-user", "password": "secure-password"}'

# 2. Grant consumer role on the specific database
curl -s -u admin:root -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "grant",
    "scope_type": "database",
    "scope": "admin/MyDatabase",
    "user": "readonly-user",
    "roles": ["Consumer Role"]
  }'
```

### Writer user for an entire organisation

```bash
# 1. Create a writer role
curl -s -u admin:root -X POST http://localhost:6363/api/roles \
  -H "Content-Type: application/json" \
  -d '{
    "name": "writer",
    "action": ["commit_write_access", "instance_read_access", "instance_write_access", "schema_read_access", "class_frame"]
  }'

# 2. Create user and grant the role at org level
curl -s -u admin:root -X POST http://localhost:6363/api/users \
  -H "Content-Type: application/json" \
  -d '{"name": "editor", "password": "secure-password"}'

curl -s -u admin:root -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "grant",
    "scope_type": "organization",
    "scope": "admin",
    "user": "editor",
    "roles": ["writer"]
  }'
```

## See Also

- [Access Control Tutorial](/docs/access-control-tutorial) — Step-by-step guide with the JavaScript client
- [Name Mode vs ID Mode](/docs/capabilities-api-modes) — When to use `scope_type` and how to avoid 500 errors
- [JavaScript Access Control Reference](/docs/access-control-with-javascript/) — Full client library API
- [Tutorial Source Code](/docs/access-control-tutorial-source/) — Complete JavaScript example
- [OpenAPI Specification](/docs/openapi) — Interactive API explorer
