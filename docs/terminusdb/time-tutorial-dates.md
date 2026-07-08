---
tags:
  - woql
  - tutorial
  - time-processing
  - beginner
title: "Tutorial 1: Dates, Comparisons & Range Queries"
nextjs:
  metadata:
    title: "Tutorial 1: Dates, Comparisons & Range Queries"
    keywords: woql date comparison range query less greater gte lte in_range as-of half-open xsd iso8601 playground
    description: Hands-on tutorial using the WOQL Playground to learn date representation, comparison predicates, half-open range filtering, and the as-of vs period query pattern.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/time-tutorial-dates/
media: []
---

> **Series:** [Time Processing Overview](/docs/time-processing) | **Tutorial 1** | [Tutorial 2: Durations](/docs/time-tutorial-durations) | [Tutorial 3: Intervals](/docs/time-tutorial-intervals) | [Tutorial 4: Creative Patterns](/docs/time-tutorial-patterns)

## What you will learn

By the end of this tutorial you will be able to:
- Represent dates and times as typed literals in WOQL
- Compare dates with `less`, `greater`, `lte`, and `gte`
- Filter values into half-open ranges with `in_range`
- Explain the difference between "as of" and "period" queries
- Know when to use `triple_slice` for O(log n) lookups on stored data

## Prerequisites

- A running TerminusDB instance (Docker is easiest: `docker run --rm -p 6363:6363 terminusdb/terminusdb-server`)
- Basic familiarity with WOQL concepts ([WOQL Tutorial](/docs/woql-tutorial))

Every playground below runs against your local TerminusDB. Click **Settings** on any playground to change the server URL or credentials. The time predicates work in **anonymous mode** — no database needed — because they are pure computations.

---

## Part 1: Typed Literals — How Dates Are Represented

Every value in WOQL is a **typed literal**: a value string paired with a type. The `literal()` function creates one. Run this to see what a date looks like:

{% woql-playground code="eq(\"v:today\", literal(\"2025-03-31\", \"xsd:date\"))" title="Step 1: A typed date literal" description="Bind a date literal to a variable. Notice the @type and @value in the result." /%}

**What happened:** The variable `v:today` is now bound to a value with type `xsd:date` and value `"2025-03-31"`. Every date in TerminusDB carries its type — this is how the system knows to compare it as a calendar date, not as a string.

### The date and time types you will use most

| Type | Format | Example | When to use |
|------|--------|---------|-------------|
| `xsd:date` | `YYYY-MM-DD` | `"2025-03-31"` | Calendar dates: periods, filings, deadlines |
| `xsd:dateTime` | `YYYY-MM-DDThh:mm:ssZ` | `"2025-03-31T16:00:00Z"` | Timestamps: trades, audit trails |
| `xsd:gYear` | `YYYY` | `"2025"` | Fiscal years |
| `xsd:gYearMonth` | `YYYY-MM` | `"2025-03"` | Reporting months |

Try changing the type and value in the playground above. What happens if you use `xsd:gYearMonth` with `"2025-03"`?

---

## Part 2: Comparing Dates

WOQL provides four comparison predicates. They work on every ordered type — dates, numbers, strings.

| Predicate | Meaning | Succeeds when |
|-----------|---------|---------------|
| `less(A, B)` | A < B | A is strictly before B |
| `greater(A, B)` | A > B | A is strictly after B |
| `lte(A, B)` | A ≤ B | A is before or equal to B |
| `gte(A, B)` | A ≥ B | A is after or equal to B |

### Step 2: Strict comparison

{% woql-playground showResultOnly=true code="less(\n  literal(\"2025-01-01\", \"xsd:date\"),\n  literal(\"2025-03-31\", \"xsd:date\")\n)" title="Step 2: less — Is January before March?" description="Check if the first date is before the second. Result shows true/false." /%}

**What happened:** The result shows **true** (1 row returned). The `less` predicate succeeds because January 1 is before March 31. 

