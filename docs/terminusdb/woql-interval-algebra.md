---
tags:
  - woql
  - reference
  - time-processing
  - advanced
title: Allen's Interval Algebra and ISO 8601 Intervals
nextjs:
  metadata:
    title: Allen's Interval Algebra and ISO 8601 Intervals
    keywords: allen interval algebra iso8601 half-open inclusive duration dateRange dateTimeInterval temporal reasoning woql
    description: Guide to Allen's Interval Algebra in WOQL — covering the xdd:dateTimeInterval type, typecasting between inclusive and half-open intervals, all 13 Allen relations, interval classification, and converting between reporting durations and temporal intervals.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-interval-algebra/
media: []
---

Every calendar has two stories to tell. One is the human story: "Q1 runs from January 1 to March 31." The other is the computational story: "Q1 is the half-open interval [January 1, April 1]." These two stories describe the same period, but they use different conventions — and mixing them up causes real bugs in financial systems, scheduling engines, and regulatory reporting.

This page explains the difference, introduces TerminusDB's `xdd:dateTimeInterval` type for ISO 8601 intervals, and shows how to use Allen's Interval Algebra to reason about temporal relationships in WOQL. Unfortunately the XSD Data Types do not include an explicit interval type, so we include it in the xdd namespace instead.

> **See also:** [WOQL Time Handling](/docs/woql-time-handling/) | [Data Types Reference](/docs/data-types/) | [WOQL Class Reference](/docs/woql-class-reference-guide/)

---

## The Gap in XSD: No Interval Type

The W3C XML Schema Definition (XSD) provides types for individual time points (`xsd:date`, `xsd:dateTime`) and for durations (`xsd:duration`), but it does not define a type for **time intervals** — periods anchored to specific dates.

ISO 8601, the international standard for date and time representation, does define time intervals in four forms:

| Form | Example | Description |
|------|---------|-------------|
| Start/end | `2025-01-01/2025-04-01` | Two explicit endpoints |
| Start/duration | `2025-01-01/P3M` | Start date plus a duration |
| Duration/end | `P3M/2025-04-01` | Duration ending at a date |
| Duration only | `P3M` | A length of time with no anchor |

XSD's `xsd:duration` only covers the fourth form. The first three — which are the most useful for real-world scheduling and reporting — have no standard XSD type.

TerminusDB fills this gap with `xdd:dateTimeInterval`.

---

## Inclusive vs Half-Open: The Conversion Problem

### How humans express periods

In financial reporting, regulatory filings, and everyday communication, date ranges are **inclusive** — both the start and end dates are part of the period:

> "Q1 2025 runs from **January 1** to **March 31**."

Both January 1 and March 31 are included in Q1. This is the convention used by `xdd:dateRange`, which stores inclusive ranges as `[2025-01-01, 2025-03-31]`.

### How temporal algebra works

Temporal reasoning systems — including Allen's Interval Algebra, SQL's `PERIOD` type, and most database internals — use **half-open** intervals where the start is included but the end is excluded:

> Q1 2025 = `[2025-01-01, 2025-04-01]` — includes January 1, excludes April 1.

Half-open intervals have three properties that make them essential for computation:

- **No gaps**: Q1 ends at April 1; Q2 starts at April 1. There is no missing day between them.
- **No overlaps**: March 31 belongs to Q1 only. April 1 belongs to Q2 only.
- **Clean partitioning**: The year `[Jan 1, Jan 1 next year]` divides exactly into 12 monthly intervals with no gaps or overlaps.

### The one-day offset

Converting between inclusive and half-open is straightforward — add one day to the inclusive end:

```
Inclusive:   [2025-01-01, 2025-03-31]     ← xdd:dateRange
Half-open:   [2025-01-01, 2025-04-01]     ← xdd:dateTimeInterval
                                  ↑
                        March 31 + 1 day = April 1
```

And to convert back, subtract one day from the exclusive end:

```
Half-open:   [2025-01-01, 2025-04-01]     ← xdd:dateTimeInterval
Inclusive:   [2025-01-01, 2025-03-31]     ← xdd:dateRange
                                  ↑
                        April 1 - 1 day = March 31
```

TerminusDB handles this conversion automatically through typecasting and the `interval_inclusive` predicate.

---

## The Three Temporal Interval Types

TerminusDB provides three types for temporal extent, each serving a different purpose:

