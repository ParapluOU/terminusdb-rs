---
tags:
  - tutorial
  - access-control
  - curl
  - beginner
title: Access Control Tutorial
nextjs:
  metadata:
    title: TerminusDB Access Control Tutorial — Create Users and Grant Permissions
    description: Step-by-step tutorial to create a read-only user and grant database access in TerminusDB using curl. Takes less than 10 minutes.
    keywords: terminusdb access control, terminusdb rbac, terminusdb user permissions, access control, RBAC, tutorial, users, roles, permissions
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/access-control-tutorial/
---

In this tutorial you will create a second user with read-only access to a database. By the end you will have:

1. Created a database and inserted a document
2. Created a new user (`alice`) who can read — but not write — that database
3. Verified that permission enforcement works

**Time:** ~10 minutes  
**Prerequisites:** TerminusDB running on `localhost:6363` ([Install guide →](/docs/install-terminusdb-as-a-docker-container))

{% callout type="warning" title="TerminusDB must be running" %}
If you have trouble connecting, see [Troubleshooting Connection Failures](/docs/troubleshooting-connection) and [Authentication Errors](/docs/troubleshooting-auth).
{% /callout %}

## Step 1 — Create a database

First, create a database that we will protect with access control:

```bash
curl -s -u admin:root -X POST "http://localhost:6363/api/db/admin/MyDatabase" \
  -H "Content-Type: application/json" \
  -d '{"label": "MyDatabase", "comment": "Access control tutorial"}'
```

Expected response:

```json
{"@type":"api:DbCreateResponse","api:status":"api:success"}
```

## Step 2 — Insert a document

Now insert a document into the database so we have something to protect:

```bash
curl -s -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/MyDatabase?author=admin&message=Add+document&raw_json=true" \
  -H "Content-Type: application/json" \
  -d '{"@id": "terminusdb:///data/jane", "name": "Jane Smith", "email": "jane@example.com"}'
```

Expected response:

```json
["terminusdb:///data/jane"]
```

Verify the document is accessible:

```bash
curl -s -u admin:root \
  "http://localhost:6363/api/document/admin/MyDatabase?raw_json=true&id=terminusdb:///data/jane"
```

You should see Jane's document returned as JSON.

## Step 3 — Create a new user

Create a user called `alice` with a password:

```bash
curl -s -u admin:root -X POST http://localhost:6363/api/users \
  -H "Content-Type: application/json" \
  -d '{"name": "alice", "password": "alice-secret"}'
```

Expected response:
```json
"terminusdb://system/data/User/alice"
```

Verify the user exists:

```bash
curl -s -u admin:root http://localhost:6363/api/users/alice
```

## Step 4 — Grant read-only access

Grant the built-in "Consumer Role" (read-only) to `alice` on `MyDatabase`:

```bash
curl -s -u admin:root -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "grant",
    "scope_type": "database",
    "scope": "admin/MyDatabase",
    "user": "alice",
    "roles": ["Consumer Role"]
  }'
```

The Consumer Role includes three actions: `class_frame`, `instance_read_access`, and `schema_read_access`. This means `alice` can read documents and schema but cannot insert, update, or delete anything.

## Step 5 — Verify read access works

Authenticate as `alice` and read the document:

```bash
curl -s -u alice:alice-secret \
  "http://localhost:6363/api/document/admin/MyDatabase?raw_json=true&id=terminusdb:///data/jane"
```

You should see Jane's document — `alice` has read access.

## Step 6 — Verify write access is denied

Now try to insert a document as `alice`:

```bash
curl -s -u alice:alice-secret -X POST \
  "http://localhost:6363/api/document/admin/MyDatabase?author=alice&message=Attempt+write&raw_json=true" \
  -H "Content-Type: application/json" \
  -d '{"@id": "terminusdb:///data/bob", "name": "Bob Jones", "email": "bob@example.com"}'
```

You should receive a **403 Forbidden** error:
```json
{
  "@type": "api:ErrorResponse",
  "api:message": "Insufficient capabilities for this operation",
  "api:status": "api:forbidden"
}
```

This confirms that `alice` cannot write — the access control is working correctly.

## Step 7 — Upgrade to write access (optional)

If you want to give `alice` write access, create a custom writer role and grant it:

```bash
# Create a writer role
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

# Grant the writer role to alice on MyDatabase
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

Now `alice` can insert documents:

```bash
curl -s -u alice:alice-secret -X POST \
  "http://localhost:6363/api/document/admin/MyDatabase?author=alice&message=Add+Bob&raw_json=true" \
  -H "Content-Type: application/json" \
  -d '{"@id": "terminusdb:///data/bob", "name": "Bob Jones", "email": "bob@example.com"}'
```

## Step 8 — Clean up (optional)

Revoke access and delete the user:

```bash
# Revoke the consumer role
curl -s -u admin:root -X POST http://localhost:6363/api/capabilities \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "revoke",
    "scope_type": "database",
    "scope": "admin/MyDatabase",
    "user": "alice",
    "roles": ["Consumer Role"]
  }'

# Delete the user
curl -s -u admin:root -X DELETE http://localhost:6363/api/users/alice
```

## What you learned

- **Users** are identities that authenticate with username/password
- **Roles** are named sets of actions (the built-in Consumer Role is read-only)
- **Capabilities** link users to roles on specific resources (databases or organisations)
- The `/api/capabilities` endpoint grants and revokes access
- Permissions are enforced — unauthorized operations return 403

## Next steps

- [Access Control Reference](/docs/access-control/) — Full API reference for all endpoints and actions
- [JavaScript Client Access Control](/docs/access-control-with-javascript/) — Manage access control programmatically
- [Tutorial Source Code](/docs/access-control-tutorial-source/) — Full JavaScript example with custom roles and multiple users
