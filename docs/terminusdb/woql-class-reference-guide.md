---
tags:
  - woql
  - reference
  - advanced
title: WOQL Class Reference Guide
nextjs:
  metadata:
    title: WOQL Class Reference Guide
    description: Complete reference for all WOQL query language classes, generated from the authoritative schema definition.
    keywords: terminusdb, datalog, query, query language, schema, terminusdb query, woql, woql class reference guide
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/woql-class-reference-guide/
media: []
lastUpdated: "2026-05-01"
---

This page is the complete reference for all WOQL query classes — the building blocks you combine to construct queries. Each class listed here corresponds to a WOQL operation (e.g., `Triple`, `And`, `Select`, `GroupBy`). Use this as a lookup when constructing queries programmatically or debugging query JSON. For a hands-on introduction, see [WOQL Basics](/docs/woql-basics/).

## WOQL Schema

This is the WOQL schema. It gives a complete specification of the syntax of the WOQL query language. This allows WOQL queries to be checked for syntactic correctness, helps to prevent errors and detect conflicts in merge of queries, and allows the storage and retrieval of queries so that queries can be associated with data products.

**Authored by:** Gavin Mendel-Gleason

**Schema version:** v1.0.3

{% callout type="note" %}
**Auto-generated reference**
This page is generated from the authoritative `woql.json` schema definition (131 classes). Run `npm run generate:woql` to regenerate after schema changes.
{% /callout %}

**Contents:**