{% table %}
* Type
* Notation
* Semantics
* Use case
---
* **`xsd:duration`**
* `P3M`, `P1Y2M10DT2H30M`
* A length of time — no anchor to specific dates
* Offsets, deadlines, durations between events
---
* **`xdd:dateRange`**
* `[2025-01-01, 2025-03-31]`
* Inclusive/closed — both endpoints included
* Human-readable reporting periods, regulatory filings
---
* **`xdd:dateTimeInterval`**
* `2025-01-01/2025-04-01`
* Half-open — start included, end excluded. ISO 8601 interval notation.
* Temporal algebra, scheduling, partitioning, Allen's relations
{% /table %}

### Type hierarchy

`xdd:dateTimeInterval` inherits from `xsd:duration`. This means any place that accepts a duration can also accept an interval, and intervals can be typecast to durations to extract the duration component.

```
xsd:anySimpleType
  └─ xsd:duration              ← pure durations (P3M, P1Y)
       ├─ xsd:yearMonthDuration
       ├─ xsd:dayTimeDuration
       └─ xdd:dateTimeInterval ← ISO 8601 intervals (all 4 forms)
```

---

## `xdd:dateTimeInterval` — ISO 8601 Intervals

### String notation

The type uses the ISO 8601 solidus (`/`) separator. All four interval forms are supported:

```
2025-01-01/2025-04-01              ← Form 1: start/end (dates)
2025-01-01T00:00:00Z/2025-04-01T00:00:00Z  ← Form 1: start/end (dateTimes)
2025-01-01/P3M                     ← Form 2: start/duration
P3M/2025-04-01                     ← Form 3: duration/end
P3M                                ← Form 4: duration only
```

### Extracting dates from intervals

When you unpack an interval to get its start and end dates, the result depends on which form was used:

{% table %}
* Form
* Start date
* End date
---
* **1. Start/end**
* Directly available
* Directly available
---
* **2. Start/duration**
* Directly available
* Computed from start + duration
---
* **3. Duration/end**
* Computed from end - duration
* Directly available
---
* **4. Duration only**
* `rdf:nil` (no anchor)
* `rdf:nil` (no anchor)
{% /table %}

### Using in a schema

```json
{
  "@type": "Class",
  "@id": "ReportingPeriod",
  "label": { "@type": "xsd:string" },
  "interval": { "@type": "xdd:dateTimeInterval" }
}
```

---

## Typecasting Between Types

TerminusDB's `typecast` predicate converts between temporal types. Each conversion direction is shown below.

### String ↔ dateTimeInterval

Parse an ISO 8601 interval string, or format an interval back to a string:

```javascript
// Parse string to interval
WOQL.typecast(
  literal("2025-01-01/2025-04-01", "xsd:string"),
  "xdd:dateTimeInterval",
  v.interval)

// Format interval to string
WOQL.typecast(v.interval, "xsd:string", v.str)
```

### dateRange → dateTimeInterval (inclusive → half-open)

Converts the inclusive end date by adding one day:

```javascript
// [2025-01-01, 2025-03-31] → 2025-01-01/2025-04-01
WOQL.typecast(v.date_range, "xdd:dateTimeInterval", v.interval)
```

### dateTimeInterval → dateRange (half-open → inclusive)

Converts the exclusive end date by subtracting one day:

```javascript
// 2025-01-01/2025-04-01 → [2025-01-01, 2025-03-31]
WOQL.typecast(v.interval, "xdd:dateRange", v.date_range)
```

### duration ↔ dateTimeInterval

A plain duration becomes a form-4 interval (duration only). Extracting the duration from an interval returns the duration component:

```javascript
// Wrap duration as interval
WOQL.typecast(
  literal("P3M", "xsd:duration"),
  "xdd:dateTimeInterval",
  v.interval)

// Extract duration from interval
WOQL.typecast(v.interval, "xsd:duration", v.dur)
```

---

## Constructing and Unpacking Intervals

Two WOQL predicates provide direct construction and deconstruction of `xdd:dateTimeInterval` values.

### `interval` — Half-Open Construction

Creates or unpacks an interval using half-open semantics. The end date is exclusive.

```javascript
// Construct: two dates → interval value
let v = Vars("q1");
WOQL.interval(
  literal("2025-01-01", "xsd:date"),
  literal("2025-04-01", "xsd:date"),
  v.q1)
// v.q1 = "2025-01-01/2025-04-01"^^xdd:dateTimeInterval

// Unpack: interval value → two dates
let v = Vars("start", "end");
WOQL.interval(v.start, v.end, some_interval)
// v.start = "2025-01-01"^^xsd:date
// v.end   = "2025-04-01"^^xsd:date
```

