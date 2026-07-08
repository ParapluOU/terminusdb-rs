---
tags:
  - woql
  - tutorial
  - beginner
title: "Learn WOQL: An Interactive Tutorial"
nextjs:
  metadata:
    title: "Learn WOQL: An Interactive Tutorial"
    keywords: woql tutorial interactive datalog query language learn beginner
    description: A hands-on interactive tutorial that teaches the WOQL query language from first principles, with runnable examples you can edit and execute against your local TerminusDB instance.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-tutorial/
media: []
---

WOQL (Web Object Query Language) is a declarative query language for TerminusDB. Instead of telling the database *how* to find data step by step, you describe *what* you want and the engine figures out the rest.

This tutorial teaches WOQL from scratch through interactive examples you can edit and run directly from this page. Each step builds on the last, introducing one concept at a time.

**What you'll learn:**
- How variables and binding work — the core mental model
- How `and` and `or` combine constraints
- How to read, insert, update, and delete documents
- How to filter, sort, group, and shape results

---

## Before You Start

**Start TerminusDB** using Docker:

```bash
docker run --rm -p 6363:6363 terminusdb/terminusdb-server
```

**That's it!** The first examples (Steps 1-6) work against the `_system` database and don't need any setup. They're pure logic exercises.

**When you reach Step 7** (reading triples from actual data), you'll need to create the tutorial database. Run the three setup steps below **in order** before continuing to Step 7.

### Setup Step 1: Create the Database

{% api-step title="Create Database" description="Creates the woql_tutorial database. Run this first." method="POST" path="/api/db/admin/woql_tutorial" body="{\"label\":\"WOQL Tutorial\",\"comment\":\"Interactive tutorial database\"}" /%}

### Setup Step 2: Add the Schema

{% api-step title="Add Person Schema" description="Adds a Person class with name, age, city (optional), and email (optional)." method="POST" path="/api/document/admin/woql_tutorial?graph_type=schema&author=admin&message=Add+schema" body="{\"@type\":\"Class\",\"@id\":\"Person\",\"@key\":{\"@type\":\"Lexical\",\"@fields\":[\"name\"]},\"name\":\"xsd:string\",\"age\":\"xsd:integer\",\"city\":{\"@type\":\"Optional\",\"@class\":\"xsd:string\"},\"email\":{\"@type\":\"Optional\",\"@class\":\"xsd:string\"}}" /%}

### Setup Step 3: Insert Sample Data

{% api-step title="Insert People" description="Adds five people: Alice, Bob, Carol, David, and Eve." method="POST" path="/api/document/admin/woql_tutorial?author=admin&message=Add+people" body="[{\"@type\":\"Person\",\"name\":\"Alice\",\"age\":28,\"city\":\"New York\",\"email\":\"alice@example.com\"},{\"@type\":\"Person\",\"name\":\"Bob\",\"age\":35,\"city\":\"San Francisco\",\"email\":\"bob@example.com\"},{\"@type\":\"Person\",\"name\":\"Carol\",\"age\":28,\"city\":\"New York\"},{\"@type\":\"Person\",\"name\":\"David\",\"age\":42,\"city\":\"Austin\",\"email\":\"david@example.com\"},{\"@type\":\"Person\",\"name\":\"Eve\",\"age\":31,\"city\":\"San Francisco\"}]" /%}

Each playground below sends WOQL queries to `http://localhost:6363` with credentials `admin:root` and database `woql_tutorial`. Click **Settings** on any playground to change these.

---

## Phase 1: How WOQL Thinks

Before touching any data, let's understand the core mechanics. These exercises are pure logic — they don't read or write documents. They work against any database.

### Step 1: Your First Variable — `eq`

`eq` binds a variable to a value. Think of it as declaring "this variable equals this value". The result comes back as one row with the variable filled in.

Variables are written as `"v:name"`. The `v:` prefix tells WOQL it's a variable, not a literal string.

{% woql-playground code="eq(\"v:greeting\", \"Hello, WOQL!\")" title="Step 1: eq — Bind a Variable" description="Run this to see v:greeting bound to 'Hello, WOQL!' in the results." /%}

**What happened:** WOQL found exactly one way to satisfy the constraint "greeting equals Hello, WOQL!" — by binding `v:greeting` to that string. One constraint, one solution, one row.

---

### Step 2: Two Variables with `and`

`and` means "all of these must hold at the same time". Each constraint adds a new column to the result.

{% woql-playground code="and(\n  eq(\"v:name\", \"Alice\"),\n  eq(\"v:city\", \"New York\")\n)" title="Step 2: and — Multiple Variables" description="Two eq constraints, two variables, one row where both hold." /%}

**What happened:** Both constraints are satisfied simultaneously. You get one row with `name = Alice` and `city = New York`. The `and` doesn't multiply rows — it requires all parts to hold in the same solution.

