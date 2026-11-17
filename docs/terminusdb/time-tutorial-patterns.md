---
tags:
  - woql
  - tutorial
  - time-processing
  - advanced
title: "Tutorial 4: Creative Temporal Patterns"
nextjs:
  metadata:
    title: "Tutorial 4: Creative Temporal Patterns"
    keywords: woql temporal pattern generator matcher unification backtracking business day calendar fiscal scheduling constraint satisfaction gap overlap detection playground
    description: Hands-on tutorial using the WOQL Playground to solve real business problems by exploiting WOQL's logic-programming nature — generators, matchers, unification, and composition of temporal predicates.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/time-tutorial-patterns/
media: []
---

> **Series:** [Time Processing Overview](/docs/time-processing) | [Tutorial 1: Dates](/docs/time-tutorial-dates) | [Tutorial 2: Durations](/docs/time-tutorial-durations) | [Tutorial 3: Intervals](/docs/time-tutorial-intervals) | **Tutorial 4**

## What you will learn

By the end of this tutorial you will be able to:
- Exploit the three modes of WOQL predicates: compute, validate, and classify
- Compose generators with filters to build business day calendars
- Use `and` to chain temporal predicates into multi-step reasoning
- Build fiscal calendar structures and verify them with Allen's algebra
- Detect gaps and overlaps between periods
- Coordinate deadlines across multiple entities with `range_min` / `range_max`

## Prerequisites

- Completed Tutorials [1](/docs/time-tutorial-dates), [2](/docs/time-tutorial-durations), and [3](/docs/time-tutorial-intervals)
- A running TerminusDB instance (optional — the walkthrough simulates every query)

---

## The Big Idea: Three Modes of Every Predicate

Most programming languages have functions that go one way: input → output. WOQL predicates are **multi-directional**. The same predicate can:

1. **Compute** — give it inputs, get outputs
2. **Validate** — give it all arguments, get yes/no
3. **Generate** — give it partial information, get the missing piece, used to Classify here.

You have already seen this with `date_duration` (Tutorial 2) and `interval_relation_typed` (Tutorial 3). This tutorial shows you how to *compose* these modes to solve problems that would require complex procedural code in other systems.

---

## Part 1: Business Day Calendars {% step %}

Financial systems need to know which dates are business days. WOQL has no built-in "is_business_day" predicate — but you can build one by composing `sequence` (generator) with `weekday` (classifier) and `less`/`lte` (filter).

### Step 1: Generate all dates in January, then filter to weekdays {% step %}

{% woql-playground code="let v = Vars(\"date\", \"dow\")\nand(\n  sequence(v.date,\n    literal(\"2025-01-01\", \"xsd:date\"),\n    literal(\"2025-02-01\", \"xsd:date\")),\n  weekday(v.date, v.dow),\n  lte(v.dow, 5)\n)" title="Step 1: Business days in January 2025" description="Generate all dates in Jan, get each weekday number (Mon=1..Sun=7), keep only Mon-Fri." /%}