For a duration-only interval (form 4), both start and end unify with `rdf:nil`.

### `interval_inclusive` — Inclusive Construction

Creates or unpacks an interval using inclusive semantics. Internally, the inclusive end is converted to an exclusive end by adding one day.

```javascript
// Construct from inclusive dates (the reporting convention)
let v = Vars("q1");
WOQL.interval_inclusive(
  literal("2025-01-01", "xsd:date"),
  literal("2025-03-31", "xsd:date"),  // inclusive end
  v.q1)
// v.q1 = "2025-01-01/2025-04-01"^^xdd:dateTimeInterval
// (March 31 + 1 day = April 1 stored as exclusive end)

// Unpack to inclusive dates for display
let v = Vars("start", "incl_end");
WOQL.interval_inclusive(v.start, v.incl_end, some_interval)
// v.start    = "2025-01-01"^^xsd:date
// v.incl_end = "2025-03-31"^^xsd:date
```

---

## Allen's Interval Algebra — The 13 Relations

James F. Allen introduced his interval algebra in 1983 as a framework for temporal reasoning. Given any two proper intervals (where start < end), exactly one of 13 relations holds between them. This makes the algebra **exhaustive** (one always applies) and **mutually exclusive** (only one applies).

### The relations

The 13 relations form 6 pairs of inverses plus `equals`:

{% table %}
* Relation
* Inverse
* X condition (half-open)
* Description
---
* `before`
* `after`
* X.end < Y.start
* X finishes before Y starts (with a gap)
---
* `meets`
* `met_by`
* X.end = Y.start
* X finishes exactly where Y starts (no gap, no overlap)
---
* `overlaps`
* `overlapped_by`
* X.start < Y.start, X.end > Y.start, X.end < Y.end
* X starts first and partially overlaps Y
---
* `starts`
* `started_by`
* X.start = Y.start, X.end < Y.end
* X starts at the same time as Y but ends earlier
---
* `during`
* `contains`
* X.start > Y.start, X.end < Y.end
* X is entirely within Y
---
* `finishes`
* `finished_by`
* X.start > Y.start, X.end = Y.end
* X starts after Y but they end together
---
* `equals`
* (self-inverse)
* X.start = Y.start, X.end = Y.end
* Identical intervals
{% /table %}

### Timeline visualization

```
before:        X━━━━━━━┛         ┗━━━━━━━Y
meets:         X━━━━━━━┫━━━━━━━Y
overlaps:      X━━━━━━━╋━━━┛
                       ┗━━━━━━━Y
starts:        ┣━━━X━━━┛
               ┣━━━━━━━━━━━Y━━━┛
during:            ┣━━X━━┛
               ┣━━━━━━━━━━━Y━━━┛
finishes:              ┣━━━X━━━┫
               ┣━━━━━━━━━━━Y━━━┫
equals:        ┣━━━━━━━━━━━━━━━┫  (same interval)
```

---

## Checking All 13 Relations — Worked Examples

Each example uses the existing `interval_relation` predicate with four explicit endpoints (half-open). The `/2` arity using `xdd:dateTimeInterval` values will be available after the interval type is implemented.

### TC-AIR-01: `before` — Q1 ends before Q3 starts {#tc-air-01}

```javascript
// Q1 [Jan 1, Apr 1) is before Q3 [Jul 1, Oct 1) — there's a gap (Q2)
WOQL.interval_relation("before",
  literal("2025-01-01", "xsd:date"), literal("2025-04-01", "xsd:date"),
  literal("2025-07-01", "xsd:date"), literal("2025-10-01", "xsd:date"))
// Succeeds
```

### TC-AIR-02: `after` — Q3 is after Q1 {#tc-air-02}

```javascript
// Q3 [Jul 1, Oct 1) is after Q1 [Jan 1, Apr 1)
WOQL.interval_relation("after",
  literal("2025-07-01", "xsd:date"), literal("2025-10-01", "xsd:date"),
  literal("2025-01-01", "xsd:date"), literal("2025-04-01", "xsd:date"))
// Succeeds — "after" is the inverse of "before"
```

### TC-AIR-03: `meets` — Q1 meets Q2 {#tc-air-03}

