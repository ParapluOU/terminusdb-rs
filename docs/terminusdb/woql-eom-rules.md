---
tags:
  - woql
  - reference
  - time-processing
  - advanced
title: End-of-Month Preservation Rules
nextjs:
  metadata:
    title: End-of-Month Preservation Rules
    keywords: woql duration month arithmetic eom end-of-month financial reporting date_duration date_add date_subtract
    description: Complete reference for TerminusDB's end-of-month (EOM) preservation rules — the financial convention for month-based duration arithmetic that keeps month-end dates on month-end boundaries.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-eom-rules/
media: []
---

Month-based duration arithmetic is the single trickiest area in calendar computation. Adding "1 month" to January 31 cannot produce "February 31" because that date does not exist. Different systems handle this differently — and the choice matters for financial reporting, interest calculations, and regulatory deadlines.

TerminusDB implements **end-of-month (EOM) preservation**, the convention used in most financial systems. This page explains the rules in full, with worked examples.

> **See also:** [WOQL Time Handling](/docs/woql-time-handling/) | [Allen's Interval Algebra](/docs/woql-interval-algebra/) | [WOQL Class Reference](/docs/woql-class-reference-guide/)

---

## Why Month Arithmetic Is Hard

Days have a fixed length: 86,400 seconds. Weeks have a fixed length: 7 days. But months do not have a fixed length. They vary between 28 and 31 days, and February's length depends on whether the year is a leap year.

This means "add 1 month" is ambiguous:

```
January 15 + 1 month = February 15     ← straightforward, day 15 exists
January 31 + 1 month = February ???    ← February has no 31st day
```

Three common strategies exist:

1. **Clamp to last day**: January 31 + 1M → February 28/29 (the day is clamped)
2. **Overflow to next month**: January 31 + 1M → March 2/3 (the excess days carry over)
3. **EOM preservation**: January 31 + 1M → February 28/29, AND the result is flagged as "end of month" so that adding another month produces March 31

TerminusDB uses strategy 3 — EOM preservation. This is the same convention used by Bloomberg, Reuters, and most financial calculation engines.

---

## The Two Rules

EOM preservation is governed by two simple rules:

### Rule 1: Is the start date an end-of-month?

A date is "end-of-month" (EOM) if its day number equals the last day of its month.

| Date | Last day of month | EOM? |
|------|------------------|------|
| 2025-01-31 | 31 | Yes |
| 2025-02-28 | 28 (non-leap) | Yes |
| 2024-02-29 | 29 (leap) | Yes |
| 2024-02-28 | 29 (leap) | **No** — February has 29 days in 2024 |
| 2025-04-30 | 30 | Yes |
| 2025-04-15 | 30 | No |

### Rule 2: What happens when you add months?

- **If the start is EOM**: the result is the last day of the target month, regardless of whether the day numbers match.
- **If the start is not EOM**: use the same day number, clamped to the last day of the target month if necessary.

That is the complete rule set. Everything else follows from these two rules.

---

## Addition Examples: +P1M

Each row shows the start date, whether it is EOM, the result of adding one month, and which rule applies.

| Start Date | EOM? | +P1M Result | Rule Applied |
|------------|------|-------------|--------------|
| 2020-01-28 | No | 2020-02-28 | Same day (28 exists in Feb) |
| 2020-01-29 | No | 2020-02-29 | Same day (leap year, 29 exists) |
| 2020-01-30 | No | 2020-02-29 | Clamped (30 > 29, use last day) |
| **2020-01-31** | **Yes** | **2020-02-29** | **EOM preserved** (last day of Feb) |
| **2020-02-29** | **Yes** | **2020-03-31** | **EOM preserved** (last day of Mar) |
| **2020-03-31** | **Yes** | **2020-04-30** | **EOM preserved** (last day of Apr) |
| **2020-04-30** | **Yes** | **2020-05-31** | **EOM preserved** (last day of May) |
| **2020-12-31** | **Yes** | **2021-01-31** | **EOM preserved** (year boundary) |

### The EOM chain

Notice how EOM preservation creates a consistent chain:

```
Jan 31 → Feb 29 → Mar 31 → Apr 30 → May 31 → Jun 30 → Jul 31 → ...
```

Every date in this chain is the last day of its month. The chain never breaks. This is exactly what a financial system needs for monthly interest accruals, payment schedules, and reporting deadlines.

### Clamping vs EOM

The distinction between clamping and EOM matters when February is involved:

```
2020-01-30 + P1M = 2020-02-29    ← clamped (30 > 29)
2020-01-31 + P1M = 2020-02-29    ← EOM preserved

Both land on Feb 29, but the next step differs:

2020-02-29 (from Jan 30, NOT EOM) + P1M = 2020-03-29  ← same day
2020-02-29 (from Jan 31, IS EOM)  + P1M = 2020-03-31  ← EOM preserved
```

The internal EOM flag matters. TerminusDB tracks this automatically through `date_duration`.

---

## Subtraction Examples: -P1M

Subtraction follows the same rules in reverse.

| Start Date | EOM? | -P1M Result | Rule Applied |
|------------|------|-------------|--------------|
| **2020-03-31** | **Yes** | **2020-02-29** | **EOM preserved** (leap year) |
| **2021-03-31** | **Yes** | **2021-02-28** | **EOM preserved** (non-leap) |
| **2020-04-30** | **Yes** | **2020-03-31** | **EOM preserved** |
| **2021-01-31** | **Yes** | **2020-12-31** | **EOM preserved** (year boundary) |
| 2020-03-28 | No | 2020-02-28 | Same day (28 exists in Feb) |
| 2020-03-30 | No | 2020-02-29 | Clamped (30 > 29, leap year) |

---

## Non-Reversibility: The Critical Caveat

Month arithmetic with EOM is **not a simple inverse**. Adding P1M and then subtracting P1M does not always return to the starting date:

```javascript
// Forward: Jan 31 + P1M = Feb 29
WOQL.date_duration(
  literal("2020-01-31", "xsd:date"),
  v.x,
  literal("P1M", "xsd:duration"))
// v.x = "2020-02-29"

// Reverse: Feb 29 - P1M = Jan 29 (NOT Jan 31!)
WOQL.date_duration(
  v.y,
  literal("2020-02-29", "xsd:date"),
  literal("P1M", "xsd:duration"))
// v.y = "2020-01-29"
```

Why? Because when subtracting, Feb 29 is EOM, so the result is the last day of January... wait, that would be Jan 31. But the subtraction rule asks "what date, plus P1M, gives Feb 29?" — and that date is Jan 29 (which is not EOM, so +P1M keeps day 29, and Feb has 29 days in a leap year).

### The practical implication

If you need to verify that two dates are exactly one month apart, use `date_duration` with all three arguments ground:

```javascript
// Verify: are start and end exactly P1M apart?
WOQL.date_duration(
  literal("2020-01-31", "xsd:date"),
  literal("2020-02-29", "xsd:date"),
  literal("P1M", "xsd:duration"))
// Succeeds — confirms the relationship
```

Do not rely on round-tripping (add then subtract) for verification. Test the relationship directly.

### Day-only durations ARE reversible

This non-reversibility only affects month-based and year-based durations. Day-based durations (`P90D`, `P60D`) are always exact and fully reversible:

```javascript
WOQL.date_duration(
  literal("2025-01-01", "xsd:date"),
  v.x,
  literal("P90D", "xsd:duration"))
// v.x = "2025-04-01"

WOQL.date_duration(
  v.y,
  literal("2025-04-01", "xsd:date"),
  literal("P90D", "xsd:duration"))
// v.y = "2025-01-01" — exact round-trip
```

**Recommendation**: for financial calculations where reversibility matters (e.g., back-calculating a start date from an end date and tenor), prefer day-count durations over month-based durations.

---

## Multi-Month Durations

Durations larger than P1M follow the same rules. The month count is applied in a single step, not iterated one month at a time:

```javascript
// +P3M from Jan 31
WOQL.date_duration(
  literal("2025-01-31", "xsd:date"),
  v.x,
  literal("P3M", "xsd:duration"))
// v.x = "2025-04-30" — Jan 31 is EOM, target month (April) has 30 days
```

This is equivalent to asking "what is the last day of the month that is 3 months after January?" — April, which ends on the 30th.

---

## Year Durations

Year durations (`P1Y`) work like 12-month durations with the same EOM rules:

```javascript
WOQL.date_duration(
  literal("2024-02-29", "xsd:date"),
  v.x,
  literal("P1Y", "xsd:duration"))
// v.x = "2025-02-28" — Feb 29 is EOM in 2024; Feb 28 is EOM in 2025

WOQL.date_duration(
  literal("2024-02-28", "xsd:date"),
  v.x,
  literal("P1Y", "xsd:duration"))
// v.x = "2025-02-28" — Feb 28 is NOT EOM in 2024 (leap year has 29);
//                       same day (28) used, happens to be EOM in 2025
```

The second example illustrates a subtle point: Feb 28 in a leap year is *not* EOM (the last day is the 29th), so the same-day rule applies, and it happens to land on Feb 28 in a non-leap year (which *is* EOM). The EOM flag is not transferred across the year boundary in this case.

---

## Combined Year-Month Durations

Durations like `P1Y2M` apply the year and month components together:

```javascript
WOQL.date_duration(
  literal("2024-01-31", "xsd:date"),
  v.x,
  literal("P1Y2M", "xsd:duration"))
// Target month: January + 14 months = March of next year
// Jan 31 is EOM → last day of March 2025 = March 31
// v.x = "2025-03-31"
```

---

## Financial Use Cases

### Monthly interest accrual

A bond pays monthly interest on the last business day of each month. Using EOM preservation ensures the accrual dates track correctly:

```javascript
let v = Vars("accrual_date");
WOQL.month_end_dates(v.accrual_date,
  literal("2025-01-01", "xsd:date"),
  literal("2026-01-01", "xsd:date"))
// Produces: Jan 31, Feb 28, Mar 31, Apr 30, ..., Dec 31
// Each date is correctly the last day of its month
```

### Payment schedule with P1M tenor

A loan with monthly payments starting on Jan 31:

```
Payment 1:  2025-01-31  (start)
Payment 2:  2025-02-28  (EOM: last day of Feb)
Payment 3:  2025-03-31  (EOM: last day of Mar)
Payment 4:  2025-04-30  (EOM: last day of Apr)
...
```

The EOM chain keeps every payment on the last day of the month, which is the expected behavior for financial instruments.

### Regulatory filing deadlines

SEC 10-Q filings are due "within 40 days after the end of each fiscal quarter." If Q1 ends March 31:

```javascript
WOQL.date_duration(
  literal("2025-03-31", "xsd:date"),
  v.deadline,
  literal("P40D", "xsd:duration"))
// v.deadline = "2025-05-10"
```

Note: use day-count (`P40D`), not months (`P1M10D`), for regulatory deadlines. The regulation specifies "40 days," not "1 month and 10 days."

---

## Summary

| Situation | Rule | Example |
|-----------|------|---------|
| Start is EOM, target month shorter | Last day of target month | Jan 31 +P1M → Feb 28 |
| Start is EOM, target month longer | Last day of target month | Feb 28 +P1M → Mar 31 |
| Start is not EOM, day fits | Same day number | Jan 15 +P1M → Feb 15 |
| Start is not EOM, day doesn't fit | Clamped to last day | Jan 30 +P1M → Feb 28 (non-leap) |
| Day-count duration | Exact arithmetic | Always reversible |
| Month-count duration | EOM-aware | Not always reversible |
| Round-trip verification | Test directly | Use 3-argument `date_duration` |
