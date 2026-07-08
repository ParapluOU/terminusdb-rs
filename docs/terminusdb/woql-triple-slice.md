---
tags:
  - woql
  - reference
  - advanced
title: Range Queries with triple_slice and quad_slice
nextjs:
  metadata:
    title: Range Queries with triple_slice and quad_slice
    keywords: woql range query triple_slice quad_slice dateTime integer string performance
    description: How to use triple_slice and quad_slice for fast range queries on typed values in TerminusDB, with automatic type inference from the schema
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-triple-slice/
media: []
---

Range queries let you efficiently find documents whose property values fall within a specified range. `triple_slice` and `quad_slice` push range constraints directly into the storage engine for O(log n) lookups instead of scanning every triple.

## Why Range Queries Matter

When you need to find documents within a time window, a numeric band, or a lexicographic string range, the traditional WOQL approach requires fetching every value and filtering in the query layer:

```javascript
// The slow way: fetch all timestamps, then filter
let v = Vars("doc", "time");
and(
  triple(v.doc, "timestamp", v.time),
  greater(v.time, literal("2025-01-01T00:00:00Z", "xsd:dateTime")),
  less(v.time, literal("2026-01-01T00:00:00Z", "xsd:dateTime"))
)
```

This works, but it materializes every triple for the `timestamp` predicate before discarding the ones outside the range. For large datasets, this is prohibitively slow.

`triple_slice` solves this by binary-searching the sorted value dictionary and iterating only the matching range. The storage engine never touches values outside your bounds.

### When to Use `triple_slice`

- **Time-series data**: Find events, logs, or measurements within a time window
- **Numeric filtering**: Select records in a price band, age range, or score bracket
- **String ranges**: Lexicographic slicing for alphabetical partitions
- **Membership checks**: Verify that a known value falls within expected bounds

## What `triple_slice` Does

### Signature

```
triple_slice(Subject, Predicate, Object, Low, High)
quad_slice(Subject, Predicate, Object, Low, High, Graph)
```

`triple_slice` is a superset of `triple`. It adds two optional bound parameters, `Low` and `High`, that constrain the Object to a half-open range `[Low, High)`. When both bounds are unbound, it behaves identically to `triple`.

`quad_slice` is the same predicate with an explicit graph selector (like `quad` extends `triple`).

### Half-Open Range `[Low, High)`

- **Low is inclusive**: the first matching value is >= Low
- **High is exclusive**: all matching values are strictly < High
- Adjacent slices `[A, B)` and `[B, C)` partition the space cleanly with no overlap and no gaps

### Binding Modes

`triple_slice` works as both a **generator** (when Object is unbound, it produces matching triples) and a **pattern matcher** (when Object is ground, it checks membership).

| Object | Low | High | Behavior |
|--------|-----|------|----------|
| unbound | unbound | unbound | Same as `triple` — generates all triples. Low and High unify with Object. |
| unbound | bound | unbound | Generates triples where Object >= Low. |
| unbound | unbound | bound | Generates triples where Object < High. |
| unbound | bound | bound | Generates triples where Low <= Object < High. |
| ground | unbound | unbound | Same as `triple` — membership check. Low and High unify with Object. |
| ground | bound | unbound | Checks triple exists AND Object >= Low. Fails if Object < Low. |
| ground | unbound | bound | Checks triple exists AND Object < High. Fails if Object >= High. |
| ground | bound | bound | Checks triple exists AND Low <= Object < High. **Fails if out of range**, even if the triple exists. |

The key subtlety: when Object is ground and the triple exists but the value falls outside the range, the predicate **fails**. The range constraint always takes precedence.

### Automatic Type Inference

When the predicate is ground (which is the common case), `triple_slice` infers the correct XSD type from the schema and automatically casts untyped bounds. You do not need explicit `typecast` calls.

For example, if the schema declares `timestamp` as `xsd:dateTime`, then the string `"2025-01-01"` is automatically cast to `xsd:dateTime`. If the cast fails (e.g., `"not-a-date"` to `xsd:dateTime`), an error is raised — there are no silent failures.

When the predicate is unbound, bounds must either be already typed or be unbound variables.