```javascript
// Q1 [Jan 1, Apr 1) meets Q2 [Apr 1, Jul 1) — no gap, no overlap
WOQL.interval_relation("meets",
  literal("2025-01-01", "xsd:date"), literal("2025-04-01", "xsd:date"),
  literal("2025-04-01", "xsd:date"), literal("2025-07-01", "xsd:date"))
// Succeeds — Q1's exclusive end equals Q2's start
```

### TC-AIR-04: `met_by` — Q2 is met by Q1 {#tc-air-04}

```javascript
// Q2 [Apr 1, Jul 1) is met_by Q1 [Jan 1, Apr 1)
WOQL.interval_relation("met_by",
  literal("2025-04-01", "xsd:date"), literal("2025-07-01", "xsd:date"),
  literal("2025-01-01", "xsd:date"), literal("2025-04-01", "xsd:date"))
// Succeeds — inverse of "meets"
```

### TC-AIR-05: `overlaps` — Overlapping project phases {#tc-air-05}

```javascript
// Design phase [Jan, Jun) overlaps with Development phase [Apr, Oct)
WOQL.interval_relation("overlaps",
  literal("2025-01-01", "xsd:date"), literal("2025-06-01", "xsd:date"),
  literal("2025-04-01", "xsd:date"), literal("2025-10-01", "xsd:date"))
// Succeeds — Design starts first, ends during Development
```

### TC-AIR-06: `overlapped_by` — Reverse overlap {#tc-air-06}

```javascript
// Development [Apr, Oct) is overlapped_by Design [Jan, Jun)
WOQL.interval_relation("overlapped_by",
  literal("2025-04-01", "xsd:date"), literal("2025-10-01", "xsd:date"),
  literal("2025-01-01", "xsd:date"), literal("2025-06-01", "xsd:date"))
// Succeeds — inverse of "overlaps"
```

### TC-AIR-07: `starts` — First week starts with January {#tc-air-07}

```javascript
// First week of January [Jan 1, Jan 8) starts with January [Jan 1, Feb 1]
WOQL.interval_relation("starts",
  literal("2025-01-01", "xsd:date"), literal("2025-01-08", "xsd:date"),
  literal("2025-01-01", "xsd:date"), literal("2025-02-01", "xsd:date"))
// Succeeds — same start, week ends before month ends
```

### TC-AIR-08: `started_by` — January started by first week {#tc-air-08}

```javascript
// January [Jan 1, Feb 1] is started_by the first week [Jan 1, Jan 8)
WOQL.interval_relation("started_by",
  literal("2025-01-01", "xsd:date"), literal("2025-02-01", "xsd:date"),
  literal("2025-01-01", "xsd:date"), literal("2025-01-08", "xsd:date"))
// Succeeds — inverse of "starts"
```

### TC-AIR-09: `during` — Meeting during a work day {#tc-air-09}

```javascript
// A 1-hour meeting [10:00, 11:00) during a work day [09:00, 17:00)
WOQL.interval_relation("during",
  literal("2025-03-15T10:00:00Z", "xsd:dateTime"),
  literal("2025-03-15T11:00:00Z", "xsd:dateTime"),
  literal("2025-03-15T09:00:00Z", "xsd:dateTime"),
  literal("2025-03-15T17:00:00Z", "xsd:dateTime"))
// Succeeds — meeting is entirely within the work day
```

### TC-AIR-10: `contains` — Work day contains meeting {#tc-air-10}

```javascript
// Work day [09:00, 17:00) contains the meeting [10:00, 11:00)
WOQL.interval_relation("contains",
  literal("2025-03-15T09:00:00Z", "xsd:dateTime"),
  literal("2025-03-15T17:00:00Z", "xsd:dateTime"),
  literal("2025-03-15T10:00:00Z", "xsd:dateTime"),
  literal("2025-03-15T11:00:00Z", "xsd:dateTime"))
// Succeeds — inverse of "during"
```

### TC-AIR-11: `finishes` — Last week finishes with January {#tc-air-11}

```javascript
// Last week of Jan [Jan 25, Feb 1] finishes with January [Jan 1, Feb 1]
WOQL.interval_relation("finishes",
  literal("2025-01-25", "xsd:date"), literal("2025-02-01", "xsd:date"),
  literal("2025-01-01", "xsd:date"), literal("2025-02-01", "xsd:date"))
// Succeeds — same end, week starts after month starts
```

