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
      canonical: https://terminusdb.com/docs/explore-a-real-dataset/
media: []
lastUpdated: "2026-04-30"
---

Clone a pre-populated Star Wars database to your local TerminusDB instance and explore it with queries, branches, and diffs — in 15 minutes. This tutorial focuses on **querying relationships** across documents — the graph traversal that distinguishes TerminusDB from a plain document store.

New to TerminusDB? Do the [First 10 Minutes quickstart](/docs/get-started/) first — it teaches the branch/diff/merge workflow this tutorial builds on. To build a database from scratch instead, see [Your First 15 Minutes](/docs/first-15-minutes/).

## What you will build

You will clone a complete Star Wars database (characters, films, planets, starships, species) from a public server, run queries that traverse document relationships, create a speculative branch ("what if Anakin turned to the Dark Side?"), and see a field-level structural diff of your changes.

## Step 1 — Clone the Star Wars database {% step %}

Pull the entire Star Wars dataset from the public templates server to your local instance — one POST is all it takes. `Authorization-Remote` carries the public server's anonymous credentials (`public:public`):

{% http-example method="POST" path="/api/clone/admin/star-wars" headers="{\"Authorization-Remote\": \"Basic cHVibGljOnB1YmxpYw==\"}" %}
{"remote_url": "https://data.terminusdb.org/public/star-wars", "label": "star-wars", "comment": "Cloned star-wars from data.terminusdb.org"}
{% http-expected %}
{"@type":"api:CloneResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

You just pulled a complete Star Wars database — characters, films, planets, starships — from a public TerminusDB server to your local instance. No account needed, no sign-up, no credentials. The data is now yours to query, branch, and modify. If the database already exists locally, the server answers `api:DatabaseAlreadyExists` — that's fine, you're ready.

## Step 2 — List the document types {% step %}

List the document types defined in the schema:

{% http-example method="GET" path="/api/document/admin/star-wars/local/branch/main?graph_type=schema&as_list=true&skip=1" /%}

{% http-expected %}
[{"@id":"Film","@key":{"@fields":["title"],"@type":"Lexical"},"@type":"Class","characters":{"@class":"Person","@type":"Set"},"director":{"@class":"xsd:string","@type":"Optional"},"episode_id":"xsd:integer","opening_crawl":{"@class":"xsd:string","@type":"Optional"},"planets":{"@class":"Planet","@type":"Set"},"producer":{"@class":"xsd:string","@type":"Optional"},"release_date":{"@class":"xsd:string","@type":"Optional"},"species":{"@class":"Species","@type":"Set"},"title":"xsd:string"},{"@id":"Person","@key":{"@fields":["name"],"@type":"Lexical"},"@type":"Class","birth_year":{"@class":"xsd:string","@type":"Optional"},"died_in_film":{"@class":"Film","@type":"Optional"},"eye_color":{"@class":"xsd:string","@type":"Optional"},"faction":{"@class":"xsd:string","@type":"Optional"},"films":{"@class":"Film","@type":"Set"},"gender":{"@class":"xsd:string","@type":"Optional"},"hair_color":{"@class":"xsd:string","@type":"Optional"},"height":{"@class":"xsd:decimal","@type":"Optional"},"homeworld":{"@class":"Planet","@type":"Optional"},"mass":{"@class":"xsd:decimal","@type":"Optional"},"name":"xsd:string","quote":{"@class":"xsd:string","@type":"Optional"},"side":"xsd:string","species":{"@class":"Species","@type":"Set"}},{"@id":"Species","@key":{"@fields":["name"],"@type":"Lexical"},"@type":"Class","average_height":{"@class":"xsd:decimal","@type":"Optional"},"average_lifespan":{"@class":"xsd:string","@type":"Optional"},"classification":{"@class":"xsd:string","@type":"Optional"},"designation":{"@class":"xsd:string","@type":"Optional"},"eye_colors":{"@class":"xsd:string","@type":"Optional"},"hair_colors":{"@class":"xsd:string","@type":"Optional"},"homeworld":{"@class":"Planet","@type":"Optional"},"language":{"@class":"xsd:string","@type":"Optional"},"name":"xsd:string","skin_colors":{"@class":"xsd:string","@type":"Optional"}},{"@id":"Planet","@key":{"@fields":["name"],"@type":"Lexical"},"@type":"Class","climate":{"@class":"xsd:string","@type":"Optional"},"diameter":{"@class":"xsd:decimal","@type":"Optional"},"gravity":{"@class":"xsd:string","@type":"Optional"},"name":"xsd:string","orbital_period":{"@class":"xsd:decimal","@type":"Optional"},"population":{"@class":"xsd:decimal","@type":"Optional"},"rotation_period":{"@class":"xsd:decimal","@type":"Optional"},"terrain":{"@class":"xsd:string","@type":"Optional"}}]
{% /http-expected %}

The response lists 4 document types — `Film`, `Person`, `Planet`, and `Species`.

## Step 3 — List the characters {% step %}

List the first few characters:

{% http-example method="GET" path="/api/document/admin/star-wars/local/branch/main?type=Person&count=5&as_list=true" /%}

{% http-expected %}
[{"@id":"Person/Anakin%20Skywalker","@type":"Person","birth_year":"41.9BBY","eye_color":"blue","faction":"Jedi Order","films":["Film/Attack%20of%20the%20Clones","Film/Revenge%20of%20the%20Sith","Film/The%20Phantom%20Menace"],"gender":"male","hair_color":"blond","height":188,"homeworld":"Planet/Tatooine","mass":84,"name":"Anakin Skywalker","quote":"This is where the fun begins.","side":"Light Side","species":["Species/Human"]},{"@id":"Person/Beru%20Whitesun%20lars","@type":"Person","birth_year":"47BBY","died_in_film":"Film/A%20New%20Hope","eye_color":"blue","films":["Film/A%20New%20Hope","Film/Attack%20of%20the%20Clones","Film/Revenge%20of%20the%20Sith"],"gender":"female","hair_color":"brown","height":165,"homeworld":"Planet/Tatooine","mass":75,"name":"Beru Whitesun lars","side":"Neutral","species":["Species/Human"]},{"@id":"Person/Biggs%20Darklighter","@type":"Person","birth_year":"24BBY","died_in_film":"Film/A%20New%20Hope","eye_color":"brown","faction":"Rebel Alliance","films":["Film/A%20New%20Hope"],"gender":"male","hair_color":"black","height":183,"homeworld":"Planet/Tatooine","mass":84,"name":"Biggs Darklighter","side":"Light Side","species":["Species/Human"]},{"@id":"Person/C-3PO","@type":"Person","birth_year":"112BBY","eye_color":"yellow","faction":"Rebel Alliance","films":["Film/A%20New%20Hope","Film/Attack%20of%20the%20Clones","Film/Return%20of%20the%20Jedi","Film/Revenge%20of%20the%20Sith","Film/The%20Empire%20Strikes%20Back","Film/The%20Phantom%20Menace"],"height":167,"homeworld":"Planet/Tatooine","mass":75,"name":"C-3PO","quote":"We're doomed!","side":"Light Side","species":["Species/Droid"]},{"@id":"Person/Chewbacca","@type":"Person","birth_year":"200BBY","eye_color":"blue","faction":"Rebel Alliance","films":["Film/A%20New%20Hope","Film/Return%20of%20the%20Jedi","Film/Revenge%20of%20the%20Sith","Film/The%20Empire%20Strikes%20Back","Film/The%20Force%20Awakens"],"gender":"male","hair_color":"brown","height":228,"homeworld":"Planet/Kashyyyk","mass":112,"name":"Chewbacca","side":"Light Side","species":["Species/Wookiee"]}]
{% /http-expected %}

You see 5 Person documents (limited by `count=5`). The database has 20 characters total, plus films, planets, and species. All of Star Wars, versioned and queryable. Let's ask it some questions.

## Step 4 — Query the data {% step %}

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
{% http-expected %}
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":["Film","Character","CharacterName"],"bindings":[{"Character":"Person/Beru%20Whitesun%20lars","CharacterName":{"@type":"xsd:string","@value":"Beru Whitesun lars"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/Biggs%20Darklighter","CharacterName":{"@type":"xsd:string","@value":"Biggs Darklighter"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/C-3PO","CharacterName":{"@type":"xsd:string","@value":"C-3PO"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/Chewbacca","CharacterName":{"@type":"xsd:string","@value":"Chewbacca"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/Darth%20Vader","CharacterName":{"@type":"xsd:string","@value":"Darth Vader"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/Greedo","CharacterName":{"@type":"xsd:string","@value":"Greedo"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/Han%20Solo","CharacterName":{"@type":"xsd:string","@value":"Han Solo"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/Jabba%20Desilijic%20Tiure","CharacterName":{"@type":"xsd:string","@value":"Jabba Desilijic Tiure"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/Jek%20Tono%20Porkins","CharacterName":{"@type":"xsd:string","@value":"Jek Tono Porkins"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/Leia%20Organa","CharacterName":{"@type":"xsd:string","@value":"Leia Organa"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/Luke%20Skywalker","CharacterName":{"@type":"xsd:string","@value":"Luke Skywalker"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/Obi-Wan%20Kenobi","CharacterName":{"@type":"xsd:string","@value":"Obi-Wan Kenobi"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/Owen%20Lars","CharacterName":{"@type":"xsd:string","@value":"Owen Lars"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/R2-D2","CharacterName":{"@type":"xsd:string","@value":"R2-D2"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/R5-D4","CharacterName":{"@type":"xsd:string","@value":"R5-D4"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/Wedge%20Antilles","CharacterName":{"@type":"xsd:string","@value":"Wedge Antilles"},"Film":"Film/A%20New%20Hope"},{"Character":"Person/Wilhuff%20Tarkin","CharacterName":{"@type":"xsd:string","@value":"Wilhuff Tarkin"},"Film":"Film/A%20New%20Hope"}],"deletes":0,"inserts":0,"transaction_retry_count":0}
{% /http-expected %}
{% /http-example %}

Expected output includes character names: Luke Skywalker, Leia Organa, Han Solo, Obi-Wan Kenobi, Chewbacca, R2-D2, C-3PO, and more (17 characters total).

Notice what happened: the query says "find a Film with this title, follow its `characters` link to Person documents, then get their `name`." Three hops through the graph — Film → characters → Person → name. No junction tables, no foreign key declarations, no JOINs. The relationships are part of the data structure itself, and traversing them is how you query.

## Step 5 — Create a branch {% step %}

What if Anakin Skywalker had turned to the Dark Side? Let's update his record on a branch and see what TerminusDB tracks.

Create a branch called `what-if`:

{% http-example method="POST" path="/api/branch/admin/star-wars/local/branch/what-if" %}
{"origin": "admin/star-wars/local/branch/main"}
{% http-expected %}
{"@type":"api:BranchResponse","api:status":"api:success"}
{% /http-expected %}
{% /http-example %}

## Step 6 — Fetch Anakin's document {% step %}

Now modify Anakin Skywalker's record on the branch — rewriting him as if he fell to the Dark Side. First, fetch his full document:

{% http-example method="GET" path="/api/document/admin/star-wars/local/branch/what-if?id=Person/Anakin%2520Skywalker" /%}

{% http-expected %}
{"@id":"Person/Anakin%20Skywalker","@type":"Person","birth_year":"41.9BBY","eye_color":"blue","faction":"Jedi Order","films":["Film/Attack%20of%20the%20Clones","Film/Revenge%20of%20the%20Sith","Film/The%20Phantom%20Menace"],"gender":"male","hair_color":"blond","height":188,"homeworld":"Planet/Tatooine","mass":84,"name":"Anakin Skywalker","quote":"This is where the fun begins.","side":"Light Side","species":["Species/Human"]}
{% /http-expected %}

## Step 7 — Write the change {% step %}

Now change four fields to tell the Dark Side story and PUT the modified document back:
- `"eye_color"`: `"blue"` → `"yellow"`
- `"mass"`: `84` → `120`
- `"side"`: `"Light Side"` → `"Dark Side"`
- `"faction"`: `"Jedi Order"` → `"Sith Order"`

{% http-example method="PUT" path="/api/document/admin/star-wars/local/branch/what-if?author=admin&message=What+if+Anakin+turned+to+the+Dark+Side" %}
{"@id": "Person/Anakin%20Skywalker", "@type": "Person", "birth_year": "41.9BBY", "eye_color": "yellow", "faction": "Sith Order", "films": ["Film/Attack%20of%20the%20Clones", "Film/Revenge%20of%20the%20Sith", "Film/The%20Phantom%20Menace"], "gender": "male", "hair_color": "blond", "height": 188, "homeworld": "Planet/Tatooine", "mass": 120, "name": "Anakin Skywalker", "quote": "You underestimate my power!", "side": "Dark Side", "species": ["Species/Human"]}
{% http-expected %}
["Person/Anakin%20Skywalker"]
{% /http-expected %}
{% /http-example %}

You just rewrote history — on a branch. Main still has Anakin Skywalker (blue eyes, Light Side, mass 84, Jedi Order). Your `what-if` branch has him turned Dark (yellow eyes, Dark Side, mass 120, Sith Order). The `@id` stays the same (`Person/Anakin%20Skywalker`) — TerminusDB tracks object identity through changes, not content. Let's see exactly what changed.

## Step 8 — See what changed (the diff) {% step %}

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

## Step 9 — Clean up {% step %}

{% callout type="warning" %}
This deletes the **star-wars** data product permanently. Only run this when you are finished experimenting.
{% /callout %}

{% http-example method="DELETE" path="/api/db/admin/star-wars" confirm="This will permanently delete the star-wars data product." /%}

{% http-expected %}
{"@type":"api:DbDeleteResponse","api:status":"api:success"}
{% /http-expected %}

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
