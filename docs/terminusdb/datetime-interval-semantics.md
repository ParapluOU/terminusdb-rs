---
tags:
  - reference
  - time-processing
title: xdd:dateTimeInterval Semantics
nextjs:
  metadata:
    title: xdd:dateTimeInterval Semantics
    keywords: datetime, interval, dateTimeInterval, xdd, ISO 8601, half-open, inclusive end, next day, UTC, midnight, day_after
    description: Reference for xdd:dateTimeInterval storage semantics — how plain date endpoints are converted to fully qualified UTC datetimes and why.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/datetime-interval-semantics/
media: []
---

# `xdd:dateTimeInterval` Semantics

TerminusDB stores time intervals as `xdd:dateTimeInterval` values. Every interval is a half-open range `[start, end)` — the start point is included, the end point is excluded — and every endpoint is stored as a fully qualified UTC datetime.

This page explains the special rule for plain-date endpoints, shows the canonical stored form, and describes how it interacts with `xsd:dateTime` endpoints and typecasting.

> **See also:** [WOQL Time Handling](/docs/woql-time-handling/) | [Allen's Interval Algebra](/docs/woql-interval-algebra/) | [Data Types](/docs/xsd-data-types/#xdd-datetimeinterval)

---

## The rule for endpoints of intervals in TerminusDB

If an endpoint is written as a plain `xsd:date` without a time component, TerminusDB stores it as the **start of the next calendar day at `00:00:00Z`**. The interval remains half-open, so the stored end is the first instant *after* the intended inclusive end date. This follows the XBRL-JSON standard for financial reporting where periods are represented as start + inclusive end unless fully qualified.

The way to think about the end date follows the XBRL convention (XBRL 2.1 spec): xbrli:endDate in a duration context is inclusive of that whole calendar day that marks the end date. Implementers who treat it as an instant boundary at T00:00:00Z of that date (rather than the following day) undercount the period by a day — this is one of the most common XBRL parsing bugs. The internal storage is precise for the start of the next day, being the end of the previous day.

| What you write | What TerminusDB stores and retrieves |
|---|---|
| `2019-01-01/2019-12-31` | `2019-01-01T00:00:00Z/2020-01-01T00:00:00Z` |
| `2020-02-01/2020-02-29` | `2020-02-01T00:00:00Z/2020-03-01T00:00:00Z` |
| `2020-02-27/2020-02-28` | `2020-02-27T00:00:00Z/2020-02-29T00:00:00Z` |

Fully qualified `xsd:dateTime` endpoints are stored exactly as given:

| What you write | What TerminusDB stores and retrieves |
|---|---|
| `2024-03-15T08:00:00Z/2024-03-15T17:00:00Z` | `2024-03-15T08:00:00Z/2024-03-15T17:00:00Z` |

The rule applies to every place a date-only endpoint can appear:

- `interval(Start, End, Interval)` — when `End` is a `xsd:date`
- `interval_start_duration(Start, Duration, Interval)` — when the computed end is a date
- `interval_duration_end(Duration, End, Interval)` — when `End` is a `xsd:date`
- `typecast` from `xsd:string` or `xdd:dateRange` into `xdd:dateTimeInterval`

The conversion uses the existing `day_after` logic, so month and year boundaries — including leap days — are handled correctly.

---

## Why Use the Next Day?

Plain dates are ambiguous: does `2025-03-31` mean the end of March 31 or the start of April 1? Storing the date-only end as `2025-04-01T00:00:00Z` keeps the half-open model unambiguous and consistent with:

- **XBRL-JSON** and other financial-reporting formats that represent periods as start + inclusive end.
- **Allen's Interval Algebra**, which requires the end of one interval to equal the start of the next for a `meets` relation.
- **Clean quarter partitioning**: four quarters tile a fiscal year with no gaps and no overlaps.

For example, when a user types Q1 2025 as `2025-01-01/2025-03-31`, TerminusDB stores  `[2025-01-01T00:00:00Z, 2025-04-01T00:00:00Z`. When a user types Q2 2025 as `2025-04-01/2025-06-30`, TerminusDB stores `[2025-04-01T00:00:00Z, 2025-07-01T00:00:00Z]`. The two stored intervals meet exactly at `2025-04-01T00:00:00Z`.

This is due to the ambiguity of ISO8601 where dates are interpreted as the start of the day, and the end of a period is typically the start of the next period. But the standard is unclear there. TerminusDB instead uses the XBRL standard for financial reporting for guidance on how to interpret ISO8601. The end date, if specified as a date and not a timestamp, the interpretation is to reflect the end of that day. This means the timestamp is set to the start of the next day. An alternative is to use 24:00:00Z which is discouraged.

---

## Constructing Intervals

### From two plain dates

```javascript
let v = Vars("q1");
WOQL.interval(
  literal("2025-01-01", "xsd:date"),
  literal("2025-03-31", "xsd:date"),  // inclusive end as written
  v.q1)
// v.q1 = "2025-01-01T00:00:00Z/2025-04-01T00:00:00Z"^^xdd:dateTimeInterval
```

### From a fully qualified datetime

```javascript
let v = Vars("shift");
WOQL.interval(
  literal("2025-03-15T08:00:00Z", "xsd:dateTime"),
  literal("2025-03-15T17:00:00Z", "xsd:dateTime"),
  v.shift)
// v.shift = "2025-03-15T08:00:00Z/2025-03-15T17:00:00Z"^^xdd:dateTimeInterval
```

---

## Typecasting

### String → `xdd:dateTimeInterval`

```javascript
WOQL.typecast(
  literal("2025-01-01/2025-03-31", "xsd:string"),
  "xdd:dateTimeInterval",
  v.interval)
// v.interval = "2025-01-01T00:00:00Z/2025-04-01T00:00:00Z"^^xdd:dateTimeInterval
```

### `xdd:dateTimeInterval` → `xdd:dateRange`

Because the interval is half-open, the exclusive end is moved back one day to make the date range inclusive:

```javascript
WOQL.typecast(
  literal("2025-01-01T00:00:00Z/2025-04-01T00:00:00Z", "xdd:dateTimeInterval"),
  "xdd:dateRange",
  v.range)
// v.range = "[2025-01-01, 2025-03-31]"^^xdd:dateRange
```

### `xdd:dateRange` → `xdd:dateTimeInterval`

`xdd:dateRange` uses inclusive dates, so the inclusive end is advanced by one day:

```javascript
WOQL.typecast(
  literal("[2025-01-01, 2025-03-31]", "xdd:dateRange"),
  "xdd:dateTimeInterval",
  v.interval)
// v.interval = "2025-01-01T00:00:00Z/2025-04-01T00:00:00Z"^^xdd:dateTimeInterval
```

### `xdd:dateTimeInterval` → `xsd:string`

```javascript
WOQL.typecast(v.interval, "xsd:string", v.str)
// v.str = "2025-01-01T00:00:00Z/2025-04-01T00:00:00Z"
```

---

## Quarter Boundaries and Leap Days

The next-day rule works across month and year boundaries, including leap years:

| Written interval | Stored interval | Duration covered |
|---|---|---|
| `2024-01-01/2024-03-31` | `2024-01-01T00:00:00Z/2024-04-01T00:00:00Z` | Q1 2024 |
| `2024-04-01/2024-06-30` | `2024-04-01T00:00:00Z/2024-07-01T00:00:00Z` | Q2 2024 |
| `2024-02-29/2024-03-01` | `2024-02-29T00:00:00Z/2024-03-02T00:00:00Z` | leap day and March 1 |
| `2024-12-31/2025-01-01` | `2024-12-31T00:00:00Z/2025-01-02T00:00:00Z` | New Year's Eve |

Because the end date is advanced with `day_after`, the 29th of February in a leap year is handled correctly: `2024-02-29` becomes `2024-03-01T00:00:00Z`, not `2024-02-30`.

---

## Deconstructing Intervals

When you decompose an interval with `interval(Start, End, Interval)`, the `Start` and `End` variables are always bound to `xsd:dateTime` values — even if the interval was built from plain `xsd:date` inputs.

```javascript
let v = Vars("start", "end");
WOQL.interval(v.start, v.end, literal("2025-01-01/2025-03-31", "xdd:dateTimeInterval"))
// v.start = "2025-01-01T00:00:00Z"^^xsd:dateTime
// v.end   = "2025-04-01T00:00:00Z"^^xsd:dateTime  (stored half-open end)
```

To convert a single endpoint back to a plain date, use `typecast`:

```javascript
WOQL.typecast(v.start, "xsd:date", v.startDate)
// v.startDate = "2025-01-01"^^xsd:date
```

## Displaying Inclusive Dates

When you need to show an interval back to a user as inclusive dates, cast the interval to `xdd:dateRange`:

```javascript
WOQL.typecast(v.q1, "xdd:dateRange", v.range)
// v.range = "[2025-01-01, 2025-03-31]"^^xdd:dateRange
```

This keeps the conversion transparent: inclusive dates go in, half-open intervals are stored and compared, and inclusive dates come back out for display.
