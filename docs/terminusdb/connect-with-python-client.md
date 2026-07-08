---
tags:
  - python
  - documents
  - tutorial
  - beginner
title: Python Client — Quickstart
nextjs:
  metadata:
    title: Python Client — Quickstart
    description: Connect to TerminusDB from Python. Branch, edit, diff, and merge documents using the terminusdb-client Python library.
    keywords: terminusdb, branch, client, connect, connection, diff, document, document database
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/connect-with-python-client/
---

{% callout type="note" %}
This guide uses **Python**. Also available in [TypeScript](/docs/connect-with-the-javascript-client/) · [Rust](/docs/rust-client-quickstart/)
{% /callout %}

## Prerequisites

- TerminusDB running locally ([Install guide →](/docs/install-terminusdb-as-a-docker-container))
- Python 3.8+ installed
- A terminal

If you haven't run the [curl quickstart](/docs/get-started/) yet, do that first — it takes 5 minutes and introduces the branch/diff/merge concepts this guide builds on.

{% callout type="warning" title="TerminusDB must be running" %}
This guide assumes TerminusDB is running on `localhost:6363`. If you have trouble connecting, see [Troubleshooting Connection Failures](/docs/troubleshooting-connection) and [Authentication Errors](/docs/troubleshooting-auth).
{% /callout %}

## Install the client

```bash
pip install terminusdb-client
```

Create a file called `quickstart.py`. You will build it up step by step below.

## Connect to TerminusDB

```python test-example id="py-quickstart-connect" file="examples/py-quickstart-connect.example.py"
```

**What you should see:** A dict with server version and storage details. If you see a connection error, check that Docker is running and TerminusDB is on port 6363.

## Create a database and insert a document

No schema needed — insert any dict with `raw_json=True` and give it a human-readable ID:

```python test-example id="py-quickstart-create-db" fixture="docs-test" file="examples/py-quickstart-create-db.example.py"
```

**What you should see:** A list containing `"terminusdb:///data/jane"`. The `@id` gives the document a stable, human-readable identifier. Schema comes later when you are ready.

## Create a branch

Just like `git branch`, this creates an isolated copy of your data:

```python test-example id="py-quickstart-branch" file="examples/py-quickstart-branch.example.py"
```

**What you should see:** `Now on branch: feature`

Everything you do now happens on `feature` — main is untouched.

## Edit the document on the branch

```python test-example id="py-quickstart-edit" file="examples/py-quickstart-edit.example.py"
```

**What you should see:** The document retrieved, then confirmation of the update. The change only exists on `feature` — `main` still has the original email.

## See the diff

This is the moment. TerminusDB can show you exactly what changed between branches — structurally, field by field:

```python test-example id="py-quickstart-diff" file="examples/py-quickstart-diff.example.py"
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
TerminusDB computed a **structural diff** — not a line-by-line text diff, but a semantic operation (`SwapValue`) that knows exactly which field changed, what the old value was, and what the new value is. This patch can be applied, reversed, or composed with other patches.
{% /callout %}

## Merge the branch

Bring the changes back to `main`:

```python test-example id="py-quickstart-merge" file="examples/py-quickstart-merge.example.py"
```

**What you should see:** `Merged feature into main`

## Verify the merge

Confirm the changes are now on `main`:

```python test-example id="py-quickstart-verify" file="examples/py-quickstart-verify.example.py"
```

**What you should see:** Jane Smith with `jane.smith@company.com` — the changes from the feature branch are now on `main`.

## What just happened?

You just used your database like a git repository:

1. **Branched** — created an isolated workspace (`feature`) without copying data
2. **Edited** — made changes that only exist on that branch
3. **Diffed** — asked TerminusDB to show exactly what changed, field by field
4. **Merged** — brought changes back to `main` cleanly

## Troubleshooting

### Error: Connection refused

**You see:**
```
requests.exceptions.ConnectionError: ... Connection refused
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
Exception: 401
```
or
```
DatabaseError: Unauthorized
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
Exception: 404 - {"@type":"api:ErrorResponse","api:error":{"@type":"api:UnresolvableAbsoluteDescriptor"}}
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

### Error: ModuleNotFoundError

**You see:**
```
ModuleNotFoundError: No module named 'terminusdb_client'
```

**Cause:** The package is not installed, or you are using a different Python environment than the one you installed it in.

**Fix:**
```bash
# Install the package
pip install terminusdb-client

# If you have multiple Python versions, ensure you use the right pip
python3 -m pip install terminusdb-client

# Verify it is installed
python3 -c "import terminusdb_client; print(terminusdb_client.__version__)"
```

### Error: SSL certificate verify failed

**You see:**
```
requests.exceptions.SSLError: ... certificate verify failed
```

**Cause:** Your Python environment cannot verify the server's SSL certificate. This only affects cloud connections, not `localhost`.

**Fix:**
```bash
# Update certificates (macOS)
pip install --upgrade certifi
```

[Certificate Issues →](/docs/python-certificate-issues)

### Error: Database already exists

**You see:**
```
Exception: Error 400 - Database already exists
```

**Cause:** You ran this quickstart before and the database still exists.

**Fix:**
```python
client.delete_database("MyDatabase")
```

## Next steps

- [**Your First Schema**](/docs/schema-reference-guide/) — Add type safety to your documents with schema validation
- [**WOQL Query Guide**](/docs/woql-basics/) — Query your data with TerminusDB's pattern-matching query language
- [**JSON Diff and Patch**](/docs/json-diff-and-patch/) — Deep dive into structural diff operations
- [**API Reference**](/docs/python/) — Full Python client API documentation

{% details title="Running this again? Clean up first" %}
If you've already run this quickstart, delete the database before running again:

```python
client.delete_database("MyDatabase")
```
{% /details %}