---

### Step 3: Conflicting Constraints — Zero Results

What if you bind the *same* variable to two different values?

{% woql-playground code="and(\n  eq(\"v:x\", \"apple\"),\n  eq(\"v:x\", \"banana\")\n)" title="Step 3: Conflict — Zero Results" description="Same variable, two different values. How many solutions exist?" /%}

**What happened:** Zero results. There's no value of `v:x` that is simultaneously "apple" and "banana". This is constraint satisfaction at work — WOQL doesn't error, it simply finds no valid solutions. This is one of the most important behaviors to internalize.

---

### Step 4: Alternative Solutions with `or`

`or` means "at least one of these holds". Each alternative that succeeds produces its own result row.

{% woql-playground code="or(\n  eq(\"v:fruit\", \"apple\"),\n  eq(\"v:fruit\", \"banana\")\n)" title="Step 4: or — Alternative Solutions" description="Two alternatives for the same variable. How many rows come back?" /%}

**What happened:** Two rows — one for each way to satisfy the constraint. `or` expands the solution space: it says "give me every alternative that works". This is fundamentally different from `and`, which narrows it.

**Key insight:** `or` drives *cardinality* — the number of result rows. Each successful branch is a separate solution.

---

### Step 5: Controlling Output with `select`

By default, WOQL returns all variables. `select` lets you choose which ones appear in the results.

{% woql-playground code="select(\n  \"v:visible\",\n  and(\n    eq(\"v:visible\", \"you see me\"),\n    eq(\"v:hidden\", \"you don't see me\")\n  )\n)" title="Step 5: select — Control Output" description="Only v:visible appears in the results, even though v:hidden is also bound." /%}

**What happened:** Both variables were bound inside the query, but only `v:visible` made it to the output. `select` is a filter on what comes back, not on what gets evaluated. The inner query still runs fully — `select` just hides variables from the result.

---

### Phase 1 Summary

| Concept | What it does |
|---------|-------------|
| `eq(var, value)` | Binds a variable to a value |
| `and(a, b, ...)` | All constraints must hold — narrows solutions |
| `or(a, b, ...)` | Any alternative can hold — expands solutions |
| `select(vars..., query)` | Controls which variables appear in results |
| Conflicting constraints | Produce zero results, not errors |

You now understand the core execution model. Every WOQL query, no matter how complex, is built from these primitives.

---

## Phase 2: Working with Data

Now let's query the Person documents you inserted during setup.

### Step 6: The `Vars` Helper and Dot Syntax

So far we've written variables as raw strings like `"v:name"`. In practice, the JavaScript client provides a `Vars` helper that lets you declare variables up front and reference them with **dot notation**. This is cleaner and less error-prone.

{% woql-playground code="let v = Vars(\"greeting\", \"audience\")\nand(\n  eq(v.greeting, \"Hello\"),\n  eq(v.audience, \"World\")\n)" title="Step 6: Vars — Dot Syntax" description="Vars creates an object. v.greeting is equivalent to \"v:greeting\"." /%}

`Vars("greeting", "audience")` returns `{ greeting: "v:greeting", audience: "v:audience" }`. The dot operator is just regular JavaScript property access — nothing magic. You can mix both styles freely, but dot notation catches typos at write time and reads more naturally.

From here on, the examples use whichever style fits best: `"v:name"` for one-off variables, `Vars` + dot for multi-variable queries.

---

### Step 7: Reading Triples — `triple`

Every document in TerminusDB is stored as a set of *triples*: subject–predicate–object. The `triple` predicate matches these.

{% woql-playground database="woql_tutorial" code="triple(\"v:person\", \"name\", \"v:name\")" title="Step 7: triple — Read Properties" description="Find every person's name. v:person gets the document ID, v:name gets the value." /%}

**What happened:** WOQL scanned all triples where the predicate is `name` and returned each match as a row. The `v:person` variable holds the document identifier (like `Person/Alice`), and `v:name` holds the string value.

Triples are the atoms of TerminusDB. Documents are assembled from triples, and `triple` is how WOQL accesses individual properties.

---

### Step 8: Joining Triples — Shared Variables

When two `triple` calls share a variable, WOQL naturally joins them — like a SQL join, but implicit. This is where dot syntax shines: the shared variable is obvious at a glance.

{% woql-playground database="woql_tutorial" code="let v = Vars(\"person\", \"name\", \"age\")\nand(\n  triple(v.person, \"name\", v.name),\n  triple(v.person, \"age\", v.age)\n)" title="Step 8: Join — Shared Variable" description="v.person appears in both triples, so they match on the same document." /%}

**What happened:** Because `v.person` appears in both `triple` calls, WOQL ensures they refer to the same document. For each person, you get their name *and* their age in the same row. This is a natural join — no explicit JOIN keyword needed. This is also called "unification".