{% http-expected %}
```json
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":["date","dow"],"bindings":[{"date":{"@type":"xsd:date","@value":"2025-01-01"},"dow":{"@type":"xsd:integer","@value":3}},{"date":{"@type":"xsd:date","@value":"2025-01-02"},"dow":{"@type":"xsd:integer","@value":4}},{"date":{"@type":"xsd:date","@value":"2025-01-03"},"dow":{"@type":"xsd:integer","@value":5}},{"date":{"@type":"xsd:date","@value":"2025-01-06"},"dow":{"@type":"xsd:integer","@value":1}},{"date":{"@type":"xsd:date","@value":"2025-01-07"},"dow":{"@type":"xsd:integer","@value":2}},{"date":{"@type":"xsd:date","@value":"2025-01-08"},"dow":{"@type":"xsd:integer","@value":3}},{"date":{"@type":"xsd:date","@value":"2025-01-09"},"dow":{"@type":"xsd:integer","@value":4}},{"date":{"@type":"xsd:date","@value":"2025-01-10"},"dow":{"@type":"xsd:integer","@value":5}},{"date":{"@type":"xsd:date","@value":"2025-01-13"},"dow":{"@type":"xsd:integer","@value":1}},{"date":{"@type":"xsd:date","@value":"2025-01-14"},"dow":{"@type":"xsd:integer","@value":2}},{"date":{"@type":"xsd:date","@value":"2025-01-15"},"dow":{"@type":"xsd:integer","@value":3}},{"date":{"@type":"xsd:date","@value":"2025-01-16"},"dow":{"@type":"xsd:integer","@value":4}},{"date":{"@type":"xsd:date","@value":"2025-01-17"},"dow":{"@type":"xsd:integer","@value":5}},{"date":{"@type":"xsd:date","@value":"2025-01-20"},"dow":{"@type":"xsd:integer","@value":1}},{"date":{"@type":"xsd:date","@value":"2025-01-21"},"dow":{"@type":"xsd:integer","@value":2}},{"date":{"@type":"xsd:date","@value":"2025-01-22"},"dow":{"@type":"xsd:integer","@value":3}},{"date":{"@type":"xsd:date","@value":"2025-01-23"},"dow":{"@type":"xsd:integer","@value":4}},{"date":{"@type":"xsd:date","@value":"2025-01-24"},"dow":{"@type":"xsd:integer","@value":5}},{"date":{"@type":"xsd:date","@value":"2025-01-27"},"dow":{"@type":"xsd:integer","@value":1}},{"date":{"@type":"xsd:date","@value":"2025-01-28"},"dow":{"@type":"xsd:integer","@value":2}},{"date":{"@type":"xsd:date","@value":"2025-01-29"},"dow":{"@type":"xsd:integer","@value":3}},{"date":{"@type":"xsd:date","@value":"2025-01-30"},"dow":{"@type":"xsd:integer","@value":4}},{"date":{"@type":"xsd:date","@value":"2025-01-31"},"dow":{"@type":"xsd:integer","@value":5}}],"deletes":0,"inserts":0,"transaction_retry_count":0}
```
{% /http-expected %}

**What happened:** `sequence` generated all 31 dates. `weekday` classified each one. `lte(dow, 5)` filtered out Saturday (6) and Sunday (7). The result is every business day in January 2025.

### Step 2: Count business days in a month {% step %}

{% woql-playground code="let v = Vars(\"date\", \"dow\", \"days\", \"count\", \"show_date\")\nand(\n  group_by([], [\"show_date\"], v.days,\n    and(\n      sequence(v.date,\n        literal(\"2025-01-01\", \"xsd:date\"),\n        literal(\"2025-02-01\", \"xsd:date\")),\n      weekday(v.date, v.dow),\n      lte(v.dow, 5),\n      typecast(v.date, \"xsd:string\", v.show_date),\n    )\n  ),\n  length(v.days, v.count)\n)" title="Step 2: Count business days" description="How many business days in January 2025? Converts dates to strings before collecting, then counts." /%}

{% http-expected %}
```json
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":["show_date","date","dow","days","count"],"bindings":[{"count":{"@type":"xsd:decimal","@value":23},"date":null,"days":[{"@type":"xsd:string","@value":"2025-01-01"},{"@type":"xsd:string","@value":"2025-01-02"},{"@type":"xsd:string","@value":"2025-01-03"},{"@type":"xsd:string","@value":"2025-01-06"},{"@type":"xsd:string","@value":"2025-01-07"},{"@type":"xsd:string","@value":"2025-01-08"},{"@type":"xsd:string","@value":"2025-01-09"},{"@type":"xsd:string","@value":"2025-01-10"},{"@type":"xsd:string","@value":"2025-01-13"},{"@type":"xsd:string","@value":"2025-01-14"},{"@type":"xsd:string","@value":"2025-01-15"},{"@type":"xsd:string","@value":"2025-01-16"},{"@type":"xsd:string","@value":"2025-01-17"},{"@type":"xsd:string","@value":"2025-01-20"},{"@type":"xsd:string","@value":"2025-01-21"},{"@type":"xsd:string","@value":"2025-01-22"},{"@type":"xsd:string","@value":"2025-01-23"},{"@type":"xsd:string","@value":"2025-01-24"},{"@type":"xsd:string","@value":"2025-01-27"},{"@type":"xsd:string","@value":"2025-01-28"},{"@type":"xsd:string","@value":"2025-01-29"},{"@type":"xsd:string","@value":"2025-01-30"},{"@type":"xsd:string","@value":"2025-01-31"}],"dow":null,"show_date":null}],"deletes":0,"inserts":0,"transaction_retry_count":0}
```
{% /http-expected %}

