---
tags:
  - woql
  - cookbook
  - advanced
title: "WOQL Cookbook: Pattern generation"
nextjs:
  metadata:
    title: "WOQL Cookbook: Pattern generation"
    description: Examples of WOQL pattern generation using substr(), sequence(), weekday(), date arithmetic, interval algebra, range operations, and other predicates that generate all possible solutions via backtracking.
    keywords: woql, query, datalog, cookbook, declarative logic, sequence, weekday, date, interval, range, pattern generation
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/pattern-generation-cookbook/
media: []
---

When faced with combinatorial problems, it is hard to know where to start. Using a logic engine to exhaust possible solutions is a novel way to approach such problems, and leverages an engine to do the hard work for us.

In this example we will explore specifically how the `substr()` predicate can be used to generate all possible substrings of a string using rules.

## Example of pattern generation

A simple pattern that shows the pattern generation is the `substr()` predicate:

```woql
substr(string, before, length, after, subString)
```

### Introducing pattern generation with two simple examples

Notice that there is only one solution for example 1 below, and two solutions for example 2, as the possible solutions for the open ended variables will be generated automatically by the engine.

#### Code: Example 1 of substr

```javascript
substr("string", 2, 2, "v:after", "ri")
```

This returns `2`, as it has 2 characters before the substring `ri`, and we use 2 characters of the substring `ri`.

#### Code: Example 2 of substr

```javascript
substr("string", "v:before", 5, "v:after", "v:subString")
```

Now, this query will return two solutions:
* First solution has `before=0` and `after=1`, and `subString="strin"`
* Second solution has `before=1` and `after=0`, and `subString="tring"`



### Combining the pattern generation with rules

Let's increase the complexity of the solution by adding rules for the allowed solutions and make the string a bit more complex to match against.

Note that values in TerminusDB default to being treated as IRIs, unless specifically typed as specific literals, or that the context makes a specific choice for how to interpret a parameter, such as when supplying a pattern to match to `substr()`.

What we are doing here is matching a string that has a pattern of 8 groups of 4 digits separated by hyphens. We want to get one solution per number as an integer, and know the string positions where that number was found.

By matching on `-` we can filter out substrings that do not include a hyphen.

#### Code: Example 3 of substr

```woql
select().and(
  // Let variable string have the string of numbers
  eq("v:string", literal("0000-0001-0002-0003-0004-0005-0006-0007", "xsd:string")),

  // Get every possible substring of 4 characters
  substr("v:string", "v:start", 4, "v:end", "v:str"),
  // Filter out substrings with a hyphen and convert to integer
  and(
      not().substr("v:str", "v:n_1", "v:n_2", "v:n_3", "-"),
      typecast("v:str", "xsd:integer", "v:number")
  )
)
```

Here is the result of the logic.

{% table %}

- 0
- 0
- 0000

---

- 1
- 5
- 0001

---

- 2
- 10
- 0002

---

- 3
- 15
- 0003

---

- 4
- 20
- 0004

---

- 5
- 25
- 0005

---

- 6
- 30
- 0006

---

- 7
- 35
- 0007

{% /table %}

## Temporal and numeric pattern generation

Beyond string patterns, WOQL provides a rich set of predicates that generate sequences of dates, numbers, and other temporal values. These predicates exploit the same pattern-generation principle: leave a variable unbound and the engine exhausts every valid solution.

### Sequence generation

The `sequence()` predicate generates values in a half-open range `[start, end)`. It supports integers, decimals, doubles, dates, dateTimes, gYearMonth, gYear, time, gMonth, gDay, and gMonthDay. An optional `step` controls the increment (defaults to 1 in the natural unit of the type).

#### Generate integers 1 through 5

```javascript
WOQL.sequence("v:n",
  literal(1, "xsd:integer"),
  literal(6, "xsd:integer"))
```

Returns five results: `n` = 1, 2, 3, 4, 5.

#### Generate dates in January 2025

```javascript
WOQL.sequence("v:date",
  literal("2025-01-01", "xsd:date"),
  literal("2025-02-01", "xsd:date"))
```

Returns 31 results, one per day. Each `date` is an `xsd:date` value.

#### Generate every other day with a step

```javascript
WOQL.sequence("v:date",
  literal("2025-01-01", "xsd:date"),
  literal("2025-01-10", "xsd:date"),
  literal(2, "xsd:integer"))
```

Returns dates: Jan 1, Jan 3, Jan 5, Jan 7, Jan 9.

#### Generate months in the first half of 2025

```javascript
WOQL.sequence("v:ym",
  literal("2025-01", "xsd:gYearMonth"),
  literal("2025-07", "xsd:gYearMonth"))
```

Returns six gYearMonth values: 2025-01 through 2025-06.

### Weekday classification

`weekday()` returns the ISO 8601 day-of-week number (Monday = 1, Sunday = 7). `weekday_sunday_start()` uses the US convention (Sunday = 1, Saturday = 7).

#### Get the weekday for a specific date

```javascript
WOQL.weekday(literal("2025-01-06", "xsd:date"), "v:dow")
```

Returns `dow` = 1 (Monday).

#### Combine sequence + weekday to find all Fridays in January

