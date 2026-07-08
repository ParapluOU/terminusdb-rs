---
tags:
  - woql
  - tutorial
  - time-processing
  - intermediate
title: "Tutorial 2: Durations, Month Arithmetic & Sequences"
nextjs:
  metadata:
    title: "Tutorial 2: Durations, Month Arithmetic & Sequences"
    keywords: woql duration iso8601 month arithmetic eom end-of-month sequence date_duration day_before day_after weekday iso_week month_end range_min range_max playground
    description: Hands-on tutorial using the WOQL Playground to learn duration arithmetic, EOM preservation, sequence generation, weekday extraction, and month boundary predicates.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/time-tutorial-durations/
media: []
---

> **Series:** [Time Processing Overview](/docs/time-processing) | [Tutorial 1: Dates](/docs/time-tutorial-dates) | **Tutorial 2** | [Tutorial 3: Intervals](/docs/time-tutorial-intervals) | [Tutorial 4: Creative Patterns](/docs/time-tutorial-patterns)

## What you will learn

By the end of this tutorial you will be able to:
- Add and subtract durations from dates using `date_duration`
- Explain why `P90D` and `P3M` produce different results
- Describe the end-of-month preservation rule and why it matters for finance
- Generate sequences of dates, months, and numbers with `sequence`
- Extract weekdays and ISO week numbers from dates
- Find minimum and maximum values with `range_min` / `range_max`

## Prerequisites

- Completed [Tutorial 1: Dates](/docs/time-tutorial-dates)
- A running TerminusDB instance

---

## Part 1: Duration Arithmetic with `date_duration`

`date_duration(Start, End, Duration)` relates three values. It is **tri-directional** — give it any two and it computes the third.

### Step 1: Add days to a date

{% woql-playground code="date_duration(\n  literal(\"2025-03-31\", \"xsd:date\"),\n  \"v:deadline\",\n  literal(\"P60D\", \"xsd:duration\")\n)" title="Step 1: Add 60 days" description="What date is 60 days after March 31?" /%}

**What happened:** `v:deadline` is bound to `2025-05-30`. The duration `P60D` means exactly 60 calendar days. The `P` prefix is ISO 8601 for "period".

### Step 2: Compute the duration between two dates

{% woql-playground code="date_duration(\n  literal(\"2025-01-01\", \"xsd:date\"),\n  literal(\"2025-04-01\", \"xsd:date\"),\n  \"v:days\"\n)" title="Step 2: Days between dates" description="How many days from Jan 1 to Apr 1 in 2025?" /%}

**Result:** `P90D` — that is 31 (Jan) + 28 (Feb, non-leap) + 31 (Mar) = 90 days.

### Step 3: Compute the start from end minus duration

{% woql-playground code="date_duration(\n  \"v:start\",\n  literal(\"2025-04-01\", \"xsd:date\"),\n  literal(\"P90D\", \"xsd:duration\")\n)" title="Step 3: Subtract duration" description="What date is 90 days before April 1?" /%}

### Step 4: Validate a relationship

When all three arguments are ground, `date_duration` acts as a **validator** — it succeeds only if the relationship is correct:

{% woql-playground code="date_duration(\n  literal(\"2025-01-01\", \"xsd:date\"),\n  literal(\"2025-04-01\", \"xsd:date\"),\n  literal(\"P90D\", \"xsd:duration\")\n)" title="Step 4: Validate — correct" description="Is it really 90 days from Jan 1 to Apr 1? Succeeds if yes." /%}

Now try changing `P90D` to `P89D` — it will return zero results because the relationship is wrong.

---

## Part 2: Why `P90D` and `P3M` Are Different

This is the most important concept in this tutorial. **Day durations are exact. Month durations are calendar-relative.**

### Step 5: Day-based duration

{% woql-playground code="date_duration(\n  literal(\"2025-01-01\", \"xsd:date\"),\n  \"v:result\",\n  literal(\"P90D\", \"xsd:duration\")\n)" title="Step 5: P90D — exact days" description="90 days from Jan 1. Always the same result regardless of year." /%}

### Step 6: Month-based duration

