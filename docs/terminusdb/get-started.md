---
tags:
  - tutorial
  - version-control
  - curl
  - beginner
  - self-hosted
title: Your First 10 Minutes with TerminusDB
nextjs:
  metadata:
    title: Get Started with TerminusDB in 10 Minutes
    description: Run TerminusDB with one Docker command, clone a sample dataset, then branch, diff and merge it. The full git-for-data workflow in 10 minutes, no sign-up.
    keywords: terminusdb, branch, clone, commit, diff, fetch, get, git for data
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/get-started/
---

TerminusDB is a database built for collaboration. In this quickstart you will clone a pre-populated dataset and immediately use the git-for-data workflow: branch, edit, diff, and merge. No schema to write, no data to invent.

Using an AI coding agent? It can do the setup for you — see [Set Up TerminusDB with an AI Agent](/docs/ai-agent-setup/).

## Step 1 — Start TerminusDB {% step %}

Pull and run TerminusDB in one command:

```bash
docker run --pull always -d -p 127.0.0.1:6363:6363 \
  -v terminusdb_storage:/app/terminusdb/storage \
  --name terminusdb terminusdb/terminusdb-server:v12
```

Wait a few seconds for the server to start, then confirm it is running:

{% http-example method="GET" path="/api/info" /%}

{% http-expected %}
{"@type":"api:InfoResponse","api:info":{"authority":"terminusdb://system/data/User/admin","edition":"community","storage":{"version":"2"},"terminusdb":{"version":"12.1.0-rc2"},"terminusdb_store":{"version":"0.19.8"}},"api:status":"api:success"}
{% /http-expected %}

You should see a JSON response with `"authority": "terminusdb://system/data/User/admin"` confirming the server is ready.

{% callout type="warning" title="Not working?" %}
If the command hangs or returns a connection error: Docker may not be running, port 6363 may already be in use, or a previous container named `terminusdb` may already exist. See [Troubleshooting Connection Failures](/docs/troubleshooting-connection) and [Authentication Errors](/docs/troubleshooting-auth) for solutions.
{% /callout %}

## Step 2 — Clone a ready-made dataset {% step %}

Skip the manual database creation — one POST pulls a complete Star Wars dataset from the public server. `Authorization-Remote` carries the public server's anonymous credentials (`public:public`):

