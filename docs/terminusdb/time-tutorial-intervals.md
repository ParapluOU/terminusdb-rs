---
tags:
  - woql
  - tutorial
  - time-processing
  - advanced
title: "Tutorial 3: Intervals & Allen's Temporal Algebra"
nextjs:
  metadata:
    title: "Tutorial 3: Intervals & Allen's Temporal Algebra"
    keywords: woql interval allen algebra xdd dateTimeInterval half-open inclusive meets before during overlaps contains starts finishes equals playground
    description: Hands-on tutorial using the WOQL Playground to learn interval construction, deconstruction, and Allen's 13 temporal relations for verifying how periods relate to each other.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/time-tutorial-intervals/
media: []
---

> **Series:** [Time Processing Overview](/docs/time-processing) | [Tutorial 1: Dates](/docs/time-tutorial-dates) | [Tutorial 2: Durations](/docs/time-tutorial-durations) | **Tutorial 3** | [Tutorial 4: Creative Patterns](/docs/time-tutorial-patterns)

## What you will learn

By the end of this tutorial you will be able to:
- Construct and deconstruct `xdd:dateTimeInterval` values
- Explain the difference between half-open and inclusive interval representations
- Use `interval_relation_typed` to classify or validate how two periods relate
- Name and recognize all 13 Allen interval relations
- Verify that a set of periods tiles a larger period without gaps or overlaps

## Prerequisites

- Completed [Tutorial 2: Durations](/docs/time-tutorial-durations)
- A running TerminusDB instance

---

## Part 1: What Is an Interval?

An **interval** is a pair of dates that defines a period: a start and an end. In TerminusDB, intervals are stored as `xdd:dateTimeInterval` values using ISO 8601 notation: `"start/end"`.

Critically, TerminusDB intervals are **half-open**: the start is inclusive, the end is exclusive. `"2025-01-01/2025-04-01"` means "from January 1 up to but not including April 1" — which is exactly Q1 2025.

### Step 1: Construct an interval from two dates

{% woql-playground code="interval(\n  literal(\"2025-01-01\", \"xsd:date\"),\n  literal(\"2025-04-01\", \"xsd:date\"),\n  \"v:q1\"\n)" title="Step 1: Construct an interval" description="Build a Q1 interval from start and end dates. The result is an xdd:dateTimeInterval." /%}

**What happened:** `v:q1` is bound to `"2025-01-01/2025-04-01"` with type `xdd:dateTimeInterval`. This single value encodes both the start and end.

### Step 2: Deconstruct an interval

{% woql-playground code="interval(\n  \"v:start\",\n  \"v:end\",\n  literal(\"2025-01-01/2025-04-01\", \"xdd:dateTimeInterval\")\n)" title="Step 2: Deconstruct an interval" description="Extract the start and end dates from an interval value." /%}

`interval` is bidirectional — give it an interval and it extracts the endpoints.

### Step 3: Interval from start + duration

{% woql-playground code="interval_start_duration(\n  literal(\"2025-01-01\", \"xsd:date\"),\n  literal(\"P90D\", \"xsd:duration\"),\n  \"v:q1\"\n)" title="Step 3: Start + Duration → Interval" description="Build an interval from a start date and a duration." /%}

---

## Part 2: Allen's 13 Interval Relations

Given any two intervals, exactly one of 13 relationships holds. This is Allen's Interval Algebra — a complete classification of how two time periods can relate.

`interval_relation_typed(Relation, X, Y)` works in two modes:
- **Validation:** Give it a specific relation name → succeeds if that relation holds
- **Classification:** Give it a variable → binds to the relation that holds

### The 13 relations at a glance

| Relation | X relative to Y | Timeline |
|----------|----------------|----------|
| `before` | X ends before Y starts (gap between) | `XX___YY` |
| `after` | X starts after Y ends | `YY___XX` |
| `meets` | X ends exactly where Y starts | `XXYY` |
| `met_by` | X starts exactly where Y ends | `YYXX` |
| `overlaps` | X starts first, they share a middle, Y ends last | `XXXX` / `__YYYY` |
| `overlapped_by` | Y starts first, X ends last | `YYYY` / `__XXXX` |
| `starts` | X and Y start together, X ends first | `XX` / `YYYY` |
| `started_by` | X and Y start together, Y ends first | `XXXX` / `YY` |
| `during` | X is entirely inside Y | `_XX_` / `YYYY` |
| `contains` | Y is entirely inside X | `XXXX` / `_YY_` |
| `finishes` | X and Y end together, X starts later | `__XX` / `YYYY` |
| `finished_by` | X and Y end together, Y starts later | `XXXX` / `__YY` |
| `equals` | Same start, same end | `XXXX` / `YYYY` |

