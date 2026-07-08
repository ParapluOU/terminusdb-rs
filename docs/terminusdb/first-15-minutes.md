---
title: Your First 15 Minutes — Build from Scratch
nextjs:
  metadata:
    title: Your First 15 Minutes — Build from Scratch with TerminusDB
    description: Build a complete git-for-data workflow from scratch. Create a database, insert a document, branch, edit, diff branches, and merge — step by step with curl.
    keywords: terminusdb, branch, curl, diff, document, javascript, merge, terminusdb javascript client
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/first-15-minutes/
tags:
  - typescript
  - tutorial
  - beginner
---

Build the entire git-for-data workflow from scratch. You will create a database, insert a document, branch, edit on the branch, diff the two branches, and merge — all with curl. No schema, no SDK.

{% prerequisites /%}

{% callout type="note" %}
**Want the 10-minute path instead?**
If you want to skip the manual setup and start exploring immediately, use the [clone-based quickstart](/docs/get-started/) — it gives you a pre-populated database in one click.
{% /callout %}

## Step 1 — Start TerminusDB

Pull and run TerminusDB in one command:

```bash
docker run --pull always -d -p 127.0.0.1:6363:6363 \
  -v terminusdb_storage:/app/terminusdb/storage \
  --name terminusdb terminusdb/terminusdb-server:v12
```

Wait a few seconds for the server to start, then confirm it is running:

```bash
curl -s -u admin:root http://localhost:6363/api/info
```

You should see a JSON response with `"authority": "admin"` confirming the server is ready.

{% callout type="warning" title="Not working?" %}
If the command hangs or returns a connection error: Docker may not be running, port 6363 may already be in use, or a previous container named `terminusdb` may already exist. See [Troubleshooting Connection Failures](/docs/troubleshooting-connection) and [Authentication Errors](/docs/troubleshooting-auth) for solutions.
{% /callout %}

## Step 2 — Create a database

