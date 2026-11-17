---
tags:
  - explanation
  - knowledge-graph
  - version-control
  - curl
title: Knowledge Graph Versioning
techArticle: true
nextjs:
  metadata:
    title: Knowledge Graph Versioning — Approaches and Trade-offs
    description: What knowledge graph versioning is, four ways to do it, and the snapshot vs delta trade-off. For diffs or git-like version control, see the linked guides.
    keywords: knowledge graph versioning, versioning knowledge graphs, versioned knowledge graph, graph versioning, snapshot vs delta, named graphs, commit graph, graph diff, branch and merge, time travel, terminusdb
    openGraph:
      images: https://assets.terminusdb.com/docs/terminusdb-knowledge-graph-database-og.png
    alternates:
      canonical: https://terminusdb.com/docs/knowledge-graph-versioning/
media: []
faq:
  - question: What is knowledge graph versioning?
    answer: It is a way to keep the earlier states of a knowledge graph when its facts change. You can then read an old version, see what changed between two versions, and go back if a change was wrong.
  - question: Is knowledge graph versioning the same as version control?
    answer: Not quite. Versioning means you keep past versions. Version control adds tools to work with them, such as branches, diffs, merges and a log of who changed what. Most people who ask for versioning end up needing some of these tools too.
  - question: Should I store full snapshots or only the changes?
    answer: Snapshots are simple and fast to read, but they use a lot of storage. Deltas use little storage, but an old version must be built from the changes. Many systems mix the two, and keep deltas with a combined copy from time to time.
  - question: Can I version only the schema or ontology?
    answer: You can, but it is often not enough. When the schema changes, the data usually has to change with it. If both are in the same version, you can read old data with the schema that was valid at that time.
  - question: Does versioning make queries slower?
    answer: It depends on how the history is stored. In TerminusDB, a read at an old commit is a normal read, and delta rollup runs automatically in standard setups to keep the layer stack small. The main cost is disk storage for any later granular data or structural changes.
  - question: How do I see what changed between two versions?
    answer: You compare them with a diff. A good graph diff tells you which facts were added, removed or changed. In TerminusDB, the diff endpoint compares two branches or commits and returns the change for each document, down to the field.
---

**Knowledge graph versioning** is a way to keep the earlier states of a knowledge graph when its facts change. With it, you can read the graph as it was, see what changed between two versions, and undo a bad change.

This page explains the common ways to do graph versioning and what each one costs. At the end, it shows how TerminusDB does it, with a short example you can run.

{% callout type="note" title="Looking for something else?" %}
- To compare two versions of a graph, read [Knowledge Graph Diff](/docs/knowledge-graph-diff/).
- To version control a knowledge graph in TerminusDB (commits, branches, merge, rollback), read [Knowledge Graph Version Control](/docs/knowledge-graph-database-version-control/).
{% /callout %}

## What is knowledge graph versioning?

A knowledge graph stores facts as links between things: "Alice works for Acme", "Acme is in Oslo". These facts change. People change jobs, a source sends a correction, a data load goes wrong.

In most databases, a change overwrites the old value. After that, nobody can say what the graph said yesterday. Versioning fixes this. Each change makes a new version, and the old versions stay.

This matters for some common jobs:

- **Audit.** Show what a fact said on a given day, and who changed it.
- **Reproducible results.** Run a report or train a model on a fixed version, where the numbers do not move.
- **Safe changes.** Try a large change away from the live graph, check it, then make it live.
- **Recovery.** Go back to a known good version after a bad import.

## Four ways to version a graph

There is no single standard for this. Teams use one of these four approaches, or a mix.

### 1. Full snapshots
You copy the whole graph at set times, for example every night. Each copy is one version.

This is easy to understand and easy to read. But storage grows with every copy, even if only one fact changed. You also lose the changes made between two snapshots.

### 2. Change logs (deltas)
You store a base version, then a log of each change: which facts were added and which were removed. To read an old version, you start from the base and apply the changes up to that point.

This uses much less storage. Reading an old version can be slow if the log is long, unless the system keeps extra copies or indexes to help.

### 3. Timestamps on facts or named graphs
You put the time in the data itself. Each fact, or each group of facts, gets a "valid from" and "valid to" value. In RDF, people often use a named graph per version, or extra statements about each triple.

This lets you ask "what was true at time T" in a normal query. But every query must filter on time, and the data model gets more complex. It is also easy to forget the filter and mix versions by mistake.