### Step 4: Validate — Do Q1 and Q2 meet?

{% woql-playground showResultOnly=true code="interval_relation_typed(\n  literal(\"meets\", \"xsd:string\"),\n  literal(\"2025-01-01/2025-04-01\", \"xdd:dateTimeInterval\"),\n  literal(\"2025-04-01/2025-07-01\", \"xdd:dateTimeInterval\")\n)" title="Step 4: Validation — Q1 meets Q2" description="Succeeds if Q1 ends exactly where Q2 starts. One result = yes." /%}

**What happened:** One result — the `meets` relation holds. Q1's end (Apr 1) equals Q2's start (Apr 1). No gap, no overlap.

### Step 5: Classify — What is the relation?

{% woql-playground code="interval_relation_typed(\n  \"v:relation\",\n  literal(\"2025-01-01/2025-04-01\", \"xdd:dateTimeInterval\"),\n  literal(\"2025-04-01/2025-07-01\", \"xdd:dateTimeInterval\")\n)" title="Step 5: Classification — unbound relation" description="Let TerminusDB determine the relation. v:relation will be bound to exactly one of the 13." /%}

### Step 6: Before — with a gap

{% woql-playground code="interval_relation_typed(\n  \"v:rel\",\n  literal(\"2025-01-01/2025-03-01\", \"xdd:dateTimeInterval\"),\n  literal(\"2025-04-01/2025-06-01\", \"xdd:dateTimeInterval\")\n)" title="Step 6: before — gap between intervals" description="X ends March 1, Y starts April 1. There's a gap → 'before'." /%}

### Step 7: Overlaps

{% woql-playground code="interval_relation_typed(\n  \"v:rel\",\n  literal(\"2025-01-01/2025-05-01\", \"xdd:dateTimeInterval\"),\n  literal(\"2025-03-01/2025-07-01\", \"xdd:dateTimeInterval\")\n)" title="Step 7: overlaps" description="X starts first (Jan), they share Mar-Apr, Y ends last (Jul)." /%}

### Step 8: During — containment

{% woql-playground code="interval_relation_typed(\n  \"v:rel\",\n  literal(\"2025-03-01/2025-06-01\", \"xdd:dateTimeInterval\"),\n  literal(\"2025-01-01/2026-01-01\", \"xdd:dateTimeInterval\")\n)" title="Step 8: during — X inside Y" description="Mar-Jun is entirely within the fiscal year Jan-Jan." /%}

### Step 9: Equals

{% woql-playground code="interval_relation_typed(\n  \"v:rel\",\n  literal(\"2025-01-01/2025-04-01\", \"xdd:dateTimeInterval\"),\n  literal(\"2025-01-01/2025-04-01\", \"xdd:dateTimeInterval\")\n)" title="Step 9: equals — same interval" description="Identical start and end → 'equals'." /%}

---

## Part 3: Practical Application — Verify Fiscal Quarters

The real power of Allen's algebra: verify that a set of periods forms a correct partition.

### Step 10: Quarters meet each other

{% woql-playground showResultOnly=true code="and(\n  interval_relation_typed(\n    literal(\"meets\", \"xsd:string\"),\n    literal(\"2025-01-01/2025-04-01\", \"xdd:dateTimeInterval\"),\n    literal(\"2025-04-01/2025-07-01\", \"xdd:dateTimeInterval\")),\n  interval_relation_typed(\n    literal(\"meets\", \"xsd:string\"),\n    literal(\"2025-04-01/2025-07-01\", \"xdd:dateTimeInterval\"),\n    literal(\"2025-07-01/2025-10-01\", \"xdd:dateTimeInterval\")),\n  interval_relation_typed(\n    literal(\"meets\", \"xsd:string\"),\n    literal(\"2025-07-01/2025-10-01\", \"xdd:dateTimeInterval\"),\n    literal(\"2025-10-01/2026-01-01\", \"xdd:dateTimeInterval\"))\n)" title="Step 10: All quarters meet" description="Q1 meets Q2, Q2 meets Q3, Q3 meets Q4. If any fail → gap or overlap." /%}

One result means all three `meets` checks passed. Your quarters tile cleanly.

### Step 11: Q1 starts the fiscal year