### Step 3: Find month-end dates that fall on weekdays {% step %}

{% woql-playground code="let v = Vars(\"eom\", \"dow\")\nand(\n  month_end_dates(v.eom,\n    literal(\"2025-01-01\", \"xsd:date\"),\n    literal(\"2026-01-01\", \"xsd:date\")),\n  weekday(v.eom, v.dow),\n  lte(v.dow, 5)\n)" title="Step 3: Business-day month-ends" description="Month-end dates that are also business days. If a month ends on a weekend, it's excluded." /%}

{% http-expected %}
```json
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":["eom","dow"],"bindings":[{"dow":{"@type":"xsd:integer","@value":5},"eom":{"@type":"xsd:date","@value":"2025-01-31"}},{"dow":{"@type":"xsd:integer","@value":5},"eom":{"@type":"xsd:date","@value":"2025-02-28"}},{"dow":{"@type":"xsd:integer","@value":1},"eom":{"@type":"xsd:date","@value":"2025-03-31"}},{"dow":{"@type":"xsd:integer","@value":3},"eom":{"@type":"xsd:date","@value":"2025-04-30"}},{"dow":{"@type":"xsd:integer","@value":1},"eom":{"@type":"xsd:date","@value":"2025-06-30"}},{"dow":{"@type":"xsd:integer","@value":4},"eom":{"@type":"xsd:date","@value":"2025-07-31"}},{"dow":{"@type":"xsd:integer","@value":2},"eom":{"@type":"xsd:date","@value":"2025-09-30"}},{"dow":{"@type":"xsd:integer","@value":5},"eom":{"@type":"xsd:date","@value":"2025-10-31"}},{"dow":{"@type":"xsd:integer","@value":3},"eom":{"@type":"xsd:date","@value":"2025-12-31"}}],"deletes":0,"inserts":0,"transaction_retry_count":0}
```
{% /http-expected %}

---

## Part 2: Fiscal Calendar Construction {% step %}

Build a complete fiscal calendar structure and verify it in a single query.

### Step 4: Define and validate four quarters {% step %}

{% woql-playground showResultOnly=true code="let v = Vars(\"q1\", \"q2\", \"q3\", \"q4\")\nand(\n  interval(\n    literal(\"2025-01-01\", \"xsd:date\"),\n    literal(\"2025-04-01\", \"xsd:date\"), v.q1),\n  interval(\n    literal(\"2025-04-01\", \"xsd:date\"),\n    literal(\"2025-07-01\", \"xsd:date\"), v.q2),\n  interval(\n    literal(\"2025-07-01\", \"xsd:date\"),\n    literal(\"2025-10-01\", \"xsd:date\"), v.q3),\n  interval(\n    literal(\"2025-10-01\", \"xsd:date\"),\n    literal(\"2026-01-01\", \"xsd:date\"), v.q4),\n  interval_relation_typed(\n    literal(\"meets\", \"xsd:string\"), v.q1, v.q2),\n  interval_relation_typed(\n    literal(\"meets\", \"xsd:string\"), v.q2, v.q3),\n  interval_relation_typed(\n    literal(\"meets\", \"xsd:string\"), v.q3, v.q4)\n)" title="Step 4: Fiscal quarters — construct and validate" description="Build Q1-Q4 as intervals, then verify each adjacent pair meets. One result = valid partition." /%}