### 4. A commit graph
This is how Git works. Each change is a commit. A commit points to the commit before it, and it records who made the change, when and why. A branch is a name that points to the newest commit in one line of history.

Under the hood, a commit graph usually stores deltas. On top of that, it gives you branches, diffs and merges. It is the most complete approach, but you need a store that supports it.

| Approach | Storage | Read an old version | Branches and merge | Who changed what |
|---|---|---|---|---|
| Full snapshots | High | Fast | No | No |
| Change logs | Low | Can be slow | No | Only if you log it |
| Timestamps in the data | Medium | In the query | No | Only if you model it |
| Commit graph | Low | Fast, if the store is built for it | Yes | Yes, per commit |

## Snapshot vs delta: the trade-off

The main choice is between storing whole copies and storing changes.

- **Snapshots** cost storage, but every version is ready to read.
- **Deltas** save storage, but an old version must be built from the changes.

Most real systems mix the two. They store deltas, and from time to time they also store a combined copy. Then a read only needs to go back to the nearest combined copy, not to the start.

TerminusDB works this way. Each commit adds a [delta layer](/docs/immutability-explanation/) with the triples it added and removed. A [delta rollup](/docs/delta-rollup/) combines many layers into fewer cached layers. In standard setups the rollup runs automatically, so reads stay fast as history grows. The cost is a bit of disk storage.

## What a versioned knowledge graph should let you do

Keeping old versions is only the first step. To be useful, versioning should let you:

- **Query a past version** with the same query language you use for live data.
- **Versioned hybrid search** with both full text search (FTS) and vector search.
- **Diff** two versions, and see each fact and value that changed.
- **Branch**, so you can make changes away from the live graph.
- **Merge** a branch back, and get a clear report if two changes conflict.
- **Clone, push and pull** data products between networks across unidirectional links.
- **Roll back** to an earlier version when a change was wrong.
- **See who changed what**, with an author and a message on each change.

If your tool only does the first item, you have versioning but not version control. That can be enough. But if people or programs change the graph often, you will probably want the rest.

## How TerminusDB versions a knowledge graph

TerminusDB uses a commit graph. Every write is an [immutable commit](/docs/immutability-explanation/) with an author, a message and a timestamp. Branches are cheap, because a new branch shares its history with the branch it came from. You can query any branch or commit with the same API, compare two versions with a [structural diff](/docs/json-diff-and-patch/), and merge with a three-way merge.

The example below uses the HTTP API on a local TerminusDB at `localhost:6363` with the default `admin:root` login. It is a tiny graph: one person who works for one organization.

### 1. Create the database {% step %}
Create the database first. Each write is a commit.

