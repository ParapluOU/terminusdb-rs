---
tags:
  - woql
  - reference
  - advanced
title: "WOQL Control Flow: Backtracking, Scope, and Performance"
nextjs:
  metadata:
    title: "WOQL Control Flow: Backtracking, Scope, and Performance"
    keywords: woql control flow backtracking scope variables member group_by collect distinct select triple triple_slice performance streaming deterministic non-deterministic datalog
    description: Deep guide to WOQL's execution model — how backtracking generates solutions, how variable scope works across sub-queries, and how to structure queries for optimal performance with triple, triple_slice, and streaming results.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-control-flow/
media: []
---

WOQL queries look declarative — you describe *what* you want and the engine figures out the rest. But underneath, the engine makes choices about *how* to find answers, and those choices follow specific rules. Understanding these rules is the difference between a query that streams results in milliseconds and one that materializes an entire dataset into memory before returning anything.

This page explains the execution model from the ground up: how solutions are generated, how variables move between inner and outer scopes, and how to structure queries so the engine can do its job efficiently.

> **Prerequisites:** This page assumes you've completed the [WOQL Tutorial](/docs/woql-tutorial/) and understand basic concepts like `and`, `or`, `triple`, and `eq`. If terms like "binding" and "variable" are unfamiliar, start there.

> **See also:** [WOQL Explanation](/docs/woql-explanation/) | [WOQL Tutorial](/docs/woql-tutorial/) | [Range Queries with triple_slice](/docs/woql-triple-slice/) | [Time Handling](/docs/woql-time-handling/) | [WOQL Class Reference](/docs/woql-class-reference-guide/)

---

## The Execution Model: Solutions and Backtracking

### What is a solution?

A WOQL query produces a table of results. Each row is a **solution** — a complete assignment of values to variables that satisfies every constraint in the query. The engine's job is to find all such solutions.

```javascript
// This query has exactly 1 solution:
eq("v:x", 42)
// Result: x = 42

// This query has 0 solutions:
and(eq("v:x", 42), eq("v:x", 99))
// No value of x satisfies both constraints simultaneously
```

### How backtracking works

The engine evaluates a query left to right. When it reaches a predicate that can produce multiple results — like `triple`, `or`, or `member` — it picks the first result and continues forward. If a later constraint fails, the engine **backtracks** to the most recent choice point and tries the next option.

Think of it as exploring a tree of possibilities:

```
and(
  triple(v.person, "city", v.city),     ← choice point: many people
  triple(v.person, "age", v.age),       ← for each person, look up age
  greater(v.age, 30)                    ← filter: does age > 30?
)
```

The engine:
1. Picks the first person/city triple.
2. Looks up that person's age.
3. Checks if age > 30. If yes, emits a solution. If no, backtracks to step 1.
4. Picks the next person/city triple. Repeats.

This is **backtracking** — the engine systematically explores all possibilities by going forward when constraints succeed and going backward when they fail.

### Deterministic vs. non-deterministic predicates

Every WOQL predicate falls into one of two categories:

| Category | Behavior | Examples |
|----------|----------|---------|
| **Non-deterministic** (generators) | Can produce multiple solutions via backtracking | `triple`, `triple_slice`, `member`, `or`, `sequence`, `path` |
| **Deterministic** (filters/transforms) | Produce exactly one solution or fail | `eq`, `greater`, `less`, `not`, `typecast`, `length`, `sum`, `group_by`, `limit`, `distinct` |

Non-deterministic predicates are **choice points** — they are where the engine branches. Deterministic predicates either pass or block a solution, but never multiply the number of solutions.

Understanding this distinction is crucial for reasoning about how many rows a query will return and where the computational work happens.

---

## Variable Scope: Inner and Outer Queries

### The default: all variables are globally scoped

In a flat WOQL query, all variables live in the same scope. A binding made anywhere is visible everywhere:

```javascript
and(
  eq("v:x", 42),
  eq("v:y", "v:x")   // v:x is already bound to 42, so v:y = 42
)
```

This is simple and intuitive for flat queries. It gets more interesting with sub-queries.