### Supported Types

All data is stored in ordered form with succinct data structures, including the XSD types. Examples of the ordered types that can be used with `triple_slice`:

- **Date/Time**: `xsd:dateTime`, `xsd:dateTimeStamp`, `xsd:date`, `xsd:time`
- **Numeric**: `xsd:integer`, `xsd:decimal`, `xsd:float`, `xsd:double`, `xsd:nonNegativeInteger`, `xsd:positiveInteger`, `xsd:long`, `xsd:int`, `xsd:short`, `xsd:byte`
- **String**: `xsd:string`

## How to Use `triple_slice` — Worked Examples

The following examples use a schema with a `SensorReading` class:

```json
{
  "@type": "Class",
  "@id": "SensorReading",
  "@key": { "@type": "Random" },
  "sensor_id": "xsd:string",
  "timestamp": "xsd:dateTime",
  "temperature": "xsd:decimal",
  "label": { "@type": "Optional", "@class": "xsd:string" }
}
```

With sample data: five readings across January 2025 with temperatures from 18.5 to 23.1 and labels "A" through "E".

---

### Example 1 — Classic DateTime Range

Find all sensor readings from the first half of January 2025.

```javascript
let v = Vars("doc", "time");
triple_slice(v.doc, "timestamp", v.time, "2025-01-01T00:00:00Z", "2025-01-15T00:00:00Z")
```

**What happens**: The engine looks up `timestamp` in the schema, sees it is `xsd:dateTime`, casts the string bounds to `xsd:dateTime`, and binary-searches the value dictionary. Only readings with timestamps in `[Jan 1, Jan 15)` are returned.

---

### Example 2 — Open-Ended High: From a Date Onward

Find all readings from January 20 onward (no upper bound).

```javascript
let v = Vars("doc", "time");
triple_slice(v.doc, "timestamp", v.time, "2025-01-20T00:00:00Z")
```

With Low bound and High unbound, this generates all triples where timestamp >= Jan 20.

---

### Example 3 — Open-Ended Low: Before a Date

Find all readings before January 10.

```javascript
let v = Vars("doc", "time", "low");
and(
  eq(v.low, literal("2025-01-10T00:00:00Z", "xsd:dateTime")),
  triple_slice(v.doc, "timestamp", v.time, null, v.low)
)
```

With Low unbound and High bound, this generates all triples where timestamp < Jan 10.

---

### Example 4 — No Bounds: Degenerates to `triple`

When neither Low nor High are set, `triple_slice` behaves identically to `triple`.

```javascript
let v = Vars("doc", "time", "low", "high");
triple_slice(v.doc, "timestamp", v.time, v.low, v.high)
```

Returns all `timestamp` triples. For each result, `low` and `high` both unify with the timestamp value, so `low == high == time`.

---

### Example 5 — Membership Check: Object In Range

Check that a specific reading's timestamp falls within the expected window.

```javascript
let v = Vars("doc");
and(
  isa(v.doc, "SensorReading"),
  triple_slice(v.doc, "timestamp",
    literal("2025-01-10T08:30:00Z", "xsd:dateTime"),
    "2025-01-01T00:00:00Z", "2025-01-15T00:00:00Z")
)
```

The Object is ground. The predicate checks: does this triple exist AND does `2025-01-10T08:30:00Z` fall in `[Jan 1, Jan 15)`? If yes, succeeds. If the timestamp exists but is outside the range, **fails**.

---

### Example 6 — Membership Check: Object Out of Range

This demonstrates the key subtlety: a triple can exist but still fail the range check.

```javascript
let v = Vars("doc");
and(
  isa(v.doc, "SensorReading"),
  triple_slice(v.doc, "timestamp",
    literal("2025-01-25T14:00:00Z", "xsd:dateTime"),
    "2025-01-01T00:00:00Z", "2025-01-15T00:00:00Z")
)
```

The timestamp `Jan 25` exists in the database, but it is outside `[Jan 1, Jan 15)`, so the predicate **fails**. This is not an error — it is correct behavior. The range constraint takes precedence over triple existence.

---

### Example 7 — Numeric Range: Integer Values