```javascript
WOQL.and(
  WOQL.sequence("v:date",
    literal("2025-01-01", "xsd:date"),
    literal("2025-02-01", "xsd:date")),
  WOQL.weekday("v:date", "v:dow"),
  WOQL.eq("v:dow", literal(5, "xsd:integer"))
)
```

Returns every Friday in January 2025.

### ISO week number

`iso_week()` computes the ISO 8601 week-numbering year and week number for a date.

```javascript
WOQL.iso_week(literal("2025-01-01", "xsd:date"), "v:year", "v:week")
```

Returns `year` = 2025, `week` = 1. Note: the ISO year may differ from the calendar year near year boundaries.

### Day navigation

`day_after()` and `day_before()` compute the next or previous calendar day. Both are bidirectional — give either argument and the engine computes the other.

```javascript
WOQL.day_after(literal("2025-01-31", "xsd:date"), "v:next")
```

Returns `next` = 2025-02-01. Month and year boundaries are handled automatically.

```javascript
WOQL.day_before(literal("2025-03-01", "xsd:date"), "v:prev")
```

Returns `prev` = 2025-02-28 (or Feb 29 in a leap year).

### Month boundary helpers

`month_start_date()` and `month_end_date()` convert a gYearMonth to the first or last day of that month.

```javascript
WOQL.and(
  WOQL.month_start_date(literal("2025-02", "xsd:gYearMonth"), "v:first"),
  WOQL.month_end_date(literal("2025-02", "xsd:gYearMonth"), "v:last")
)
```

Returns `first` = 2025-02-01, `last` = 2025-02-28. Leap years are handled correctly.

`month_start_dates()` and `month_end_dates()` are generators that produce every first-of-month or last-of-month date in a range.

```javascript
WOQL.month_end_dates("v:eom",
  literal("2025-01-01", "xsd:date"),
  literal("2025-07-01", "xsd:date"))
```

Returns six results: Jan 31, Feb 28, Mar 31, Apr 30, May 31, Jun 30.

### Duration arithmetic

`date_duration()` is tri-directional: given any two of start, end, and duration, it computes the third.

#### Compute a duration between two dates

```javascript
WOQL.date_duration(
  literal("2025-01-01", "xsd:date"),
  literal("2025-04-01", "xsd:date"),
  "v:duration")
```

Returns `duration` = P90D (or equivalent).

#### Compute an end date from start + duration

```javascript
WOQL.date_duration(
  literal("2025-03-31", "xsd:date"),
  "v:end",
  literal("P40D", "xsd:duration"))
```

Returns `end` = 2025-05-10.

### Interval construction

`interval()` constructs or deconstructs an `xdd:dateTimeInterval` from start and end dates. `interval_start_duration()` and `interval_duration_end()` relate an interval to a start or end point and a duration.

```javascript
WOQL.interval(
  literal("2025-01-01", "xsd:date"),
  literal("2025-04-01", "xsd:date"),
  "v:iv")
```

Returns an `xdd:dateTimeInterval` value representing Q1 2025.

### Allen's interval algebra

`interval_relation_typed()` classifies the temporal relationship between two intervals using Allen's 13 interval relations (before, meets, overlaps, starts, during, finishes, equals, and their inverses).

```javascript
WOQL.interval_relation_typed("v:rel",
  literal("2025-01-01/2025-04-01", "xdd:dateTimeInterval"),
  literal("2025-04-01/2025-07-01", "xdd:dateTimeInterval"))
```

Returns `rel` = "meets" — the first interval ends exactly where the second begins.

`interval_relation()` does the same on raw start/end pairs without constructing interval values.

### Range comparison

`range_min()` and `range_max()` find the minimum or maximum value in a list. They work with any comparable type: numbers, dates, strings.

```javascript
WOQL.and(
  WOQL.range_min([
    literal("2025-05-10", "xsd:date"),
    literal("2025-05-05", "xsd:date"),
    literal("2025-05-15", "xsd:date")
  ], "v:earliest"),
  WOQL.range_max([
    literal("2025-05-10", "xsd:date"),
    literal("2025-05-05", "xsd:date"),
    literal("2025-05-15", "xsd:date")
  ], "v:latest")
)
```

Returns `earliest` = 2025-05-05, `latest` = 2025-05-15.

### Range checking

`in_range()` tests whether a value falls within a half-open range `[start, end)`.

```javascript
WOQL.in_range(
  literal("2025-03-15", "xsd:date"),
  literal("2025-01-01", "xsd:date"),
  literal("2025-04-01", "xsd:date"))
```

Succeeds (one result) because March 15 is within Q1.

### Composing patterns

The real power comes from combining these predicates. Here is a composed pattern that finds the last business day of each month in H1 2025:

```javascript
WOQL.and(
  WOQL.month_end_dates("v:eom",
    literal("2025-01-01", "xsd:date"),
    literal("2025-07-01", "xsd:date")),
  WOQL.weekday("v:eom", "v:dow"),
  WOQL.lte("v:dow", 5)
)
```

`month_end_dates` generates month-end dates, `weekday` classifies each, and `lte` filters to weekdays only (Mon-Fri). Months whose last day is a Saturday or Sunday are excluded — combine with `day_before` to step backward to the preceding Friday if needed.

## Conclusion

The examples show how to use pattern generation to match against string patterns and extract values from them, as well as how to generate and compose temporal sequences, date arithmetic, interval algebra, and range operations. Every possible solution is generated automatically by the engine to match against.