### `select` — restricting which variables escape

`select` runs a sub-query but only exposes the listed variables to the outer query. Variables inside the sub-query that are not selected are invisible outside:

```javascript
and(
  select("v:name",
    and(
      triple("v:person", "name", "v:name"),
      triple("v:person", "age", "v:age")     // v:age is computed but not exposed
    )
  ),
  eq("v:age", "unknown")  // v:age is unbound here — select hid it
)
```

**When to use `select`:** When a sub-query binds helper variables that would pollute the outer scope or cause unintended unification. It is the primary tool for variable hygiene.

### `group_by` — grouping with scoped aggregation

`group_by` runs a sub-query, groups the results by one or more variables, and collects the grouped values into lists. The sub-query's variables are scoped — only the grouping variables and the collected list are visible outside.

```javascript
// Count people per city
and(
  group_by(
    ["city"],                                          // group by these variables
    ["person"],                                        // collect these into lists
    "v:city_group",                                    // the collected list
    and(
      isa("v:person", "Person"),
      triple("v:person", "city", "v:city")
    )
  ),
  length("v:city_group", "v:count")
)
```

**Key scoping behavior:**
- `v:city` is the grouping variable — it is bound in the outer scope (one value per group).
- `v:person` is the template variable — its individual values are not visible outside, only the collected list is.
- `v:city_group` is the result list — visible in the outer scope.
- The inner query runs to exhaustion for each group.

**Common pitfall:** Using `group_by` with an empty grouping key just to collect all values into a list:

```javascript
// Verbose way to collect all ages into a list
group_by(
  [],
  ["age"],
  "v:all_ages",
  triple("v:person", "age", "v:age")
)
```

This works but is semantically misleading — you're not grouping by anything. The proposed `Collect` predicate (see below) makes this intent explicit.

### `Collect` — gathering bindings into a list (proposed)

`Collect` is the inverse of `Member`. Where `Member` unpacks a list into individual bindings, `Collect` runs a sub-query to exhaustion and gathers all values of a template variable into a list.

```javascript
// Collect all ages into a list
WOQL.collect(
  WOQL.var("age"),
  WOQL.var("all_ages"),
  WOQL.triple(WOQL.var("person"), "age", WOQL.var("age"))
)
// Result: all_ages = [28, 35, 28, 42, 31]
```

**Scoping behavior:**
- The `query` parameter is a self-contained sub-query. It runs independently.
- The `into` variable receives the resulting list in the outer scope.
- The `template` specifies what to collect from each solution (last to support fluent queries)
- Variables in the sub-query that are not in the template are existentially quantified — they exist only to help generate solutions.

**The key insight:** `Collect` must **control** the backtracking. You cannot collect values from a variable that is already backtracking in the outer scope — the sub-query that generates the values must be **inside** the `Collect`. This is the same constraint as `group_by` and the `findall` Prolog predicate.

```javascript
// This does NOT collect outer backtracking into a list:
and(
  triple("v:person", "age", "v:age"),   // generates many bindings
  WOQL.collect("v:age", ???, "v:list")  // too late — v:age is already bound
)

// This DOES work — the generating query is inside Collect:
WOQL.collect(
  WOQL.var("age"),
  WOQL.triple(WOQL.var("person"), "age", WOQL.var("age")),
  WOQL.var("all_ages")
)
```

### `member` — unpacking a list into bindings

`Member` is the inverse of `Collect`. Given a list, it generates one solution per element:

```javascript
and(
  eq("v:fruits", ["apple", "banana", "cherry"]),
  member("v:fruit", "v:fruits")
)
// Result: 3 rows — fruit = "apple", fruit = "banana", fruit = "cherry"
```

`Member` is non-deterministic — it creates a choice point with as many branches as there are list elements. Each branch produces one binding.

**The Member/Collect symmetry:**

| Predicate | Direction | Behavior |
|-----------|-----------|----------|
| `Member` | List → Bindings | Non-deterministic: generates one solution per element |
| `Collect` | Bindings → List | Deterministic: runs sub-query to exhaustion, produces one solution |

