---
tags:
  - explanation
  - knowledge-graph
  - version-control
  - diff-patch
  - curl
title: Knowledge Graph Diff
techArticle: true
nextjs:
  metadata:
    title: Knowledge Graph Diff — How to Diff Two Versions of a Graph
    description: How to diff two versions of a knowledge graph. See which facts changed, triple vs document diffs, and how to diff and patch a graph in TerminusDB.
    keywords: knowledge graph diff, graph diff, compare two versions of a graph, rdf diff, triple diff, document diff, structural diff, patch, json diff and patch, branch diff, commit diff, terminusdb
    openGraph:
      images: https://assets.terminusdb.com/docs/terminusdb-knowledge-graph-database-og.png
    alternates:
      canonical: https://terminusdb.com/docs/knowledge-graph-diff/
media: []
faq:
  - question: What is a knowledge graph diff?
    answer: It is a list of the facts that are different between two versions of a knowledge graph. It tells you what was added, what was removed and which values changed.
  - question: How do I compare two versions of a graph?
    answer: You need both versions, and a way to match the same node in each version. Then you compare the facts on each node. In TerminusDB, you send two branch names or two commit IDs to the diff endpoint.
  - question: What is the difference between a triple diff and a document diff?
    answer: A triple diff lists the triples that were added and removed. A document diff groups the changes by document and field, and shows the old and new value side by side. Both describe the same change at a different level.
  - question: Why are blank nodes a problem for graph diffs?
    answer: A blank node has no stable name. When you compare two files, the tool must find out which blank node in one file matches which blank node in the other. This can make a small change look like a large one.
  - question: Can I apply a diff to another branch?
    answer: Yes, in TerminusDB. The output of the diff endpoint is a patch that the patch endpoint accepts. If a document no longer matches what the patch expects, the call fails with a conflict and nothing is written.
---

A **knowledge graph diff** shows which facts changed between two versions of a graph: what was added, what was removed and which values are different. You use it to review a change before it goes live, to find out what a data load did, or to copy a change to another branch.

This page explains why graph diffs are harder than text diffs, and the two common ways to show them. Then it shows a short TerminusDB example that diffs two branches, diffs two commits and applies a diff as a patch.

{% callout type="note" title="Try it in TerminusDB" %}
TerminusDB has a diff endpoint for any two branches, commits or documents. For the full format, see [JSON Diff and Patch](/docs/json-diff-and-patch/).
{% /callout %}

## Why diffing a graph is harder than diffing text

A text diff compares lines. The line order is fixed, so the tool can line up the two files and mark the lines that are different.

A graph has no line order. You can write the same graph to a file in many orders and in many formats. If you export a graph two times and diff the files as text, you can get a large diff even if no fact changed.

There are more problems:

- **Identity.** To compare a node, you must find the same node in the other version. With stable IDs (IRIs) this is easy. With blank nodes it is not, because a blank node has no name that stays the same between exports.
- **Size.** A graph can have millions of facts. A diff tool must find the changed ones without a slow compare of everything.
- **Meaning.** A change of one value often shows up as one removed triple and one added triple. That is correct, but it is hard for a person to read.

So a good graph diff compares facts, not files. It matches nodes by ID and shows the change in a form that people can review.

## Triple diff vs document diff

There are two common ways to show a graph diff.

