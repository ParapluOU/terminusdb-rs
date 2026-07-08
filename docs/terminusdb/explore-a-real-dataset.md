---
tags:
  - woql
  - tutorial
  - beginner
title: Explore a Real Dataset — Star Wars Tutorial
nextjs:
  metadata:
    title: Explore a Real Dataset — Star Wars Tutorial
    description: Clone, query, branch, and diff a Star Wars database in 15 minutes. Learn TerminusDB's git-for-data workflow on real data.
    keywords: terminusdb tutorial, clone database, star wars dataset, branch diff merge, git for data example
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/explore-a-real-dataset/
media: []
lastUpdated: "2026-04-30"
---

Clone a pre-populated Star Wars database to your local TerminusDB instance and explore it with queries, branches, and diffs — in 15 minutes. This tutorial focuses on **querying relationships** across documents — something the [10 Minute quickstart](/docs/get-started/) does not cover.

{% callout type="note" %}
**Prerequisites**
- **TerminusDB running on localhost:6363.** Verify: `curl -s -u admin:root http://localhost:6363/api/info` should return JSON containing `"authority": "admin"`. If you get "connection refused", [start TerminusDB first](/docs/get-started/#step-1).
- **Completed the [First 10 Minutes quickstart](/docs/get-started/)** — you should be comfortable with branches and diffs. This tutorial builds on those concepts with a richer dataset.
{% /callout %}

{% callout type="note" title="How this differs from other tutorials" %}
- **[Your First 10 Minutes](/docs/get-started/)** teaches the branch/diff/merge workflow on a cloned dataset.
- **[Your First 15 Minutes](/docs/first-15-minutes/)** builds a database from scratch — schema, insert, branch, diff, merge.
- **This tutorial** adds WOQL queries that traverse relationships between documents (films → characters, characters → planets) — showing the graph query power that distinguishes TerminusDB from a plain document store.
{% /callout %}

## What you will build

You will clone a complete Star Wars database (characters, films, planets, starships, species) from a public server, run queries that traverse document relationships, create a speculative branch ("what if Anakin turned to the Dark Side?"), and see a field-level structural diff of your changes.

## Step 1 — Clone the Star Wars database

Pull the entire Star Wars dataset from the public templates server to your local instance:

{% quickstart-clone remoteUrl="https://data.terminusdb.org/public/star-wars" localPath="star-wars" label="Clone Star Wars Database" description="Get the Star Wars dataset on your local TerminusDB — ready to branch and diff." /%}

You just pulled a complete Star Wars database — characters, films, planets, starships — from a public TerminusDB server to your local instance. No account needed, no sign-up, no credentials. The data is now yours to query, branch, and modify.

## Step 2 — Explore what you have

List the document types defined in the schema:

{% http-example method="GET" path="/api/document/admin/star-wars/local/branch/main?graph_type=schema&as_list=true&skip=1" /%}

The response lists 4 document types — `Film`, `Person`, `Planet`, and `Species`.

List the first few characters:

{% http-example method="GET" path="/api/document/admin/star-wars/local/branch/main?type=Person&count=5&as_list=true" /%}

You see 5 Person documents (limited by `count=5`). The database has 20 characters total, plus films, planets, and species. All of Star Wars, versioned and queryable. Let's ask it some questions.

## Step 3 — Query the data

Which characters appear in "A New Hope" (Episode IV)?

In a relational database, you would write a JOIN across a junction table: `SELECT c.name FROM characters c JOIN film_characters fc ON ... JOIN films f ON ... WHERE f.title = 'A New Hope'`. In TerminusDB, documents link directly to other documents — a Film has a `characters` property that points to Person documents. You traverse the link, not a junction table:

{% http-example method="POST" path="/api/woql/admin/star-wars/local/branch/main" %}
{% http-woql %}
import TerminusClient from "@terminusdb/terminusdb-client";

const client = new TerminusClient.WOQLClient("http://localhost:6363", {
  user: "admin",
  organization: "admin",
  key: "root",
});
client.db("star-wars");

const WOQL = TerminusClient.WOQL;
const query = WOQL.and(
  WOQL.triple("v:Film", "title", WOQL.string("A New Hope")),
  WOQL.triple("v:Film", "characters", "v:Character"),
  WOQL.triple("v:Character", "name", "v:CharacterName")
);

const result = await client.query(query);
console.log(result.bindings.map((b) => b["CharacterName"]["@value"]));
{% /http-woql %}
```json
{"query": {"@type": "And", "and": [{"@type": "Triple", "subject": {"@type": "NodeValue", "variable": "Film"}, "predicate": {"@type": "NodeValue", "node": "title"}, "object": {"@type": "DataValue", "data": "A New Hope"}}, {"@type": "Triple", "subject": {"@type": "NodeValue", "variable": "Film"}, "predicate": {"@type": "NodeValue", "node": "characters"}, "object": {"@type": "NodeValue", "variable": "Character"}}, {"@type": "Triple", "subject": {"@type": "NodeValue", "variable": "Character"}, "predicate": {"@type": "NodeValue", "node": "name"}, "object": {"@type": "DataValue", "variable": "CharacterName"}}]}}
```
{% /http-example %}

Expected output includes character names: Luke Skywalker, Leia Organa, Han Solo, Obi-Wan Kenobi, Chewbacca, R2-D2, C-3PO, and more (17 characters total).

Notice what happened: the query says "find a Film with this title, follow its `characters` link to Person documents, then get their `name`." Three hops through the graph — Film → characters → Person → name. No junction tables, no foreign key declarations, no JOINs. The relationships are part of the data structure itself, and traversing them is how you query.

## Step 4 — Branch and modify

What if Anakin Skywalker had turned to the Dark Side? Let's update his record on a branch and see what TerminusDB tracks.

Create a branch called `what-if`:

{% http-example method="POST" path="/api/branch/admin/star-wars/local/branch/what-if" %}
{"origin": "admin/star-wars/local/branch/main"}
{% http-expected %}
{"@type":"api:BranchResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

Now modify Anakin Skywalker's record on the branch — rewriting him as if he fell to the Dark Side. First, fetch his full document:

{% http-example method="GET" path="/api/document/admin/star-wars/local/branch/what-if?id=Person/Anakin%2520Skywalker" /%}

Now change four fields to tell the Dark Side story and PUT the modified document back:
- `"eye_color"`: `"blue"` → `"yellow"`
- `"mass"`: `84` → `120`
- `"side"`: `"Light Side"` → `"Dark Side"`
- `"faction"`: `"Jedi Order"` → `"Sith Order"`

{% http-example method="PUT" path="/api/document/admin/star-wars/local/branch/what-if?author=admin&message=What+if+Anakin+turned+to+the+Dark+Side" %}
{"@id": "Person/Anakin%20Skywalker", "@type": "Person", "birth_year": "41.9BBY", "eye_color": "yellow", "faction": "Sith Order", "films": ["Film/Attack%20of%20the%20Clones", "Film/Revenge%20of%20the%20Sith", "Film/The%20Phantom%20Menace"], "gender": "male", "hair_color": "blond", "height": 188, "homeworld": "Planet/Tatooine", "mass": 120, "name": "Anakin Skywalker", "quote": "You underestimate my power!", "side": "Dark Side", "species": ["Species/Human"]}
{% http-expected %}
["terminusdb:///data/Person/Anakin%20Skywalker"]
{% /http-expected %}
{% /http-example %}

You just rewrote history — on a branch. Main still has Anakin Skywalker (blue eyes, Light Side, mass 84, Jedi Order). Your `what-if` branch has him turned Dark (yellow eyes, Dark Side, mass 120, Sith Order). The `@id` stays the same (`Person/Anakin%20Skywalker`) — TerminusDB tracks object identity through changes, not content. Let's see exactly what changed.

## Step 5 — See what changed (the diff)

This is the moment. In any other database, answering "what changed between these two versions?" means writing audit triggers, maintaining changelog tables, or exporting both states and diffing them externally. In TerminusDB, you ask the database directly:

{% http-example method="POST" path="/api/diff/admin/star-wars" %}
{"before_data_version": "main", "after_data_version": "what-if"}
{% http-expected %}
[{"@id": "Person/Anakin%20Skywalker", "eye_color": {"@op": "SwapValue", "@before": "blue", "@after": "yellow"}, "faction": {"@op": "SwapValue", "@before": "Jedi Order", "@after": "Sith Order"}, "mass": {"@op": "SwapValue", "@before": 84, "@after": 120}, "quote": {"@op": "SwapValue", "@before": "This is where the fun begins.", "@after": "You underestimate my power!"}, "side": {"@op": "SwapValue", "@before": "Light Side", "@after": "Dark Side"}}]
{% /http-expected %}
{% /http-example %}

This diff is **structural, not textual**. TerminusDB is not comparing strings line by line — it knows the document schema, understands which field changed, what the old value was, and what the new value is. Each change is a typed operation (`SwapValue`) that can be applied, reversed, or composed with other patches programmatically.

You sent a full document replacement, but TerminusDB detected only the five fields that actually changed: `eye_color`, `faction`, `mass`, `quote`, and `side`. Compare this to the alternative: export both database states as JSON, run a generic diff tool, then parse the text output to figure out what actually changed. Or maintain an audit table with triggers that fire on every update. Or write custom comparison logic in your application.

TerminusDB replaces all of that with one API call. The database *is* the version history — diffs are a native operation, not an afterthought bolted on top.

---

## Clean up

{% callout type="warning" %}
This deletes the **star-wars** data product permanently. Only run this when you are finished experimenting.
{% /callout %}

{% http-example method="DELETE" path="/api/db/admin/star-wars" confirm="This will permanently delete the star-wars data product." /%}

---

## What you just did

In 15 minutes, you:

1. **Cloned** a complete Star Wars database from a public server — one command, no account
2. **Queried** relationships across documents (films → characters) using WOQL
3. **Branched** the database and made a speculative change ("what if Anakin turned to the Dark Side?")
4. **Diffed** the branch against main and saw field-level structural changes

These four operations — clone, query, branch, diff — are the core of TerminusDB. Every operation you just ran works at any scale: 20 documents or 2 million.

{% callout type="note" title="Prefer a business dataset?" %}
Try [Explore an Ecommerce Dataset →](/docs/explore-ecommerce-dataset) — the same workflow with customers, orders, and products.
{% /callout %}

## Next steps

- [Merge your branch back to main](/docs/first-15-minutes/#step-7--merge-the-branch) — complete the git-for-data cycle
- [Write your own schema](/docs/schema-reference-guide/) — add validation and type safety
- [Time-travel to previous commits](/docs/time-travel-howto/) — view any previous state of your database
- [WOQL query language](/docs/woql-getting-started/) — learn the full query language
- [Connect with TypeScript](/docs/connect-with-the-javascript-client/) — use the SDK in your application