{% woql-playground code="interval_relation_typed(\n  \"v:rel\",\n  literal(\"2025-01-01/2025-04-01\", \"xdd:dateTimeInterval\"),\n  literal(\"2025-01-01/2026-01-01\", \"xdd:dateTimeInterval\")\n)" title="Step 11: Q1 starts FY" description="Q1 and FY share the same start, Q1 ends first → 'starts'." /%}

### Step 12: Q4 finishes the fiscal year

{% woql-playground code="interval_relation_typed(\n  \"v:rel\",\n  literal(\"2025-10-01/2026-01-01\", \"xdd:dateTimeInterval\"),\n  literal(\"2025-01-01/2026-01-01\", \"xdd:dateTimeInterval\")\n)" title="Step 12: Q4 finishes FY" description="Q4 and FY share the same end, Q4 starts later → 'finishes'." /%}

### Step 13: Non-adjacent quarters — before

{% woql-playground code="interval_relation_typed(\n  \"v:rel\",\n  literal(\"2025-01-01/2025-04-01\", \"xdd:dateTimeInterval\"),\n  literal(\"2025-07-01/2025-10-01\", \"xdd:dateTimeInterval\")\n)" title="Step 13: Q1 before Q3" description="Q1 ends Apr 1, Q3 starts Jul 1 — gap between them → 'before'." /%}

---

## Part 4: DateTime Intervals — Sub-Day Precision

Intervals work with `xsd:dateTime` too, not just dates.

### Step 14: Shift scheduling

{% woql-playground code="interval_relation_typed(\n  \"v:rel\",\n  literal(\"2025-03-15T08:00:00Z/2025-03-15T12:00:00Z\", \"xdd:dateTimeInterval\"),\n  literal(\"2025-03-15T12:00:00Z/2025-03-15T17:00:00Z\", \"xdd:dateTimeInterval\")\n)" title="Step 14: Morning meets afternoon shift" description="Morning shift ends at noon, afternoon starts at noon → 'meets'." /%}

### Step 15: Overlapping meetings

{% woql-playground code="interval_relation_typed(\n  \"v:rel\",\n  literal(\"2025-03-15T09:00:00Z/2025-03-15T10:30:00Z\", \"xdd:dateTimeInterval\"),\n  literal(\"2025-03-15T10:00:00Z/2025-03-15T11:00:00Z\", \"xdd:dateTimeInterval\")\n)" title="Step 15: Overlapping meetings" description="First meeting 9:00-10:30, second 10:00-11:00 → overlaps." /%}

---

## Part 5: The 5-Argument Form — `interval_relation`

When your endpoints are already in separate variables (not packaged as intervals), use the 5-argument form:

{% woql-playground code="interval_relation(\n  \"v:rel\",\n  literal(\"2025-01-01\", \"xsd:date\"),\n  literal(\"2025-04-01\", \"xsd:date\"),\n  literal(\"2025-04-01\", \"xsd:date\"),\n  literal(\"2025-07-01\", \"xsd:date\")\n)" title="Step 16: 5-argument form" description="interval_relation(Rel, X_start, X_end, Y_start, Y_end). Same result as typed form." /%}

### When to use which form

| Your data | Use | Why |
|-----------|-----|-----|
| `xdd:dateTimeInterval` values | `interval_relation_typed(rel, x, y)` | Cleaner — no unpacking |
| Separate start/end variables | `interval_relation(rel, xs, xe, ys, ye)` | No need to construct intervals |

---

## Self-Check

1. What type is `"2025-01-01/2025-04-01"`? Is April 1 included?
2. If `interval_relation_typed("meets", Q1, Q2)` succeeds, what can you say about the boundary?
3. How many Allen relations exist? Can more than one hold for the same pair?
4. What is the relation between Q2 and the fiscal year `[Jan 1, Jan 1 next year)`?
5. How would you detect that two audit periods overlap?

---

## What You Learned

| Concept | Key Point |
|---------|-----------|
| **`xdd:dateTimeInterval`** | Half-open `[start, end)` encoded as `"start/end"` |
| **`interval`** | Bidirectional: construct ↔ deconstruct |
| **`interval_relation_typed`** | 3-argument Allen's algebra on interval values |
| **Validation mode** | Ground relation → succeeds/fails |
| **Classification mode** | Variable relation → binds to the one that holds |
| **13 Allen relations** | Exactly one always holds for any two intervals |
| **`meets`** | Adjacent periods with no gap — the partition test |
| **`starts` / `finishes`** | Sub-period shares a boundary with the outer |
| **`during` / `contains`** | Full containment |

## Next

[Tutorial 4: Creative Temporal Patterns →](/docs/time-tutorial-patterns)