{% woql-playground code="date_duration(\n  literal(\"2025-01-01\", \"xsd:date\"),\n  \"v:result\",\n  literal(\"P3M\", \"xsd:duration\")\n)" title="Step 6: P3M — calendar months" description="3 months from Jan 1. Same result here, but P3M can mean different day counts in different years." /%}

Both land on April 1 — but `P3M` covers 90 days in 2025 (non-leap) and 91 days in 2024 (leap). The month duration preserves the *calendar position*, not the *day count*.

**Practical rule:** Use `PnD` for exact day counts (regulatory deadlines: "within 60 days"). Use `PnM` for calendar positioning (payment schedules: "every month on the same date").

---

## Part 3: End-of-Month Preservation

What happens when you add a month to January 31? February has no 31st day.

### Step 7: The EOM rule

{% woql-playground code="date_duration(\n  literal(\"2025-01-31\", \"xsd:date\"),\n  \"v:result\",\n  literal(\"P1M\", \"xsd:duration\")\n)" title="Step 7: Jan 31 + 1 month" description="January 31 is the last day of its month. What happens?" /%}

**Result:** `2025-02-28`. Because January 31 is the **last day of January**, the result is the **last day of February**. This is end-of-month (EOM) preservation.

### Step 8: The EOM chain

{% woql-playground code="and(\n  date_duration(literal(\"2025-01-31\", \"xsd:date\"), \"v:feb\", literal(\"P1M\", \"xsd:duration\")),\n  date_duration(literal(\"2025-02-28\", \"xsd:date\"), \"v:mar\", literal(\"P1M\", \"xsd:duration\")),\n  date_duration(literal(\"2025-03-31\", \"xsd:date\"), \"v:apr\", literal(\"P1M\", \"xsd:duration\"))\n)" title="Step 8: EOM chain" description="Jan 31 → Feb 28 → Mar 31 → Apr 30. Every step lands on the last day." /%}

The chain never breaks. This is exactly what financial systems need for monthly payment schedules and interest accruals.

### Step 9: Non-reversibility warning

{% woql-playground code="and(\n  date_duration(literal(\"2025-01-31\", \"xsd:date\"), \"v:forward\", literal(\"P1M\", \"xsd:duration\")),\n  date_duration(\"v:backward\", literal(\"2025-02-28\", \"xsd:date\"), literal(\"P1M\", \"xsd:duration\"))\n)" title="Step 9: Non-reversibility" description="Jan 31 + P1M = Feb 28. But Feb 28 - P1M = Jan 28, NOT Jan 31." /%}

**Key insight:** Month arithmetic is not always reversible. Use day-count durations (`P90D`) when round-trip accuracy matters.

> **Deep dive:** The [EOM Preservation Rules](/docs/woql-eom-rules/) page has the complete rule set with addition and subtraction tables.

---

## Part 4: Date Navigation

### Step 10: `day_after` and `day_before`

{% woql-playground code="and(\n  day_after(literal(\"2025-02-28\", \"xsd:date\"), \"v:next\"),\n  day_before(literal(\"2025-03-01\", \"xsd:date\"), \"v:prev\")\n)" title="Step 10: Day navigation" description="day_after Feb 28 = Mar 1 (non-leap). day_before Mar 1 = Feb 28." /%}

Try changing the year to 2024 (leap year) — `day_after` Feb 28 will give Feb 29, not Mar 1.

### Step 11: Weekday extraction

{% woql-playground code="weekday(\n  literal(\"2025-03-31\", \"xsd:date\"),\n  \"v:dow\"\n)" title="Step 11: Weekday" description="ISO 8601: Monday=1, Sunday=7. What day is March 31, 2025?" /%}

### Step 12: ISO week number

{% woql-playground code="iso_week(\n  literal(\"2025-01-01\", \"xsd:date\"),\n  \"v:year\",\n  \"v:week\"\n)" title="Step 12: ISO week" description="Jan 1, 2025 — which ISO week? The ISO year may differ from the calendar year." /%}

---

## Part 5: Sequence Generation

`sequence(Value, Start, End)` generates values in a half-open range `[Start, End)`. When `Value` is unbound, it **produces each value** via backtracking.

### Step 13: Integer sequence

{% woql-playground code="sequence(\"v:i\", 1, 6)" title="Step 13: Integer sequence" description="Generate integers 1 through 5. Half-open: [1, 6) = 1,2,3,4,5." /%}

