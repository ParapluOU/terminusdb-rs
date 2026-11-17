---
tags:
  - reference
  - schema
  - woql
title: XSD Data Types
nextjs:
  metadata:
    title: XSD Datatypes
    description: Comprehensive reference for using XSD data types with RDF and schema.
    keywords: xsd data types, datatypes, data model, document type, schema, information interoperability, xml datatypes, xml xsd datatypes
    alternates:
      canonical: https://terminusdb.com/docs/xsd-data-types/
---

TerminusDB leverages XML schema definition data types (XSD Datatypes) in schema definitions. This enables information interoperability with other systems that use XSD data types. This reference covers all supported types organized by namespace, including type hierarchy, storage details, and typecasting rules. This reference also includes an extended set of types that are specific to TerminusDB.

For schema definition syntax, see the [Schema Reference Guide](/docs/schema-reference-guide/). For more information on XML data types, see [XML Schema Part 2: Datatypes Second Edition](https://www.w3.org/TR/xmlschema-2/).

Extensions such as for iso 8601 intervals exist with the xdd prefix in terminusdb as they are not defined in the XML XSD Datatypes specification.

---

## Quick Reference

{% table %}
* Namespace
* Types
* Description
---
* **XSD Core**
* `xsd:string`, `xsd:boolean`, `xsd:decimal`, `xsd:double`, `xsd:float`
* Fundamental data types
---
* **XSD Integers**
* `xsd:integer`, `xsd:byte`, `xsd:short`, `xsd:int`, `xsd:long`, unsigned variants
* Signed and unsigned integer types with range constraints
---
* **XSD Date/Time**
* `xsd:dateTime`, `xsd:date`, `xsd:time`, `xsd:duration`, and Gregorian types
* Temporal data types with timezone support
---
* **XSD Strings**
* `xsd:normalizedString`, `xsd:token`, `xsd:language`, `xsd:Name`, `xsd:NCName`, `xsd:NMTOKEN`
* Constrained string types
---
* **XSD Binary**
* `xsd:base64Binary`, `xsd:hexBinary`
* Binary data encoding
---
* **RDF**
* `rdf:langString`, `rdf:PlainLiteral`, `rdf:XMLLiteral`
* RDF-specific literal types
---
* **XDD Extensions**
* `xdd:coordinate`, `xdd:coordinatePolygon`, `xdd:json`, `xdd:url`, `xdd:email`, `xdd:html`, range types, `xdd:dateTimeInterval`
* TerminusDB extension types for specialized data
---
* **System**
* `sys:JSON`, `sys:JSONDocument`, `sys:Dictionary`
* Used for JSON as literals and for document storage
{% /table %}

---

## Type Hierarchy

TerminusDB implements a type hierarchy where child types inherit from parent types. This enables type subsumption—a value of a child type can be used where a parent type is expected.

```
xsd:anySimpleType
├── xsd:string
│   ├── xsd:normalizedString
│   │   └── xsd:token
│   │       ├── xsd:language
│   │       ├── xsd:NMTOKEN
│   │       └── xsd:Name
│   │           └── xsd:NCName
│   ├── xdd:url
│   ├── xdd:email
│   └── xdd:html
├── xsd:boolean
├── xsd:decimal
│   ├── xsd:integer
│   │   ├── xsd:long
│   │   │   └── xsd:int
│   │   │       └── xsd:short
│   │   │           └── xsd:byte
│   │   ├── xsd:nonNegativeInteger
│   │   │   ├── xsd:positiveInteger
│   │   │   └── xsd:unsignedLong
│   │   │       └── xsd:unsignedInt
│   │   │           └── xsd:unsignedShort
│   │   │               └── xsd:unsignedByte
│   │   ├── xsd:nonPositiveInteger
│   │   │   └── xsd:negativeInteger
│   │   └── xdd:integerRange
│   └── xdd:decimalRange
├── xsd:double
├── xsd:float
├── xsd:dateTime
│   └── xsd:dateTimeStamp
├── xsd:date
│   └── xdd:dateRange
├── xsd:time
├── xsd:duration
│   ├── xsd:yearMonthDuration
│   ├── xsd:dayTimeDuration
│   └── xdd:dateTimeInterval
├── xsd:gYear
│   └── xdd:gYearRange
├── xsd:gMonth
├── xsd:gDay
├── xsd:gYearMonth
├── xsd:gMonthDay
├── xsd:base64Binary
├── xsd:hexBinary
└── xsd:anyURI

rdfs:Literal
└── rdf:langString
    └── rdf:PlainLiteral

xdd:json (standalone)
xdd:coordinate (standalone)
xdd:coordinatePolygon (standalone)
xdd:coordinatePolyline (standalone)

sys:JSON (system type)
sys:JSONDocument (system type)
sys:Dictionary (system type)
```

---

## XSD Core Types

### xsd:string

The fundamental string type. All string-derived types can be cast to and from `xsd:string`.

{% table %}
* Property
* Value
---
* **IRI**
* `http://www.w3.org/2001/XMLSchema#string`
---
* **Parent**
* `xsd:anySimpleType`
---
* **JSON Representation**
* JSON string
---
* **Example**
* `"Hello, World!"`
{% /table %}

### xsd:boolean

Boolean true/false values.

{% table %}
* Property
* Value
---
* **IRI**
* `http://www.w3.org/2001/XMLSchema#boolean`
---
* **Parent**
* `xsd:anySimpleType`
---
* **Valid Values**
* `true`, `false`, `1`, `0`
---
* **JSON Representation**
* JSON boolean (`true` or `false`)
---
* **Typecasting**
* String `"true"` or `"1"` → `true`; `"false"` or `"0"` → `false`
{% /table %}

### xsd:decimal

Arbitrary-precision decimal numbers. TerminusDB stores these internally as **rational numbers** to preserve exact precision.

{% table %}
* Property
* Value
---
* **IRI**
* `http://www.w3.org/2001/XMLSchema#decimal`
---
* **Parent**
* `xsd:anySimpleType`
---
* **Internal Storage**
* Arbitrary-precision rationals
---
* **JSON Serialization**
* JSON number (20 significant digits)
---
* **String Serialization**
* Minimal form without trailing zeros (e.g., `33` → `"33"`)
{% /table %}

**Precision Notes:**
- Input values are parsed as exact rationals (e.g., `0.5679` → `5679/10000`)
- JSON output uses 20 significant digits for decimal representation
- Round-trip conversion preserves exactness within precision limits

### xsd:double

IEEE 754 double-precision (64-bit) floating-point numbers.

{% table %}
* Property
* Value
---
* **IRI**
* `http://www.w3.org/2001/XMLSchema#double`
---
* **Parent**
* `xsd:anySimpleType`
---
* **Internal Storage**
* IEEE 754 binary64
---
* **Special Values**
* `INF`, `-INF`, `NaN`
---
* **String Serialization**
* Always includes decimal point (e.g., `33` → `"33.0"`)
{% /table %}

**Important:** `xsd:double` is **not** a subtype of `xsd:decimal`. They use different numeric representations.

### xsd:float

IEEE 754 single-precision (32-bit) floating-point numbers.

{% table %}
* Property
* Value
---
* **IRI**
* `http://www.w3.org/2001/XMLSchema#float`
---
* **Parent**
* `xsd:anySimpleType`
---
* **Internal Storage**
* IEEE 754 binary32
---
* **Special Values**
* `INF`, `-INF`, `NaN`
---
* **String Serialization**
* Always includes decimal point (e.g., `33` → `"33.0"`)
{% /table %}

**Important:** `xsd:float` is **not** a subtype of `xsd:decimal`. Like `xsd:double`, it uses IEEE 754 representation.

---

## XSD Integer Types

All integer types derive from `xsd:decimal` → `xsd:integer` and use **arbitrary-precision integers** (GMP integers in Prolog).

### xsd:integer

Arbitrary-precision signed integer with no range limits.

{% table %}
* Property
* Value
---
* **IRI**
* `http://www.w3.org/2001/XMLSchema#integer`
---
* **Parent**
* `xsd:decimal`
---
* **Range**
* Unlimited (arbitrary precision)
---
* **String Serialization**
* No decimal point (e.g., `33` → `"33"`)
{% /table %}

### Signed Integer Types

{% table %}
* Type
* Range
* Parent
---
* **xsd:long**
* −9,223,372,036,854,775,808 to 9,223,372,036,854,775,807 (64-bit)
* `xsd:integer`
---
* **xsd:int**
* −2,147,483,648 to 2,147,483,647 (32-bit)
* `xsd:long`
---
* **xsd:short**
* −32,768 to 32,767 (16-bit)
* `xsd:int`
---
* **xsd:byte**
* −128 to 127 (8-bit)
* `xsd:short`
{% /table %}

### Unsigned Integer Types

{% table %}
* Type
* Range
* Parent
---
* **xsd:unsignedLong**
* 0 to 18,446,744,073,709,551,615 (64-bit)
* `xsd:nonNegativeInteger`
---
* **xsd:unsignedInt**
* 0 to 4,294,967,295 (32-bit)
* `xsd:unsignedLong`
---
* **xsd:unsignedShort**
* 0 to 65,535 (16-bit)
* `xsd:unsignedInt`
---
* **xsd:unsignedByte**
* 0 to 255 (8-bit)
* `xsd:unsignedShort`
{% /table %}

### Constrained Integer Types

{% table %}
* Type
* Constraint
* Parent
---
* **xsd:positiveInteger**
* > 0
* `xsd:nonNegativeInteger`
---
* **xsd:nonNegativeInteger**
* ≥ 0
* `xsd:integer`
---
* **xsd:negativeInteger**
* < 0
* `xsd:nonPositiveInteger`
---
* **xsd:nonPositiveInteger**
* ≤ 0
* `xsd:integer`
{% /table %}

**Typecasting:** Integer types validate range constraints during casting. Attempting to cast `-1` to `xsd:nonNegativeInteger` throws a casting error.

---

## XSD Date and Time Types

### xsd:dateTime

Combined date and time with optional timezone.

{% table %}
* Property
* Value
---
* **IRI**
* `http://www.w3.org/2001/XMLSchema#dateTime`
---
* **Format**
* `YYYY-MM-DDTHH:MM:SS[.sss][Z|±HH:MM]`
---
* **Example**
* `"2012-10-09T00:00:00Z"`
---
* **Internal**
* `date_time(Year, Month, Day, Hour, Minute, Second, Offset)`
{% /table %}

### xsd:dateTimeStamp

DateTime that **requires** a timezone. Interchangeable with `xsd:dateTime` in practice.

### xsd:date

Calendar date without time component.

{% table %}
* Property
* Value
---
* **IRI**
* `http://www.w3.org/2001/XMLSchema#date`
---
* **Format**
* `YYYY-MM-DD[Z|±HH:MM]`
---
* **Example**
* `"2023-12-25"`
{% /table %}

### xsd:time

Time of day without date component.

{% table %}
* Property
* Value
---
* **IRI**
* `http://www.w3.org/2001/XMLSchema#time`
---
* **Format**
* `HH:MM:SS[.sss][Z|±HH:MM]`
---
* **Example**
* `"14:30:00Z"`
{% /table %}

### Duration Types

{% table %}
* Type
* Format
* Example
---
* **xsd:duration**
* `PnYnMnDTnHnMnS`
* `"P1Y2M3DT4H5M6S"`
---
* **xsd:yearMonthDuration**
* `PnYnM`
* `"P1Y6M"`
---
* **xsd:dayTimeDuration**
* `PnDTnHnMnS`
* `"P5DT3H"`
{% /table %}

Durations keep their denominations — `P2W` ≠ `P14D`, and a leading `-` negates the whole duration (`-P1D` is legal). The full `xsd:duration` specification — grammar, ordering and the 12.1 storage format — is available for study under NDA together with the interval specification below.

### Gregorian Types

{% table %}
* Type
* Format
* Example
---
* **xsd:gYear**
* `YYYY[Z|±HH:MM]`
* `"1990"`
---
* **xsd:gMonth**
* `--MM[Z|±HH:MM]`
* `"--12"`
---
* **xsd:gDay**
* `---DD[Z|±HH:MM]`
* `"---25"`
---
* **xsd:gYearMonth**
* `YYYY-MM[Z|±HH:MM]`
* `"2023-12"`
---
* **xsd:gMonthDay**
* `--MM-DD[Z|±HH:MM]`
* `"--12-25"`
{% /table %}

**Typecasting:** `xsd:dateTime` can be cast to `xsd:decimal` to obtain a Unix timestamp.

---

## XSD String-Derived Types

### xsd:normalizedString

String with no carriage returns, line feeds, or tabs.

### xsd:token

Normalized string with no leading/trailing whitespace and no consecutive spaces.

### xsd:language

BCP 47 language tag (e.g., `"en"`, `"en-US"`, `"fr-CA"`).

### xsd:NMTOKEN

XML name token—letters, digits, hyphens, underscores, colons, and periods only.

### xsd:Name

XML Name—must start with a letter or underscore.

### xsd:NCName

Non-colonized Name—XML Name without colons.

---

## Qualified Name Types

### xsd:QName

Qualified name — a `prefix:localname` pair stored expanded in Clark notation (`{namespace}localname`). See [QName and SQName Data Types](/docs/qname-sqname-data-types/) for the storage form, grammar, and casting rules, and the related `xbrl:SQName` type for XBRL OIM.

---

## XSD Binary Types

### xsd:base64Binary

Base64-encoded binary data.

{% table %}
* Property
* Value
---
* **IRI**
* `http://www.w3.org/2001/XMLSchema#base64Binary`
---
* **Example**
* `"SGVsbG8gV29ybGQ="`
{% /table %}

### xsd:hexBinary

Hexadecimal-encoded binary data.

{% table %}
* Property
* Value
---
* **IRI**
* `http://www.w3.org/2001/XMLSchema#hexBinary`
---
* **Example**
* `"48656C6C6F"`
{% /table %}

---

## XSD URI Type

### xsd:anyURI

URI reference (absolute or relative). Per XSD 1.1 spec and RFC 3987, accepts both absolute and relative URI references.

{% table %}
* Property
* Value
---
* **IRI**
* `http://www.w3.org/2001/XMLSchema#anyURI`
---
* **Valid Examples**
* `"http://example.com"`, `"../path/file.txt"`, `"#section"`, `"rdf:type"`
{% /table %}

---

## RDF Types

### rdf:langString

String with an associated language tag.

{% table %}
* Property
* Value
---
* **IRI**
* `http://www.w3.org/1999/02/22-rdf-syntax-ns#langString`
---
* **Parent**
* `rdfs:Literal`
---
* **Example**
* `"Hello"@en`, `"Bonjour"@fr`
{% /table %}

### rdf:PlainLiteral

Plain literal that can be cast from `xsd:string`. Represented with an empty language tag internally.

### rdf:XMLLiteral

XML content as a literal. *Note: Not fully implemented.*

---

## XDD Extension Types

TerminusDB provides custom extension types under the `xdd:` namespace (`http://terminusdb.com/schema/xdd#`).

### xdd:coordinate

Geographic coordinate as `[longitude, latitude]`.

{% table %}
* Property
* Value
---
* **IRI**
* `http://terminusdb.com/schema/xdd#coordinate`
---
* **Format**
* `[longitude, latitude]`
---
* **Example**
* `[-122.4194, 37.7749]`
{% /table %}

### xdd:coordinatePolygon

Polygon defined as an array of coordinate arrays.

{% table %}
* Property
* Value
---
* **IRI**
* `http://terminusdb.com/schema/xdd#coordinatePolygon`
---
* **Format**
* `[[[lng, lat], [lng, lat], ...], ...]`
---
* **Example**
* `[[[-122.4, 37.7], [-122.5, 37.8], [-122.4, 37.8], [-122.4, 37.7]]]`
{% /table %}

### xdd:coordinatePolyline

Polyline defined as an array of coordinates.

{% table %}
* Property
* Value
---
* **IRI**
* `http://terminusdb.com/schema/xdd#coordinatePolyline`
---
* **Format**
* `[[lng, lat], [lng, lat], ...]`
{% /table %}

### xdd:json

Arbitrary JSON data stored as a dictionary/object.

{% table %}
* Property
* Value
---
* **IRI**
* `http://terminusdb.com/schema/xdd#json`
---
* **Typecasting**
* String containing valid JSON → `xdd:json` dictionary
---
* **Example**
* `{"key": "value", "count": 42}`
{% /table %}

**Typecasting from string:** When casting from `xsd:string`, the string is parsed as JSON. Invalid JSON throws a casting error.

### xdd:url

Validated URL (subtype of `xsd:string`). Can also be cast from `xsd:anyURI`.

{% table %}
* Property
* Value
---
* **IRI**
* `http://terminusdb.com/schema/xdd#url`
---
* **Parent**
* `xsd:string`
---
* **Example**
* `"https://terminusdb.com"`
{% /table %}

### xdd:email

Email address format validation.

{% table %}
* Property
* Value
---
* **IRI**
* `http://terminusdb.com/schema/xdd#email`
---
* **Parent**
* `xsd:string`
---
* **Example**
* `"user@example.com"`
{% /table %}

### xdd:html

HTML content as a string.

{% table %}
* Property
* Value
---
* **IRI**
* `http://terminusdb.com/schema/xdd#html`
---
* **Parent**
* `xsd:string`
---
* **Example**
* `"<p>Hello <strong>World</strong></p>"`
{% /table %}

### Range Types

Range types store a pair of values representing a range.

{% table %}
* Type
* Base Type
* Format
* Example
---
* **xdd:dateRange**
* `xsd:date`
* `[startDate, endDate]`
* `["2023-01-01", "2023-12-31"]`
---
* **xdd:gYearRange**
* `xsd:gYear`
* `[startYear, endYear]`
* `[1990, 2000]`
---
* **xdd:integerRange**
* `xsd:integer`
* `[min, max]`
* `[1, 100]`
---
* **xdd:decimalRange**
* `xsd:decimal`
* `[min, max]`
* `[0.0, 1.0]`
{% /table %}

### xdd:dateTimeInterval

ISO 8601 time interval with half-open semantics `[start, end)`, inheriting from `xsd:duration`. The supplied spelling is part of the value: a bare `dateTime` is a zero-length instant, a bare `date` is its whole local day, and a `date/date` pair is inclusive of both dates — all read back as written. Designed for XBRL periods, temporal algebra (Allen's Interval Algebra) and clean interval partitioning.

{% callout type="warning" title="Experimental" %}
`xdd:dateTimeInterval` is experimental. The value semantics and lexical round-trips documented here are stable, but the on-disk index format may still change in a future release.
{% /callout %}

{% table %}
* Property
* Value
---
* **IRI**
* `http://terminusdb.com/schema/xdd#dateTimeInterval`
---
* **Parent**
* `xsd:duration`
---
* **Semantics**
* Half-open interval `[start, end)` — start is included, end is excluded
---
* **Example input → stored form**
* `"2025-01-01/2025-03-31"` → `"2025-01-01/2025-03-31"` (spelling preserved)
---
* **Forms**
* Point `T`, `D`; pair `T/T`, `D/D`, mixed; anchored duration `T/P`, `P/T`; declared period `T/T/P`; open `../F`; unknown `F/`; unspecified `F/X*` (where `F` = any finite endpoint)
{% /table %}

The supported processing modes, briefly:

- A single ISO **date** is processed as a local-time *day duration* — `2025-01-01` is the whole local calendar day, not a midnight instant.
- `date/date` is processed as the inclusive duration from the beginning of the start date through the end of the end date — `2025-01-01/2025-03-31` covers all of Q1, internally `[Jan 1@00:00, Apr 1@00:00)`.
- A single **dateTime** is supported in local time without a zone specifier, and both `Z` and `±HH:MM` timezone forms are supported — as are start/end timestamp pairs in any mix of the three forms.
- Intervals with **durations** are supported: `2025-01-01/P3M` (the quarter starting January), `P3M/2025-03-31` (the quarter ending March), and `2025-01-01/2025-04-01/P90D` (a pair with a declared period). Negative durations are not valid inside intervals — `-P1D/T` is rejected.

A bare `P…` is `xsd:duration`, not an interval — a duration has no anchor. For usage, see the [Allen's Interval Algebra guide](/docs/woql-interval-algebra/).

The complete `xdd:dateTimeInterval` specification — lexical grammar, timezone and local-time rules, durations, open/unknown/unspecified endpoints, ordering and storage format — is available for study, contact the Gettier team.

---

## System Types

System types under the `sys:` namespace (`http://terminusdb.com/schema/sys#`) provide special handling for JSON and document data.

### sys:JSON

Arbitrary JSON value that can be any JSON type (object, array, string, number, boolean, null).

{% table %}
* Property
* Value
---
* **IRI**
* `http://terminusdb.com/schema/sys#JSON`
---
* **Usage**
* Schema property type for arbitrary JSON
---
* **Example**
* `{"any": "json", "values": [1, 2, 3]}`
{% /table %}

### sys:JSONDocument

A complete JSON document stored as a top-level entity.

{% table %}
* Property
* Value
---
* **IRI**
* `http://terminusdb.com/schema/sys#JSONDocument`
---
* **Usage**
* Document class for JSON-only documents
{% /table %}

### sys:Dictionary

Internal dictionary/object type used for JSON handling.

{% table %}
* Property
* Value
---
* **IRI**
* `http://terminusdb.com/schema/sys#Dictionary`
---
* **Usage**
* Internal type for Prolog dictionary representation
{% /table %}

For detailed information on using `sys:JSON` and `sys:JSONDocument` in schemas, see [sys:JSON Reference](/docs/sysjson/).

---

## Typecasting Rules

TerminusDB supports explicit typecasting between compatible types. The general rules are:

### String Conversions

All types can typically be cast to and from `xsd:string`:

{% table %}
* Direction
* Behavior
---
* **To string**
* Value is serialized to its canonical string representation
---
* **From string**
* String is parsed and validated according to type constraints
{% /table %}

### Numeric Type Conversions

{% table %}
* Source
* Target
* Rule
---
* `xsd:decimal`
* `xsd:integer`
* Succeeds only if value is whole number
---
* `xsd:double`/`xsd:float`
* `xsd:decimal`
* Converts IEEE 754 float to rational
---
* `xsd:double`/`xsd:float`
* `xsd:integer`
* Succeeds only if value equals its floor
---
* `xsd:integer` subtypes
* Each other
* Validates range constraints
{% /table %}

### String Serialization of Numeric Types

When casting numeric types to `xsd:string`:

{% table %}
* Source Type
* String Format
* Example
---
* `xsd:double`
* Always with decimal point
* `33` → `"33.0"`
---
* `xsd:float`
* Always with decimal point
* `33` → `"33.0"`
---
* `xsd:decimal`
* Minimal form
* `33` → `"33"`
---
* `xsd:integer`
* No decimal point
* `33` → `"33"`
{% /table %}

### Type Subsumption

Child types can be used where parent types are expected. For example:
- `xsd:int` values are valid `xsd:long` values
- `xdd:url` values are valid `xsd:string` values

---

## Reserved/Unimplemented Types

The following XSD types are defined but **not implemented**:

{% table %}
* Type
* Status
---
* `xsd:NOTATION`
* Unimplemented
---
* `xsd:ID`
* Unimplemented
---
* `xsd:IDREF`
* Unimplemented
---
* `xsd:ENTITY`
* Unimplemented
{% /table %}

---

## See Also

- [Schema Reference Guide](/docs/schema-reference-guide/) — Schema definition syntax and keywords
- [QName and SQName Data Types](/docs/qname-sqname-data-types/) — Qualified names, Clark notation, XBRL OIM
- [Numeric Precision Reference](/docs/numeric-precision-reference/) — Detailed numeric handling
- [sys:JSON Reference](/docs/sysjson/) — Working with JSON types
- [WOQL Class Reference](/docs/woql-class-reference-guide/) — Query language type handling
