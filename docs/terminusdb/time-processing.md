---
tags:
  - woql
  - reference
  - time-processing
title: "ISO 8601 Time Processing in TerminusDB"
nextjs:
  metadata:
    title: "ISO 8601 Time Processing in TerminusDB"
    keywords: iso8601 time date datetime duration interval allen temporal woql financial reporting xbrl as-of half-open eom sequence range_min range_max triple_slice
    description: Learn to process dates, times, durations, and intervals accurately in TerminusDB. Four progressive tutorials cover everything from basic date comparison to creative temporal pattern-solving for financial reporting and scheduling.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/time-processing/
media: []
---

Time processing is one of the most error-prone areas in software engineering. A one-day boundary error in a financial reporting system can misclassify an entire quarter of transactions. A naive month-addition that ignores end-of-month rules can silently shift payment schedules. A missing timezone normalization can duplicate or skip hours around DST transitions.

TerminusDB provides a comprehensive set of WOQL predicates for working with ISO 8601 dates, times, durations, and intervals. More importantly, because WOQL is a logic language, its temporal predicates work bidirectionally — the same predicate that *computes* a date can also *validate* one, and the same predicate that *generates* a sequence can *match* against it. This dual nature is what makes WOQL uniquely powerful for temporal reasoning.

This section contains four progressive tutorials, a set of reference pages, and a collection of creative patterns for solving real business problems with temporal logic.

---

## Getting Started

Every tutorial uses the **WOQL Playground** — an interactive editor embedded in each page that sends queries directly to your local TerminusDB instance. You can edit the code, run it, and see results immediately.

### Setup

**Option 1: Local TerminusDB (Recommended)**

1. **Start TerminusDB** using Docker:
   ```bash
   docker run --rm -p 6363:6363 terminusdb/terminusdb-server
   ```

2. **That's it!** The playgrounds are pre-configured to use the `_system` database, which is always available. No database creation needed for time predicates.

3. **Optional**: Click **Settings** on any playground to verify the connection:
   - Server URL: `http://localhost:6363`
   - Organization: `admin`
   - Database: `_system`
   - User: `admin`
   - Password: `root`

**Option 2: DFRNT Cloud**

