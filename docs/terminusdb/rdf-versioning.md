---
tags:
  - explanation
  - rdf
  - version-control
  - knowledge-graph
  - curl
title: RDF Versioning
techArticle: true
nextjs:
  metadata:
    title: RDF Versioning — How to Version RDF Data and Triples
    description: "RDF has no built-in versioning. Compare named graphs, RDF-star, changesets and RDF archives, then version RDF triples with commits in TerminusDB."
    keywords: rdf versioning, versioning rdf data, rdf version control, rdf triples, named graphs, rdf-star, reification, rdf patch, rdf archive, ostrich, triple store versioning, terminusdb
    openGraph:
      images: https://assets.terminusdb.com/docs/terminusdb-knowledge-graph-database-og.png
    alternates:
      canonical: https://terminusdb.com/docs/rdf-versioning/
media: []
faq:
  - question: Does RDF have built-in versioning?
    answer: No. An RDF graph is a set of triples, and the RDF standards do not say how to keep past versions of it. You add versioning with a modelling pattern, such as named graphs or timestamps on statements, or you use a store that versions the data for you.
  - question: How do I version RDF data?
    answer: There are four common ways. Keep one named graph per version, add time data to each statement with reification or RDF-star, store the changes as changesets, or use an RDF archive that keeps a snapshot plus deltas. A store with commits, such as TerminusDB, does this at the storage level.
  - question: What is the difference between named graphs and changesets for versioning?
    answer: Named graphs keep a full copy of the data for each version, so it is easy to query one version but it uses a lot of space. Changesets keep only the triples that were added and removed, so they are small, but you must apply them in order to see an old version.
  - question: Does TerminusDB support SPARQL?
    answer: No. TerminusDB stores RDF triples, but you query them with WOQL, a Datalog query language, or with GraphQL. If your tools need a SPARQL endpoint, TerminusDB is not a direct replacement.
  - question: Can I import and export Turtle files into TerminusDB?
    answer: Turtle is an export format. Both Turtle document import and export, and RDF/XML import and export are features of TerminusDB Enterprise. The open source edition reads and writes the same triples through the JSON document API and WOQL.
---

**RDF versioning** means that you keep the past versions of an RDF graph, so you can see what the data was at a point in time and what changed between two points. RDF itself has no built-in way to do this. Teams add versioning with modelling patterns, with change logs, or with a store that versions the triples for them.

This page explains the common approaches and what each one costs. Then it shows how TerminusDB versions triples with commits, with a short example that changes a fact and reads the old version.

{% callout type="note" title="Version control in TerminusDB" %}
For the full tour of commits, branches, merge, diff and time travel, see [Knowledge Graph Database Version Control](/docs/knowledge-graph-database-version-control/).
{% /callout %}

## Why RDF has no built-in versioning

In RDF, a graph is a set of triples: subject, predicate, object. A set has no order and no history. If you remove a triple, the graph does not remember that it was there. The RDF standards describe the data model and the file formats, such as Turtle and JSON-LD, but not how data changes over time.

SPARQL 1.1 Update lets you insert and delete triples, but it does not keep the old state either. So if you need to answer "what did we know last month?", you must add something.

A versioning approach should let you do three things:

- **Read an old version.** Get the graph as it was at a point in time.
- **See a change.** Get the triples that were added and removed between two versions.
- **Query across versions.** For example, find when a value changed.