{% http-example method="POST" path="/api/clone/admin/star-wars" headers="{\"Authorization-Remote\": \"Basic cHVibGljOnB1YmxpYw==\"}" %}
{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "star-wars", "comment": "Cloned star-wars from data.terminusdb.org"}
{% http-expected %}
{"@type":"api:CloneResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

You now have a fully populated Star Wars database ready to explore — characters, films, planets, starships, and species. Use it to try the full git-for-data workflow: what would have changed if Anakin Skywalker had turned to the Dark Side?

## Step 3 — Create a branch {% step %}

Create a `what-if` branch to experiment without affecting the original data:

{% http-example method="POST" path="/api/branch/admin/star-wars/local/branch/what-if" %}
{"origin": "admin/star-wars/local/branch/main"}
{% http-expected %}
{"@type":"api:BranchResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

You now have two branches: `main` (the cloned data, unchanged) and `what-if` (a copy, ready for edits).

## Step 4 — Fetch Anakin's document {% step %}

Pose a "what if" — what if Anakin Skywalker fell to the Dark Side? First, fetch his current document on the `what-if` branch:

{% http-example method="GET" path="/api/document/admin/star-wars/local/branch/what-if?id=Person/Anakin%2520Skywalker" /%}

{% http-expected %}
{"@id":"Person/Anakin%20Skywalker","@type":"Person","birth_year":"41.9BBY","eye_color":"blue","faction":"Jedi Order","films":["Film/Attack%20of%20the%20Clones","Film/Revenge%20of%20the%20Sith","Film/The%20Phantom%20Menace"],"gender":"male","hair_color":"blond","height":188,"homeworld":"Planet/Tatooine","mass":84,"name":"Anakin Skywalker","quote":"This is where the fun begins.","side":"Light Side","species":["Species/Human"]}
{% /http-expected %}

## Step 5 — Write the change {% step %}

PUT the document back with four fields changed to tell the Dark Side story: `eye_color` → yellow, `mass` → 120, `quote` → "I am your father.", `side` → Dark Side:

{% http-example method="PUT" path="/api/document/admin/star-wars/local/branch/what-if?author=admin&message=What+if+Anakin+turned+to+the+Dark+Side" %}
{"@type":"Person","@id":"Person/Anakin%20Skywalker","name":"Anakin Skywalker","eye_color":"yellow","mass":120,"quote":"I am your father.","side":"Dark Side","birth_year":"41.9BBY","faction":"Jedi Order","films":["Film/Attack%20of%20the%20Clones","Film/Revenge%20of%20the%20Sith","Film/The%20Phantom%20Menace"],"gender":"male","hair_color":"blond","height":188,"homeworld":"Planet/Tatooine","species":["Species/Human"]}
{% http-expected %}
["Person/Anakin%20Skywalker"]
{% /http-expected %}
{% /http-example %}

{% callout type="note" title="Why send the full document?" %}
TerminusDB's PUT replaces the entire document. If you omit optional fields, they become null — and will appear in the diff. By sending the full document with only target fields changed, the diff shows exactly what you intended. TerminusDB detects which fields actually changed, regardless of what you sent.
{% /callout %}

The edit lives only on `what-if`. On `main`, Anakin is still himself — blue eyes, Light Side, mass 84, original quote. The branch is your sandbox.

## Step 6 — Diff the branches {% step %}

This is the moment. See exactly what changed between `main` and `what-if`:

{% http-example method="POST" path="/api/diff/admin/star-wars" %}
{"before_data_version": "main", "after_data_version": "what-if"}
{% http-expected %}
[{"@id": "Person/Anakin%20Skywalker", "eye_color": {"@op": "SwapValue", "@before": "blue", "@after": "yellow"}, "mass": {"@op": "SwapValue", "@before": 84, "@after": 120}, "quote": {"@op": "SwapValue", "@before": "This is where the fun begins.", "@after": "I am your father."}, "side": {"@op": "SwapValue", "@before": "Light Side", "@after": "Dark Side"}}]
{% /http-expected %}
{% /http-example %}

{% callout type="note" %}
**What just happened?**
TerminusDB computed a **structural diff** between two branches. This is not a line-by-line text diff — it is a semantic patch that knows exactly which fields changed, what the old values were, and what the new values are. Each change is a `SwapValue` operation that can be applied, reversed, or composed with other patches.

Four fields changed: `eye_color`, `mass`, `quote`, and `side`. You sent the full document, but TerminusDB detected only the differences — no manual tracking, no event sourcing, no change-data-capture pipeline. The database does it natively.
{% /callout %}

## Step 7 — Merge the branch {% step %}

Apply the changes from `what-if` back to `main`:

{% http-example method="POST" path="/api/apply/admin/star-wars/local/branch/main" %}
{"before_commit": "main", "after_commit": "what-if", "commit_info": {"author": "admin", "message": "Merge what-if: Anakin turns to the Dark Side"}}
{% http-expected %}
{"@type":"api:ApplyResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

Done. The `main` branch now reflects the "what if" scenario — Anakin has turned to the Dark Side.

## Step 8 — Clean up {% step %}

Delete the cloned database when you are done experimenting:

{% http-example method="DELETE" path="/api/db/admin/star-wars" confirm="This will permanently delete the star-wars data product." /%}

{% http-expected %}
{"@type":"api:DbDeleteResponse","api:status":"api:success"}
{% /http-expected %}

To stop TerminusDB entirely:

```bash
docker stop terminusdb && docker rm terminusdb
```

Your data persists in the `terminusdb_storage` Docker volume. To remove it entirely: `docker volume rm terminusdb_storage`.

---

## What you just did

In 7 steps you used the full git-for-data workflow:

| Step | Git equivalent | TerminusDB API |
|------|---------------|----------------|
| Clone dataset | `git clone` | `POST /api/clone` |
| Create branch | `git branch` | `POST /api/branch` |
| Edit on branch | commit on branch | `PUT /api/document` |
| Diff branches | `git diff main..what-if` | `POST /api/diff/{org}/{db}` with `before_data_version` / `after_data_version` |
| Merge | `git merge --squash` | `POST /api/apply` with `before_commit` / `after_commit` |

## Next steps

- **[Build from scratch (15 min)](/docs/first-15-minutes/)** — Create a database, insert data, branch, diff, and merge manually — the full walkthrough
- [Explore a Real Dataset](/docs/explore-a-real-dataset/) — Clone a Star Wars database and query relationships, branch, and diff on rich data
- [TypeScript Client Quickstart](/docs/connect-with-the-javascript-client/) — Same workflow using the TypeScript SDK
- [Python Client Quickstart](/docs/connect-with-python-client/) — Same workflow using the Python SDK
- [Your First Schema](/docs/schema-reference-guide/) — Add type safety when you are ready
- [JSON Diff and Patch](/docs/json-diff-and-patch/) — Deep dive into structural diff operations
- [What is TerminusDB?](/docs/terminusdb-explanation/) — Understand the architecture behind what you just did

### Other ways to run and use TerminusDB

- [Install from source code](/docs/install-terminusdb-from-source-code/) — build the server yourself from [GitHub](https://github.com/terminusdb/terminusdb)
- [Docker container install](/docs/install-terminusdb-as-a-docker-container/) — more options for the Docker setup you used above
- [GraphQL reference](/docs/graphql-query-reference/) — query your data with the GraphQL schema that TerminusDB makes for you
- [CLI commands](/docs/terminusdb-cli-commands/) — work with databases from the command line
- [WOQL](/docs/woql-explanation/) — the datalog query language of TerminusDB
- [TerminusDB Cloud](/docs/cloud/) — a hosted server with a visual dashboard

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

## Frequently Asked Questions

### Q: Do I need to define a schema before inserting data?

**A:** No. Using `?raw_json=true` on the document endpoint lets you insert any valid JSON without a schema. TerminusDB stores it as-is. When you are ready for type safety, add a schema later — TerminusDB will validate all future inserts against it. See [Schema Reference](/docs/schema-reference-guide/) for details.

### Q: Can I use TerminusDB without Docker?

**A:** Yes. You can [build from source](/docs/install-terminusdb-from-source-code/) on Linux or macOS, or run on [Kubernetes](/docs/install-on-kubernetes/). Docker is the recommended path because it requires no dependencies and works on all platforms, but it is not the only option.

### Q: How do I reset and start fresh?

**A:** Stop and remove the container (`docker stop terminusdb && docker rm terminusdb`), then remove the storage volume (`docker volume rm terminusdb_storage`). Restart with the original `docker run` command. This gives you a clean instance with no databases.