{% http-example method="POST" path="/api/db/admin/MyDatabase" fixture="docs-test" %}
{"label": "MyDatabase", "comment": "My first TerminusDB database"}
{% http-expected %}
{"@type":"api:DbCreateResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

## Step 3 — Define a schema and insert a document

TerminusDB is primarily a typed database, with some vanilla JSON handling too. Define a simple `Person` schema class, then insert a document:

{% http-example method="POST" path="/api/document/admin/MyDatabase?author=admin&message=Add+schema&graph_type=schema" %}
{"@type": "Class", "@id": "Person", "name": "xsd:string", "email": "xsd:string", "age": "xsd:integer"}
{% /http-example %}

Expected output:
```json
["Person"]
```

Now insert a person:

{% http-example method="POST" path="/api/document/admin/MyDatabase?author=admin&message=Add+Jane" %}
{"@type": "Person", "@id": "Person/jane", "name": "Jane Smith", "email": "jane@example.com", "age": 30}
{% /http-example %}

Expected output:
```json
["terminusdb:///data/Person/jane"]
```

## Step 4 — Create a branch

{% http-example method="POST" path="/api/branch/admin/MyDatabase/local/branch/feature" %}
{"origin": "admin/MyDatabase/local/branch/main"}
{% /http-example %}

Expected output:
```json
{"@type":"api:BranchResponse","api:status":"api:success"}
```

You now have two branches: `main` (unchanged) and `feature` (a copy, ready for edits).

## Step 5 — Edit the document on the branch

Update Jane's email on the `feature` branch only:

{% http-example method="PUT" path="/api/document/admin/MyDatabase/local/branch/feature?author=admin&message=Update+Jane+email" %}
{"@type": "Person", "@id": "Person/jane", "name": "Jane Smith", "email": "jane.smith@company.com", "age": 30}
{% /http-example %}

Expected output:
```json
["terminusdb:///data/Person/jane"]
```

The edit lives only on `feature`. The `main` branch still has the original email.

## Step 6 — Diff the branches

This is the moment. TerminusDB computes a structural, semantic diff between any two branches:

{% http-example method="POST" path="/api/diff/admin/MyDatabase" %}
{"before_data_version": "main", "after_data_version": "feature"}
{% http-expected %}
[{"@id": "Person/jane", "email": {"@op": "SwapValue", "@before": "jane@example.com", "@after": "jane.smith@company.com"}}]
{% /http-expected %}
{% /http-example %}

{% callout type="note" %}
**What just happened?**
TerminusDB computed a **structural diff** between two branches — not a line-by-line text diff, but a semantic operation (`SwapValue`) that knows exactly which field changed, what the old value was, and what the new value is. This patch can be applied, reversed, or composed with other patches.

The URL path (`/api/diff/admin/MyDatabase`) identifies the database; the body specifies which branches to compare. You can also diff a single document by adding `"document_id": "Person/jane"` to the request body.
{% /callout %}

## Step 7 — Merge the branch

Apply the changes from `feature` onto `main`:

{% http-example method="POST" path="/api/apply/admin/MyDatabase/local/branch/main" %}
{"before_commit": "main", "after_commit": "feature", "commit_info": {"author": "admin", "message": "Merge feature into main"}}
{% /http-example %}

Expected output:
```json
{"@type":"api:ApplyResponse","api:status":"api:success"}
```

Verify the merge — `main` now has the updated email:

{% http-example method="GET" path="/api/document/admin/MyDatabase?id=Person/jane" /%}

```json
{"@id":"Person/jane","@type":"Person","age":30,"email":"jane.smith@company.com","name":"Jane Smith"}
```

---

## What you just built

In 7 commands you used the full git-for-data workflow:

| Step | Git equivalent | TerminusDB API |
|------|---------------|----------------|
| Create database | `git init` | `POST /api/db` |
| Define schema + insert | first commit | `POST /api/document` with `graph_type=schema` |
| Create branch | `git branch` | `POST /api/branch` |
| Edit on branch | commit on branch | `PUT /api/document` |
| Diff branches | `git diff main..feature` | `POST /api/diff/{org}/{db}` with `before_data_version` / `after_data_version` |
| Merge | `git merge --squash` | `POST /api/apply` with `before_commit` / `after_commit` |

## Troubleshooting

### Error: Connection refused

**You see:**
```text
curl: (7) Failed to connect to localhost port 6363 after 0 ms: Connection refused
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
```json
{"@type":"api:ErrorResponse","api:error":{"@type":"api:IncorrectAuthenticationError"},"api:status":"api:failure"}
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
```json
{"@type":"api:ErrorResponse","api:error":{"@type":"api:UnresolvableAbsoluteDescriptor"},"api:status":"api:failure"}
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

Common path mistakes: using `MyDatabase` instead of `admin/MyDatabase` in the URL, or forgetting that the database was deleted in a previous tutorial run.

### Error: "jq: command not found"

**You see:**
```text
bash: jq: command not found
```

**Cause:** The `jq` JSON formatter is not installed. All `| jq` commands in this tutorial are optional — they format the output for readability.

**Fix:** Either install jq or remove `| jq` from the commands:
```bash
# macOS
brew install jq

# Ubuntu/Debian
sudo apt-get install jq

# Or just skip it — the commands work without jq, output is just unformatted
```

### Error: Database already exists

**You see:**
```json
{"@type":"api:DbCreateErrorResponse","api:error":{"@type":"api:DatabaseAlreadyExists"},"api:status":"api:failure"}
```

**Cause:** You ran this quickstart before and the database still exists.

**Fix:**
```bash
# Delete the old database, then re-run from Step 2
curl -u admin:root -X DELETE "http://localhost:6363/api/db/admin/MyDatabase"
```

## Next steps

- [Explore a Real Dataset](/docs/explore-a-real-dataset/) — clone a Star Wars database and query relationships, branch, and diff on rich data
- [TypeScript Client Quickstart](/docs/connect-with-the-javascript-client/) — same workflow using the TypeScript SDK
- [Python Client Quickstart](/docs/connect-with-python-client/) — same workflow using the Python SDK
- [Your First Schema](/docs/schema-reference-guide/) — add type safety when you are ready
- [JSON Diff and Patch](/docs/json-diff-and-patch/) — deep dive into structural diff operations
- [What is TerminusDB?](/docs/terminusdb-explanation/) — understand the architecture behind what you just did
- [WOQL Basics](/docs/woql-basics/) — query your data with the Datalog-based query language

## Frequently Asked Questions

### Q: Do I need to define a schema before inserting data?

**A:** No. Using `?raw_json=true` on the document endpoint lets you insert any valid JSON without a schema. TerminusDB stores it as-is. When you are ready for type safety, add a schema later — TerminusDB will validate all future inserts against it. See [Schema Reference](/docs/schema-reference-guide/) for details.

### Q: Can I use TerminusDB without Docker?

**A:** Yes. You can [build from source](/docs/install-terminusdb-from-source-code/) on Linux or macOS, or run on [Kubernetes](/docs/install-on-kubernetes/). Docker is the recommended path because it requires no dependencies and works on all platforms, but it is not the only option.

### Q: What happens if Docker is not running or port 6363 is in use?

**A:** You will see a "connection refused" error from curl. Check that Docker Desktop is running (`docker info`), that no other process is using port 6363 (`lsof -i :6363`), and that no previous container named `terminusdb` exists (`docker rm terminusdb`). See [Troubleshooting Connection Failures](/docs/troubleshooting-connection/) for a complete guide.

### Q: How is TerminusDB diff different from git diff?

**A:** Git diff compares text files line by line. TerminusDB diff compares structured JSON documents field by field, producing semantic operations like `SwapValue`, `InsertList`, and `PatchList`. The result is a machine-readable patch that can be applied, reversed, or composed — not a human-readable text diff. See [JSON Diff and Patch](/docs/json-diff-and-patch/) for the full specification.

### Q: Can I use this quickstart on Windows?

**A:** Yes. Install Docker Desktop for Windows and run the same commands in PowerShell or WSL2. All curl commands work identically. If you prefer a graphical interface, the documentation site's "Run" buttons execute each command in your browser against your local TerminusDB instance — no terminal needed.

### Q: What is the default username and password?

**A:** The default credentials are `admin` / `root`. These are set when the Docker container first initialises. To change them, set the `TERMINUSDB_ADMIN_PASS` environment variable when starting the container: `docker run -e TERMINUSDB_ADMIN_PASS=mysecretpass ...`. See [Authentication Errors](/docs/troubleshooting-auth/) for troubleshooting.

### Q: How do I reset and start fresh?

**A:** Stop and remove the container (`docker stop terminusdb && docker rm terminusdb`), then remove the storage volume (`docker volume rm terminusdb_storage`). Restart with the original `docker run` command. This gives you a clean instance with no databases.

### Q: Can I connect to this database from my application code?

**A:** Yes. After completing this quickstart, try the [TypeScript SDK Quickstart](/docs/connect-with-the-javascript-client/) or [Python SDK Quickstart](/docs/connect-with-python-client/) — they use the same local instance you just created. Any HTTP client that can send JSON to `localhost:6363` works too.

## Clean up

{% callout type="warning" %}
This deletes the **MyDatabase** data product permanently. Only run this when you are finished experimenting.
{% /callout %}

{% http-example method="DELETE" path="/api/db/admin/MyDatabase" confirm="This will permanently delete the MyDatabase data product." /%}

Your TerminusDB instance is still running. To stop it entirely:

```bash
docker stop terminusdb && docker rm terminusdb
```

Your data persists in the `terminusdb_storage` Docker volume. To remove it entirely: `docker volume rm terminusdb_storage`.
