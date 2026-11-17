---
tags:
  - explanation
  - schema
  - version-control
  - curl
title: Ontology Versioning
techArticle: true
nextjs:
  metadata:
    title: Ontology Versioning — Evolve a Schema Without Breaking Data
    description: How to version an ontology or knowledge graph schema, breaking vs non-breaking changes, and how to change a class on a branch and migrate the data.
    keywords: ontology versioning, ontology version control, knowledge graph schema evolution, schema evolution, schema versioning, schema migration, schema weakening, breaking schema change, deprecate class, terminusdb
    openGraph:
      images: https://assets.terminusdb.com/docs/terminusdb-knowledge-graph-database-og.png
    alternates:
      canonical: https://terminusdb.com/docs/ontology-versioning/
media: []
faq:
  - question: What is ontology versioning?
    answer: It is a way to keep track of how an ontology or schema changes over time. Each version has a clear identity, and you can see what changed between two versions and which data was valid under which version. Sometimes called slowly moving dimensions.
  - question: What is the difference between a breaking and a non-breaking schema change?
    answer: A non-breaking change (a weakening) cannot make existing data invalid, for example a new class or a new optional property. A breaking change (a strengthening) can make existing data invalid, for example a new required property or a narrower type, so the data must change too.
  - question: Should the ontology and the data be versioned together?
    answer: Usually yes. If they are versioned apart, it is hard to know which data was valid under which schema. When both are in the same commit, an old commit shows the data together with the schema that was valid at that time.
  - question: How do I deprecate a class or property without breaking clients?
    answer: Do it in steps. First add the replacement and mark the old term as deprecated in its documentation. Then make the old property optional, so new data does not need it. Remove it only when no clients read it any more.
  - question: Can I test a schema change before it reaches production?
    answer: Yes. In TerminusDB you can make the change on a branch, run the migration as a dry run first, check the result, and only then move the change to main.
---

**Ontology versioning** is a way to keep track of how an ontology, or the schema of a knowledge graph, changes over time. You give each version a clear identity, you can see what changed, and you keep the data valid while the model moves on.

This page explains why ontologies change, which changes break data, and why the schema and the data are best versioned together. At the end, it shows how TerminusDB does it: you change a class on a branch, migrate the data, check it, and then bring the change to `main`.

{% callout type="note" title="Related reference" %}
For the full list of migration operations, see the [Schema Migration Reference](/docs/schema-migration-reference-guide/). For the idea behind safe changes, see [What is Schema Weakening](/docs/what-is-schema-weakening/). For git-like commands on data, see [Knowledge Graph Version Control](/docs/knowledge-graph-database-version-control/).
{% /callout %}

## Why ontologies change

An ontology is a model of a domain. The domain changes, and so does what people know about it. Some common reasons for a change:

- **New things to describe.** A product team adds a new kind of item, or a research group adds a new type of measurement.
- **Better names.** A class or property was named badly at the start, and people keep reading it the wrong way.
- **Tighter rules.** A property that was optional is now needed for every record, for example a status or an owner.
- **Merged models.** Two teams had their own vocabularies, and now they must share one.
- **Old terms.** Some classes or properties are no longer used and should go, but slowly, because clients still read them.

Early in a project the ontology changes fast, and there is often little data. Later there is a lot of data, and other systems read it. Then each change needs more care. This is where versioning helps most.

## Breaking vs non-breaking changes

The most useful question about a schema change is: **can it make data that is valid today invalid?**

- A **non-breaking** change, also called a **weakening**, cannot make existing data invalid. All the data stays valid, so it does not need to change. An example it to make a mandatory property optional.
- A **breaking** change, also called a **strengthening**, can make existing data invalid. The data must change with it, or the change must be refused. An example is to make an optional property mandatory, or add a mandatory property. 

| Change | Kind | What happens to existing data |
|---|---|---|
| Add a new class | Non-breaking | Nothing. There are no instances yet. |
| Add an optional property | Non-breaking | Nothing. Old records just do not have it. |
| Make a required property optional, or a set | Non-breaking | Nothing. Old values are still valid. |
| Change documentation or labels | Non-breaking | Nothing. Only the description changes. |
| Add a required property | Breaking | Each record needs a value, for example a default. |
| Rename a class or property | Breaking | Each record must use the new name. |
| Change a property to a narrower type | Breaking | Each value must be cast, or the change fails. |
| Delete a class or property | Breaking | The matching data is removed. |