Try changing the dates so the first is after the second — you'll see **false** (0 rows) because the comparison fails.

### Step 3: Boundary equality

{% woql-playground showResultOnly=true code="gte(\n  literal(\"2025-02-28\", \"xsd:date\"),\n  literal(\"2025-02-28\", \"xsd:date\")\n)" title="Step 3: gte — Equal dates" description="Does >= succeed when both dates are the same?" /%}

**What happened:** The result shows **true**. The `gte` (greater than or equal) predicate succeeds when values are equal — that's the "or equal" part. If you used `greater` instead, you'd see **false** because the dates are not strictly greater.

**Key insight:** This distinction matters for "as of" queries (covered below). Use `gte` when you want to include the boundary date, `greater` when you want to exclude it.

### Step 4: Compare different types

{% woql-playground showResultOnly=true code="less(\n  literal(\"2025-01\", \"xsd:gYearMonth\"),\n  literal(\"2025-06\", \"xsd:gYearMonth\")\n)" title="Step 4: Compare gYearMonth values" description="Comparisons work on gYearMonth too. January is before June." /%}

---

## Part 3: Half-Open Range Filtering with `in_range`

`in_range(Value, Start, End)` succeeds when `Start ≤ Value < End`. The start is **inclusive**, the end is **exclusive**. This `[start, end)` convention is called a *half-open range*.

### Step 5: Value inside the range

{% woql-playground showResultOnly=true code="in_range(\n  literal(\"2025-02-15\", \"xsd:date\"),\n  literal(\"2025-01-01\", \"xsd:date\"),\n  literal(\"2025-04-01\", \"xsd:date\")\n)" title="Step 5: in_range — Is Feb 15 in Q1?" description="Checks if February 15 falls within [Jan 1, Apr 1)." /%}

### Step 6: Start is inclusive

{% woql-playground showResultOnly=true code="in_range(\n  literal(\"2025-01-01\", \"xsd:date\"),\n  literal(\"2025-01-01\", \"xsd:date\"),\n  literal(\"2025-04-01\", \"xsd:date\")\n)" title="Step 6: Start boundary — inclusive" description="The start date itself IS in the range." /%}

### Step 7: End is exclusive

{% woql-playground showResultOnly=true code="in_range(\n  literal(\"2025-04-01\", \"xsd:date\"),\n  literal(\"2025-01-01\", \"xsd:date\"),\n  literal(\"2025-04-01\", \"xsd:date\")\n)" title="Step 7: End boundary — exclusive" description="April 1 is NOT in [Jan 1, Apr 1). Zero results." /%}

**Why half-open?** Because adjacent ranges partition cleanly:
- Q1 = `[Jan 1, Apr 1)` — contains Mar 31, not Apr 1
- Q2 = `[Apr 1, Jul 1)` — contains Apr 1, not Jul 1
- No date falls in both. No date falls in neither. No gap between them.

### Step 8: Integer ranges work too

{% woql-playground showResultOnly=true code="in_range(7, 5, 10)" title="Step 8: Integer in_range" description="Is 7 in [5, 10)? Works with any ordered type." /%}

---

## Part 4: "As Of" vs "Period" Queries {#as-of}

This is the most important distinction in financial time processing. Mixing up these two patterns is the #1 source of off-by-one-day bugs.

### Pattern 1: "As of" — cumulative state at a point in time

"What is the balance **as of** March 31?" means: all transactions up to and **including** that date.

The query pattern uses `lte` — the as-of date is **inclusive**:

```javascript
// "As of March 31" — lte is inclusive
and(
  triple(v.doc, "transaction_date", v.date),
  lte(v.date, literal("2025-03-31", "xsd:date"))
)
```

### Pattern 2: "During period" — transactions in a range

"Show me Q1 transactions" means: transactions in the half-open interval `[Jan 1, Apr 1)`.

The query pattern uses `in_range` — the end date is **exclusive**:

```javascript
// "During Q1" — in_range has exclusive end
and(
  triple(v.doc, "transaction_date", v.date),
  in_range(v.date,
    literal("2025-01-01", "xsd:date"),
    literal("2025-04-01", "xsd:date"))
)
```

### The difference that matters

| Query | Predicate | Mar 31 included? | Apr 1 included? |
|-------|-----------|-------------------|-----------------|
| "As of Mar 31" | `lte(date, Mar31)` | Yes | No |
| "Q1 period" | `in_range(date, Jan1, Apr1)` | Yes | No |

Both include March 31. But "as of" also includes everything before Q1. The period query only includes Q1. And critically, the "as of" query uses the **inclusive** end (`Mar 31`), while the period query uses the **exclusive** end (`Apr 1`).

---

## Part 5: Working with Stored Data

The playgrounds above used anonymous mode (pure computation). For queries on stored data, you need a database. The patterns below work with any schema that has date properties.

### `triple_slice` — O(log n) range queries

When data is stored in TerminusDB, `triple_slice` pushes range constraints into the storage engine. It replaces `triple` + `in_range` with a single indexed lookup:

```javascript
// Instead of this (scans all values):
and(
  triple(v.doc, "transaction_date", v.date),
  in_range(v.date, start, end)
)

// Use this (indexed lookup):
triple_slice(v.doc, "transaction_date", v.date, start, end)
```

| Situation | Use | Why |
|-----------|-----|-----|
| Filtering stored property values | `triple_slice` | O(log n), uses storage index |
| Filtering computed values | `in_range` | Works on any bound value |
| Checking a single value | `in_range` | Simpler for one-off checks |

> **Deep dive:** See the [triple_slice guide](/docs/woql-triple-slice/) for advanced usage.

---

## Part 6: Timezone Handling

### Dates are timezone-agnostic

Calendar dates ignore timezone offsets for comparison and arithmetic:

{% woql-playground showResultOnly=true code="less(\n  literal(\"2025-03-31\", \"xsd:date\"),\n  literal(\"2025-04-01\", \"xsd:date\")\n)" title="Step 9: Date comparison ignores timezone" description="Calendar dates compare by date value only." /%}

### DateTimes normalize to UTC

DateTime values are converted to UTC before comparison. `23:00+02:00` is `21:00Z`, which is before `22:00Z`:

{% woql-playground showResultOnly=true code="less(\n  literal(\"2025-03-31T23:00:00+02:00\", \"xsd:dateTime\"),\n  literal(\"2025-03-31T22:00:00Z\", \"xsd:dateTime\")\n)" title="Step 10: DateTime UTC normalization" description="23:00+02:00 = 21:00 UTC, which is before 22:00 UTC." /%}

**For financial systems:** store timestamps in UTC. Timezone conversion is a display concern, not a data concern.

---

## Self-Check

Before moving to the next tutorial, make sure you can answer these:

1. What is the difference between `less` and `lte`?
2. Why does `in_range(10, 5, 10)` fail?
3. When would you use `lte` instead of `in_range` for a date query?
4. Why does `triple_slice` exist when `in_range` already works?

---

## What You Learned

| Concept | Key Point |
|---------|-----------|
| **Typed literals** | `literal("2025-03-31", "xsd:date")` — always pair value with type |
| **Comparators** | `less`/`lt` (strict), `greater`/`gt` (strict), `lte`, `gte` |
| **`in_range`** | Half-open `[start, end)` — start inclusive, end exclusive |
| **"As of" queries** | Use `lte` — the as-of date is included |
| **Period queries** | Use `in_range` — the end date is excluded |
| **`triple_slice`** | O(log n) indexed range lookup for stored data |
| **Timezones** | Dates: agnostic. DateTimes: UTC-normalized |

## Next

[Tutorial 2: Durations, Month Arithmetic & Sequences →](/docs/time-tutorial-durations)