---

### Step 9: Type Checking with `isa`

`isa` checks if a document is of a specific type. Useful when your database has multiple classes.

{% woql-playground database="woql_tutorial" code="let v = Vars(\"doc\", \"name\")\nand(\n  triple(v.doc, \"name\", v.name),\n  isa(v.doc, \"Person\")\n)" title="Step 9: isa — Type Check" description="Only match documents that are of type Person." /%}

---

### Step 10: Reading Whole Documents — `read_document`

Instead of extracting individual triples, `read_document` gives you the full JSON document.

{% woql-playground database="woql_tutorial" code="let v = Vars(\"id\", \"doc\")\nand(\n  isa(v.id, \"Person\"),\n  read_document(v.id, v.doc)\n)" title="Step 10: read_document — Full Documents" description="v.doc contains the complete JSON document, not just a single property." /%}

**What happened:** `read_document` assembled all the triples for each document into a complete JSON object — including nested subdocuments if any existed. Use `triple` when you need specific properties for filtering or joining; use `read_document` when you want the whole thing.

---

## Phase 3: Filtering and Shaping

### Step 11: Filtering with Comparisons

`greater` and `less` compare values. Combined with `triple`, they filter results.

{% woql-playground database="woql_tutorial" code="and(\n  triple(\"v:person\", \"name\", \"v:name\"),\n  triple(\"v:person\", \"age\", \"v:age\"),\n  greater(\"v:age\", 30)\n)" title="Step 11: Filtering — People Over 30" description="Find people whose age is greater than 30." /%}

**What happened:** WOQL first matched all person names and ages, then kept only the rows where age > 30. The `greater` predicate acts as a filter — it doesn't generate new bindings, it eliminates rows that don't satisfy the condition.

---

### Step 12: Matching Specific Values

You can match a specific property value by putting a literal in the object position of a `triple`.

{% woql-playground database="woql_tutorial" code="and(\n  triple(\"v:person\", \"city\", literal(\"New York\", \"xsd:string\")),\n  triple(\"v:person\", \"name\", \"v:name\")\n)" title="Step 12: Exact Match" description="Find people in New York. The literal() wraps the string with its XSD type." /%}

**Why `literal()`?** Properties in TerminusDB are stored as typed RDF values. The `city` field is `xsd:string`, so matching against it requires a typed literal, not a bare string. `literal(value, type)` creates the right wrapper.

---

### Step 13: Optional Properties with `opt`

Some fields are optional — not every Person has an email. `opt` succeeds even when the inner pattern fails.

{% woql-playground database="woql_tutorial" code="and(\n  triple(\"v:person\", \"name\", \"v:name\"),\n  opt(triple(\"v:person\", \"email\", \"v:email\"))\n)" title="Step 13: opt — Optional Properties" description="Get names and emails. People without email still appear (with email unbound)." /%}

**What happened:** Everyone appears in the results. For people with email, `v:email` is bound. For people without, it's left unbound (empty). Without `opt`, people lacking email would be excluded entirely — the `triple` would fail to match and eliminate that row.

---

### Step 14: Negation with `not`

`not` succeeds when the inner pattern *fails*. Use it to find documents that are missing something.

{% woql-playground database="woql_tutorial" code="and(\n  isa(\"v:person\", \"Person\"),\n  triple(\"v:person\", \"name\", \"v:name\"),\n  not(triple(\"v:person\", \"email\", \"v:email\"))\n)" title="Step 14: not — Missing Properties" description="Find people who do NOT have an email address." /%}

**What happened:** `not` inverts the match. The inner `triple` looks for email — where it succeeds, `not` fails (excluding that person). Where the inner pattern fails (no email exists), `not` succeeds. Carol and Eve have no email, so they appear.

---

### Step 15: Combining `or` with Data

Use `or` to match documents in multiple cities.

{% woql-playground database="woql_tutorial" code="and(\n  triple(\"v:person\", \"name\", \"v:name\"),\n  or(\n    triple(\"v:person\", \"city\", literal(\"New York\", \"xsd:string\")),\n    triple(\"v:person\", \"city\", literal(\"San Francisco\", \"xsd:string\"))\n  ),\n  triple(\"v:person\", \"city\", \"v:city\")\n)" title="Step 15: or — Multiple Cities" description="People in New York OR San Francisco. The city appears in the results." /%}

---

### Step 16: Ordering and Limiting

`order_by` sorts results; `limit` caps how many come back.

{% woql-playground database="woql_tutorial" code="limit(3,\n  order_by(\"v:age\", \"asc\",\n    and(\n      triple(\"v:person\", \"name\", \"v:name\"),\n      triple(\"v:person\", \"age\", \"v:age\")\n    )\n  )\n)" title="Step 16: limit + order_by — Youngest 3" description="The three youngest people, sorted by age ascending." /%}