Non-breaking changes are the easy path. If you can, add new terms and relax old rules instead of tightening them. Clients also help here: they should check that a field exists before they read it, and not depend on the exact shape of a record.

Breaking changes are sometimes the right choice. Then you need a **migration**: a description of the change that also says what to do with the data, for example which default to use for a new required property.

What can be done for breaking changes (strengthening) is to add a weakening operation first that enables the change first, then once all instances conform to the breaking change, apply the strengthened schema.

## Version the schema and the data together

Many teams keep the ontology in one place, such as a file in a git repository, and the data in another, such as a database. This works, but it leaves a gap. The file history tells you how the schema changed. It does not tell you which data was valid under which version, or which data was changed by a migration.

It is easier when the schema and the data are in the same version history:

- **One commit, one state.** A commit holds the schema and the data as they were at that point. When you read an old commit, you get the data together with the schema that was valid then.
- **Review before release.** You can try a schema change on a branch, look at the result, and only then move it to the main line.
- **The migration is part of the history.** You can see when the change was made, by whom, and what it did to the data.

For more on the general idea, see [Knowledge Graph Versioning](/docs/knowledge-graph-versioning/).

## How TerminusDB versions an ontology

In TerminusDB, the schema is a graph of its own, next to the instance data, and both are in the same commit. The schema is enforced: each write is checked against it, and a write that does not fit is refused. See the [Schema Reference](/docs/schema-reference-guide/) for the schema language.

You can change the schema in two ways:

- **Update the schema documents directly.** This works well for non-breaking changes. TerminusDB can infer some changes when they do not affect the instance data.
- **Use the migration endpoint**, `POST /api/migration/{path}`. You send an ordered list of operations, such as `CreateClass`, `MoveClass`, `ExpandEnum`, `CreateClassProperty`, `MoveClassProperty`, `UpcastClassProperty`, `CastClassProperty` or `DeleteClassProperty`. TerminusDB changes the schema and the data together in one commit, and stores the migration with that commit.

Because the migration is stored with the commit, it can be replayed. You can make a change on one branch, then ask your branch to migrate to the **target** branch's schema (using the `/api/migration` API). TerminusDB then runs the same operations on your branch's data, as for the target's branch schema using the migrations.

## Change a class on a branch, migrate, check, merge

This example uses the shared example database `tdb-example-mydb`. It has a `Task` class with a required `priority` property. You will:

1. Add a required `estimate` property, with a default for the tasks that exist (breaking).
2. Make `priority` optional, as a first step to deprecate it (non-breaking).

You do this on a branch, check the result, and then bring the same change to `main`.

{% prerequisites-clone /%}

The examples target TerminusDB 12.1 (`terminusdb/terminusdb-server:v12.1.0-rc1`) on `localhost:6363` with the default `admin:root` login.

### 1. Make a branch for the schema change

