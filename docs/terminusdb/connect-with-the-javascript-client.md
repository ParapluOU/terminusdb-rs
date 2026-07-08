---
title: TypeScript Client — Quickstart
nextjs:
  metadata:
    title: TypeScript Client — Quickstart
    description: Connect to TerminusDB from TypeScript. Branch, edit, diff, and merge documents using the terminusdb npm client.
    keywords: terminusdb, branch, client, connect, connection, diff, document, javascript
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/connect-with-the-javascript-client/
tags:
  - typescript
  - how-to
  - beginner
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have established a connection to TerminusDB using the JavaScript client.
{% /callout %}

{% callout type="note" %}
This guide uses **TypeScript**. Also available in [Python](/docs/connect-with-python-client/) · [Rust](/docs/rust-client-quickstart/)
{% /callout %}

## Prerequisites

- TerminusDB running locally ([Install guide →](/docs/install-terminusdb-as-a-docker-container))
- Node.js 20+ installed
- A terminal

If you haven't run the [curl quickstart](/docs/get-started/) yet, do that first — it takes 5 minutes and introduces the branch/diff/merge concepts this guide builds on.

{% callout type="warning" title="TerminusDB must be running" %}
This guide assumes TerminusDB is running on `localhost:6363`. If you have trouble connecting, see [Troubleshooting Connection Failures](/docs/troubleshooting-connection) and [Authentication Errors](/docs/troubleshooting-auth).
{% /callout %}

## Install the client

```bash
mkdir my-terminusdb-app && cd my-terminusdb-app
npm init -y
npm install terminusdb
```