{% http-example method="POST" path="/api/db/admin/MyDatabase" %}
{"label": "MyDatabase", "comment": "Knowledge graph versioning demo"}
{% http-expected %}
{"@type":"api:DbCreateResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

### 2. Add the schema {% step %}

The schema has two linked classes: `Organization` and `Person`. A person works for an organization.

{% http-example method="POST" path="/api/document/admin/MyDatabase?graph_type=schema&author=alice@example.com&message=Add+schema" %}
[{"@type": "Class", "@id": "Organization", "name": "xsd:string"},
 {"@type": "Class", "@id": "Person", "name": "xsd:string", "role": "xsd:string", "worksFor": "Organization"}]
{% http-expected %}
["Organization", "Person"]
{% /http-expected %}
{% /http-example %}

### 3. Add the first facts {% step %}

One organization, Acme, and one person, Alice, who works there as an engineer.

{% http-example method="POST" path="/api/document/admin/MyDatabase?author=alice@example.com&message=Add+first+facts" %}
[{"@id": "Organization/acme", "@type": "Organization", "name": "Acme"},
 {"@id": "Person/alice", "@type": "Person", "name": "Alice", "role": "Engineer", "worksFor": "Organization/acme"}]
{% http-expected %}
["Organization/acme", "Person/alice"]
{% /http-expected %}
{% /http-example %}

### 4. Make a branch {% step %}
The branch starts with the same data as `main`. It shares history with `main`, so the data is not copied.

{% http-example method="POST" path="/api/branch/admin/MyDatabase/local/branch/update" %}
{"origin": "admin/MyDatabase/local/branch/main"}
{% http-expected %}
{"@type":"api:BranchResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

### 5. Change a fact on the branch {% step %}
Alice gets a new role. Only the `update` branch changes. `main` still says "Engineer".

{% http-example method="PUT" path="/api/document/admin/MyDatabase/local/branch/update?author=alice@example.com&message=New+role+for+Alice" %}
{"@id": "Person/alice", "@type": "Person", "name": "Alice", "role": "CTO", "worksFor": "Organization/acme"}
{% http-expected %}
["Person/alice"]
{% /http-expected %}
{% /http-example %}

### 6. Diff the two versions {% step %}
The diff shows the one field that changed, with its old and new value.

{% http-example method="POST" path="/api/diff/admin/MyDatabase" %}
{"before_data_version": "main", "after_data_version": "update"}
{% http-expected %}
[{"@id":"Person/alice","role":{"@after":"CTO","@before":"Engineer","@op":"SwapValue"}}]
{% /http-expected %}
{% /http-example %}

### 7. Merge into main {% step %}
Merge the branch into `main`. Here `main` has not changed since the branch was made, so `main` is the right merge base. If both branches had changed the same field, the merge would return HTTP 409 with a conflict report.

{% http-example method="POST" path="/api/apply/admin/MyDatabase/local/branch/main" %}
{"before_commit": "main", "after_commit": "update", "commit_info": {"author": "alice@example.com", "message": "Merge update into main"}}
{% http-expected %}
{"@type":"api:ApplyResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

### 8. Find the old commit {% step %}

`main` now says "CTO". The old version is still there. The commit log lists each commit with an `identifier`, an author and a message, newest first. This query uses `start=1` to skip the merge commit, so it returns the "Add first facts" commit:

```bash publishes="kg_commit" publishColumn="identifier" publishLabel="message"
curl -u admin:root "http://localhost:6363/api/log/admin/MyDatabase/local/branch/main?start=1&count=1"
```

{% http-expected %}
[{"@type":"Commit","author":"alice@example.com","identifier":"b7e3d9f1a2c4567890abcdef1234567890abcd34","message":"Add first facts","timestamp":1720000000.0}]
{% /http-expected %}

### 9. Read the graph at the old commit {% step %}

To read the graph as it was before the change, put a commit `identifier` in the path in place of `branch/main`. Run the query above and the `identifier` is filled in for you:

```bash slot="kg_commit" placeholder="<identifier>"
curl -u admin:root \
  "http://localhost:6363/api/document/admin/MyDatabase/local/commit/<identifier>?id=Person/alice"
```

{% http-expected %}
{"@id":"Person/alice","@type":"Person","name":"Alice","role":"Engineer","worksFor":"Organization/acme"}
{% /http-expected %}

This is a normal read. TerminusDB does not rebuild the past from a log. See [time travel queries](/docs/graph-database-with-time-travel-queries/) for more.

### 10. Clean up {% step %}

When you are done, delete the demo database:

{% http-example method="DELETE" path="/api/db/admin/MyDatabase" /%}
{% http-expected %}
{"@type":"api:DbDeleteResponse","api:status":"api:success"}
{% /http-expected %}

The [time travel how-to](/docs/time-travel-howto/) and the [merge how-to](/docs/merge-howto/) show the same ideas with the TypeScript and Python clients.

## When you don't need versioning

Versioning is not always worth it. You may not need it if:

- The data is a cache or a copy that you can rebuild from the source at any time.
- You only append new events and never change old ones. A log or time-series database may fit better.
- Nobody needs to know what the data said in the past, and nobody reviews changes anyway.

Also keep in mind that history uses disk storage. And if a branch has many small steps that nobody needs to see, you can [squash](/docs/squash-projects/) them into one commit before you merge.

## Next steps

- [Knowledge Graph Version Control](/docs/knowledge-graph-database-version-control/) — clone, commits, branch, merge, diff, push, pull and time travel, one by one
- [Time Travel Queries](/docs/graph-database-with-time-travel-queries/) — read the graph at any commit
- [Immutability](/docs/immutability-explanation/) — why TerminusDB never changes data in place
- [Knowledge Graph Diff](/docs/knowledge-graph-diff/) — compare two versions of a graph, and apply the change as a patch
- [JSON Diff and Patch](/docs/json-diff-and-patch/) — how structural diffs work
- [Delta Rollup](/docs/delta-rollup/) — how layers are compacted