### Step 14: Monthly reporting calendar

{% woql-playground code="sequence(\n  \"v:month\",\n  literal(\"2025-01\", \"xsd:gYearMonth\"),\n  literal(\"2025-07\", \"xsd:gYearMonth\")\n)" title="Step 14: H1 months" description="Every month in the first half of 2025." /%}

### Step 15: Weekly dates with a step

{% woql-playground code="sequence(\n  \"v:date\",\n  literal(\"2025-01-01\", \"xsd:date\"),\n  literal(\"2025-02-01\", \"xsd:date\"),\n  literal(7, \"xsd:integer\")\n)" title="Step 15: Weekly dates" description="Every 7 days starting from Jan 1." /%}

### Step 16: Decimal sequence — no floating-point drift

{% woql-playground code="sequence(\n  \"v:rate\",\n  literal(\"0.0\", \"xsd:decimal\"),\n  literal(\"1.0\", \"xsd:decimal\"),\n  literal(\"0.3\", \"xsd:decimal\")\n)" title="Step 16: Decimal sequence" description="0.0, 0.3, 0.6, 0.9 — exact rational arithmetic, no accumulation error." /%}

### Step 17: Empty range = zero results

{% woql-playground code="sequence(\"v:i\", 5, 5)" title="Step 17: Empty range" description="Start == End means empty range. Zero results, not an error." /%}

---

## Part 6: Month Boundaries

Financial reporting revolves around month boundaries — period starts, period ends, accrual dates.

### Step 18: Last day of a month

{% woql-playground code="and(\n  month_end_date(literal(\"2025-02\", \"xsd:gYearMonth\"), \"v:feb_end\"),\n  month_end_date(literal(\"2024-02\", \"xsd:gYearMonth\"), \"v:feb_end_leap\")\n)" title="Step 18: Month end date" description="February end: 28 in 2025, 29 in leap year 2024." /%}

### Step 19: All month-end dates in a range

{% woql-playground code="month_end_dates(\n  \"v:month_end\",\n  literal(\"2025-01-01\", \"xsd:date\"),\n  literal(\"2025-07-01\", \"xsd:date\")\n)" title="Step 19: Month-end dates H1" description="Every month-end date in the first half of 2025." /%}

---

## Part 7: Range Min and Max

`range_min` and `range_max` find the smallest and largest values in a list.

### Step 20: Find the min and max

{% woql-playground code="and(\n  range_min(\n    [literal(\"2025-06-15\", \"xsd:date\"),\n     literal(\"2025-01-01\", \"xsd:date\"),\n     literal(\"2025-03-31\", \"xsd:date\")],\n    \"v:earliest\"),\n  range_max(\n    [literal(\"2025-06-15\", \"xsd:date\"),\n     literal(\"2025-01-01\", \"xsd:date\"),\n     literal(\"2025-03-31\", \"xsd:date\")],\n    \"v:latest\")\n)" title="Step 20: range_min / range_max" description="Find the earliest and latest dates in a set." /%}

---

## Self-Check

1. What is the difference between `P90D` and `P3M`?
2. What does `date_duration("2025-01-31", v.x, "P1M")` produce? Why?
3. Why is `Jan 31 + P1M - P1M` not always `Jan 31`?
4. What does `sequence(v.i, 5, 5)` produce?
5. How would you generate all Fridays in January 2025?

---

## What You Learned

| Concept | Key Point |
|---------|-----------|
| **`date_duration`** | Tri-directional: start ↔ end ↔ duration |
| **Day vs month durations** | `PnD` is exact; `PnM` is calendar-relative |
| **EOM preservation** | Last-day-of-month stays last-day-of-month |
| **Non-reversibility** | `+P1M` then `-P1M` may not round-trip |
| **`sequence`** | Generator for integers, decimals, dates, months, years |
| **`weekday`** | ISO 8601: Mon=1, Sun=7 |
| **`month_end_dates`** | Generator for month-end dates in a range |
| **`range_min` / `range_max`** | Min/max of any comparable list |

## Next

[Tutorial 3: Intervals & Allen's Temporal Algebra →](/docs/time-tutorial-intervals)