Find readings with temperature between 19.0 and 22.0.

```javascript
let v = Vars("doc", "temp");
triple_slice(v.doc, "temperature", v.temp, "19.0", "22.0")
```

The engine infers `xsd:decimal` from the schema for `temperature` and casts the string bounds accordingly.

---

### Example 8 — String Range: Lexicographic Slice

Find readings with labels in the lexicographic range `[B, D)`.

```javascript
let v = Vars("doc", "lbl");
triple_slice(v.doc, "label", v.lbl, "B", "D")
```

Returns labels "B" and "C" (but not "D", since the high bound is exclusive).

---

### Example 9 — Adjacent Non-Overlapping Slices

Two adjacent slices partition the data cleanly — no double-counting, no gaps.

```javascript
let v = Vars("doc", "time");

// First half of January
let slice1 = triple_slice(v.doc, "timestamp", v.time,
  "2025-01-01T00:00:00Z", "2025-01-15T00:00:00Z");

// Second half of January
let slice2 = triple_slice(v.doc, "timestamp", v.time,
  "2025-01-15T00:00:00Z", "2025-02-01T00:00:00Z");
```

The union of `slice1` and `slice2` equals the full month of January. A reading at exactly `2025-01-15T00:00:00Z` appears only in `slice2` (since Low is inclusive and High is exclusive).

---

### Example 10 — Unbound Predicate with Typed Bounds

When the predicate is unbound, auto-type-inference cannot determine the target type. Bounds must be explicitly typed.

```javascript
let v = Vars("doc", "pred", "val");
triple_slice(v.doc, v.pred, v.val,
  literal("2025-01-01T00:00:00Z", "xsd:dateTime"),
  literal("2025-01-15T00:00:00Z", "xsd:dateTime"))
```

This iterates over all predicates on each document, returning any triple whose object is a `xsd:dateTime` value in the given range.

---

### Example 11 — Before and After: The Old Way vs `triple_slice`

**Before** (full scan + filter):

```javascript
let v = Vars("doc", "time");
and(
  triple(v.doc, "timestamp", v.time),
  greater(v.time, literal("2025-01-01T00:00:00Z", "xsd:dateTime")),
  less(v.time, literal("2025-01-15T00:00:00Z", "xsd:dateTime"))
)
```

**After** (range pushed to storage):

```javascript
let v = Vars("doc", "time");
triple_slice(v.doc, "timestamp", v.time,
  "2025-01-01T00:00:00Z", "2025-01-15T00:00:00Z")
```

Both return the same results. The `triple_slice` version binary-searches the value dictionary and never touches values outside the range.

---

### Example 12 — `quad_slice` on Instance Graph

`quad_slice` adds an explicit graph parameter, just like `quad` extends `triple`.

```javascript
let v = Vars("doc", "time");
quad_slice(v.doc, "timestamp", v.time,
  "2025-01-01T00:00:00Z", "2025-01-15T00:00:00Z",
  "instance")
```

Equivalent to `triple_slice` but explicitly targets the instance graph.

---

### Example 13 — `quad_slice` on Schema Graph

Query the schema graph for range information.

```javascript
let v = Vars("cls", "pred", "val");
quad_slice(v.cls, v.pred, v.val,
  literal("A", "xsd:string"),
  literal("M", "xsd:string"),
  "schema")
```

Searches the schema graph for any triple whose object is a string in `[A, M)`.

## Reference

### Error Conditions

- **Invalid cast**: Bounds that cannot be cast to the predicate's type produce a clear error (e.g., `"not-a-date"` for an `xsd:dateTime` predicate)
- **Unbound predicate + untyped bounds**: If the predicate is unbound and bounds are plain strings (not typed), an error is raised since the target type cannot be inferred
- **Non-existent predicate**: Behaves like `triple` — no results, no error

### Performance Characteristics

- **Binary search**: O(log n) to locate range bounds in the sorted value dictionary
- **Streaming**: Results are yielded one at a time — memory usage is O(1) regardless of range size
- **No full scan**: Values outside the range are never touched
- **Layer stack**: Each layer in the stack requires a separate binary search; rollups improve performance for deep stacks