### `distinct` — deduplication within a scope

`distinct` eliminates duplicate values for a specified variable while preserving backtracking for everything else:

```javascript
distinct("v:city",
  triple("v:person", "city", "v:city")
)
// Returns each city once, even if multiple people live there
```

**Scoping behavior:** `distinct` observes the values of the specified variable as the inner query generates them, and suppresses solutions that would repeat a previously seen value. The inner query still runs normally — `distinct` is a filter on the output stream.

### `not` — negation as failure

`not` succeeds when its inner query **fails** (produces zero solutions). It does not bind any variables:

```javascript
and(
  isa("v:person", "Person"),
  triple("v:person", "name", "v:name"),
  not(triple("v:person", "email", "v:email"))
)
// People without an email address
```

**Critical scoping rule:** Variables that appear only inside `not` are existentially quantified. `not` checks whether *any* solution exists — it does not expose which specific values matched. If you need to know which values were excluded, use a different pattern (e.g., `opt` + filter).

### `opt` — optional matching

`opt` tries to match its inner query. If the inner query succeeds, its bindings flow to the outer scope. If it fails, the outer query continues with those variables unbound:

```javascript
and(
  triple("v:person", "name", "v:name"),
  opt(triple("v:person", "email", "v:email"))
)
// All people, with email if they have one
```

**Scoping behavior:** Unlike `not`, `opt` **does** expose inner variable bindings when the inner query succeeds. When it fails, those variables remain unbound (empty in the result).

---

## Structuring Queries for Performance

### Principle 1: Put the most selective constraint first

The engine evaluates left to right. Constraints that eliminate more possibilities early mean less backtracking overall:

```javascript
// Slow: scan all triples, then filter
and(
  triple("v:person", "name", "v:name"),
  triple("v:person", "city", "v:city"),
  triple("v:person", "age", "v:age"),
  greater("v:age", 60)
)

// Faster: if few people are over 60, this is easiest to reason about
// (But only if you can — triple doesn't support filtering and will materialize every triple for the non-grounded variable)
```

In practice, `triple` always scans all matching triples for a given predicate. The real optimization opportunity is choosing between `triple` and `triple_slice`.

### Principle 2: Use `triple_slice` for range constraints

`triple` iterates every triple for a predicate. `triple_slice` binary-searches the sorted value dictionary and only touches values in the range:

```javascript
// O(n) — scans all timestamps, then filters
and(
  triple("v:doc", "timestamp", "v:time"),
  greater("v:time", literal("2025-01-01T00:00:00Z", "xsd:dateTime")),
  less("v:time", literal("2025-02-01T00:00:00Z", "xsd:dateTime"))
)

// O(log n) — binary search, only touches matching values
triple_slice("v:doc", "timestamp", "v:time",
  "2025-01-01T00:00:00Z", "2025-02-01T00:00:00Z")
```

Both return the same results. The performance difference grows with dataset size. For time-series data or large numeric ranges, `triple_slice` is almost always the right choice.

See the [triple_slice guide](/docs/woql-triple-slice/) for full details on binding modes, type inference, and worked examples.

### Principle 3: Understand what streams and what materializes

WOQL predicates fall into two performance categories:

**Streaming predicates** yield one solution at a time, using constant memory regardless of how many total solutions exist:

- `triple` / `quad`
- `triple_slice` / `quad_slice`
- `member`
- `sequence`
- `path`
- `and`, `or` (composition)

**Materializing predicates** must collect all solutions from their sub-query before producing output. Memory usage scales with the number of inner solutions:

- `group_by` — groups all solutions, then emits groups one at a time
- `Collect` — collects all template values into a list in memory
- `distinct` — tracks all seen values to detect duplicates
- `order_by` — must see all solutions before it can sort
- `aggregate` / `count` / `sum` / `length` (on query results)

**The practical consequence:** If a sub-query inside `group_by` or `Collect` produces millions of solutions, those millions are held in memory. For very large result sets, consider whether you can narrow the sub-query with `limit`, `triple_slice`, or more selective constraints.