In the research on RDF archives, these are often called version materialization, delta materialization and version queries. The [BEAR benchmark](https://aic.ai.wu.ac.at/qadlod/bear.html) uses these terms. Each approach below is good at some of them and weaker at others.

## Common approaches and their costs

### Named graphs, one per version

An RDF dataset can hold many named graphs. You can put each version of the data in its own named graph, for example `<urn:version:1>` and `<urn:version:2>`, and query the graph you need.

- **Good:** It is simple and works in any SPARQL store. Reading one version is fast.
- **Cost:** Each version is a full copy. A large graph with many versions uses a lot of space. To see what changed, you must compare two full graphs. You also cannot easily use named graphs for other things, such as keeping the source of the data.

### Reification and RDF-star timestamps

You can attach time data to each statement. With classic RDF reification, you describe the triple as an `rdf:Statement` with `rdf:subject`, `rdf:predicate` and `rdf:object`, and then add properties such as "valid from" and "valid to". RDF-star, which is now part of the W3C work on RDF 1.2, lets you add these properties to a triple more directly.

- **Good:** All versions live in one graph, and you can ask "when was this true?" in one query.
- **Cost:** Each fact needs more triples, so the data grows. Every query must filter on time, which makes queries longer and easier to get wrong. Tools must agree on the time properties you use.

### Changesets and deltas

You can store each change as a list of added and removed triples. The [RDF Patch](https://jena.apache.org/documentation/rdf-patch/) format from Apache Jena is one example. Some teams keep a log of SPARQL Update requests for the same reason.

- **Good:** Each change is small, and it is easy to see what one change did.
- **Cost:** To read an old version, you must start from a known state and apply the changes in order. With a long history, this gets slow unless you also keep snapshots.

### RDF archives

An RDF archive is a store built to keep many versions. The research describes three storage policies: independent copies of each version, change-based storage, and timestamp-based storage. Some systems, such as [OSTRICH](https://github.com/rdfostrich/ostrich), mix them. They keep a snapshot and then store the deltas, so they can answer all three kinds of version query.

- **Good:** It is designed for version queries, and it saves space.
- **Cost:** Most of these systems are research tools. They often focus on queries over the archive, and less on day-to-day editing, branching or merging.

| Approach | Read an old version | See a change | Space use |
|---|---|---|---|
| Named graph per version | Fast | Compare two copies | High |
| Reification or RDF-star | Filter by time | Filter by time | Medium to high |
| Changesets | Replay changes | Fast | Low |
| RDF archive | Fast | Fast | Low to medium |

## Versioning at the store level: commits on triples

There is one more approach: let the store version the triples for you. You do not change your data model. Each write makes a commit, and the store keeps the old states.

This is how TerminusDB works. It stores the data as RDF triples. Each commit adds a new [immutable layer](/docs/immutability-explanation/) with the triples that the commit added and the triples it removed. Older layers do not change. A commit also records an author, a message and a time.

From this, you get the three things from the list above:

- **Read an old version.** Use a commit ID in place of the branch name, and you read the data as it was at that commit.
- **See a change.** The [diff endpoint](/docs/json-diff-and-patch/) compares any two commits or branches. The [Knowledge Graph Diff](/docs/knowledge-graph-diff/) page explains this in more detail.
- **Query across versions.** Read the commit log, and query each commit you need.

You also get branches and merges, like in Git. You can load a change on a branch, check it, and then merge it. For a vendor-neutral view of these ideas, see [Knowledge Graph Versioning](/docs/knowledge-graph-versioning/).

The cost is that you version the whole database, not single statements. If your model needs "valid from" and "valid to" dates for real-world facts, you still model those dates as data. Commits record when the data changed in the database, not when a fact was true in the world.

## Example: load data, change it, query the old version

The example uses the HTTP API on a local TerminusDB at `localhost:6363` with the default `admin:root` login. It uses the JSON document API, which is in the open source edition. TerminusDB stores each document as triples.

You do not need to clone any data — step 1 creates a new database for the example. The interactive demo simulates every step without a server, or runs them live against your local TerminusDB; the demo's start screen shows the Docker command and a connect control. For other ways to install, see [Install with Docker](/docs/install-terminusdb-as-a-docker-container/).

{% callout type="note" title="Turtle and RDF/XML" %}
Turtle and RDF/XML import and export are in TerminusDB Enterprise. With Enterprise, you can post the same data as Turtle to the document endpoint. See [Turtle Support](/docs/enterprise-turtle/) and [JSON-LD Context Processing](/docs/enterprise-jsonld-context/).
{% /callout %}

### 1. Create the database {% step %}
{% http-example method="POST" path="/api/db/admin/RdfDemo" %}
{"label": "RdfDemo", "comment": "RDF versioning demo"}
{% http-expected %}
{"@type":"api:DbCreateResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

### 2. Add the schema {% step %}

The schema has two classes: `Organization` and `Person`. A person works for an organization.

{% http-example method="POST" path="/api/document/admin/RdfDemo?graph_type=schema&author=alice@example.com&message=Add+schema" %}
[{"@type": "Class", "@id": "Organization", "name": "xsd:string"},
 {"@type": "Class", "@id": "Person", "name": "xsd:string", "role": "xsd:string", "worksFor": "Organization"}]
{% http-expected %}
["Organization", "Person"]
{% /http-expected %}
{% /http-example %}

### 3. Add the first facts {% step %}

One organization, Acme, and one person, Alice, who works there as an engineer.

{% http-example method="POST" path="/api/document/admin/RdfDemo?author=alice@example.com&message=Add+first+facts" %}
[{"@id": "Organization/acme", "@type": "Organization", "name": "Acme"},
 {"@id": "Person/alice", "@type": "Person", "name": "Alice", "role": "Engineer", "worksFor": "Organization/acme"}]
{% http-expected %}
["Organization/acme", "Person/alice"]
{% /http-expected %}
{% /http-example %}

### 4. Look at the triples {% step %}
A WOQL query returns the triples for Alice. `Equals` sets `1_subject` to `Person/alice`. Then `Triple` finds every predicate and object for that subject. Each binding is one triple, with the variables in subject, predicate, object order.

{% http-example method="POST" path="/api/woql/admin/RdfDemo/local/branch/main" %}
{"query": {"@type": "And", "and": [
  {"@type": "Equals",
    "left": {"@type": "Value", "variable": "1_subject"},
    "right": {"@type": "Value", "node": "Person/alice"}},
  {"@type": "Triple",
    "subject": {"@type": "NodeValue", "variable": "1_subject"},
    "predicate": {"@type": "NodeValue", "variable": "2_predicate"},
    "object": {"@type": "Value", "variable": "3_object"}}]}}
{% http-expected %}
{"@type":"api:WoqlResponse","api:status":"api:success","bindings":[
 {"1_subject":"Person/alice","2_predicate":"rdf:type","3_object":"@schema:Person"},
 {"1_subject":"Person/alice","2_predicate":"@schema:name","3_object":{"@type":"xsd:string","@value":"Alice"}},
 {"1_subject":"Person/alice","2_predicate":"@schema:role","3_object":{"@type":"xsd:string","@value":"Engineer"}},
 {"1_subject":"Person/alice","2_predicate":"@schema:worksFor","3_object":"Organization/acme"}]}
{% /http-expected %}
{% /http-example %}

You should get one binding for each of `rdf:type`, `name`, `role` and `worksFor`. Node IDs such as `Person/alice` are short forms of full IRIs, such as `terminusdb:///data/Person/alice`. To query the same data with GraphQL and full IRIs, see [GraphQL with RDF URI/IRI](/docs/graphql-with-rdf-iri/).

### 5. Change a fact {% step %}
Alice gets a new role. This write is a new commit. In triple terms, it removes the triple with `"Engineer"` and adds a triple with `"CTO"`.

{% http-example method="PUT" path="/api/document/admin/RdfDemo?author=alice@example.com&message=New+role+for+Alice" %}
{"@id": "Person/alice", "@type": "Person", "name": "Alice", "role": "CTO", "worksFor": "Organization/acme"}
{% http-expected %}
["Person/alice"]
{% /http-expected %}
{% /http-example %}

### 6. Find the old commit {% step %}
The commit log lists each commit with an `identifier`, an `author`, a `message` and a `timestamp`, newest first. This query uses `start=1` to skip the newest entry — the role change — so it returns the "Add first facts" commit:

```bash publishes="rdf_demo_commit" publishColumn="identifier" publishLabel="message"
curl -u admin:root "http://localhost:6363/api/log/admin/RdfDemo/local/branch/main?start=1&count=1"
```

{% http-expected %}
[{"@type":"Commit","author":"alice@example.com","identifier":"c8f2a1b394d6e7f0a1b2c3d4e5f6a7b8c9d0e1f2","message":"Add first facts","timestamp":1720000000.0}]
{% /http-expected %}

Run it, and the `identifier` is filled into the examples below. To see the whole history, run the same query with `count=5` and no `start`.

### 7. Read the document at the old commit {% step %}
Put `local/commit/<identifier>` in the path in place of `local/branch/main`. The document API then returns Alice as she was at that commit:

```bash slot="rdf_demo_commit" placeholder="<identifier>"
curl -u admin:root "http://localhost:6363/api/document/admin/RdfDemo/local/commit/<identifier>?id=Person/alice"
```

{% http-expected %}
{"@id":"Person/alice","@type":"Person","name":"Alice","role":"Engineer","worksFor":"Organization/acme"}
{% /http-expected %}

The `role` is `"Engineer"`. On `main`, it is `"CTO"`.

### 8. Query the old triples {% step %}

A WOQL `Triple` query to the same commit path reads the old triple:

```bash slot="rdf_demo_commit" placeholder="<identifier>"
curl -u admin:root -X POST "http://localhost:6363/api/woql/admin/RdfDemo/local/commit/<identifier>" \
  -H "Content-Type: application/json" \
  -d '{"query": {"@type": "Triple", "subject": {"@type": "NodeValue", "node": "Person/alice"}, "predicate": {"@type": "NodeValue", "node": "@schema:role"}, "object": {"@type": "Value", "variable": "Role"}}}'
```

{% http-expected %}
{"@type":"api:WoqlResponse","api:status":"api:success","bindings":[{"Role":{"@type":"xsd:string","@value":"Engineer"}}]}
{% /http-expected %}

### 9. Diff the change {% step %}

A commit path is read-only, so you cannot change history by mistake. To see the change itself, diff the old commit against `main`:

```bash slot="rdf_demo_commit" placeholder="<identifier>"
curl -u admin:root -X POST "http://localhost:6363/api/diff/admin/RdfDemo" \
  -H "Content-Type: application/json" \
  -d '{"before_data_version": "<identifier>", "after_data_version": "main"}'
```

{% http-expected %}
[{"@id":"terminusdb:///data/Person/alice","role":{"@after":"CTO","@before":"Engineer","@op":"SwapValue"}}]
{% /http-expected %}

The result has a `SwapValue` for `role` on `Person/alice`, with `"Engineer"` before and `"CTO"` after. The [Time-Travel How-To](/docs/time-travel-howto/) has more ways to read and compare old versions.

### 10. Clean up {% step %}

When you are done, delete the demo database:

{% http-example method="DELETE" path="/api/db/admin/RdfDemo" /%}
{% http-expected %}
{"@type":"api:DbDeleteResponse","api:status":"api:success"}
{% /http-expected %}

## Where TerminusDB differs from a SPARQL triple store

TerminusDB stores RDF triples and uses IRIs in its triple store, but it has no SPARQL endpoint (yet). Some differences matter if you come from the semantic web:

- **No SPARQL.** TerminusDB does not support SPARQL yet. You query with GraphQL, [WOQL](/docs/woql-basics/), a Datalog query language, or with the document interface. If your tools need a SPARQL endpoint, TerminusDB is not a direct replacement (yet).
- **Schema first, closed world.** Writes are checked against a schema, and a write that does not match the schema fails. Most SPARQL stores accept any triple and use open-world semantics.
- **Documents on top of triples.** You usually write JSON or JSON-LD documents, and TerminusDB stores them as triples. You can also read and write single triples with WOQL.
- **Versioning is built in.** You do not need a named graph per version or timestamps on each statement. Each branch has its own history of commits.
- **RDF formats.** Turtle and RDF/XML import and export, and JSON-LD `@context` processing, are in TerminusDB Enterprise. The open source edition uses a JSON-LD-like document format.

For a longer comparison, see [TerminusDB Explanation](/docs/terminusdb-explanation/).

## When this is not the right tool

Store-level versioning may not fit if:

- You must keep a SPARQL endpoint for existing tools and queries.
- You need to version single statements with real-world validity times. Then model those times as data, for example with RDF-star or with "valid from" and "valid to" properties.
- You only need an archive of published dataset releases. Release files in a repository, or an RDF archive, may be enough.

## Next steps

- [Knowledge Graph Database Version Control](/docs/knowledge-graph-database-version-control/) — commits, branches, merge, diff and time travel in TerminusDB
- [Knowledge Graph Versioning](/docs/knowledge-graph-versioning/) — ways to keep the past versions of a graph
- [Knowledge Graph Diff](/docs/knowledge-graph-diff/) — compare two versions of a graph
- [Turtle Support](/docs/enterprise-turtle/) — import and export Turtle (Enterprise)
- [JSON-LD Context Processing](/docs/enterprise-jsonld-context/) — use your own JSON-LD context (Enterprise)
- [GraphQL with RDF URI/IRI](/docs/graphql-with-rdf-iri/) — query data with full IRIs