### TC-AIR-12: `finished_by` — January finished by last week {#tc-air-12}

```javascript
// January [Jan 1, Feb 1] is finished_by last week [Jan 25, Feb 1]
WOQL.interval_relation("finished_by",
  literal("2025-01-01", "xsd:date"), literal("2025-02-01", "xsd:date"),
  literal("2025-01-25", "xsd:date"), literal("2025-02-01", "xsd:date"))
// Succeeds — inverse of "finishes"
```

### TC-AIR-13: `equals` — Same interval {#tc-air-13}

```javascript
// Two references to the same fiscal year
WOQL.interval_relation("equals",
  literal("2025-01-01", "xsd:date"), literal("2026-01-01", "xsd:date"),
  literal("2025-01-01", "xsd:date"), literal("2026-01-01", "xsd:date"))
// Succeeds — identical start and end
```

---

## Classifying an Interval Relation

When you do not know which relation holds between two intervals, leave the relation as an unbound variable. TerminusDB will determine the unique relation:

```javascript
let v = Vars("rel");
WOQL.interval_relation(v.rel,
  literal("2025-01-01", "xsd:date"), literal("2025-04-01", "xsd:date"),
  literal("2025-04-01", "xsd:date"), literal("2025-07-01", "xsd:date"))
// v.rel = "meets"
```

This is useful for auditing temporal data:

```javascript
// Classify the relationship between every pair of reporting periods
let v = Vars("p1", "p2", "s1", "e1", "s2", "e2", "rel");
WOQL.and(
  WOQL.triple(v.p1, "period_start", v.s1),
  WOQL.triple(v.p1, "period_end", v.e1),
  WOQL.triple(v.p2, "period_start", v.s2),
  WOQL.triple(v.p2, "period_end", v.e2),
  WOQL.not_equals(v.p1, v.p2),
  WOQL.interval_relation(v.rel, v.s1, v.e1, v.s2, v.e2)
)
// Returns one row per pair with the Allen relation
```

---

## Producing Intervals from Dates and Durations

### From two explicit dates (half-open)

```javascript
let v = Vars("interval");
WOQL.interval(
  literal("2025-01-01", "xsd:date"),
  literal("2025-04-01", "xsd:date"),
  v.interval)
// v.interval = "2025-01-01/2025-04-01"^^xdd:dateTimeInterval
```

### From two inclusive dates (reporting convention)

```javascript
let v = Vars("interval");
WOQL.interval_inclusive(
  literal("2025-01-01", "xsd:date"),
  literal("2025-03-31", "xsd:date"),
  v.interval)
// v.interval = "2025-01-01/2025-04-01"^^xdd:dateTimeInterval
// (internally: March 31 + 1 day = April 1)
```

### From a `dateRange` value via typecast

```javascript
let v = Vars("interval");
WOQL.typecast(
  literal("[2025-01-01, 2025-03-31]", "xdd:dateRange"),
  "xdd:dateTimeInterval",
  v.interval)
// v.interval = "2025-01-01/2025-04-01"^^xdd:dateTimeInterval
```

### From a start date and duration (form 2)

```javascript
let v = Vars("interval");
WOQL.typecast(
  literal("2025-01-01/P3M", "xsd:string"),
  "xdd:dateTimeInterval",
  v.interval)
// v.interval = "2025-01-01/P3M"^^xdd:dateTimeInterval
```

### From a duration and end date (form 3)

```javascript
let v = Vars("interval");
WOQL.typecast(
  literal("P3M/2025-04-01", "xsd:string"),
  "xdd:dateTimeInterval",
  v.interval)
// v.interval = "P3M/2025-04-01"^^xdd:dateTimeInterval
```

---

## `interval_relation_typed` — Allen's on Interval Values {#interval-relation-typed}

The `interval_relation` predicate requires four separate endpoint arguments. When your intervals are already stored or constructed as `xdd:dateTimeInterval` values, `interval_relation_typed` is more convenient — it takes the interval values directly.

| Predicate | Signature | Description |
|-----------|-----------|-------------|
| `interval_relation_typed` | `interval_relation_typed(Rel, X, Y)` | Allen's algebra on two `xdd:dateTimeInterval` values |

All 13 Allen relations work identically. The predicate internally decomposes each interval into start/end endpoints and delegates to the same logic as `interval_relation`.

### Example: Typed Validation — Q1 meets Q2