### Principle 4: Streaming responses — results flow as they complete

When you execute a WOQL query via the API, TerminusDB can stream results back as they are found. The client receives the first solution as soon as it is computed — it does not have to wait for all solutions to be found.

This means:

- **Streaming queries** (built from `triple`, `and`, `or`, etc.) start returning results immediately. Time-to-first-result is fast even for queries with millions of solutions.
- **Materializing queries** (containing `group_by`, `order_by`, etc.) must finish their inner computation before the first result streams out. Time-to-first-result depends on the inner query's total execution time.

**Designing for responsiveness:** If your application can process results incrementally (e.g., displaying a table row by row), prefer streaming query shapes. If you need aggregation, do it as late as possible in the query so the streaming portion runs first.

### Principle 5: Leverage bound variables for index access

`triple` uses different internal access patterns depending on which arguments are bound, examples:

| Subject | Predicate | Object | Access pattern |
|---------|-----------|--------|----------------|
| bound | bound | unbound | Direct lookup — fast |
| bound | unbound | unbound | Iterate predicates for one subject |
| unbound | bound | unbound | Iterate subjects for one predicate |
| unbound | unbound | unbound | Full scan — slowest |

The engine is fastest when it can use bound arguments to narrow the search. A common pattern is to bind the subject first (via a previous `triple` or `isa`), then look up specific properties:

```javascript
// Good: v.person is bound by isa, then used to look up properties directly
and(
  isa("v:person", "Person"),
  triple("v:person", "name", "v:name"),
  triple("v:person", "age", "v:age")
)
```

When you use `and`, the shared variable `v:person` is bound by the first predicate and used as an index key by the subsequent ones. This is the natural join pattern — and it is efficient because each subsequent `triple` does a direct lookup rather than a scan.

### Principle 6: Compose `Collect` with list operators

Rather than building custom aggregation logic, use `Collect` to bridge query results into lists, then apply existing list operators:

```javascript
// Min salary
and(
  WOQL.collect(
    WOQL.var("salary"),
    WOQL.triple(WOQL.var("emp"), "salary", WOQL.var("salary")),
    WOQL.var("salaries")
  ),
  WOQL.range_min(WOQL.var("salaries"), WOQL.var("lowest"))
)

// Count of results
and(
  WOQL.collect(
    WOQL.var("doc"),
    WOQL.isa(WOQL.var("doc"), "Person"),
    WOQL.var("docs")
  ),
  WOQL.length(WOQL.var("docs"), WOQL.var("count"))
)

// Window of results
and(
  WOQL.collect(
    WOQL.var("x"),
    WOQL.sequence(WOQL.var("x"), 0, 100),
    WOQL.var("all")
  ),
  WOQL.slice(WOQL.var("all"), WOQL.var("window"), 10, 20)
)
```

This pattern keeps the API surface small — one new predicate (`Collect`) unlocks all existing list operators (`RangeMin`, `RangeMax`, `Slice`, `Length`, `Sum`, `ListToSet`, `SetDifference`, `SetIntersection`, `SetUnion`) for use over query results.

---

## Predicate Quick Reference: Control Flow Properties

| Predicate | Deterministic? | Streams? | Scopes variables? | Description |
|-----------|---------------|----------|-------------------|-------------|
| `and` | — | Yes | No (shared scope) | All constraints must hold |
| `or` | No | Yes | No (shared scope) | Any branch can hold; one row per successful branch |
| `triple` | No | Yes | No | Pattern match on subject-predicate-object |
| `triple_slice` | No | Yes | No | Range-constrained triple with O(log n) access |
| `triple_slice_rev` | No | Yes | No | Range-constrained triple with O(log n) access, reversed |
| `triple_first` | No | Yes | No | Convenience range function, it's a limit(1) of the slice |
| `triple_last` | No | Yes | No | Convenience range function, it's a limit(1) of the reverse slice |
| `member` | No | Yes | No | Generates one binding per list element |
| `sequence` | No | Yes | No | Generates values in a range |
| `path` | No | Yes | No | Graph traversal with regex-like path expressions |
| `select` | — | Yes | **Yes** — hides non-selected variables | Filter which variables are exposed |
| `distinct` | — | Partial | No | Deduplicates; must track seen values |
| `not` | Yes | — | **Yes** — inner variables are existential | Succeeds when inner query fails |
| `opt` | — | Yes | Partial — bindings escape on success | Optional match; continues on failure |
| `group_by` | Yes | No (materializes) | **Yes** — template variables are scoped | Groups results, collects into lists |
| `collect` | Yes | No (materializes) | **Yes** — sub-query is self-contained | Gathers all template values into a list |
| `order_by` | — | No (materializes) | No | Sorts all solutions |
| `limit` | — | Yes | No | Caps the number of solutions |
| `eq` | Yes | — | No | Unifies two values |
| `greater` / `less` | Yes | — | No | Strict comparison filter |

