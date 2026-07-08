---
tags:
  - woql
  - reference
  - time-processing
title: WOQL ISO8601 Time and Date Handling
nextjs:
  metadata:
    title: WOQL ISO8601 Time and Date Handling
    keywords: datalog, iso8601, prolog, date, time, datetime, duration, sequence, range, comparison, financial reporting, xbrl, interval, allen, as-of, eom, half-open, inclusive, interval_relation_typed, range_min, range_max
    description: Complete guide to WOQL date, time, duration, and interval handling — ISO 8601 types, comparators, sequences, Allen's Interval Algebra, and financial EOM rules.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-time-handling/
media: []
---

Time is complicated. Especially in finance, where a one-day error in a reporting period can trigger regulatory findings, and where "March 31" means something different depending on whether you are displaying a period label or computing a duration.

This page teaches you to think about time correctly, then shows you every WOQL predicate that puts that thinking into practice.

> **See also:** [Allen's Interval Algebra](/docs/woql-interval-algebra/) | [EOM Preservation Rules](/docs/woql-eom-rules/) | [WOQL Class Reference](/docs/woql-class-reference-guide/) | [Range queries with triple_slice](/docs/woql-triple-slice/)

---

## Before You Write a Single Query: The Four Temporal Concepts

Most time-related bugs come from conflating four distinct concepts. Before learning any predicate, internalize these distinctions — they will save you from entire categories of error.

### 1. Instants — "When did it happen?"

An instant is a single point on the timeline. A filing timestamp. A trade execution time. A regulatory deadline.

```
2025-03-31T16:00:00Z      ← a specific moment
2025-03-31                 ← a specific calendar date (implicitly the whole day)
```

In TerminusDB, instants are `xsd:date`, `xsd:dateTime`, or `xsd:dateTimeStamp` values. They answer the question *when*.

### 2. Durations — "How long?"

A duration is a length of time with no anchor to the calendar. "90 days" is a duration. "3 months" is a duration. Neither tells you when something starts or ends.

```
P90D       ← 90 days
P3M        ← 3 months (but 3 months from January 31 is not the same as 3 months from February 28)
PT8H30M    ← 8 hours and 30 minutes
```

In TerminusDB, durations are `xsd:duration` values. They answer the question *how long*.

**The critical distinction**: day-based durations (`P90D`) are unambiguous — 90 days is always 90 days. Month-based durations (`P3M`) depend on the starting date because months have different lengths. This is where [EOM preservation rules](/docs/woql-eom-rules/) become essential.

### 3. Intervals — "From when to when?"

An interval is a duration anchored to specific dates. "Q1 2025" is an interval: it starts January 1 and ends April 1 (exclusive). An interval has a definite start, a definite end, and a computable duration.

```
2025-01-01/2025-04-01      ← Q1 2025 as a half-open interval
```

In TerminusDB, intervals are `xdd:dateTimeInterval` values. They answer *from when to when* and *how long* simultaneously.

### 4. Relations — "How do two intervals relate?"

Given two intervals, you can ask: does one come before the other? Do they overlap? Does one contain the other? Allen's Interval Algebra gives you exactly 13 possible answers, and exactly one of them is always correct.

```
Q1 meets Q2     ← Q1 ends exactly where Q2 starts
Q2 during FY    ← Q2 is entirely contained within the fiscal year
```

In TerminusDB, relations are computed by `interval_relation` and `interval_relation_typed`. They answer *how do these periods relate*.

### Why this matters for financial systems

Financial reporting systems routinely need all four concepts working together:

- **"What is the balance as of March 31, 2025?"** — an instant query
- **"How many days in Q1?"** — a duration computation
- **"Show me Q1 transactions"** — an interval filter
- **"Does the audit period overlap the reporting period?"** — a relation check

Get any one of these wrong — treat a duration as an interval, or an inclusive date as a half-open boundary — and your numbers are off by a day, a transaction, or an entire period.

---

## The Display vs Calculation Problem

This is the single most common source of temporal bugs in financial systems: **the dates you display to users are not the dates you use for calculation**.

### How humans talk about periods

In every annual report, every regulatory filing, every board presentation, periods are expressed with **inclusive** endpoints:

> "Q1 2025: January 1 – March 31"
> "Fiscal Year 2025: January 1 – December 31"

Both dates are part of the period. March 31 is the last day of Q1. December 31 is the last day of the year.

### How computers must calculate with periods

For computation — duration arithmetic, gap detection, partition verification, Allen's algebra — you need **half-open** intervals where the end is exclusive:

> Q1 2025: [January 1, April 1]
> Fiscal Year 2025: [January 1, January 1 next year]

Why? Because half-open intervals give you three properties that inclusive ranges do not:

1. **No gaps**: Q1 = [Jan 1, Apr 1]. Q2 = [Apr 1, Jul 1]. There is no missing date between them.
2. **No overlaps**: March 31 belongs to Q1 only. April 1 belongs to Q2 only.
3. **Clean partition**: The fiscal year [Jan 1, Jan 1 next year] divides into exactly 4 quarters with no gaps or overlaps.

With inclusive ranges, Q1 = [Jan 1, Mar 31] and Q2 = [Apr 1, Jun 30] — and you have to constantly ask "is this boundary inclusive or exclusive?" every time you compare periods.

### The TerminusDB pattern: inclusive in, half-open inside, inclusive out

TerminusDB's `interval_inclusive` predicate handles the conversion transparently:

```javascript
// Step 1: User provides inclusive reporting dates
WOQL.interval_inclusive(
  literal("2025-01-01", "xsd:date"),
  literal("2025-03-31", "xsd:date"),   // inclusive end — what the user sees
  v.q1)
// Internally stored as: 2025-01-01/2025-04-01 (half-open)

// Step 2: Allen's algebra operates on the half-open representation
WOQL.interval_relation_typed("meets", v.q1, v.q2)  // correct!

// Step 3: Unpack back to inclusive dates for display
WOQL.interval_inclusive(v.start, v.end_display, v.q1)
// v.end_display = "2025-03-31" — what the user sees again
```

**Rule of thumb**: accept inclusive dates from users, convert once, compute in half-open, convert back for display. Never mix conventions in the same computation.

### Duration display vs duration calculation

The same display-vs-calculation split applies to durations:

| What the user sees | What the system computes | Why they differ |
|---|---|---|
| "Q1: 90 days" | `date_duration("2025-01-01", "2025-04-01", D)` → `P90D` | The *display* says "Jan 1 to Mar 31 is 90 days" but the *calculation* needs the half-open end (Apr 1) to get 90 |
| "Period: 3 months" | `P3M` added to Jan 31 → Feb 28 (not Feb 31) | Month durations require [EOM rules](/docs/woql-eom-rules/) |
| "Filing deadline: 60 days after period end" | `date_duration("2025-03-31", v.deadline, "P60D")` | The deadline calculation uses the inclusive end as a *starting point*, not as a boundary |

### "As of" dates in financial reporting

An "as of" date (also called a reporting date or valuation date) is an instant that anchors all queries for a reporting period. In XBRL and SEC filings:

- **"Balance sheet as of December 31, 2024"** means: show the state at the end of that calendar day
- **"Income statement for the period January 1 – December 31, 2024"** means: aggregate all transactions in that interval

The "as of" date is the inclusive end of the period. To query it correctly:

```javascript
// "As of March 31, 2025" means all transactions up to and including March 31
let v = Vars("doc", "date", "amount");
WOQL.and(
  WOQL.triple(v.doc, "transaction_date", v.date),
  WOQL.triple(v.doc, "amount", v.amount),
  WOQL.lte(v.date, literal("2025-03-31", "xsd:date"))
)

// But for period queries, use the half-open interval:
// "Transactions in Q1 2025" = [Jan 1, Apr 1]
WOQL.and(
  WOQL.triple(v.doc, "transaction_date", v.date),
  WOQL.in_range(v.date,
    literal("2025-01-01", "xsd:date"),
    literal("2025-04-01", "xsd:date"))  // exclusive end — April 1 excluded
)
```

The "as of" pattern uses `lte` (inclusive). The "during period" pattern uses `in_range` (half-open). Mixing them up loses or gains one day of transactions.

---

## XSD Date and Time Types

TerminusDB supports the full set of XML Schema date and time types. Each has a specific use case — choosing the right type matters for correct arithmetic and comparison.

| Type | Format | Example | Use case |
|------|--------|---------|----------|
| `xsd:date` | `YYYY-MM-DD` | `"2024-03-15"` | Calendar dates (reporting periods, filing dates) |
| `xsd:dateTime` | `YYYY-MM-DDThh:mm:ssZ` | `"2024-03-15T14:30:00Z"` | Timestamps (trade times, audit trails) |
| `xsd:dateTimeStamp` | `YYYY-MM-DDThh:mm:ss±hh:mm` | `"2024-03-15T14:30:00+01:00"` | Timestamps with mandatory timezone |
| `xsd:time` | `hh:mm:ss` | `"14:30:00"` | Time of day without date |
| `xsd:duration` | `PnYnMnDTnHnMnS` | `"P1Y2M3DT4H"` | Durations (offsets, deadlines) |
| `xdd:dateTimeInterval` | ISO 8601 interval | `"2025-01-01/2025-04-01"` | Half-open intervals for Allen's algebra |
| `xsd:dayTimeDuration` | `PnDTnHnMnS` | `"P60DT12H"` | Day-and-time-only durations |
| `xsd:yearMonthDuration` | `PnYnM` | `"P1Y6M"` | Year-and-month-only durations |
| `xsd:gYear` | `YYYY` | `"2024"` | Calendar years (fiscal years) |
| `xsd:gYearMonth` | `YYYY-MM` | `"2024-03"` | Year-month pairs (reporting months) |
| `xsd:gMonth` | `--MM` | `"--03"` | Month without year |
| `xsd:gMonthDay` | `--MM-DD` | `"--03-15"` | Month-day without year |
| `xsd:gDay` | `---DD` | `"---15"` | Day of month without year or month |

**Timezone offsets** are optional on `xsd:date` and `xsd:dateTime`. When present, they are preserved through arithmetic but do not affect calendar date calculations (see [Timezone Handling](#timezone-handling)).

## Existing Time Predicates

These predicates are already available in WOQL today.

### `timestamp_now` — Current Time

Returns the current system time as a decimal Unix timestamp.

```javascript
let v = Vars("now");
timestamp_now(v.now)
// v.now binds to something like 1711024200.123
```

### `less` and `greater` — Strict Comparison

Compare two values using strict less-than or greater-than. Works on all ordered types including dates, datetimes, numbers, and strings.

```javascript
let v = Vars("doc", "date");
and(
  triple(v.doc, "filing_date", v.date),
  greater(v.date, literal("2024-01-01", "xsd:date"))
)
```

### `triple_slice` — Range Queries on Stored Values

For efficient range queries on data already stored in the database, `triple_slice` pushes range constraints into the storage engine for O(log n) lookups. See the [dedicated triple_slice guide](/docs/woql-triple-slice/) for full details.

```javascript
let v = Vars("doc", "time");
triple_slice(v.doc, "timestamp", v.time,
  "2025-01-01T00:00:00Z", "2025-02-01T00:00:00Z")
```

The predicates below extend WOQL with new capabilities for computed date sequences, comparisons, containment checks, and calendar arithmetic.

---

## Comparators: `gte`, `lte` + Aliases `gt`, `lt`

WOQL has strict `less` (`<`) and `greater` (`>`), but many operations — especially half-open ranges — need greater-than-or-equal and less-than-or-equal.

| Predicate | Signature | Description |
|-----------|-----------|-------------|
| `gte` | `gte(Left, Right)` | Succeeds if Left >= Right |
| `lte` | `lte(Left, Right)` | Succeeds if Left <= Right |
| `gt` | `gt(Left, Right)` | Alias for `greater` (Left > Right) |
| `lt` | `lt(Left, Right)` | Alias for `less` (Left < Right) |

All comparators work on every ordered type: integers, decimals, dates, datetimes, gYears, and strings.

### Example: Half-Open Range Check

```javascript
// All dates in fiscal year 2024: [2024-01-01, 2025-01-01]
let v = Vars("doc", "date");
and(
  triple(v.doc, "filing_date", v.date),
  gte(v.date, literal("2024-01-01", "xsd:date")),
  lt(v.date, literal("2025-01-01", "xsd:date"))
)
```

### Example: Boundary Equality

```javascript
// gte succeeds when values are equal
gte(literal("2024-02-29", "xsd:date"), literal("2024-02-29", "xsd:date"))
// Succeeds — leap day equals itself
```

### Example: Decimal Comparison

```javascript
lte(literal("0.1", "xsd:decimal"), literal("0.2", "xsd:decimal"))
// Succeeds — exact rational arithmetic, no floating-point issues
```

---

## Range Containment: `in_range`

A general-purpose half-open range check. Equivalent to `gte(Value, Start), lt(Value, End)` but more readable and intentional.

```
in_range(Value, Start, End)
```

**Semantics**: Succeeds if `Start <= Value < End` (half-open `[Start, End)`).

All three arguments must be ground. This is a filter, not a generator — use `sequence` to generate values.

### Example: Is a Filing Date Within the Reporting Period?

```javascript
let v = Vars("filing", "date", "start", "end");
and(
  triple(v.filing, "filing_date", v.date),
  triple(v.filing, "period_start", v.start),
  triple(v.filing, "period_end", v.end),
  in_range(v.date, v.start, v.end)
)
```

### Example: Boundary Behavior — Start Inclusive, End Exclusive

```javascript
in_range(literal(5, "xsd:integer"), literal(5, "xsd:integer"), literal(10, "xsd:integer"))
// Succeeds — value equals start (inclusive)

in_range(literal(10, "xsd:integer"), literal(5, "xsd:integer"), literal(10, "xsd:integer"))
// Fails — value equals end (exclusive)
```

### Example: Adjacent Ranges Partition Cleanl

```javascript
// These two ranges cover [5, 15) with no overlap and no gap:
in_range(v.x, literal(5, "xsd:integer"), literal(10, "xsd:integer"))   // [5, 10)
in_range(v.x, literal(10, "xsd:integer"), literal(15, "xsd:integer"))  // [10, 15)
// Value 10 appears only in the second range
```

### Example: Leap Day Within February

```javascript
in_range(
  literal("2024-02-29", "xsd:date"),
  literal("2024-02-01", "xsd:date"),
  literal("2024-03-01", "xsd:date")
)
// Succeeds — Feb 29 is within February in a leap year
```

---

## Sequence Generation: `sequence`

Generates a sequence of values in a half-open range `[Start, End)`. Works as both a **generator** (Value unbound → produces each value) and a **matcher** (Value ground → succeeds if value is in the sequence).

```
sequence(Value, Start, End)
sequence(Value, Start, End, Step)
sequence(Value, Start, End, Step, Count)
```

**Parameters:**
- **Value** — generated/matched value. Type inferred from Start/End.
- **Start** — first value (inclusive). Must be ground.
- **End** — exclusive upper bound. Must be ground.
- **Step** — increment per iteration (optional; type-specific default).
- **Count** — total values in the sequence (optional; validates or unifies).

### Supported Types and Default Steps

| Value type | Default step | Step type | Example |
|------------|-------------|-----------|---------|
| `xsd:integer` | `+1` | `xsd:integer` | `sequence(v.i, 1, 11)` → 1–10 |
| `xsd:decimal` | `+1.0` | `xsd:decimal` | `sequence(v.d, 0.0, 1.0, 0.1)` → 0.0, 0.1, ..., 0.9 |
| `xsd:gYear` | `+1 year` | `xsd:integer` | `sequence(v.y, "2020", "2025")` → 2020–2024 |
| `xsd:gYearMonth` | `+1 month` | `xsd:integer` | `sequence(v.ym, "2024-01", "2025-01")` → 12 months |
| `xsd:date` | `+1 day` | `xsd:duration` | `sequence(v.d, "2024-01-01", "2024-01-08")` → 7 dates |
| `xsd:dateTime` | `+1 second` | `xsd:duration` | `sequence(v.dt, start, end, "PT1H")` → hourly |

Unsupported types (e.g., `xsd:string`) produce an error.

### Example: Every Month in FY2024

```javascript
let v = Vars("month");
sequence(v.month,
  literal("2024-01", "xsd:gYearMonth"),
  literal("2025-01", "xsd:gYearMonth"))
// Produces 12 bindings: 2024-01, 2024-02, ..., 2024-12
```

### Example: Integer Range for Scenario Indice

```javascript
let v = Vars("id");
sequence(v.id, 1, 1001)
// Produces 1, 2, 3, ..., 1000
```

### Example: Weekly Dates with Count

```javascript
let v = Vars("date", "count");
sequence(v.date,
  literal("2024-01-01", "xsd:date"),
  literal("2025-01-01", "xsd:date"),
  literal(7, "xsd:integer"),
  v.count)
// Produces weekly dates; v.count unifies with actual count (52 or 53)
```

### Example: Decimal Steps — No Floating-Point Drif

```javascript
let v = Vars("rate");
sequence(v.rate,
  literal("0.0", "xsd:decimal"),
  literal("1.0", "xsd:decimal"),
  literal("0.3", "xsd:decimal"))
// Produces exactly: 0.0, 0.3, 0.6, 0.9
// Uses rational arithmetic — no accumulation error
```

### Example: Date Sequence Crossing Leap Year February

```javascript
let v = Vars("date");
sequence(v.date,
  literal("2024-02-27", "xsd:date"),
  literal("2024-03-02", "xsd:date"))
// Produces: 2024-02-27, 2024-02-28, 2024-02-29, 2024-03-01
// (4 dates — includes leap day)
```

### Example: Empty Range Produces No Result

```javascript
let v = Vars("i");
sequence(v.i, 5, 5)
// Produces zero results — Start == End means empty range
```

### Example: Year-Month Rollover

```javascript
let v = Vars("ym");
sequence(v.ym,
  literal("2024-11", "xsd:gYearMonth"),
  literal("2025-02", "xsd:gYearMonth"))
// Produces: 2024-11, 2024-12, 2025-01 — crosses year boundary
```

---

## Month Boundary Predicates

Financial reporting revolves around month boundaries — period starts, period ends, accrual dates, settlement dates.

| Predicate | Signature | Description |
|-----------|-----------|-------------|
| `month_start_date` | `month_start_date(YearMonth, Date)` | First day of the month. YearMonth must be ground. |
| `month_end_date` | `month_end_date(YearMonth, Date)` | Last day of the month. Handles leap years. |
| `month_start_dates` | `month_start_dates(Date, Start, End)` | Generator: every first-of-month in `[Start, End)`. |
| `month_end_dates` | `month_end_dates(Date, Start, End)` | Generator: every last-of-month in `[Start, End)`. |

These predicates are **not reversible**. Given a Date that is not a month boundary, the predicate fails.

### Example: All Month-End Dates in FY2024

```javascript
let v = Vars("month_end");
month_end_dates(v.month_end,
  literal("2024-01-01", "xsd:date"),
  literal("2025-01-01", "xsd:date"))
// Produces 12 dates: 2024-01-31, 2024-02-29, ..., 2024-12-31
```

### Example: Leap Year February End

```javascript
let v = Vars("date");
month_end_date(literal("2024-02", "xsd:gYearMonth"), v.date)
// v.date = 2024-02-29

month_end_date(literal("2023-02", "xsd:gYearMonth"), v.date)
// v.date = 2023-02-28
```

### Example: Century Leap Year Rules

```javascript
month_end_date(literal("2000-02", "xsd:gYearMonth"), v.date)
// v.date = 2000-02-29 (century divisible by 400 → leap)

month_end_date(literal("1900-02", "xsd:gYearMonth"), v.date)
// v.date = 1900-02-28 (century NOT divisible by 400 → not leap)
```

### Example: Non-Boundary Date Fails

```javascript
month_start_date(literal("2024-01", "xsd:gYearMonth"), literal("2024-01-15", "xsd:date"))
// Fails — January 15 is not the first day of the month
```

### Deriving Quarter and Tertial Boundaries

Quarters, tertials, and custom period subdivisions compose naturally from `sequence` + month boundaries:

```javascript
// All quarter-end dates in FY2024
let v = Vars("quarter_month", "quarter_end");
and(
  sequence(v.quarter_month,
    literal("2024-03", "xsd:gYearMonth"),
    literal("2025-03", "xsd:gYearMonth"),
    3),
  month_end_date(v.quarter_month, v.quarter_end)
)
// Produces: 2024-03-31, 2024-06-30, 2024-09-30, 2024-12-31
```

---

## Date Navigation: `day_before`, `day_after`

Simple relative date movement. Bidirectional — given either argument, computes the other.

| Predicate | Signature | Description |
|-----------|-----------|-------------|
| `day_before` | `day_before(Date, PreviousDate)` | Calendar day immediately before Date |
| `day_after` | `day_after(Date, NextDate)` | Calendar day immediately after Date |

### Example: Filing Date Context

```javascript
let v = Vars("filing", "date", "prior");
and(
  triple(v.filing, "filing_date", v.date),
  day_before(v.date, v.prior)
)
```

### Example: Leap Year Boundary Crossing

```javascript
day_after(literal("2024-02-28", "xsd:date"), v.next)
// v.next = 2024-02-29 (leap year — Feb has 29 days)

day_after(literal("2023-02-28", "xsd:date"), v.next)
// v.next = 2023-03-01 (non-leap year — jumps to March)
```

### Example: Year Boundary

```javascript
day_after(literal("2024-12-31", "xsd:date"), v.next)
// v.next = 2025-01-01

day_before(literal("2025-01-01", "xsd:date"), v.prev)
// v.prev = 2024-12-31
```

---

## Weekday and ISO Week Number

ISO 8601 defines Monday as day 1 and Sunday as day 7. ISO week numbers follow the ISO 8601 week-date system where week 1 is the week containing the year's first Thursday.

| Predicate | Signature | Description |
|-----------|-----------|-------------|
| `weekday` | `weekday(Date, DayNumber)` | ISO 8601: 1=Monday, 7=Sunday. Date must be ground. |
| `weekday_sunday_start` | `weekday_sunday_start(Date, DayNumber)` | US convention: 1=Sunday, 7=Saturday. |
| `iso_week` | `iso_week(Date, Year, WeekNumber)` | ISO week-numbering year and week (1–53). |

Date must be ground for all three predicates. They extract information, they do not generate dates.

### Example: Find All Wednesdays in a Range

```javascript
let v = Vars("date");
and(
  sequence(v.date,
    literal("2024-01-01", "xsd:date"),
    literal("2024-02-01", "xsd:date")),
  weekday(v.date, 3)
)
// Produces all Wednesdays in January 2024
```

### Example: Filter to Business Days

```javascript
let v = Vars("date", "dow");
and(
  sequence(v.date,
    literal("2024-01-01", "xsd:date"),
    literal("2025-01-01", "xsd:date")),
  weekday(v.date, v.dow),
  lte(v.dow, 5)  // Monday=1 through Friday=5
)
```

### Example: ISO Week Year Differs from Calendar Yea

```javascript
iso_week(literal("2023-01-01", "xsd:date"), v.year, v.week)
// v.year = 2022, v.week = 52
// January 1, 2023 is a Sunday — it belongs to ISO week 52 of 2022
```

### Example: Year with 53 ISO Weeks

```javascript
iso_week(literal("2020-12-31", "xsd:date"), v.year, v.week)
// v.year = 2020, v.week = 53
// 2020 is a year with 53 ISO weeks
```

---

## Duration Arithmetic: `time_interval`, `date_interval`

These predicates relate a start, an end, and a duration. They are **tri-directional**: given any two of the three arguments, they compute the third.

| Predicate | Signature | Description |
|-----------|-----------|-------------|
| `time_interval` | `time_interval(Start, End, Duration)` | Relates `xsd:dateTime` start/end with `xsd:duration` |
| `date_interval` | `date_interval(Start, End, Duration)` | Relates `xsd:date` start/end with `xsd:duration` |

### Convenience Aliases

| Alias | Equivalent |
|-------|-----------|
| `date_add(Date, Duration, Result)` | `date_interval(Date, Result, Duration)` |
| `date_subtract(Date, Duration, Result)` | `date_interval(Result, Date, Duration)` |
| `datetime_add(DateTime, Duration, Result)` | `time_interval(DateTime, Result, Duration)` |
| `datetime_subtract(DateTime, Duration, Result)` | `time_interval(Result, DateTime, Duration)` |

### Example: Filing Deadline — 60 Days After Period End

```javascript
let v = Vars("report", "period_end", "deadline", "filing_date");
and(
  triple(v.report, "period_end", v.period_end),
  date_add(v.period_end, literal("P60D", "xsd:duration"), v.deadline),
  triple(v.report, "filing_date", v.filing_date),
  lt(v.filing_date, v.deadline)
)
```

### Example: Compute Day-Count Duration Between Date

When both Start and End are ground, `date_interval` computes the duration as a day-count (`PnD`):

```javascript
date_interval(
  literal("2024-01-01", "xsd:date"),
  literal("2024-04-01", "xsd:date"),
  v.duration)
// v.duration = "P91D"^^xsd:duration (31 + 29 + 31 = 91 days in leap year)

date_interval(
  literal("2025-01-01", "xsd:date"),
  literal("2025-04-01", "xsd:date"),
  v.duration)
// v.duration = "P90D"^^xsd:duration (31 + 28 + 31 = 90 days in non-leap year)
```

### Example: Same Date = Zero Duration

```javascript
date_interval(
  literal("2024-06-15", "xsd:date"),
  literal("2024-06-15", "xsd:date"),
  v.duration)
// v.duration = "P0D"^^xsd:duration
```

### Example: Verify Period is Exactly 3 Month

```javascript
date_add(v.start, literal("P3M", "xsd:duration"), v.end)
// Succeeds only if end is exactly 3 months after start (using EOM rules)
```

### Financial End-of-Month (EOM) Preservation

When adding or subtracting month-based durations, TerminusDB follows financial EOM preservation rules. This ensures monthly progressions like `Jan 31 → Feb 28/29 → Mar 31` maintain logical consistency.

**Core rules:**
1. A date is "end-of-month" if its day equals the last day of its month.
2. If the start date is EOM, the result is EOM of the target month.
3. If the start date is not EOM, use the same day number, clamped to the target month's last day.

#### Addition Examples (+P1M)

| Start Date | Day Type | +P1M Result | Rule |
|------------|----------|-------------|------|
| 2020-01-28 | Normal | 2020-02-28 | Direct month increment |
| 2020-01-29 | Normal | 2020-02-29 | Direct (leap year) |
| 2020-01-30 | Normal | 2020-02-29 | Clamped to last day (leap) |
| 2020-01-31 | **EOM** | 2020-02-29 | EOM preserved |
| 2020-02-29 | **EOM** | 2020-03-31 | EOM preserved |
| 2020-03-31 | **EOM** | 2020-04-30 | EOM preserved |
| 2020-04-30 | **EOM** | 2020-05-31 | EOM preserved |
| 2020-12-31 | **EOM** | 2021-01-31 | EOM preserved (year boundary) |

#### Subtraction Examples (-P1M)

| Start Date | Day Type | -P1M Result | Rule |
|------------|----------|-------------|------|
| 2020-03-31 | **EOM** | 2020-02-29 | EOM preserved (leap) |
| 2021-03-31 | **EOM** | 2021-02-28 | EOM preserved (non-leap) |
| 2020-04-30 | **EOM** | 2020-03-31 | EOM preserved |
| 2021-01-31 | **EOM** | 2020-12-31 | EOM preserved (year boundary) |
| 2020-03-28 | Normal | 2020-02-28 | Direct month decrement |
| 2020-03-30 | Normal | 2020-02-29 | Clamped to last day (leap) |

#### Non-Reversibility of Month Durations

Month arithmetic is not a simple inverse:

```javascript
date_add(literal("2020-01-31", "xsd:date"), literal("P1M", "xsd:duration"), v.x)
// v.x = 2020-02-29

date_subtract(literal("2020-02-29", "xsd:date"), literal("P1M", "xsd:duration"), v.y)
// v.y = 2020-01-29  (NOT 2020-01-31!)
```

Day-only durations (e.g., `P60D`) do not have this issue and are fully reversible.

> The complete EOM rule set and financial rationale are documented on the [EOM Preservation Rules](/docs/woql-eom-rules/) page.

---

## Allen's Interval Algebra: `interval_relation`

Allen's Interval Algebra (1983) provides a complete framework for reasoning about temporal relationships. Given two proper intervals, exactly one of 13 relations holds. WOQL implements this with half-open `[start, end)` semantics.

| Predicate | Signature | Description |
|-----------|-----------|-------------|
| `interval_relation` | `interval_relation(Rel, Xs, Xe, Ys, Ye)` | Validates or classifies the Allen relation between two half-open intervals |

The first argument is a relation name string (one of the 13 Allen relations) or an unbound variable for classification. The 13 relations are: `before`, `after`, `meets`, `met_by`, `overlaps`, `overlapped_by`, `starts`, `started_by`, `during`, `contains`, `finishes`, `finished_by`, `equals`.

### Example: Adjacent Quarters Meet

```javascript
// Q1 [Jan 1, Apr 1] meets Q2 [Apr 1, Jul 1]
WOQL.interval_relation("meets",
  literal("2025-01-01", "xsd:date"), literal("2025-04-01", "xsd:date"),
  literal("2025-04-01", "xsd:date"), literal("2025-07-01", "xsd:date"))
// Succeeds — Q1's exclusive end equals Q2's start
```

### Example: Classify the Relationship

```javascript
let v = Vars("rel");
WOQL.interval_relation(v.rel,
  literal("2025-01-01", "xsd:date"), literal("2025-04-01", "xsd:date"),
  literal("2025-04-01", "xsd:date"), literal("2025-07-01", "xsd:date"))
// v.rel = "meets"
```

> For the complete reference with all 13 relations, timeline diagrams, and worked examples, see the [Allen's Interval Algebra guide](/docs/woql-interval-algebra/).

### `interval_relation_typed` — Allen's on Interval Values

When your intervals are already stored as `xdd:dateTimeInterval` values, use `interval_relation_typed` instead of manually unpacking endpoints:

| Predicate | Signature | Description |
|-----------|-----------|-------------|
| `interval_relation_typed` | `interval_relation_typed(Rel, X, Y)` | Allen's algebra on two `xdd:dateTimeInterval` values |

This is the predicate you will use most often in practice. It accepts interval values directly — no need to unpack start and end dates.

### Example: Verify Quarters Meet

```javascript
let v = Vars("q1", "q2");
WOQL.and(
  WOQL.interval_inclusive(
    literal("2025-01-01", "xsd:date"),
    literal("2025-03-31", "xsd:date"), v.q1),
  WOQL.interval_inclusive(
    literal("2025-04-01", "xsd:date"),
    literal("2025-06-30", "xsd:date"), v.q2),
  WOQL.interval_relation_typed("meets", v.q1, v.q2)
)
// Succeeds — Q1 meets Q2 with no gap and no overlap
```

### Example: Classify the Relationship Between Two Stored Intervals

```javascript
let v = Vars("period_a", "period_b", "rel");
WOQL.and(
  WOQL.triple(v.report_a, "period", v.period_a),
  WOQL.triple(v.report_b, "period", v.period_b),
  WOQL.interval_relation_typed(v.rel, v.period_a, v.period_b)
)
// v.rel = one of the 13 Allen relations
```

### Example: Audit Period Overlap Check

A common compliance check: does the external audit window overlap with the reporting period?

```javascript
let v = Vars("audit_interval", "reporting_interval", "rel");
WOQL.and(
  WOQL.interval_inclusive(
    literal("2025-02-15", "xsd:date"),
    literal("2025-04-15", "xsd:date"), v.audit_interval),
  WOQL.interval_inclusive(
    literal("2025-01-01", "xsd:date"),
    literal("2025-03-31", "xsd:date"), v.reporting_interval),
  WOQL.interval_relation_typed(v.rel, v.audit_interval, v.reporting_interval)
)
// v.rel = "overlapped_by" — the audit started during the reporting period
// and extends beyond it
```

### Python Example: Validate Adjacent Fiscal Quarters

```python
from terminusdb_client import Client
from terminusdb_client.woqlquery import WOQLQuery as WOQL

q1 = {"@type": "xdd:dateTimeInterval", "@value": "2025-01-01/2025-04-01"}
q2 = {"@type": "xdd:dateTimeInterval", "@value": "2025-04-01/2025-07-01"}

query = WOQL().interval_relation_typed("meets", q1, q2)
result = client.query(query)
assert len(result["bindings"]) == 1  # Q1 meets Q2
```

---

## Interval Construction: `interval`, `interval_inclusive`{#interval-construction}

These predicates construct and deconstruct `xdd:dateTimeInterval` values — the typed interval representation that works with Allen's `/2` arity methods and supports typecasting to/from `xdd:dateRange`.

| Predicate | Signature | Description |
|-----------|-----------|-------------|
| `interval` | `interval(Start, End, IntervalValue)` | Half-open: Start included, End excluded |
| `interval_inclusive` | `interval_inclusive(Start, InclusiveEnd, IntervalValue)` | Inclusive: both endpoints included (converts internally) |

Both are bidirectional — construct from dates or unpack from an interval value.

### Example: Construct from Inclusive Reporting Date

```javascript
let v = Vars("q1");
WOQL.interval_inclusive(
  literal("2025-01-01", "xsd:date"),
  literal("2025-03-31", "xsd:date"),
  v.q1)
// v.q1 = "2025-01-01/2025-04-01"^^xdd:dateTimeInterval
```

### Example: Unpack to Inclusive Dates for Displa

```javascript
let v = Vars("start", "incl_end");
WOQL.interval_inclusive(v.start, v.incl_end, some_interval)
// v.start = "2025-01-01"^^xsd:date, v.incl_end = "2025-03-31"^^xsd:date
```

### Example: Convert dateRange via Typecas

```javascript
WOQL.typecast(
  literal("[2025-01-01, 2025-03-31]", "xdd:dateRange"),
  "xdd:dateTimeInterval",
  v.interval)
// v.interval = "2025-01-01/2025-04-01"^^xdd:dateTimeInterval
```

> For the full `xdd:dateTimeInterval` type reference, see [Data Types](/docs/data-types/#xdd-datetimeinterval). For the complete interval algebra guide, see [Allen's Interval Algebra](/docs/woql-interval-algebra/).

---

## Edge-of-Range Helpers: `range_min`, `range_max`

Find the minimum or maximum value in a list of comparable values. Works with any type that supports ordering: integers, decimals, dates, datetimes, strings.

| Predicate | Signature | Description |
|-----------|-----------|-------------|
| `range_min` | `range_min(List, Min)` | Minimum value in a list |
| `range_max` | `range_max(List, Max)` | Maximum value in a list |

Both predicates use the same ordering as `less`/`greater`. An empty list produces no bindings (the query fails gracefully). A single-element list returns that element.

### Example: Earliest and Latest Filing Date

```javascript
let v = Vars("dates", "earliest", "latest");
WOQL.and(
  WOQL.group_by([], ["date"], v.dates,
    WOQL.triple(v.doc, "filing_date", v.date)),
  WOQL.range_min(v.dates, v.earliest),
  WOQL.range_max(v.dates, v.latest)
)
```

### Example: Find the Reporting Period Boundaries from Transaction Dates

Given a set of transactions, determine the earliest and latest dates to define the reporting window:

```javascript
let v = Vars("doc", "date", "all_dates", "period_start", "period_end");
WOQL.and(
  WOQL.group_by([], ["date"], v.all_dates,
    WOQL.triple(v.doc, "transaction_date", v.date)),
  WOQL.range_min(v.all_dates, v.period_start),
  WOQL.range_max(v.all_dates, v.period_end)
)
// v.period_start = earliest transaction date
// v.period_end = latest transaction date
```

### Example: Compare Numbers

```javascript
let v = Vars("m");
WOQL.range_min([
  literal(7, "xsd:integer"),
  literal(2, "xsd:integer"),
  literal(9, "xsd:integer"),
  literal(1, "xsd:integer"),
  literal(5, "xsd:integer")
], v.m)
// v.m = 1
```

### Python Example: Find Extremes of a Date Set

```python
from terminusdb_client.woqlquery import WOQLQuery as WOQL

dates = [
    {"@type": "xsd:date", "@value": "2025-06-15"},
    {"@type": "xsd:date", "@value": "2025-01-01"},
    {"@type": "xsd:date", "@value": "2025-03-01"},
]

earliest = WOQL().range_min(dates, "v:earliest")
latest = WOQL().range_max(dates, "v:latest")
# earliest binds to 2025-01-01, latest binds to 2025-06-15
```

---

## Putting It All Together: A Financial Reporting Workflow{#financial-workflow}

This section walks through a realistic financial reporting scenario that exercises every category of time predicate. The goal is to show how the four temporal concepts — instants, durations, intervals, and relations — compose in practice.

### Scenario: Quarterly Close and Filing

A company needs to:
1. Define fiscal quarters with correct boundaries
2. Calculate the day-count for each quarter
3. Verify the quarters partition the fiscal year cleanly
4. Compute filing deadlines (60 days after period end)
5. Find the earliest and latest filing dates across subsidiaries

### Step 1: Define Fiscal Quarters from Inclusive Dates

The finance team provides inclusive dates (what appears on reports). Convert once:

```javascript
let v = Vars("q1", "q2", "q3", "q4", "fy");
WOQL.and(
  // Convert inclusive reporting dates to half-open intervals
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
  WOQL.interval_inclusive(
    literal("2025-01-01", "xsd:date"),
    literal("2025-12-31", "xsd:date"), v.fy)
)
```

### Step 2: Verify Temporal Structure with Allen's Algebra

Check that quarters are properly adjacent and contained within the fiscal year:

```javascript
WOQL.and(
  // Adjacent quarters must "meet" — no gaps, no overlaps
  WOQL.interval_relation_typed("meets", v.q1, v.q2),
  WOQL.interval_relation_typed("meets", v.q2, v.q3),
  WOQL.interval_relation_typed("meets", v.q3, v.q4),

  // Each quarter is "during" the fiscal year
  WOQL.interval_relation_typed("starts", v.q1, v.fy),     // Q1 starts FY
  WOQL.interval_relation_typed("during", v.q2, v.fy),     // Q2 is during FY
  WOQL.interval_relation_typed("during", v.q3, v.fy),     // Q3 is during FY
  WOQL.interval_relation_typed("finishes", v.q4, v.fy),   // Q4 finishes FY

  // Non-adjacent quarters have a gap
  WOQL.interval_relation_typed("before", v.q1, v.q3),
  WOQL.interval_relation_typed("before", v.q1, v.q4)
)
```

If any of these checks fail (0 bindings), the quarters are incorrectly defined.

### Step 3: Calculate Day-Counts

The display says "Q1: January 1 – March 31" but the day-count computation needs the half-open end:

```javascript
let v = Vars("q1_days", "q2_days");
WOQL.and(
  WOQL.date_duration(
    literal("2025-01-01", "xsd:date"),
    literal("2025-04-01", "xsd:date"),  // half-open end, not March 31
    v.q1_days),
  WOQL.date_duration(
    literal("2025-04-01", "xsd:date"),
    literal("2025-07-01", "xsd:date"),
    v.q2_days)
)
// v.q1_days = "P90D" (31+28+31 = 90 days in non-leap 2025)
// v.q2_days = "P91D" (30+31+30 = 91 days)
```

Note: the user sees "90 days" for Q1. The system computes it using [Jan 1, Apr 1), not [Jan 1, Mar 31].

### Step 4: Compute Filing Deadlines

SEC filings are typically due 60 days after the period end. The deadline is computed from the inclusive end date (the "as of" date):

```javascript
let v = Vars("q1_deadline");
WOQL.date_duration(
  literal("2025-03-31", "xsd:date"),   // as-of date (inclusive end)
  v.q1_deadline,
  literal("P60D", "xsd:duration"))
// v.q1_deadline = "2025-05-30"
```

### Step 5: Find Earliest and Latest Filing Dates

Across multiple subsidiaries, find the reporting window:

```javascript
let v = Vars("sub_dates", "earliest", "latest");
WOQL.and(
  WOQL.group_by([], ["date"], v.sub_dates,
    WOQL.triple(v.sub, "filing_date", v.date)),
  WOQL.range_min(v.sub_dates, v.earliest),
  WOQL.range_max(v.sub_dates, v.latest)
)
```

### Step 6: Generate Month-End Accrual Dates

For monthly accrual entries within Q1:

```javascript
let v = Vars("accrual_date");
WOQL.month_end_dates(v.accrual_date,
  literal("2025-01-01", "xsd:date"),
  literal("2025-04-01", "xsd:date"))
// Produces: 2025-01-31, 2025-02-28, 2025-03-31
```

### Step 7: Filter to Business Days

Exclude weekends from the accrual dates:

```javascript
let v = Vars("accrual_date", "dow");
WOQL.and(
  WOQL.month_end_dates(v.accrual_date,
    literal("2025-01-01", "xsd:date"),
    literal("2025-04-01", "xsd:date")),
  WOQL.weekday(v.accrual_date, v.dow),
  WOQL.lte(v.dow, 5)  // Monday=1 through Friday=5
)
// If March 31 falls on a weekend, it's excluded
```

### Summary: Which Predicate for Which Task

| Task | Predicate | Key Insight |
|------|-----------|-------------|
| "As of" balance queries | `lte(date, as_of_date)` | Inclusive — the as-of date is included |
| Period transaction queries | `in_range(date, start, end)` | Half-open — end date excluded |
| Define periods from user input | `interval_inclusive(start, end, iv)` | Converts inclusive → half-open |
| Verify period adjacency | `interval_relation_typed("meets", a, b)` | No gap, no overlap |
| Verify containment | `interval_relation_typed("during", sub, outer)` | Sub-period within outer |
| Day-count between dates | `date_duration(start, end, dur)` | Use half-open end for correct count |
| Filing deadline from as-of date | `date_duration(as_of, deadline, offset)` | Start from inclusive end |
| Month-end dates in a range | `month_end_dates(date, start, end)` | Generator — half-open range |
| Business day filter | `weekday(date, dow), lte(dow, 5)` | ISO: Mon=1, Sun=7 |
| Find earliest/latest in a set | `range_min(list, min)` | Works on dates, numbers, strings |

---

## Timezone Handling

### Dates (`xsd:date`)

Date predicates (`sequence`, `day_before`, `day_after`, `weekday`, `date_interval`, month boundaries) operate on **calendar dates** and are **timezone-agnostic**. If a date carries a timezone offset, the offset is preserved but does not affect arithmetic.

```javascript
day_after(literal("2024-03-10+05:00", "xsd:date"), v.next)
// v.next = "2024-03-11+05:00" — offset preserved, calendar arithmetic unchanged
```

### DateTimes (`xsd:dateTime`)

DateTime predicates operate in **UTC-normalized** arithmetic:
- All values are converted to UTC for computation
- Results carry the same timezone offset as the input (or UTC if inputs have different offsets)
- `sequence` with hourly steps produces evenly-spaced UTC hours — no DST gaps or duplicates

```javascript
time_interval(
  literal("2024-01-01T00:00:00+05:00", "xsd:dateTime"),
  literal("2024-01-01T00:00:00Z", "xsd:dateTime"),
  v.duration)
// v.duration = "PT5H" — the UTC difference between +05:00 and Z
```

**Rationale**: Financial systems are systems of record. Ambiguous timestamps (DST-duplicated wall-clock hours) are unacceptable. UTC normalization ensures every timestamp is unique and monotonically ordered.