{% http-expected %}
```json
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":["q1","q2","q3","q4"],"bindings":[],"deletes":0,"inserts":0,"transaction_retry_count":0}
```
{% /http-expected %}

If you get one result, the quarters are correctly defined. Zero results means something is wrong — try changing a date to introduce a gap and see what happens.

### Step 5: Quarter day-counts {% step %}

{% woql-playground code="let v = Vars(\"q1d\", \"q2d\", \"q3d\", \"q4d\")\nand(\n  date_duration(literal(\"2025-01-01\", \"xsd:date\"),\n    literal(\"2025-04-01\", \"xsd:date\"), v.q1d),\n  date_duration(literal(\"2025-04-01\", \"xsd:date\"),\n    literal(\"2025-07-01\", \"xsd:date\"), v.q2d),\n  date_duration(literal(\"2025-07-01\", \"xsd:date\"),\n    literal(\"2025-10-01\", \"xsd:date\"), v.q3d),\n  date_duration(literal(\"2025-10-01\", \"xsd:date\"),\n    literal(\"2026-01-01\", \"xsd:date\"), v.q4d)\n)" title="Step 5: Day-count per quarter" description="How many days in each quarter? Note they're not all equal." /%}

{% http-expected %}
```json
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":["q1d","q2d","q3d","q4d"],"bindings":[{"q1d":{"@type":"xsd:duration","@value":"P90D"},"q2d":{"@type":"xsd:duration","@value":"P91D"},"q3d":{"@type":"xsd:duration","@value":"P92D"},"q4d":{"@type":"xsd:duration","@value":"P92D"}}],"deletes":0,"inserts":0,"transaction_retry_count":0}
```
{% /http-expected %}

---

## Part 3: Filing Deadlines and Coordination {% step %}

### Step 6: Compute a filing deadline {% step %}

SEC 10-Q filings are due within 40 days after the fiscal quarter ends. The "as of" date (inclusive end of the period) is the starting point:

{% woql-playground code="let v = Vars(\"deadline\")\ndate_duration(\n  literal(\"2025-03-31\", \"xsd:date\"),\n  v.deadline,\n  literal(\"P40D\", \"xsd:duration\")\n)" title="Step 6: Filing deadline" description="40 days after the as-of date (March 31) for a Q1 filing." /%}

{% http-expected %}
```json
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":["deadline"],"bindings":[{"deadline":{"@type":"xsd:date","@value":"2025-05-10"}}],"deletes":0,"inserts":0,"transaction_retry_count":0}
```
{% /http-expected %}

### Step 7: Find earliest and latest deadlines {% step %}

When multiple subsidiaries have different filing dates, find the reporting window:

{% woql-playground code="let v = Vars(\"earliest\", \"latest\")\nand(\n  range_min([\n    literal(\"2025-05-10\", \"xsd:date\"),\n    literal(\"2025-05-05\", \"xsd:date\"),\n    literal(\"2025-05-15\", \"xsd:date\"),\n    literal(\"2025-05-08\", \"xsd:date\")\n  ], v.earliest),\n  range_max([\n    literal(\"2025-05-10\", \"xsd:date\"),\n    literal(\"2025-05-05\", \"xsd:date\"),\n    literal(\"2025-05-15\", \"xsd:date\"),\n    literal(\"2025-05-08\", \"xsd:date\")\n  ], v.latest)\n)" title="Step 7: Filing window — earliest and latest" description="Four subsidiaries, four deadlines. What's the consolidated window?" /%}

{% http-expected %}
```json
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":["earliest","latest"],"bindings":[{"earliest":{"@type":"xsd:date","@value":"2025-05-05"},"latest":{"@type":"xsd:date","@value":"2025-05-15"}}],"deletes":0,"inserts":0,"transaction_retry_count":0}
```
{% /http-expected %}