A **triple diff** lists the triples that were added and the triples that were removed. This is close to how many RDF stores keep data. The [RDF Patch](https://jena.apache.org/documentation/rdf-patch/) format from Apache Jena records changes this way, with add and delete rows. It is exact and simple to apply, but a value change is two rows that you must read together.

A **document diff** groups the triples by the thing they describe, such as one person or one product. It shows each changed field with its old and new value. This is easier to review, but it needs a schema or a rule that says which triples belong to which document.

| | Triple diff | Document diff |
|---|---|---|
| Unit of change | One triple | One field of one document |
| A value change shows as | One removal and one addition | One swap, with old and new value |
| Can be applied as a patch | Yes | Yes, with a patch tool |
| Easy for people to review | For small changes | Yes |
| Needs a schema | No | Usually (not for raw JSON) |

TerminusDB uses both levels. It stores each commit as a [delta layer](/docs/immutability-explanation/) of added and removed triples. The diff endpoint returns the change as a document diff: one entry per changed document, with an operation such as `SwapValue` for each changed field. See [Diff and Patch Operations](/docs/diff-and-patch-operations/) for the client options.

## Diff two branches in TerminusDB (example)

The example uses the HTTP API on a local TerminusDB at `localhost:6363` with the default `admin:root` login. It is a tiny graph: one person who works for one organization.

### 1. Set up a small graph {% step %}

Create a database, a schema and the first facts. Each write is a commit.

{% http-example method="POST" path="/api/db/admin/MyDatabase" %}
{"label": "MyDatabase", "comment": "Knowledge graph diff demo"}
{% http-expected %}
{"@type":"api:DbCreateResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

{% http-example method="POST" path="/api/document/admin/MyDatabase?graph_type=schema&author=alice@example.com&message=Add+schema" %}
[{"@type": "Class", "@id": "Organization", "name": "xsd:string"},
 {"@type": "Class", "@id": "Person", "name": "xsd:string", "role": "xsd:string", "worksFor": "Organization"}]
{% http-expected %}
["Organization", "Person"]
{% /http-expected %}
{% /http-example %}

{% http-example method="POST" path="/api/document/admin/MyDatabase?author=alice@example.com&message=Add+first+facts" %}
[{"@id": "Organization/acme", "@type": "Organization", "name": "Acme"},
 {"@id": "Person/alice", "@type": "Person", "name": "Alice", "role": "Engineer", "worksFor": "Organization/acme"}]
{% http-expected %}
["Organization/acme", "Person/alice"]
{% /http-expected %}
{% /http-example %}

### 2. Make a branch and change a fact {% step %}

{% http-example method="POST" path="/api/branch/admin/MyDatabase/local/branch/update" %}
{"origin": "admin/MyDatabase/local/branch/main"}
{% http-expected %}
{"@type":"api:BranchResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

{% http-example method="PUT" path="/api/document/admin/MyDatabase/local/branch/update?author=alice@example.com&message=New+role+for+Alice" %}
{"@id": "Person/alice", "@type": "Person", "name": "Alice", "role": "CTO", "worksFor": "Organization/acme"}
{% http-expected %}
["Person/alice"]
{% /http-expected %}
{% /http-example %}

### 3. Diff the branches {% step %}

Send the two branch names. The diff lists each changed document, and in it each changed field.

{% http-example method="POST" path="/api/diff/admin/MyDatabase" %}
{"before_data_version": "main", "after_data_version": "update"}
{% http-expected %}
[{"@id":"Person/alice","role":{"@after":"CTO","@before":"Engineer","@op":"SwapValue"}}]
{% /http-expected %}
{% /http-example %}

To diff only one document, add `"document_id"`. The [JSON Diff and Patch](/docs/json-diff-and-patch/) page lists all the request forms and options, such as `keep` and `unfold`.

## Diff two commits {% step %}

You can also diff any two commits, not only branch heads. Fetch the identifier of the commit just before the branch head — `start=1` skips the newest entry in the `update` branch log:

```bash publishes="commit_identifiers" publishColumn="identifier" publishLabel="message"
curl -s -u admin:root \
  "http://localhost:6363/api/log/admin/MyDatabase/local/branch/update?count=1&start=1"
```
{% http-expected %}
[ {"@id":"ValidCommit/25gy6q68b4rjjvp3ho5dd5v0oaat3zl","@type":"ValidCommit","author":"alice@example.com","identifier":"25gy6q68b4rjjvp3ho5dd5v0oaat3zl","instance":"layer_data:Layer_c5491b86be05f7d2b151c1a54ac74f72b0bc28be52a227de0deaf1dcabe4a485","message":"Add first facts","parent":"ValidCommit/hntjhfrd97tsogctcx8si1dwpzc5v63","schema":"layer_data:Layer_8ef99ffe6623c06abd7bb37d3dea93f9a07e910079b85576807cec918c26a8e3","timestamp":1793755629.152878,"user":"terminusdb://system/data/User/admin"} ]
{% /http-expected %}

Then diff that commit against the branch — the identifier flows in automatically. `after_data_version` takes a branch name just as well as a commit ID:

```bash slot="commit_identifiers" placeholder="OLDER_COMMIT"
curl -s -u admin:root -X POST "http://localhost:6363/api/diff/admin/MyDatabase" \
  -H "Content-Type: application/json" \
  -d '{"before_data_version": "OLDER_COMMIT", "after_data_version": "update"}'
```
{% http-expected %}
[ {"@id":"Person/alice","role": {"@after":"CTO","@before":"Engineer","@op":"SwapValue"}} ]
{% /http-expected %}

Comparing the "Add first facts" commit with the "New role for Alice" commit gives the same `SwapValue` for `Person/alice`. You can also pass two commit identifiers instead of a branch name. The [audit tutorial](/docs/audit-tutorial/) uses commit diffs to find out who changed a value and when.

## Turn a diff into a patch and apply it {% step %}

The diff output is also a patch. You can send it to the patch endpoint to make the same change on another branch. Here we apply it to `main`. This makes a new commit on `main` with its own author and message.

{% http-example method="POST" path="/api/patch/admin/MyDatabase/local/branch/main" %}
{"patch": [{"@id":"Person/alice","role":{"@after":"CTO","@before":"Engineer","@op":"SwapValue"}}],
 "author": "alice@example.com",
 "message": "Apply role change from update"}
{% http-expected %}
["Person/alice"]
{% /http-expected %}
{% /http-example %}

The call returns the compressed IDs of the documents it changed. If the `@before` value does not match the document on the branch, the call fails with HTTP 409 and nothing is written. You can also remove entries from the patch before you send it, or add `Insert` and `Delete` operations. See the [Patch Endpoint](/docs/patch-endpoint/) page for the formats.

To take all the changes from a branch at once, use a merge instead. The [merge how-to](/docs/merge-howto/) shows how, and how to handle conflicts.

When you are done, delete the demo database:

{% http-example method="DELETE" path="/api/db/admin/MyDatabase" /%}
{% http-expected %}
{"@type":"api:DbDeleteResponse","api:status":"api:success"}
{% /http-expected %}

## Using diffs for review and CI

A diff is JSON, so a program can read it. Some ways to use it:

- **Review a change.** Make the change on a branch, show the diff to a person, and merge only after they agree.
- **Check a data load.** After an import on a branch, diff it against `main`. If the diff touches documents or fields that it should not, stop before you merge.
- **Test in CI.** Run a job that loads data on a branch, diffs it, and fails if the diff is empty or larger than you expect.
- **Move part of a change.** Keep only some entries of a diff, and apply them as a patch to another branch.

## When this is not the right tool

A document diff may not be what you need if:

- Your data is plain RDF files with no schema, and you only want to compare two files. A triple-level tool, such as `rdfdiff` in [Apache Jena](https://jena.apache.org/documentation/tools/), may fit better.
- You only need to know *that* something changed, not *what*. Compare commit IDs instead.
- You want the imperative RFC 6902 JavaScript Object Notation (JSON) file patch diff format

## Next steps

- [JSON Diff and Patch](/docs/json-diff-and-patch/) — the full diff and patch format
- [Diff and Patch Operations](/docs/diff-and-patch-operations/) — diffs with the JavaScript client
- [Patch Endpoint](/docs/patch-endpoint/) — apply patches to documents in a branch
- [Merge How-To](/docs/merge-howto/) — merge branches and handle conflicts
- [Knowledge Graph Versioning](/docs/knowledge-graph-versioning/) — ways to keep the past versions of a graph