- [Query operations](#Query) (100 classes) — the main WOQL operations
- [Path patterns](#PathPattern) (8 classes) — graph traversal expressions
- [Arithmetic expressions](#ArithmeticExpression) (9 classes) — numeric operations
- [Utility types](#utility-types) (14 classes) — values, enums, and support types

---

## Query operations

These classes represent WOQL query operations — the building blocks of all database queries.

{% anchor id="Query" /%}
### Query

An abstract class which represents an arbitrary query AST.

**Abstract**

* * *

{% anchor id="AddData" /%}
### AddData

Add an edge with a data value.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. The variable must be bound. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. The variable must be bound. |
| `object` | `DataValue` | A data value or variable which is the target or object of the graph edge. The variable must be bound. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |

* * *

{% anchor id="AddedData" /%}
### AddedData

Specify an edge pattern with data value which was added in *this* commit*.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `DataValue` | A datatype or variable which is the target or object of the graph edge. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |

* * *

{% anchor id="AddedLink" /%}
### AddedLink

Specify an edge pattern which links between nodes at *this* commit.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `NodeValue` | A URI or variable which is the target or object of the graph edge. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |

* * *

{% anchor id="AddedTriple" /%}
### AddedTriple

Specify an edge pattern which was *added* at *this commit*.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `Value` | A URI, datatype or variable which is the target or object of the graph edge. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |

* * *

{% anchor id="AddLink" /%}
### AddLink

Add an edge which links between nodes in the graph.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `NodeValue` | A URI or variable which is the target or object of the graph edge. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |

* * *

{% anchor id="AddTriple" /%}
### AddTriple

Specify an edge to add to the graph.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `Value` | A URI, datatype or variable which is the target or object of the graph edge. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |

* * *

{% anchor id="And" /%}
### And

A conjunction of queries which must all have a solution.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `and` | List(`Query`) | List of queries which must hold. |

* * *

{% anchor id="Call" /%}
### Call

A call of a named parametric query. Variables will be passed to the named query and bound according to the results. Named queries can be (mutually) recursive.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `name` | `xsd:string` | The name of the NamedParametricQuery to be retrieved. |
| `arguments` | List(`Value`) | The arguments to use when binding formal parameters of the parametric query. |

* * *

{% anchor id="Collect" /%}
### Collect

Collect all solutions of a sub-query into a list. The template specifies what to collect from each solution, and the result is unified with into. If the query has no solutions, into is unified with the empty list.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `into` | `Value` | The variable to bind the collected list of solutions. |
| `query` | `Query` | The subquery whose solutions will be collected. |
| `template` | `Value` | The template of elements in the result list. |

* * *

{% anchor id="Comment" /%}
### Comment

A comment. The query is disabled (not executed) and the comment serves as documentation.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `comment` | `DataValue` | The comment string explaining why the query is disabled. |
| `query` | Optional(`Query`) | The query which is commented out and will not be executed. |

* * *

{% anchor id="Concatenate" /%}
### Concatenate

Concatenate a list of strings.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `list` | `DataValue` | The list to concatenate. |
| `result` | `DataValue` | The result string. |

* * *

{% anchor id="Count" /%}
### Count

Counts the number of solutions of a query.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `count` | `DataValue` | The count of the number of solutions. |
| `query` | `Query` | The query from which to obtain the count. |

* * *

{% anchor id="Data" /%}
### Data

Specify an edge pattern which is terminal, and provides a data value association.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `DataValue` | A data type or variable which is the target or object of the graph edge. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |

* * *

{% anchor id="DateDuration" /%}
### DateDuration

Tri-directional duration arithmetic for dates and dateTimes. Given any two of start, end, and duration, computes the third. Accepts xsd:date or xsd:dateTime for start/end and xsd:duration for duration. Uses end-of-month (EOM) preservation: if the input is the last day of its month, the result will be the last day of the target month. Duration output omits time components when they are zero.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `duration` | `DataValue` | The xsd:duration between start and end. |
| `end` | `DataValue` | The end date or dateTime. |
| `start` | `DataValue` | The start date or dateTime. |

* * *

{% anchor id="DayAfter" /%}
### DayAfter

Computes the calendar day after the given date. Bidirectional: given date computes next, given next computes date.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `date` | `DataValue` | The input date. |
| `next` | `DataValue` | The next calendar day. |

* * *

{% anchor id="DayBefore" /%}
### DayBefore

Computes the calendar day before the given date. Bidirectional: given date computes previous, given previous computes date.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `date` | `DataValue` | The input date. |
| `previous` | `DataValue` | The previous calendar day. |

* * *

{% anchor id="DeletedLink" /%}
### DeletedLink

An edge pattern specifying a link beween nodes deleted *at this commit*.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `NodeValue` | A URI or variable which is the target or object of the graph edge. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |

* * *

{% anchor id="DeleteDocument" /%}
### DeleteDocument

Delete a document from the graph.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `identifier` | `NodeValue` | An identifier specifying the documentation location to delete. |

* * *

{% anchor id="DeletedTriple" /%}
### DeletedTriple

Specify an edge pattern which was *deleted* at *this commit*.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `Value` | A URI, datatype or variable which is the target or object of the graph edge. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |

* * *

{% anchor id="DeleteLink" /%}
### DeleteLink

Delete an edge linking nodes.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. The variable must be bound. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. The variable must be bound. |
| `object` | `NodeValue` | A URI or variable which is the target or object of the graph edge. The variable must be bound. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |

* * *

{% anchor id="DeleteTriple" /%}
### DeleteTriple

Specify an edge pattern to remove from the graph.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `Value` | A URI, datatype or variable which is the target or object of the graph edge. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |

* * *

{% anchor id="Distinct" /%}
### Distinct

Ensure variables listed result in distinct solutions.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `query` | `Query` | The query which will be run prior to selection. |
| `variables` | List(`xsd:string`) | The variables which must be distinct from the query. |

* * *

{% anchor id="Dot" /%}
### Dot

Extract the value of a key in a bound document.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `document` | `DataValue` | Document which is being accessed. |
| `field` | `DataValue` | The field from which the document which is being accessed. |
| `value` | `DataValue` | The value for the document and field. |

* * *

{% anchor id="Equals" /%}
### Equals

True whenever 'left' is the same as 'right'. Performs unification.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `left` | `DataValue` | A URI, data value or variable. |
| `right` | `DataValue` | A URI, data value or variable. |

* * *

{% anchor id="Eval" /%}
### Eval

Evaluate an arithmetic expression to obtain a result.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `expression` | `ArithmeticExpression` | The expression to be evaluated. |
| `result` | `ArithmeticValue` | The numeric result. |

* * *

{% anchor id="From" /%}
### From

Change the default read graph (between instance/schema).

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `graph` | `xsd:string` | The graph filter: 'schema' or 'instance' or '*'. |
| `query` | `Query` | The subquery with a new default graph. |

* * *

{% anchor id="Get" /%}
### Get

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `columns` | List(`Column`) |  |
| `has_header` | Optional(`xsd:boolean`) |  |
| `resource` | `QueryResource` |  |

* * *

{% anchor id="Greater" /%}
### Greater

Predicate determining if one thing is greater than another according to natural ordering.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `left` | `DataValue` | The greater element. |
| `right` | `DataValue` | The lesser element. |

* * *

{% anchor id="GroupBy" /%}
### GroupBy

Group a query into a list with each element of the list specified by 'template' using a given variable set for the group.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `group_by` | List(`xsd:string`) | The variables which should be grouped into like solutions. |
| `grouped` | `Value` | The final list of templated solutions. |
| `query` | `Query` | The subquery providing the solutions for the grouping. |
| `template` | `Value` | The template of elements in the result list. |

* * *

{% anchor id="Gte" /%}
### Gte

Predicate determining if one thing is greater than or equal to another according to natural ordering.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `left` | `DataValue` | The greater or equal element. |
| `right` | `DataValue` | The lesser or equal element. |

* * *

{% anchor id="HashKey" /%}
### HashKey

Generates a key identical to those generated automatically by 'HashKey' specifications.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `base` | `DataValue` | The URI base to the left of the key. |
| `key_list` | List(`DataValue`) | List of data elements required to generate the key. |
| `uri` | `NodeValue` | The resulting URI. |

* * *

{% anchor id="If" /%}
### If

A conditional which runs the then clause for every success from the test clause, otherwise runs the else clause.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `else` | `Query` | A query which runs whenever test fails. |
| `test` | `Query` | A query which will provide bindings for the then clause. |
| `then` | `Query` | A query which will run for every solution of test with associated bindings. |

* * *

{% anchor id="Immediately" /%}
### Immediately

Attempts to perform all side-effecting operations immediately. Can have strange non-backtracking effects but can also increase performance. Use at your own risk.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `query` | `Query` | The query from which to obtain the side-effects. |

* * *

{% anchor id="InRange" /%}
### InRange

Predicate testing whether a value falls within a half-open range [Start, End). Succeeds if Start <= Value < End.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `value` | `DataValue` | The value to test. |
| `start` | `DataValue` | The inclusive lower bound. |
| `end` | `DataValue` | The exclusive upper bound. |

* * *

{% anchor id="InsertDocument" /%}
### InsertDocument

Insert a document in the graph.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `document` | `Value` | The document to insert. Must either have an '@id' or have a class specified key. |
| `identifier` | Optional(`NodeValue`) | An optional returned identifier specifying the documentation location. |

* * *

{% anchor id="Interval" /%}
### Interval

Constructs or deconstructs a half-open xdd:dateTimeInterval [start, end) from two date or dateTime endpoints. Bidirectional: given start+end computes interval, given interval extracts start+end. Endpoints are stored in UTC canonical form for correct lexical ordering.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `start` | `DataValue` | Inclusive start date or dateTime. |
| `end` | `DataValue` | Exclusive end date or dateTime. |
| `interval` | `DataValue` | The xdd:dateTimeInterval value. |

* * *

{% anchor id="IntervalDurationEnd" /%}
### IntervalDurationEnd

Relates an xdd:dateTimeInterval to its end endpoint and precise xsd:duration. Bidirectional: given interval extracts duration+end, given duration+end computes interval. Duration is a precise day/time count.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `duration` | `DataValue` | The xsd:duration between start and end. |
| `end` | `DataValue` | Exclusive end date or dateTime. |
| `interval` | `DataValue` | The xdd:dateTimeInterval value. |

* * *

{% anchor id="IntervalRelation" /%}
### IntervalRelation

Allen's Interval Algebra: classifies or validates the temporal relationship between two half-open intervals [x_start, x_end) and [y_start, y_end). When relation is ground, validates that the named relation holds. When relation is a variable, determines which of the 13 Allen relations holds (deterministic). Supported relations: before, after, meets, met_by, overlaps, overlapped_by, starts, started_by, during, contains, finishes, finished_by, equals.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `relation` | `DataValue` | The Allen relation name (string) or variable. |
| `x_start` | `DataValue` | Inclusive start of interval X. |
| `x_end` | `DataValue` | Exclusive end of interval X. |
| `y_start` | `DataValue` | Inclusive start of interval Y. |
| `y_end` | `DataValue` | Exclusive end of interval Y. |

* * *

{% anchor id="IntervalRelationTyped" /%}
### IntervalRelationTyped

Allen's Interval Algebra on xdd:dateTimeInterval values. Classifies or validates the temporal relationship between two interval values. When relation is ground, validates that the named relation holds. When relation is a variable, determines which of the 13 Allen relations holds (deterministic). Supported relations: before, after, meets, met_by, overlaps, overlapped_by, starts, started_by, during, contains, finishes, finished_by, equals.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `relation` | `DataValue` | The Allen relation name (string) or variable. |
| `x` | `DataValue` | The first xdd:dateTimeInterval. |
| `y` | `DataValue` | The second xdd:dateTimeInterval. |

* * *

{% anchor id="IntervalStartDuration" /%}
### IntervalStartDuration

Relates an xdd:dateTimeInterval to its start endpoint and precise xsd:duration. Bidirectional: given interval extracts start+duration, given start+duration computes interval. Duration is a precise day/time count.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `start` | `DataValue` | Inclusive start date or dateTime. |
| `duration` | `DataValue` | The xsd:duration between start and end. |
| `interval` | `DataValue` | The xdd:dateTimeInterval value. |

* * *

{% anchor id="Into" /%}
### Into

Change the default write graph (between instance/schema).

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `graph` | `xsd:string` | The graph filter: schema or instance. |
| `query` | `Query` | The subquery with a new default write graph. |

* * *

{% anchor id="IsA" /%}
### IsA

Test (or generate) the type of an element.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `element` | `NodeValue` | The element to test. |
| `type` | `NodeValue` | The type of the element. |

* * *

{% anchor id="IsoWeek" /%}
### IsoWeek

Computes the ISO 8601 week-numbering year and week number for a date. Accepts xsd:date or xsd:dateTime. Date must be ground. The ISO year may differ from the calendar year at year boundaries.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `date` | `DataValue` | The input date or dateTime. |
| `week` | `DataValue` | The ISO week number (1-53). |
| `year` | `DataValue` | The ISO week-numbering year. |

* * *

{% anchor id="Join" /%}
### Join

Join a list of strings using 'separator'.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `list` | `DataValue` | The list to concatenate. |
| `result` | `DataValue` | The result string. |
| `separator` | `DataValue` | The separator between each joined string |

* * *

{% anchor id="Length" /%}
### Length

The length of a list.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `length` | `DataValue` | The length of the list. |
| `list` | `DataValue` | The list of which to find the length. |

* * *

{% anchor id="Less" /%}
### Less

Predicate determining if one thing is less than another according to natural ordering.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `left` | `DataValue` | The lesser element. |
| `right` | `DataValue` | The greater element. |

* * *

{% anchor id="LexicalKey" /%}
### LexicalKey

Generates a key identical to those generated automatically by 'LexicalKey' specifications.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `base` | `DataValue` | The URI base to the left of the key. |
| `key_list` | List(`DataValue`) | List of data elements required to generate the key. |
| `uri` | `NodeValue` | The resulting URI. |

* * *

{% anchor id="Like" /%}
### Like

Distance between strings, similar to a Levenstein distance.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `left` | `DataValue` | The first string. |
| `right` | `DataValue` | The second string. |
| `similarity` | `DataValue` | Number between -1 and 1 which gives a scale for similarity. |

* * *

{% anchor id="Limit" /%}
### Limit

Limit a query to a particular maximum number of solutions specified by 'limit'. Can be used with start to perform paging.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `limit` | `xsd:nonNegativeInteger` | Maximum number of solutions. |
| `query` | `Query` | The query to perform. |

* * *

{% anchor id="Link" /%}
### Link

Specify an edge pattern which is not terminal, but a link between objects.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `NodeValue` | A URI or variable which is the target or object of the graph edge. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |

* * *

{% anchor id="ListToSet" /%}
### ListToSet

Converts a list to a set by removing duplicates and sorting.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `list` | `DataValue` | The input list. |
| `set` | `DataValue` | The resulting set with duplicates removed. |

* * *

{% anchor id="Lower" /%}
### Lower

Lowercase a string.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `lower` | `DataValue` | The lower case string. |
| `mixed` | `DataValue` | The mixed case string. |

* * *

{% anchor id="Lte" /%}
### Lte

Predicate determining if one thing is less than or equal to another according to natural ordering.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `left` | `DataValue` | The lesser or equal element. |
| `right` | `DataValue` | The greater or equal element. |

* * *

{% anchor id="Member" /%}
### Member

Generate or test every element of a list.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `list` | `DataValue` | The list of elements against which to generate or test. |
| `member` | `DataValue` | The element to test for membership or to supply as generated. |

* * *

{% anchor id="MonthEndDate" /%}
### MonthEndDate

Computes the last day of the month for a given year-month. Handles leap years correctly. YearMonth must be ground.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `year_month` | `DataValue` | A gYearMonth value (e.g. 2024-02). |
| `date` | `DataValue` | The resulting xsd:date for the last day of the month. |

* * *

{% anchor id="MonthEndDates" /%}
### MonthEndDates

Generator: produces every last-of-month date in the half-open range [Start, End).

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `date` | `DataValue` | The generated last-of-month date. |
| `start` | `DataValue` | The inclusive start date. |
| `end` | `DataValue` | The exclusive end date. |

* * *

{% anchor id="MonthStartDate" /%}
### MonthStartDate

Computes the first day of the month for a given year-month. YearMonth must be ground.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `year_month` | `DataValue` | A gYearMonth value (e.g. 2024-01). |
| `date` | `DataValue` | The resulting xsd:date for the first day of the month. |

* * *

{% anchor id="MonthStartDates" /%}
### MonthStartDates

Generator: produces every first-of-month date in the half-open range [Start, End).

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `date` | `DataValue` | The generated first-of-month date. |
| `start` | `DataValue` | The inclusive start date. |
| `end` | `DataValue` | The exclusive end date. |

* * *

{% anchor id="Not" /%}
### Not

The negation of a query. Provides no solution bindings, but will succeed if its sub-query fails.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `query` | `Query` | The query which must not hold. |

* * *

{% anchor id="Once" /%}
### Once

Obtains exactly one solution from a query. Simliar to a limit of 1.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `query` | `Query` | The query from which to obtain a solution. |

* * *

{% anchor id="Optional" /%}
### Optional

A query which will succeed (without bindings) even in the case of failure.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `query` | `Query` | The query to run. |

* * *

{% anchor id="Or" /%}
### Or

A disjunction of queries any of which can provide a solution.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `or` | List(`Query`) | List of queries which may hold. |

* * *

{% anchor id="OrderBy" /%}
### OrderBy

Orders query results according to an ordering specification.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `ordering` | List(`OrderTemplate`) | A specification of the ordering of solutions. |
| `query` | `Query` | The base query giving the solutions to order. |

* * *

{% anchor id="Pad" /%}
### Pad

Pad a string.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `char` | `DataValue` | The padding character. |
| `result` | `DataValue` | The result of the padding as a string. |
| `string` | `DataValue` | The starting string. |
| `times` | `DataValue` | The number of times to repeat the padding character. |

* * *

{% anchor id="Path" /%}
### Path

Find a path through the graph according to 'pattern'. This 'pattern' is a regular graph expression which avoids cycles.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `Value` | The starting node. |
| `object` | `Value` | The ending node. |
| `path` | Optional(`Value`) | An optional list of edges traversed. |
| `pattern` | `PathPattern` | The pattern which describes how to traverse edges. |

* * *

{% anchor id="Pin" /%}
### Pin

Keep a subquery from being optimized, 'Pin' it in the order given

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `query` | `Query` | The query to pin |

* * *

{% anchor id="RandomKey" /%}
### RandomKey

Generates a key identical to those generated automatically by 'RandomKey' specifications.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `base` | `DataValue` | The URI base to the left of the key. |
| `uri` | `NodeValue` | The resulting URI. |

* * *

{% anchor id="RangeMax" /%}
### RangeMax

Find the maximum value in a list using the standard ordering (woql_less). Works with any comparable types: numbers, dates, strings. Empty list produces no bindings.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `list` | `DataValue` | The list of values to search. |
| `result` | `DataValue` | The maximum value found. |

* * *

{% anchor id="RangeMin" /%}
### RangeMin

Find the minimum value in a list using the standard ordering (woql_less). Works with any comparable types: numbers, dates, strings. Empty list produces no bindings.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `list` | `DataValue` | The list of values to search. |
| `result` | `DataValue` | The minimum value found. |

* * *

{% anchor id="ReadDocument" /%}
### ReadDocument

Read a full document from an identifier.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `document` | `Value` | Variable which will be bound to the document. |
| `identifier` | `NodeValue` | The URI of the document to load. |

* * *

{% anchor id="Regexp" /%}
### Regexp

Test a string against a PCRE style regex pattern.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `pattern` | `DataValue` | The PCRE style pattern. |
| `result` | Optional(`DataValue`) | An optional result list of matches. |
| `string` | `DataValue` | The string to test. |

* * *

{% anchor id="Select" /%}
### Select

Select specific variables from a query to return.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `query` | `Query` | The query which will be run prior to selection. |
| `variables` | List(`xsd:string`) | The variables to select from the query. |

* * *

{% anchor id="Sequence" /%}
### Sequence

Generates a sequence of values in the half-open range [Start, End). When Value is unbound, produces each value via backtracking. Supports integer and decimal types.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `value` | `DataValue` | The generated sequence value. |
| `start` | `DataValue` | The inclusive start of the sequence. |
| `end` | `DataValue` | The exclusive end of the sequence. |
| `step` | Optional(`DataValue`) | Optional increment per step. Defaults to 1 for integers, 1.0 for decimals. |
| `count` | Optional(`DataValue`) | Optional total count. If bound, validates. If unbound, unifies with actual count. |

* * *

{% anchor id="SetDifference" /%}
### SetDifference

Computes the set difference of two lists (elements in list_a but not in list_b).

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `list_a` | `DataValue` | The first list. |
| `list_b` | `DataValue` | The second list. |
| `result` | `DataValue` | The resulting set difference. |

* * *

{% anchor id="SetIntersection" /%}
### SetIntersection

Computes the set intersection of two lists (elements in both list_a and list_b).

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `list_a` | `DataValue` | The first list. |
| `list_b` | `DataValue` | The second list. |
| `result` | `DataValue` | The resulting set intersection. |

* * *

{% anchor id="SetMember" /%}
### SetMember

Tests if an element is a member of a set.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `element` | `DataValue` | The element to check for membership. |
| `set` | `DataValue` | The set (list) to check membership in. |

* * *

{% anchor id="SetUnion" /%}
### SetUnion

Computes the set union of two lists (elements in either list_a or list_b).

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `list_a` | `DataValue` | The first list. |
| `list_b` | `DataValue` | The second list. |
| `result` | `DataValue` | The resulting set union. |

* * *

{% anchor id="Size" /%}
### Size

Size of a database in magic units (bytes?).

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `resource` | `xsd:string` | The resource to obtain the size of. |
| `size` | `DataValue` | The size. |

* * *

{% anchor id="Slice" /%}
### Slice

Extracts a contiguous subsequence from a list.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `end` | Optional(`DataValue`) | The end index (exclusive, optional). |
| `list` | `DataValue` | The input list to slice. |
| `result` | `DataValue` | The resulting sliced list. |
| `start` | `DataValue` | The start index (0-based). |

* * *

{% anchor id="Split" /%}
### Split

Split a string.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `list` | `DataValue` | The result list of strings. |
| `pattern` | `DataValue` | The splitting pattern. |
| `string` | `DataValue` | The starting string. |

* * *

{% anchor id="Start" /%}
### Start

Start a query at the nth solution specified by 'start'. Allows resumption and paging of queries.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `query` | `Query` | The query to perform. |
| `start` | `xsd:nonNegativeInteger` | The numbered solution to start at. |

* * *

{% anchor id="Substring" /%}
### Substring

Finds the boundaries of a substring in a string.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `after` | `DataValue` | The count of characters after substring as an integer or variable. |
| `before` | `DataValue` | The count of characters before substring as an integer or variable. |
| `length` | `DataValue` | The length of the string as an integer or variable. |
| `string` | `DataValue` | The super-string as data or variable. |
| `substring` | `DataValue` | The super-string as data or variable. |

* * *

{% anchor id="Subsumption" /%}
### Subsumption

Provides class subsumption (the inheritance model) according to the schema design. True whenver 'child' is a child of 'parent'. Can be used as a generator or a check.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `child` | `NodeValue` | The child class as a URI or variable. |
| `parent` | `NodeValue` | The parent class as a URI or variable |

* * *

{% anchor id="Sum" /%}
### Sum

Sum a list of strings.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `list` | `DataValue` | The list of numbers to sum. |
| `result` | `DataValue` | The result of the sum as a number. |

* * *

{% anchor id="Trim" /%}
### Trim

Trims whitespace from 'untrimmed'.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `trimmed` | `DataValue` | The string to be trimmed. |
| `untrimmed` | `DataValue` | The untrimmed string. |

* * *

{% anchor id="Triple" /%}
### Triple

Specify an edge pattern in the graph.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `Value` | A URI, datatype or variable which is the target or object of the graph edge. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |

* * *

{% anchor id="TripleCount" /%}
### TripleCount

The number of edges in a database.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `count` | `DataValue` | The count of edges. |
| `resource` | `xsd:string` | The resource to obtain the edges from. |

* * *

{% anchor id="TripleNext" /%}
### TripleNext

Find the next object value after a reference for a given subject-predicate pair. When object is bound and next is free, finds the smallest next value greater than object. When next is bound and object is free, finds the largest object value less than next.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `Value` | The current object value or the result of lookup by next. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |
| `next` | `Value` | The next object value or the reference upper bound. |

* * *

{% anchor id="TriplePrevious" /%}
### TriplePrevious

Find the previous object value before a reference for a given subject-predicate pair. When object is bound and previous is free, finds the largest previous value less than object. When previous is bound and object is free, finds the smallest object value greater than previous.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `Value` | The current object value or the result of lookup by previous. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |
| `previous` | `Value` | The previous object value or the reference lower bound. |

* * *

{% anchor id="TripleSlice" /%}
### TripleSlice

Specify an edge pattern with a half-open value range [low, high) on the object. Returns triples whose object value falls within the range.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `Value` | A URI, datatype or variable which is the target or object of the graph edge. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |
| `high` | `Value` | The exclusive upper bound of the value range. |
| `low` | `Value` | The inclusive lower bound of the value range. |

* * *

{% anchor id="TripleSliceRev" /%}
### TripleSliceRev

Specify an edge pattern with a half-open value range [low, high) on the object, returning triples in reverse (descending) object order. Same semantics as TripleSlice but iterates from highest to lowest value.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `subject` | `NodeValue` | A URI or variable which is the source or subject of the graph edge. |
| `predicate` | `NodeValue` | A URI or variable which is the edge-label or predicate of the graph edge. |
| `object` | `Value` | A URI, datatype or variable which is the target or object of the graph edge. |
| `graph` | Optional(`xsd:string`) | An optional graph (either 'instance' or 'schema') |
| `high` | `Value` | The exclusive upper bound of the value range. |
| `low` | `Value` | The inclusive lower bound of the value range. |

* * *

{% anchor id="True" /%}
### True

The query which is always true.

**Inherits:** `Query`

* * *

{% anchor id="Typecast" /%}
### Typecast

Casts one type as another if possible.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `result` | `Value` | The resulting value after cast. |
| `type` | `NodeValue` | The type to which to cast. |
| `value` | `Value` | The value to cast. |

* * *

{% anchor id="TypeOf" /%}
### TypeOf

TypeOf gives the type of an element.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `type` | `NodeValue` | The URI which that specifies the type. |
| `value` | `Value` | The value of which to obtain the type. |

* * *

{% anchor id="UpdateDocument" /%}
### UpdateDocument

Update a document in the graph.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `document` | `Value` | The document to update. Must either have an '@id' or have a class specified key. |
| `identifier` | Optional(`NodeValue`) | An optional returned identifier specifying the documentation location. |

* * *

{% anchor id="Upper" /%}
### Upper

Uppercase a string.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `mixed` | `DataValue` | The mixed case string. |
| `upper` | `DataValue` | The upper case string. |

* * *

{% anchor id="Using" /%}
### Using

Select a specific collection for query.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `collection` | `xsd:string` | The resource over which to run the query. |
| `query` | `Query` | The query which will be run on the selected collection. |

* * *

{% anchor id="Weekday" /%}
### Weekday

Computes the ISO 8601 weekday number (Monday=1, Sunday=7) for a date. Accepts xsd:date or xsd:dateTime. Date must be ground.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `date` | `DataValue` | The input date or dateTime. |
| `weekday` | `DataValue` | The ISO weekday number (1=Monday, 7=Sunday). |

* * *

{% anchor id="WeekdaySundayStart" /%}
### WeekdaySundayStart

Computes the US-convention weekday number (Sunday=1, Saturday=7) for a date. Accepts xsd:date or xsd:dateTime. Date must be ground.

**Inherits:** `Query`


| Property | Type | Description |
|----------|------|-------------|
| `date` | `DataValue` | The input date or dateTime. |
| `weekday` | `DataValue` | The US weekday number (1=Sunday, 7=Saturday). |

* * *

---

## Path patterns

Path patterns describe how to traverse edges in the graph. They form a regular expression-like language over graph edges.

{% anchor id="PathPattern" /%}
### PathPattern

**Abstract**

* * *

{% anchor id="InversePathPredicate" /%}
### InversePathPredicate

A predicate to traverse *backwards*.

**Inherits:** `PathPattern`


| Property | Type | Description |
|----------|------|-------------|
| `predicate` | Optional(`xsd:string`) | The predicate to use in reverse direction in the pattern traversal. |

* * *

{% anchor id="PathOr" /%}
### PathOr

A set of patterns in which each of the patterns can result in objects starting from our current subject set.

**Inherits:** `PathPattern`


| Property | Type | Description |
|----------|------|-------------|
| `or` | List(`PathPattern`) | A disjunction of path patterns. |

* * *

{% anchor id="PathPlus" /%}
### PathPlus

The path pattern specified by 'plus' must hold one or more times in succession.

**Inherits:** `PathPattern`


| Property | Type | Description |
|----------|------|-------------|
| `plus` | `PathPattern` | A path patterns. |

* * *

{% anchor id="PathPredicate" /%}
### PathPredicate

A predicate to traverse.

**Inherits:** `PathPattern`


| Property | Type | Description |
|----------|------|-------------|
| `predicate` | Optional(`xsd:string`) | The predicate to use in the pattern traversal. |

* * *

{% anchor id="PathSequence" /%}
### PathSequence

A sequence of patterns in which each of the patterns in the list must result in objects which are subjects of the next pattern in the list.

**Inherits:** `PathPattern`


| Property | Type | Description |
|----------|------|-------------|
| `sequence` | List(`PathPattern`) | A sequence of path patterns. |

* * *

{% anchor id="PathStar" /%}
### PathStar

The path pattern specified by 'star' may hold zero or more times in succession.

**Inherits:** `PathPattern`


| Property | Type | Description |
|----------|------|-------------|
| `star` | `PathPattern` | A path pattern. |

* * *

{% anchor id="PathTimes" /%}
### PathTimes

The path pattern specified by 'times' may hold 'from' to 'to' times in succession.

**Inherits:** `PathPattern`


| Property | Type | Description |
|----------|------|-------------|
| `from` | `xsd:nonNegativeInteger` | The number of times to start the repetition of the pattern |
| `times` | `PathPattern` | A path pattern. |
| `to` | `xsd:nonNegativeInteger` | The number of times after which to end the repeition of the pattern. |

* * *

---

## Arithmetic expressions

Arithmetic expression classes for numeric computations within queries.

{% anchor id="ArithmeticExpression" /%}
### ArithmeticExpression

An abstract class specifying the AST super-class of all arithemtic expressions.

**Abstract**

* * *

{% anchor id="ArithmeticValue" /%}
### ArithmeticValue

A variable or node.

**Tagged Union** · **Inherits:** `ArithmeticExpression`


| Property | Type | Description |
|----------|------|-------------|
| `data` | `xsd:anySimpleType` | An xsd data type value. |
| `variable` | `xsd:string` | A variable. |

* * *

{% anchor id="Div" /%}
### Div

Integer divide two numbers.

**Inherits:** `ArithmeticExpression`


| Property | Type | Description |
|----------|------|-------------|
| `left` | `ArithmeticExpression` | First operand of div. |
| `right` | `ArithmeticExpression` | Second operand of div. |

* * *

{% anchor id="Divide" /%}
### Divide

Divide two numbers.

**Inherits:** `ArithmeticExpression`


| Property | Type | Description |
|----------|------|-------------|
| `left` | `ArithmeticExpression` | First operand of divide. |
| `right` | `ArithmeticExpression` | Second operand of divide. |

* * *

{% anchor id="Exp" /%}
### Exp

Exponentiate a number.

**Inherits:** `ArithmeticExpression`


| Property | Type | Description |
|----------|------|-------------|
| `left` | `ArithmeticExpression` | The base. |
| `right` | `ArithmeticExpression` | The exponent. |

* * *

{% anchor id="Floor" /%}
### Floor

Find the integral part of a number.

**Inherits:** `ArithmeticExpression`


| Property | Type | Description |
|----------|------|-------------|
| `argument` | `ArithmeticExpression` | The number to floor. |

* * *

{% anchor id="Minus" /%}
### Minus

Subtract two numbers.

**Inherits:** `ArithmeticExpression`


| Property | Type | Description |
|----------|------|-------------|
| `left` | `ArithmeticExpression` | First operand of minus. |
| `right` | `ArithmeticExpression` | Second operand of minus. |

* * *

{% anchor id="Plus" /%}
### Plus

Add two numbers.

**Inherits:** `ArithmeticExpression`


| Property | Type | Description |
|----------|------|-------------|
| `left` | `ArithmeticExpression` | First operand of add. |
| `right` | `ArithmeticExpression` | Second operand of add. |

* * *

{% anchor id="Times" /%}
### Times

Multiply two numbers.

**Inherits:** `ArithmeticExpression`


| Property | Type | Description |
|----------|------|-------------|
| `left` | `ArithmeticExpression` | First operand of times. |
| `right` | `ArithmeticExpression` | Second operand of times. |

* * *

---

{% anchor id="utility-types" /%}
## Utility types

Support types used as field values in query classes — values, resources, columns, and enumerations.

{% anchor id="Column" /%}
### Column


| Property | Type | Description |
|----------|------|-------------|
| `indicator` | `Indicator` |  |
| `type` | Optional(`xsd:string`) |  |
| `variable` | `xsd:string` |  |

* * *

{% anchor id="DataValue" /%}
### DataValue

A variable or node.

**Tagged Union**


| Property | Type | Description |
|----------|------|-------------|
| `data` | `xsd:anySimpleType` | An xsd data type value. |
| `list` | List(`DataValue`) | A list of datavalues |
| `variable` | `xsd:string` | A variable. |

* * *

{% anchor id="DictionaryTemplate" /%}
### DictionaryTemplate

A representation of a JSON style dictionary, but with free variables. It is similar to an interpolated string in that it is a template with quoted data and substituted values.


| Property | Type | Description |
|----------|------|-------------|
| `data` | Set(`FieldValuePair`) | Pairs of Key-Values to be constructed into a dictionary |

* * *

{% anchor id="FieldValuePair" /%}
### FieldValuePair

A representation of a JSON style dictionary, but with free variables. It is similar to an interpolated string in that it is a template with quoted data and substituted values.


| Property | Type | Description |
|----------|------|-------------|
| `field` | `xsd:string` | The field or key of a dictionary value pair |
| `value` | `Value` | The value of a dictionary value pair. |

* * *

{% anchor id="FormatType" /%}
### FormatType

**Enum**

**Values:** `csv`


| Property | Type | Description |
|----------|------|-------------|
| `@value` | ["csv"] |  |

* * *

{% anchor id="Indicator" /%}
### Indicator

**Tagged Union**


| Property | Type | Description |
|----------|------|-------------|
| `index` | `xsd:nonNegativeInteger` |  |
| `name` | `xsd:string` |  |

* * *

{% anchor id="NamedParametricQuery" /%}
### NamedParametricQuery

A named parametric query which names a specific query for later retrieval and re-use and allows the specification of bindings for a specific set of variables in the query.


| Property | Type | Description |
|----------|------|-------------|
| `name` | `xsd:string` | The name of the NamedParametricQuery to be retrieved. |
| `parameters` | List(`xsd:string`) | Variable name list for auxilliary bindings. |
| `query` | `Query` | The query AST as WOQL JSON. |

* * *

{% anchor id="NamedQuery" /%}
### NamedQuery

A named query names a specific query for later retrieval and re-use.


| Property | Type | Description |
|----------|------|-------------|
| `name` | `xsd:string` | The name of the NamedQuery to be retrieved |
| `query` | `Query` | The query AST as WOQL JSON |

* * *

{% anchor id="NodeValue" /%}
### NodeValue

A variable or node.

**Tagged Union**


| Property | Type | Description |
|----------|------|-------------|
| `node` | `xsd:string` | A URI representing a resource. |
| `variable` | `xsd:string` | A variable. |

* * *

{% anchor id="Order" /%}
### Order

**Enum**

**Values:** `asc`, `desc`


| Property | Type | Description |
|----------|------|-------------|
| `@value` | ["asc","desc"] |  |

* * *

{% anchor id="OrderTemplate" /%}
### OrderTemplate

The order template, consisting of the variable and ordering direction.


| Property | Type | Description |
|----------|------|-------------|
| `order` | `Order` | An enum either 'asc' or 'desc'. |
| `variable` | `xsd:string` | The variable to order. |

* * *

{% anchor id="QueryResource" /%}
### QueryResource

**Tagged Union**


| Property | Type | Description |
|----------|------|-------------|
| `format` | `FormatType` |  |
| `options` | Optional(`xdd:json`) |  |
| `source` | `Source` |  |

* * *

{% anchor id="Source" /%}
### Source

**Tagged Union**


| Property | Type | Description |
|----------|------|-------------|
| `post` | `xsd:string` |  |
| `url` | `xsd:string` |  |

* * *

{% anchor id="Value" /%}
### Value

A variable, node or data point.

**Tagged Union**


| Property | Type | Description |
|----------|------|-------------|
| `data` | `xsd:anySimpleType` | An xsd data type value. |
| `dictionary` | `DictionaryTemplate` |  |
| `list` | List(`Value`) | A list of datavalues |
| `node` | `xsd:string` | A URI representing a resource. |
| `variable` | `xsd:string` | A variable. |

* * *