### Step 8: Check which is the last business day before the deadline {% step %}

{% woql-playground code="let v = Vars(\"date\", \"dow\", \"days\", \"last\")\nselect(v.last,\n  and(\n    group_by([], [\"date\"], v.days, and(\n      sequence(v.date,\n        literal(\"2025-03-31\", \"xsd:date\"),\n        literal(\"2025-05-10\", \"xsd:date\")),\n      weekday(v.date, v.dow),\n      lte(v.dow, 5)\n    )),\n    range_max(v.days, v.last)\n  )\n)" title="Step 8: Last business day before deadline" description="Collect all business days into a list, then pick the maximum." /%}

{% http-expected %}
```json
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":["last"],"bindings":[{"last":{"@type":"xsd:date","@value":"2025-05-09"}}],"deletes":0,"inserts":0,"transaction_retry_count":0}
```
{% /http-expected %}

`group_by` collects business days into a list, `range_max` picks the latest. One row, one answer.

---

## Part 4: Overlap and Gap Detection {% step %}

### Step 9: Detect overlapping periods {% step %}

{% woql-playground code="interval_relation_typed(\n  \"v:rel\",\n  literal(\"2025-02-15/2025-04-15\", \"xdd:dateTimeInterval\"),\n  literal(\"2025-01-01/2025-04-01\", \"xdd:dateTimeInterval\")\n)" title="Step 9: Overlap detection" description="Does the audit period overlap with the reporting period? Check the relation." /%}

{% http-expected %}
```json
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":["rel"],"bindings":[{"rel":{"@type":"xsd:string","@value":"overlapped_by"}}],"deletes":0,"inserts":0,"transaction_retry_count":0}
```
{% /http-expected %}

Any result containing `overlaps`, `overlapped_by`, `starts`, `started_by`, `during`, `contains`, `finishes`, `finished_by`, or `equals` means the periods share at least some time.

### Step 10: Validate no overlap between adjacent assignments {% step %}

{% woql-playground showResultOnly=true code="interval_relation_typed(\n  literal(\"meets\", \"xsd:string\"),\n  literal(\"2025-01-01/2025-04-01\", \"xdd:dateTimeInterval\"),\n  literal(\"2025-04-01/2025-07-01\", \"xdd:dateTimeInterval\")\n)" title="Step 10: No-overlap check" description="'meets' means zero gap AND zero overlap. The cleanest adjacency." /%}

{% http-expected %}
```json
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":[],"bindings":[],"deletes":0,"inserts":0,"transaction_retry_count":0}
```
{% /http-expected %}

One result = clean handover. Zero results = something is wrong (gap or overlap).

---

## Part 5: Rolling Windows and Sequence Composition {% step %}

### Step 11: Generate monthly reporting periods {% step %}

{% woql-playground code="let v = Vars(\"ym\", \"month_start\", \"month_end\")\nand(\n  sequence(v.ym,\n    literal(\"2025-01\", \"xsd:gYearMonth\"),\n    literal(\"2025-07\", \"xsd:gYearMonth\")),\n  month_start_date(v.ym, v.month_start),\n  month_end_date(v.ym, v.month_end)\n)" title="Step 11: Monthly periods — start and end" description="For each month in H1, get both the first and last day." /%}

