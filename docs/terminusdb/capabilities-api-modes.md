---
tags:
  - how-to
  - access-control
  - curl
  - intermediate
title: Access Grant with Capabilities API — Name Mode vs ID Mode
nextjs:
  metadata:
    title: TerminusDB Access Grants with Capabilities API — Name Mode vs ID Mode
    description: How to grant and revoke permissions using the two calling modes of the TerminusDB capabilities API. Includes worked examples, error diagnosis, and guidance for application developers.
    keywords: terminusdb capabilities, terminusdb grant, terminusdb revoke, scope_type, access control modes, capabilities API, grant permissions, RBAC
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/capabilities-api-modes/
---

The TerminusDB `/api/capabilities` endpoint supports two distinct calling modes. Choosing the wrong mode — or mixing elements from both — yields errors. This guide explains when to use each mode and how to avoid the most common mistakes.

{% callout type="note" %}
For a beginner introduction to access control, see the [Access Control Tutorial](/docs/access-control-tutorial). For the full endpoint reference, see [Access Control Reference](/docs/access-control).
{% /callout %}

## The two modes

| | Name Mode | ID Mode |
|---|---|---|
| **When to use** | Caller has `manage_capabilities` on SystemDatabase (typically the global admin) | Caller has `manage_capabilities` on a specific organisation only |
| **`scope_type` field** | Required (`"organization"` or `"database"`) | Must be omitted |
| **`user` field** | Bare username (e.g. `"alice"`) | Full document ID (e.g. `"User/alice"`) |
| **`scope` field** | Bare resource name — org name or `"org/db"` path | Full resource ID (e.g. `"Organization/myteam"` or `"UserDatabase/01AEV..."`) |
| **`roles` field** | Role display names (e.g. `["Consumer Role"]`) | Full document IDs (e.g. `["Role/consumer"]`) |
| **Permission required** | `manage_capabilities` on `system` (global admin) | `manage_capabilities` on the target resource |

{% callout type="warning" title="Do not mix modes" %}
If you include `scope_type` but use document IDs for `user` or `roles`, the request will fail. Conversely, if you omit `scope_type` but send bare names, TerminusDB cannot resolve them and returns an error.
{% /callout %}

## Name Mode — global admin

Name Mode is the simpler calling convention. TerminusDB resolves human-readable names to internal document IDs on your behalf. It requires the `scope_type` field and the caller must have `manage_capabilities` on the SystemDatabase, i.e. needs have the full admin capabilities.

### Grant a role on an organisation

```bash
curl -s -u admin:root -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "grant",
    "scope_type": "organization",
    "scope": "myteam",
    "user": "alice",
    "roles": ["Consumer Role"]
  }'
```

**Response (200):**
```json
{"@type": "api:CapabilityResponse", "api:status": "api:success"}
```

### Grant a role on a specific database

```bash
curl -s -u admin:root -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "grant",
    "scope_type": "database",
    "scope": "myteam/salesdata",
    "user": "alice",
    "roles": ["writer"]
  }'
```

The `scope` field uses the path format `"orgName/dbName"` when `scope_type` is `"database"`.

### Revoke a role

```bash
curl -s -u admin:root -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "revoke",
    "scope_type": "organization",
    "scope": "myteam",
    "user": "alice",
    "roles": ["Consumer Role"]
  }'
```

## ID Mode — delegated admin

ID Mode is required when the caller is not the global admin but has `manage_capabilities` on a specific organisation. Since TerminusDB cannot perform system-wide name resolution for non-global callers, you must provide full document IDs for every field.

### Find the document IDs you need

Before making a grant call in ID Mode, you need the internal IDs. The admin user can look these up:

```bash
# Get user ID
curl -s -u admin:root http://localhost:6363/api/users/alice
# Response includes: "@id": "User/alice"

# Get organisation ID
curl -s -u admin:root http://localhost:6363/api/organizations
# Response includes: "@id": "Organization/myteam"

# Get role ID
curl -s -u admin:root http://localhost:6363/api/roles
# Response includes: "@id": "Role/consumer", "name": "Consumer Role"

# Get database ID (for database-scoped grants)
curl -s -u admin:root "http://localhost:6363/api/organizations/myteam/users/admin/databases"
# Response includes database objects with "@id": "UserDatabase/01AEVhV4..."
```

### Grant a role on an organisation (ID Mode)

```bash
curl -s -u orgadmin:password -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "grant",
    "scope": "Organization/myteam",
    "user": "User/alice",
    "roles": ["Role/consumer"]
  }'
```

Notice: **no `scope_type` field**. All identifiers use their full document ID form.

### Grant a role on a database (ID Mode)