---

### Step 17: Grouping and Counting

`group_by` collects results into groups. Combined with `length`, it counts.

{% woql-playground database="woql_tutorial" code="and(\n  group_by(\n    [\"city\"],\n    [\"person\"],\n    \"v:city_group\",\n    and(\n      isa(\"v:person\", \"Person\"),\n      triple(\"v:person\", \"city\", \"v:city\")\n    )\n  ),\n  length(\"v:city_group\", \"v:count\")\n)" title="Step 17: group_by + length — Count Per City" description="How many people live in each city?" /%}

**What happened:** `group_by` groups the inner query results by city, collecting the person IDs into a list per city. Then `length` counts each group. This is pure WOQL aggregation — no client-side post-processing needed. Why is person empty? It's because it's within the group_by, so it's not part of the outer query. A select could have been used to select only the `city_group` and `city` from the `group_by`.

---

## Phase 4: Writing Data

WOQL queries can read and write in the same transaction. All writes are atomic — they either all succeed or none do. However, all read operations read from the layer that existed when the transaction started, and the writes go to the next transaction. Thus, reads can't see what is being written.

### Step 18: Inserting a Document

`insert_document` adds a new document. The document must conform to the schema.

{% woql-playground database="woql_tutorial" code="insert_document(\n  Doc({\n    \"@type\": \"Person\",\n    \"name\": \"Frank\",\n    \"age\": 27,\n    \"city\": \"Boston\"\n  }),\n  \"v:id\"\n)" title="Step 18: insert_document" description="Insert a new Person. Doc() converts the object to WOQL's internal format. v:id will contain the generated document ID." /%}

After running, try going back to Step 7 and running it again — you should see Frank in the results.

---

### Step 19: Updating a Document

`update_document` replaces a document. You need to provide the full document with its `@id`.

{% woql-playground database="woql_tutorial" code="update_document(\n  Doc({\n    \"@type\": \"Person\",\n    \"@id\": \"Person/Alice\",\n    \"name\": \"Alice\",\n    \"age\": 29,\n    \"city\": \"Boston\",\n    \"email\": \"alice@example.com\"\n  }),\n  \"v:id\"\n)" title="Step 19: update_document" description="Move Alice to Boston and update her age. Run Step 10 again to verify." /%}

---

### Step 20: Deleting a Document

`delete_document` removes a document by its ID.

{% woql-playground database="woql_tutorial" code="delete_document(\"Person/Frank\")" title="Step 20: delete_document" description="Remove Frank. Run Step 7 again to confirm he's gone." /%}

---

### Step 21: Declarative Batch Operations

The real power: combine reading and writing in a single query. Delete all people without an email address in one atomic operation.

{% woql-playground database="woql_tutorial" code="and(\n  isa(\"v:id\", \"Person\"),\n  not(triple(\"v:id\", \"email\", \"v:email\")),\n  triple(\"v:id\", \"name\", \"v:name\"),\n  delete_document(\"v:id\")\n)" title="Step 21: Declarative Batch Delete" description="Find people without email AND delete them — all in one query. v:name shows who was deleted." /%}

**What happened:** This is the kind of query that makes WOQL powerful. Instead of fetching IDs in one call and deleting in a loop, you describe the *pattern* of what to delete and WOQL handles the rest. The read part (`isa`, `not`, `triple`) identifies the targets; the write part (`delete_document`) acts on them. All atomically.

---

## Where to Go Next

You've learned the fundamentals. Here's where to deepen each area:

| Topic | Page |
|-------|------|
| Fluent vs. functional style | [WOQL Explanation](/docs/woql-explanation/) |
| Data types, dicts, CSV & JSON | [Working with Data](/docs/woql-data-handling/) |
| Variables and unification | [What is Unification?](/docs/what-is-unification/) |
| Subdocument handling | [WOQL Subdocument Handling](/docs/woql-subdocument-handling/) |
| Path queries (graph traversal) | [Path Queries](/docs/path-queries-in-woql/) |
| Math operations | [Math Queries](/docs/maths-based-queries-in-woql/) |
| Schema queries | [Schema Queries](/docs/schema-queries-with-woql/) |
| Range queries | [triple_slice Guide](/docs/woql-triple-slice/) |
| Time and date handling | [Time Handling](/docs/woql-time-handling/) |
| Backtracking, scope, and performance | [WOQL Control Flow](/docs/woql-control-flow/) |
| Complete reference | [WOQL Class Reference](/docs/woql-class-reference-guide/) |

### Cleanup

To remove the tutorial database when you're done:

{% api-step title="Delete Tutorial Database" description="Removes the woql_tutorial database and all its data." method="DELETE" path="/api/db/admin/woql_tutorial" /%}