{% http-expected %}
```json
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":["ym","month_start","month_end"],"bindings":[{"month_end":{"@type":"xsd:date","@value":"2025-01-31"},"month_start":{"@type":"xsd:date","@value":"2025-01-01"},"ym":{"@type":"xsd:gYearMonth","@value":"2025-01"}},{"month_end":{"@type":"xsd:date","@value":"2025-02-28"},"month_start":{"@type":"xsd:date","@value":"2025-02-01"},"ym":{"@type":"xsd:gYearMonth","@value":"2025-02"}},{"month_end":{"@type":"xsd:date","@value":"2025-03-31"},"month_start":{"@type":"xsd:date","@value":"2025-03-01"},"ym":{"@type":"xsd:gYearMonth","@value":"2025-03"}},{"month_end":{"@type":"xsd:date","@value":"2025-04-30"},"month_start":{"@type":"xsd:date","@value":"2025-04-01"},"ym":{"@type":"xsd:gYearMonth","@value":"2025-04"}},{"month_end":{"@type":"xsd:date","@value":"2025-05-31"},"month_start":{"@type":"xsd:date","@value":"2025-05-01"},"ym":{"@type":"xsd:gYearMonth","@value":"2025-05"}},{"month_end":{"@type":"xsd:date","@value":"2025-06-30"},"month_start":{"@type":"xsd:date","@value":"2025-06-01"},"ym":{"@type":"xsd:gYearMonth","@value":"2025-06"}}],"deletes":0,"inserts":0,"transaction_retry_count":0}
```
{% /http-expected %}

**What happened:** `sequence` generated each `gYearMonth`. `month_start_date` and `month_end_date` converted each to concrete dates. You now have a complete monthly calendar.

### Step 12: Monthly intervals for Allen's algebra {% step %}

{% woql-playground code="let v = Vars(\"ym\", \"month_start\", \"iv\")\nand(\n  sequence(v.ym,\n    literal(\"2025-01\", \"xsd:gYearMonth\"),\n    literal(\"2025-04\", \"xsd:gYearMonth\")),\n  month_start_date(v.ym, v.month_start),\n  interval_start_duration(v.month_start,\n    literal(\"P1M\", \"xsd:duration\"), v.iv)\n)" title="Step 12: Monthly intervals" description="Construct an interval for each month in Q1 using start + P1M duration." /%}

{% http-expected %}
```json
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":["ym","month_start","iv"],"bindings":[{"iv":{"@type":"xdd:dateTimeInterval","@value":"2025-01-01T00:00:00Z/P1M"},"month_start":{"@type":"xsd:date","@value":"2025-01-01"},"ym":{"@type":"xsd:gYearMonth","@value":"2025-01"}},{"iv":{"@type":"xdd:dateTimeInterval","@value":"2025-02-01T00:00:00Z/P1M"},"month_start":{"@type":"xsd:date","@value":"2025-02-01"},"ym":{"@type":"xsd:gYearMonth","@value":"2025-02"}},{"iv":{"@type":"xdd:dateTimeInterval","@value":"2025-03-01T00:00:00Z/P1M"},"month_start":{"@type":"xsd:date","@value":"2025-03-01"},"ym":{"@type":"xsd:gYearMonth","@value":"2025-03"}}],"deletes":0,"inserts":0,"transaction_retry_count":0}
```
{% /http-expected %}

---

## Part 6: Multi-Step Temporal Reasoning {% step %}

This is where WOQL's composability truly shines. Chain multiple temporal predicates to answer complex questions.

### Step 13: "What is the last business day of each month in H1 2025?" {% step %}

This requires generating month-end dates, checking each for weekday, and if it's a weekend, stepping backward until you find a weekday. Here's a simplified version that shows the month-ends and their weekdays:

{% woql-playground code="let v = Vars(\"eom\", \"dow\")\nand(\n  month_end_dates(v.eom,\n    literal(\"2025-01-01\", \"xsd:date\"),\n    literal(\"2025-07-01\", \"xsd:date\")),\n  weekday(v.eom, v.dow)\n)" title="Step 13: Month-end weekdays" description="Which month-ends are weekdays (1-5) and which are weekends (6-7)?" /%}