1. **Get your API key** from [dfrnt.com](https://dfrnt.com) after selecting your team

2. **Configure the playground**: Click **Settings** and enter:
   - Server URL: `https://dfrnt.com/api/hosted/YOUR_TEAM`
   - Organization: Your team name
   - Database: `_system` (or any existing database)
   - User: Your email
   - Password: Your API key

**Note:** Time predicates are pure computations that work against the `_system` database. You only need to create a custom database when storing actual data (covered in tutorials that use `triple` or document operations).

---

## Tutorials

Work through these in order. Each builds on concepts from the previous one.

### [Tutorial 1: Dates, Comparisons & Range Queries](/docs/time-tutorial-dates)

**You will learn:** How TerminusDB represents dates and times, how to compare them, how to filter date ranges efficiently, and when to use `triple_slice` for O(log n) lookups vs computed range predicates.

**Topics covered:**
- XSD date and time types (`xsd:date`, `xsd:dateTime`, `xsd:dateTimeStamp`, `xsd:time`)
- Granular date types (`xsd:gYear`, `xsd:gYearMonth`, `xsd:gMonth`, `xsd:gMonthDay`, `xsd:gDay`)
- Strict comparison: `less`, `greater` (and aliases `lt`, `gt`)
- Non-strict comparison: `lte`, `gte`
- Half-open range filtering: `in_range`
- Storage-level range queries: `triple_slice`
- "As of" date queries vs period queries
- Timezone handling: when offsets matter and when they don't

**Prerequisite:** Basic WOQL familiarity ([WOQL Getting Started](/docs/woql-getting-started))

### [Tutorial 2: Durations, Month Arithmetic & Sequences](/docs/time-tutorial-durations)

**You will learn:** How durations work in ISO 8601, why month-based durations are different from day-based durations, how end-of-month preservation works, and how to generate date sequences for reporting calendars.

**Topics covered:**
- Duration types: `xsd:duration`, `xsd:dayTimeDuration`, `xsd:yearMonthDuration`
- Duration arithmetic: `date_duration` (tri-directional: start ↔ end ↔ duration)
- Why `P90D` and `P3M` are fundamentally different
- End-of-month (EOM) preservation rules
- Non-reversibility of month arithmetic
- Sequence generation: `sequence` for integers, decimals, dates, gYears, gYearMonths
- Month boundary predicates: `month_start_date`, `month_end_date`, `month_start_dates`, `month_end_dates`
- Date navigation: `day_before`, `day_after`
- Calendar metadata: `weekday`, `weekday_sunday_start`, `iso_week`
- Edge-of-range helpers: `range_min`, `range_max`

**Prerequisite:** Tutorial 1

### [Tutorial 3: Intervals & Allen's Temporal Algebra](/docs/time-tutorial-intervals)

**You will learn:** What temporal intervals are, why half-open intervals are essential for correct computation, how to construct and deconstruct `xdd:dateTimeInterval` values, and how Allen's 13 interval relations let you reason precisely about how periods relate to each other.

**Topics covered:**
- The `xdd:dateTimeInterval` type and its four ISO 8601 forms
- Inclusive vs half-open: the conversion problem
- `interval` and `interval_inclusive` — construction and deconstruction
- `interval_start_duration` and `interval_duration_end`
- Typecasting between `xdd:dateRange`, `xdd:dateTimeInterval`, and `xsd:duration`
- Allen's Interval Algebra: the 13 relations and why exactly one always holds
- `interval_relation` — 5-argument form with explicit endpoints
- `interval_relation_typed` — 3-argument form with interval values
- Validation mode (ground relation) vs classification mode (unbound relation)
- Partitioning verification: proving quarters tile a fiscal year

**Prerequisite:** Tutorial 2

### [Tutorial 4: Creative Temporal Patterns](/docs/time-tutorial-patterns)

**You will learn:** How to exploit WOQL's logic-programming nature — unification, backtracking, generators, and matchers — to solve temporal business problems that would require complex procedural code in other systems.

**Topics covered:**
- The three modes of a WOQL predicate: compute, validate, classify
- Generators: producing all dates, weeks, months in a range
- Matchers: testing whether a value belongs to a generated set
- Unification: extracting unknowns from partially-known temporal relationships
- Composing generators with filters for complex calendar logic
- Business day calendars using `weekday` + `sequence`
- Rolling window analytics with `interval_relation_typed`
- Fiscal calendar construction (quarters, tertials, custom periods)
- Audit trail temporal integrity verification
- Multi-subsidiary filing deadline coordination with `range_min`/`range_max`
- Scheduling constraint satisfaction with Allen's relations
- Period gap and overlap detection

**Prerequisite:** Tutorial 3

---

## Reference Pages

These pages provide exhaustive predicate-by-predicate documentation. Use them alongside the tutorials.

- **[WOQL Time & Date Handling](/docs/woql-time-handling)** — Complete predicate reference with financial examples and a worked quarterly-close workflow
- **[Allen's Interval Algebra](/docs/woql-interval-algebra)** — All 13 relations with timeline diagrams, the `xdd:dateTimeInterval` type, typecasting, and `interval_relation_typed`
- **[EOM Preservation Rules](/docs/woql-eom-rules)** — The complete end-of-month rule set with addition/subtraction tables and financial use cases
- **[Range Queries with triple_slice](/docs/woql-triple-slice)** — O(log n) storage-level range queries for timestamps and ordered values
- **[Numeric Precision](/docs/numeric-precision-reference)** — Arbitrary-precision decimals for financial calculations without floating-point drift
- **[Data Types Reference](/docs/data-types)** — Full type catalog including `xdd:dateTimeInterval` and `xdd:dateRange`

---

## Quick Reference: Which Predicate for Which Task

| I want to... | Use this | Tutorial |
|---|---|---|
| Compare two dates | `less`, `greater`, `lte`, `gte` | [1](/docs/time-tutorial-dates) |
| Check if a date is in a range | `in_range(value, start, end)` | [1](/docs/time-tutorial-dates) |
| Query stored dates by range (fast) | `triple_slice(doc, prop, val, lo, hi)` | [1](/docs/time-tutorial-dates) |
| Add days to a date | `date_duration(start, result, "P30D")` | [2](/docs/time-tutorial-durations) |
| Add months with EOM rules | `date_duration(start, result, "P3M")` | [2](/docs/time-tutorial-durations) |
| Compute days between two dates | `date_duration(start, end, duration)` | [2](/docs/time-tutorial-durations) |
| Generate every date in a range | `sequence(date, start, end)` | [2](/docs/time-tutorial-durations) |
| Generate monthly dates | `sequence(ym, "2025-01", "2026-01")` | [2](/docs/time-tutorial-durations) |
| Get month-end dates | `month_end_dates(date, start, end)` | [2](/docs/time-tutorial-durations) |
| Get day-of-week | `weekday(date, dow)` | [2](/docs/time-tutorial-durations) |
| Find min/max in a list | `range_min(list, min)` | [2](/docs/time-tutorial-durations) |
| Construct a half-open interval | `interval(start, end, iv)` | [3](/docs/time-tutorial-intervals) |
| Construct from inclusive dates | `interval_inclusive(start, end, iv)` | [3](/docs/time-tutorial-intervals) |
| Check if intervals are adjacent | `interval_relation_typed("meets", a, b)` | [3](/docs/time-tutorial-intervals) |
| Classify the relation between periods | `interval_relation_typed(rel, a, b)` | [3](/docs/time-tutorial-intervals) |
| Detect overlapping periods | `interval_relation_typed("overlaps", a, b)` | [3](/docs/time-tutorial-intervals) |
| Generate all business days in Q1 | `sequence` + `weekday` + `lte` | [4](/docs/time-tutorial-patterns) |
| Verify quarters tile a fiscal year | `interval_relation_typed` chain | [4](/docs/time-tutorial-patterns) |
| Find filing deadline from as-of date | `date_duration(as_of, deadline, "P60D")` | [4](/docs/time-tutorial-patterns) |