Create a file called `index.ts`. You will build it up step by step below, or copy the [complete file](#the-complete-indexts) at the end.

## Connect to TerminusDB

```typescript test-example id="ts-quickstart-connect" file="examples/ts-quickstart-connect.example.ts"
```

**What you should see:** A server info object with version and storage details. If you see a connection error, check that Docker is running and TerminusDB is on port 6363.

## Create a database and insert a document

No schema needed — insert any JSON document with `raw_json: true` and give it a human-readable ID:

```typescript test-example id="ts-quickstart-create-db" fixture="docs-test" file="examples/ts-quickstart-create-db.example.ts"
```

**What you should see:** `["terminusdb:///data/jane"]`. The `@id` gives the document a stable, human-readable identifier you choose. Schema comes later when you are ready.

## Create a branch

Just like `git branch`, this creates an isolated copy of your data:

```typescript test-example id="ts-quickstart-branch" file="examples/ts-quickstart-branch.example.ts"
```

**What you should see:** `Now on branch: feature`

Everything you do now happens on `feature` — main is untouched.

## Edit the document on the branch

```typescript test-example id="ts-quickstart-edit" file="examples/ts-quickstart-edit.example.ts"
```

**What you should see:** The document retrieved, then confirmation of the update. The change only exists on `feature` — `main` still has the original email.

## See the diff

This is the moment. TerminusDB can show you exactly what changed between branches — structurally, field by field:

```typescript test-example id="ts-quickstart-diff" file="examples/ts-quickstart-diff.example.ts"
```

**What you should see:**

```json
[
  {
    "@id": "terminusdb:///data/jane",
    "email": { "@op": "SwapValue", "@before": "jane@example.com", "@after": "jane.smith@company.com" }
  }
]
```

{% callout type="note" %}
**What just happened?**
TerminusDB computed a **structural diff** — not a line-by-line text diff, but semantic operations (`SwapValue`) that know exactly which fields changed, what the old values were, and what the new values are. This patch can be applied, reversed, or composed with other patches.
{% /callout %}

## Merge the branch

Bring the changes back to `main`:

```typescript test-example id="ts-quickstart-merge" file="examples/ts-quickstart-merge.example.ts"
```

**What you should see:** `Merged feature into main`

## Verify the merge

Confirm the changes are now on `main`:

```typescript test-example id="ts-quickstart-verify" file="examples/ts-quickstart-verify.example.ts"
```

**What you should see:** Jane Smith with `jane.smith@company.com` — the changes from the feature branch are now on `main`.

## What just happened?

You just used your database like a git repository:

1. **Branched** — created an isolated workspace (`feature`) without copying data
2. **Edited** — made changes that only exist on that branch
3. **Diffed** — asked TerminusDB to show exactly what changed, field by field
4. **Merged** — brought changes back to `main` cleanly

This is the core workflow of TerminusDB. Every change is a commit. Every prior state is recoverable. Multiple branches can exist simultaneously — for testing, staging, feature development, or collaboration.

## The complete `index.ts`

Run with `npx tsx index.ts`:

```typescript
import TerminusClient from "terminusdb"

async function main() {
  // 1. Connect
  const client = new TerminusClient.WOQLClient("http://localhost:6363", {
    user: "admin",
    organization: "admin",
    key: "root",
  })

  // 2. Create database + insert document (no schema)
  await client.createDatabase("MyDatabase", {
    label: "My Database",
    comment: "TypeScript quickstart",
    schema: false,
  })

  const result = await client.addDocument(
    { "@id": "terminusdb:///data/jane", name: "Jane Smith", email: "jane@example.com", age: 30 },
    { raw_json: true },
  )
  console.log("Document created:", result)

  // 3. Branch
  await client.branch("feature")
  client.checkout("feature")
  console.log("Now on branch:", client.checkout())

  // 4. Edit on branch
  const person = await client.getDocument({ id: "terminusdb:///data/jane", as_list: true })

  await client.updateDocument(
    { "@id": "terminusdb:///data/jane", name: "Jane Smith", email: "jane.smith@company.com", age: 30 },
    { raw_json: true },
    "",
    "Updated Jane's email",
  )
  console.log("Document updated on feature branch")

  // 5. See the diff
  const diff = await client.getVersionDiff("main", "feature")
  console.log("\nChanges between main and feature:")
  console.log(JSON.stringify(diff, null, 2))

  // 6. Merge
  client.checkout("main")
  await client.rebase({
    rebase_from: "admin/MyDatabase/local/branch/feature",
    message: "Merge feature: updated Jane's email",
  })
  console.log("\nMerged feature into main")

  // 7. Verify
  const updated = await client.getDocument({ id: "terminusdb:///data/jane", as_list: true })
  console.log("\nPerson on main after merge:", updated)
}

main().catch(console.error)
```

{% details title="Running this again? Clean up first" %}
If you've already run this quickstart, delete the database before running again:

```typescript
await client.deleteDatabase("MyDatabase")
```
{% /details %}

## Troubleshooting

### Error: Connection refused

**You see:**
```
Error: connect ECONNREFUSED 127.0.0.1:6363
```

**Cause:** TerminusDB is not running, or another process is using port 6363.

**Fix:**
```bash
# Check if the container is running
docker ps | grep terminusdb

# If not running, start it
docker start terminusdb

# If it doesn't exist, create it
docker run -d --name terminusdb -p 127.0.0.1:6363:6363 \
  -v terminusdb_storage:/app/terminusdb/storage \
  terminusdb/terminusdb-server
```

[Troubleshooting Connection Failures →](/docs/troubleshooting-connection)

### Error: 401 Unauthorized

**You see:**
```
Error: 401 Unauthorized
```

**Cause:** Wrong username or password. The default local credentials are `admin` / `root`.

**Fix:**
```bash
# Verify credentials work
curl -u admin:root http://localhost:6363/api/info
```

If you set a custom password via `TERMINUSDB_ADMIN_PASS` when creating the container, use that value instead of `root`.

[Troubleshooting Authentication Errors →](/docs/troubleshooting-auth)

### Error: Database does not exist (404)

**You see:**
```
Error: 404 - {"@type":"api:ErrorResponse","api:error":{"@type":"api:UnresolvableAbsoluteDescriptor"}}
```

**Cause:** You are trying to read/write a database that has not been created yet, or the path is wrong.

**Fix:**
```bash
# List existing databases
curl -u admin:root http://localhost:6363/api/db

# Create the database if missing
curl -u admin:root -X POST "http://localhost:6363/api/db/admin/MyDatabase" \
  -H "Content-Type: application/json" \
  -d '{"label": "MyDatabase", "comment": "My database"}'
```

### Error: Cannot find module 'terminusdb'

**You see:**
```
Error [ERR_MODULE_NOT_FOUND]: Cannot find module 'terminusdb'
```
or
```
Cannot find module 'terminusdb' or its corresponding type declarations
```

**Cause:** The package is not installed, or you are running from the wrong directory.

**Fix:**
```bash
# Make sure you are in the project directory
cd my-terminusdb-app

# Install the package
npm install terminusdb
```

### Error: Cannot use import statement outside a module

**You see:**
```
SyntaxError: Cannot use import statement outside a module
```

**Cause:** You are running the file with `node index.ts` without ESM support.

**Fix:** Use `npx tsx` (recommended) which handles both TypeScript and ESM:
```bash
npx tsx index.ts
```

Alternatively, add `"type": "module"` to your `package.json` if using plain JavaScript.

### Error: Database already exists

**You see:**
```
Error: 400 - {"@type":"api:DbCreateErrorResponse","api:error":{"@type":"api:DatabaseAlreadyExists"}}
```

**Cause:** You ran this quickstart before and the database still exists.

**Fix:**
```typescript
await client.deleteDatabase("MyDatabase")
```

## Next steps

- [**Your First Schema**](/docs/schema-reference-guide/) — Add type safety to your documents with schema validation
- [**WOQL Query Guide**](/docs/woql-basics/) — Query your data with TerminusDB's pattern-matching query language
- [**JSON Diff and Patch**](/docs/json-diff-and-patch/) — Deep dive into structural diff operations
- [**API Reference**](/docs/javascript/) — Full TypeScript/JavaScript client API documentation