{% http-expected %}
```json
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":["eom","dow"],"bindings":[{"dow":{"@type":"xsd:integer","@value":5},"eom":{"@type":"xsd:date","@value":"2025-01-31"}},{"dow":{"@type":"xsd:integer","@value":5},"eom":{"@type":"xsd:date","@value":"2025-02-28"}},{"dow":{"@type":"xsd:integer","@value":1},"eom":{"@type":"xsd:date","@value":"2025-03-31"}},{"dow":{"@type":"xsd:integer","@value":3},"eom":{"@type":"xsd:date","@value":"2025-04-30"}},{"dow":{"@type":"xsd:integer","@value":6},"eom":{"@type":"xsd:date","@value":"2025-05-31"}},{"dow":{"@type":"xsd:integer","@value":1},"eom":{"@type":"xsd:date","@value":"2025-06-30"}}],"deletes":0,"inserts":0,"transaction_retry_count":0}
```
{% /http-expected %}

Inspect the results: any `dow` of 6 (Saturday) or 7 (Sunday) means that month's last business day is actually the Friday before.

### Step 14: Duration + interval + relation in one query {% step %}

"Given a start date and a project duration, construct the project interval, then check if it overlaps with the Q2 reporting period."

{% woql-playground code="let v = Vars(\"project_iv\", \"rel\")\nand(\n  interval_start_duration(\n    literal(\"2025-05-15\", \"xsd:date\"),\n    literal(\"P45D\", \"xsd:duration\"),\n    v.project_iv),\n  interval_relation_typed(v.rel,\n    v.project_iv,\n    literal(\"2025-04-01/2025-07-01\", \"xdd:dateTimeInterval\"))\n)" title="Step 14: Multi-step reasoning" description="Build a project interval from start+duration, classify its relation to Q2." /%}

{% http-expected %}
```json
{"@type":"api:WoqlResponse","api:status":"api:success","api:variable_names":["project_iv","rel"],"bindings":[{"project_iv":{"@type":"xdd:dateTimeInterval","@value":"2025-05-15T00:00:00Z/P45D"},"rel":{"@type":"xsd:string","@value":"during"}}],"deletes":0,"inserts":0,"transaction_retry_count":0}
```
{% /http-expected %}

---

## Design Patterns Summary

| Pattern | Predicates | Use case |
|---------|-----------|----------|
| **Generate + Filter** | `sequence` + `weekday` + `lte` | Business day calendars |
| **Construct + Validate** | `interval` + `interval_relation_typed("meets")` | Partition verification |
| **Compute + Generate** | `date_duration` + `interval_relation_typed(v.rel)` | Temporal reasoning |
| **Generate + Transform** | `sequence(ym)` + `month_start_date` | Monthly calendars |
| **Aggregate + Compare** | `range_min` / `range_max` | Deadline coordination |
| **Chain + Compose** | `interval_start_duration` + `interval_relation_typed` | Multi-step analysis |

---

## Self-Check

1. How would you find all Fridays in March 2025?
2. What Allen relation would you check to verify two shifts have no gap?
3. If `interval_relation_typed(v.rel, audit, reporting)` returns `"overlapped_by"`, what does that mean?
4. How would you find the next business day after a computed deadline?
5. Why is WOQL better suited for temporal reasoning than a typical SQL query?

---

## What You Learned

| Concept | Key Point |
|---------|-----------|
| **Three modes** | Compute, validate, classify — same predicate, different binding patterns |
| **Generate + Filter** | `sequence` + `weekday` = business day calendar |
| **Partition verification** | Chain `meets` checks to prove no gaps or overlaps |
| **Multi-step reasoning** | `and` composes any predicates — duration, interval, relation |
| **Deadline coordination** | `range_min` / `range_max` across entity lists |
| **Gap/overlap detection** | Classify with `interval_relation_typed` — check the relation name |

## Where to go next

- **[WOQL Time & Date Reference](/docs/woql-time-handling)** — exhaustive predicate-by-predicate documentation
- **[Allen's Interval Algebra Reference](/docs/woql-interval-algebra)** — all 13 relations with timeline diagrams
- **[EOM Preservation Rules](/docs/woql-eom-rules)** — complete rule tables for month arithmetic
- **[Range Queries with triple_slice](/docs/woql-triple-slice)** — O(log n) storage-level range queries