```bash
curl -s -u orgadmin:password -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "grant",
    "scope": "UserDatabase/01AEVhV4o2weLi0i",
    "user": "User/alice",
    "roles": ["Role/writer"]
  }'
```

For database scope in ID Mode, the `scope` is the database's internal `@id` (a hash-based identifier), not the `"org/db"` path format.

### Revoke a role (ID Mode)

```bash
curl -s -u orgadmin:password -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "revoke",
    "scope": "Organization/myteam",
    "user": "User/alice",
    "roles": ["Role/consumer"]
  }'
```

## Error diagnosis

### Errors due to bare names without `scope_type`

**Symptom:** You send bare names but omit `scope_type`.

```bash
# ❌ BROKEN: bare names without scope_type
curl -s -u admin:root -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "grant",
    "scope": "myteam",
    "user": "alice",
    "roles": ["Consumer Role"]
  }'
```

**Why it fails:** Without `scope_type`, TerminusDB treats `"alice"` as a document ID. There is no document with `@id` equal to `"alice"` — the actual ID is `"User/alice"`. The server cannot resolve the reference and returns 500.

**Fix:** Either add `scope_type` (Name Mode) or use full document IDs (ID Mode).

### Error with document IDs with `scope_type`

**Symptom:** You send `scope_type` but also use document ID format.

```bash
# ❌ BROKEN: scope_type present but IDs used instead of names
curl -s -u admin:root -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "grant",
    "scope_type": "organization",
    "scope": "Organization/myteam",
    "user": "User/alice",
    "roles": ["Role/consumer"]
  }'
```

**Why it fails:** With `scope_type`, TerminusDB wraps your values with type prefixes automatically. `"Organization/myteam"` becomes `"Organization/Organization/myteam"` — which does not exist.

**Fix:** Remove the type prefixes — use bare names when `scope_type` is present.

### 403 Forbidden — insufficient capabilities

**Symptom:** A non-admin user tries to use Name Mode.

```bash
# ❌ FAILS: orgadmin does not have manage_capabilities on SystemDatabase
curl -s -u orgadmin:password -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "grant",
    "scope_type": "organization",
    "scope": "myteam",
    "user": "alice",
    "roles": ["Consumer Role"]
  }'
```

**Why it fails:** Name-based resolution (`scope_type`) requires `manage_capabilities` on the **SystemDatabase** — a global admin privilege. Organisation-level admins must use ID Mode.

**Fix:** Remove `scope_type` and switch to full document IDs.

## Decision guide for application developers

When building an application that manages capabilities programmatically:

1. **Determine the caller's privilege level at runtime.** Does the authenticated user have global admin access (e.g. username `"admin"`, or a user with `manage_capabilities` on SystemDatabase)?

2. **Choose the mode based on privilege:**
   - Global admin → **Name Mode** (simpler, no ID lookups required)
   - Organisation admin → **ID Mode** (must look up or cache document IDs)

3. **Never default to one mode unconditionally.** If your code always omits `scope_type` but sends bare names, it will break. If it always sends `scope_type` but the user lacks global admin, it will get 403.

```text
┌─────────────────────────────┐
│ Is caller a global admin?   │
└──────────────┬──────────────┘
               │
       ┌───────┴───────┐
       │ Yes           │ No
       ▼               ▼
┌──────────────┐ ┌──────────────────┐
│ Name Mode    │ │ ID Mode          │
│              │ │                  │
│ scope_type ✓ │ │ scope_type ✗    │
│ bare names   │ │ full doc IDs     │
└──────────────┘ └──────────────────┘
```

## Summary

| Scenario | Mode | `scope_type` | Identifiers |
|----------|------|:---:|---|
| Admin grants user access to a team | Name | `"organization"` | `user: "alice"`, `scope: "myteam"` |
| Admin grants user access to a database | Name | `"database"` | `user: "alice"`, `scope: "myteam/salesdata"` |
| Org admin grants user access to their team | ID | omit | `user: "User/alice"`, `scope: "Organization/myteam"` |
| Org admin grants user access to a database | ID | omit | `user: "User/alice"`, `scope: "UserDatabase/01AEV..."` |
| Admin revokes from a team | Name | `"organization"` | `user: "alice"`, `scope: "myteam"`, `roles: ["Consumer Role"]` |
| Org admin revokes from a team | ID | omit | `user: "User/alice"`, `scope: "Organization/myteam"`, `roles: ["Role/consumer"]` |

## See Also

- [Access Control Reference](/docs/access-control) — Full API reference for users, roles, and capabilities
- [Access Control Tutorial](/docs/access-control-tutorial) — Beginner walkthrough
- [Troubleshooting Authentication Errors](/docs/troubleshooting-auth) — Connection and auth issue diagnosis