---

## Patterns and Anti-Patterns

### Pattern: Narrow early, expand late

```javascript
// Good: isa + triple_slice narrows to a small set, then join
and(
  isa("v:doc", "SensorReading"),
  triple_slice("v:doc", "timestamp", "v:time",
    "2025-01-01T00:00:00Z", "2025-01-02T00:00:00Z"),
  triple("v:doc", "temperature", "v:temp")
)
```

### Anti-pattern: Collect everything, then filter

```javascript
// Bad: materializes all readings into memory, then filters client-side
WOQL.collect(
  WOQL.var("temp"),
  WOQL.var("all_temps")
  WOQL.triple(WOQL.var("doc"), "temperature", WOQL.var("temp")),
)
// Then filter all_temps in application code
```

Instead, filter inside the sub-query:

```javascript
// Better: only collect what you need
WOQL.collect(
  WOQL.var("temp"),
  WOQL.var("daily_temps"),
  WOQL.and(
    WOQL.triple_slice(WOQL.var("doc"), "timestamp", WOQL.var("time"),
      "2025-01-01T00:00:00Z", "2025-01-02T00:00:00Z"),
    WOQL.triple(WOQL.var("doc"), "temperature", WOQL.var("temp"))
  ),
)
```

### Pattern: Use `Member` and `Collect` as inverses

```javascript
// Generate a list from a query
WOQL.collect(
  WOQL.var("city"),
  WOQL.var("all_cities"),
  WOQL.and(
    WOQL.isa(WOQL.var("person"), "Person"),
    WOQL.triple(WOQL.var("person"), "city", WOQL.var("city"))
  ),
)

// Later: iterate over that list
WOQL.member(WOQL.var("city"), WOQL.var("unique_cities"))
```

### Pattern: Streaming joins for responsive UIs

When your UI can display results incrementally, prefer streaming query shapes:

```javascript
// Streams results immediately — first row arrives fast
and(
  isa("v:person", "Person"),
  triple("v:person", "name", "v:name"),
  triple("v:person", "age", "v:age")
)
```

Over materializing shapes:

```javascript
// Must compute all results before returning any
order_by("v:age", "asc",
  and(
    isa("v:person", "Person"),
    triple("v:person", "name", "v:name"),
    triple("v:person", "age", "v:age")
  )
)
```

Both return the same data. The first starts streaming immediately; the second waits until all results are sorted.

---

## Summary

| Concept | Key takeaway |
|---------|-------------|
| **Backtracking** | The engine explores all possibilities by going forward on success and backward on failure |
| **Choice points** | Non-deterministic predicates (`triple`, `member`, `or`, `sequence`) create branches |
| **Variable scope** | Variables are global by default; `select`, `group_by`, `Collect`, and `not` create scoped boundaries |
| **Streaming vs. materializing** | `triple`, `member`, `and` stream; `group_by`, `Collect`, `order_by` materialize |
| **Performance** | Use `triple_slice` for ranges, bind variables early, filter inside sub-queries |
| **Member/Collect symmetry** | `Member` unpacks lists into bindings; `Collect` packs bindings into lists |

Understanding these mechanics lets you write WOQL queries that are not just correct, but efficient — queries that leverage the engine's strengths rather than fighting against them.