```javascript
WOQL.interval_relation_typed(
  literal("meets", "xsd:string"),
  literal("2025-01-01/2025-04-01", "xdd:dateTimeInterval"),
  literal("2025-04-01/2025-07-01", "xdd:dateTimeInterval"))
// Succeeds
```

### Example: Typed Classification

```javascript
let v = Vars("rel");
WOQL.interval_relation_typed(v.rel,
  literal("2025-01-01/2025-04-01", "xdd:dateTimeInterval"),
  literal("2025-04-01/2025-07-01", "xdd:dateTimeInterval"))
// v.rel = "meets"
```

### Example: DateTime Intervals (Sub-Day Precision)

```javascript
let v = Vars("rel");
WOQL.interval_relation_typed(v.rel,
  literal("2025-03-15T08:00:00Z/2025-03-15T12:00:00Z", "xdd:dateTimeInterval"),
  literal("2025-03-15T12:00:00Z/2025-03-15T17:00:00Z", "xdd:dateTimeInterval"))
// v.rel = "meets" — morning shift meets afternoon shift
```

### Python Example

```python
from terminusdb_client.woqlquery import WOQLQuery as WOQL

q1 = {"@type": "xdd:dateTimeInterval", "@value": "2025-01-01/2025-04-01"}
q2 = {"@type": "xdd:dateTimeInterval", "@value": "2025-04-01/2025-07-01"}

result = client.query(WOQL().interval_relation_typed("meets", q1, q2))
assert len(result["bindings"]) == 1
```

### When to Use Which Predicate

| Scenario | Predicate | Why |
|----------|-----------|-----|
| Endpoints already in separate variables | `interval_relation(rel, xs, xe, ys, ye)` | No need to construct intervals first |
| Intervals stored as `xdd:dateTimeInterval` | `interval_relation_typed(rel, x, y)` | Cleaner — no unpacking needed |
| Intervals constructed from inclusive dates | `interval_relation_typed(rel, x, y)` | Pair with `interval_inclusive` |

---

## Complete Workflow — Financial Quarterly Reporting

This example demonstrates the full cycle: reporting periods expressed as inclusive date ranges, converted to half-open intervals for Allen's algebra, then converted back for display. It uses `interval_relation_typed` for the temporal checks.

```javascript
let v = Vars("q1", "q2", "q3", "q4", "rel_q1_q2",
             "q1_start", "q1_end_incl", "q2_start", "q2_end_incl");

WOQL.and(
  // Step 1: Construct intervals from inclusive reporting dates
  WOQL.interval_inclusive(
    literal("2025-01-01", "xsd:date"),
    literal("2025-03-31", "xsd:date"), v.q1),
  WOQL.interval_inclusive(
    literal("2025-04-01", "xsd:date"),
    literal("2025-06-30", "xsd:date"), v.q2),
  WOQL.interval_inclusive(
    literal("2025-07-01", "xsd:date"),
    literal("2025-09-30", "xsd:date"), v.q3),
  WOQL.interval_inclusive(
    literal("2025-10-01", "xsd:date"),
    literal("2025-12-31", "xsd:date"), v.q4),

  // Step 2: Verify temporal relationships using typed intervals
  WOQL.interval_relation_typed("meets", v.q1, v.q2),   // Q1 meets Q2
  WOQL.interval_relation_typed("meets", v.q2, v.q3),   // Q2 meets Q3
  WOQL.interval_relation_typed("meets", v.q3, v.q4),   // Q3 meets Q4
  WOQL.interval_relation_typed("before", v.q1, v.q3),  // Q1 is before Q3

  // Step 3: Classify the relationship between Q1 and Q2
  WOQL.interval_relation_typed(v.rel_q1_q2, v.q1, v.q2),
  // v.rel_q1_q2 = "meets"

  // Step 4: Unpack back to inclusive dates for display
  WOQL.interval_inclusive(v.q1_start, v.q1_end_incl, v.q1),
  WOQL.interval_inclusive(v.q2_start, v.q2_end_incl, v.q2)
  // v.q1_start = "2025-01-01", v.q1_end_incl = "2025-03-31"
  // v.q2_start = "2025-04-01", v.q2_end_incl = "2025-06-30"
)
```

The key insight: inclusive dates go in, Allen's algebra verifies the temporal structure, and inclusive dates come back out. The half-open conversion happens transparently inside the interval type.