{% http-example method="POST" path="/api/branch/admin/tdb-example-mydb/local/branch/schema-v2" %}
{"origin": "admin/tdb-example-mydb/local/branch/main"}
{% http-expected %}
{"@type":"api:BranchResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

### 2. Try the migration as a dry run

With `"dry_run": true`, TerminusDB checks the operations against the schema and the data, but does not make a commit. The operations run in the order you give them.

{% http-example method="POST" path="/api/migration/admin/tdb-example-mydb/local/branch/schema-v2" %}
{
  "author": "alice@example.com",
  "message": "Add estimate to Task, make priority optional",
  "dry_run": true,
  "operations": [
    {
      "@type": "CreateClassProperty",
      "class": "Task",
      "property": "estimate",
      "type": "xsd:string",
      "default": "unknown"
    },
    {
      "@type": "UpcastClassProperty",
      "class": "Task",
      "property": "priority",
      "type": {"@type": "Optional", "@class": "xsd:string"}
    }
  ]
}
{% http-expected %}
{"@type":"api:MigrationResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

The response also says how many schema and instance operations the migration would make.

### 3. Run the migration on the branch

Send the same request without `dry_run`. This makes one commit on `schema-v2` with the new schema, the migrated data and the migration itself.

{% http-example method="POST" path="/api/migration/admin/tdb-example-mydb/local/branch/schema-v2" %}
{
  "author": "alice@example.com",
  "message": "Add estimate to Task, make priority optional",
  "operations": [
    {
      "@type": "CreateClassProperty",
      "class": "Task",
      "property": "estimate",
      "type": "xsd:string",
      "default": "unknown"
    },
    {
      "@type": "UpcastClassProperty",
      "class": "Task",
      "property": "priority",
      "type": {"@type": "Optional", "@class": "xsd:string"}
    }
  ]
}
{% http-expected %}
{"@type":"api:MigrationResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

### 4. Check the result on the branch

Each existing task now has the default `estimate`:

{% http-example method="GET" path="/api/document/admin/tdb-example-mydb/local/branch/schema-v2?id=Task/PROJ-001" %}
{% http-expected %}
{"@id":"Task/PROJ-001","@type":"Task","estimate":"unknown"}
{% /http-expected %}
{% /http-example %}

And the `Task` class in the branch schema has the new property, with `priority` now optional:

{% http-example method="GET" path="/api/document/admin/tdb-example-mydb/local/branch/schema-v2?graph_type=schema&id=Task" /%}

`main` has not changed. It still has the old schema, and its tasks do not have `estimate`.

### 5. Bring the change to main

To move the change to `main`, ask `main` to **target** the schema of `schema-v2`. TerminusDB finds the migrations made on `schema-v2` since the two branches split, and runs them on the data in `main`.

{% http-example method="POST" path="/api/migration/admin/tdb-example-mydb/local/branch/main" %}
{
  "author": "alice@example.com",
  "message": "Migrate main to the schema-v2 schema",
  "target": "admin/tdb-example-mydb/local/branch/schema-v2"
}
{% http-expected %}
{"@type":"api:MigrationResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

This replays the schema migration only. If the branch also has data commits, merge those with [apply](/docs/merge-howto/) as usual. If `main` has its own migrations that `schema-v2` does not have, the request fails, because the two histories no longer agree on the order of the changes.

Check `main`:

{% http-example method="GET" path="/api/document/admin/tdb-example-mydb/local/branch/main?id=Task/PROJ-001" %}
{% http-expected %}
{"@id":"Task/PROJ-001","@type":"Task","estimate":"unknown"}
{% /http-expected %}
{% /http-example %}

The old schema is still in the history. The [commit log](/docs/knowledge-graph-database-version-control/) shows the migration commit, and you can read any earlier commit to see the data with the schema it had then.

{% http-example-cleanup fixture="tdb-example-mydb" /%}

The [branch how-to](/docs/branch-howto/) shows how to list and delete branches when you are done.

## Deprecating classes and properties

Removing a term at once is a breaking change for every client that reads it. A slower path is safer. A common pattern:

1. **Add the replacement first.** Add the new class or property as a non-breaking change, or with a default as in the example above.
2. **Mark the old term as deprecated.** Say so in its documentation, for example with `ReplaceClassDocumentation`. This changes only the description, not the data.
3. **Relax the old term.** Make a required property optional with `UpcastClassProperty`, so new data does not need it. Old data stays valid.
4. **Move the data, if needed.** Copy or convert old values to the new term, in a normal commit or with a migration operation such as `MoveClassProperty`.
5. **Remove it last.** When no clients read the old term any more, remove it with `DeleteClassProperty` or `DeleteClass`. For a class, first drop the properties that point to it. This deletes the matching data in that commit, but older commits still have it.

Do each step on a branch, so you can check the result before it reaches `main`. Also tell the people who use the data, for example in the commit message or in a changelog.

## Next steps

- [Schema Migration Reference](/docs/schema-migration-reference-guide/) — every migration operation, with parameters and examples
- [What is Schema Weakening](/docs/what-is-schema-weakening/) — which changes are safe, and why
- [Schema Reference](/docs/schema-reference-guide/) — classes, properties, keys and types
- [How to Branch Your Database](/docs/branch-howto/) — create, list and delete branches
- [Knowledge Graph Versioning](/docs/knowledge-graph-versioning/) — approaches and trade-offs for versioning graph data
- [Knowledge Graph Version Control](/docs/knowledge-graph-database-version-control/) — commits, branch, merge, diff and time travel
