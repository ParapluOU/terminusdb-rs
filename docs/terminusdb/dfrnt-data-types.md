---
tags:
  - reference
  - schema
  - dfrnt-cloud
title: TerminusDB Data Types
nextjs:
  metadata:
    title: TerminusDB Data Types
    description: The core data types for TerminusDB, derived from XML Data Types providing strong data interoperability
    keywords: TerminusDB, data types, XML Data Types, data interoperability
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/dfrnt-data-types/
media: []
---

{% callout type="note" %}
**TerminusCMS is now DFRNT Hub**
TerminusCMS has been renamed to **DFRNT Hub**. All features described on this page are available at [dfrnt.com](https://dfrnt.com/hypergraph-content-studio/). The TerminusDB open source database remains unchanged.
{% /callout %}


{% table %}

- **Type**
- **W3C spec**
- **Description**

--- 

- `xsd:string`
- [W3C](https://www.w3.org/TR/xmlschema-2/#string)
- Character strings (but not all Unicode character strings)

---

- `xsd:boolean`
- [W3C](https://www.w3.org/TR/xmlschema-2/#boolean)
- true, false

---

- `xsd:decimal`
- [W3C](https://www.w3.org/TR/xmlschema-2/#decimal)
- Equivalent to `xsd:double`

---

- `xsd:integer`
- [W3C](https://www.w3.org/TR/xmlschema-2/#integer)
- Arbitrary-size integer numbers

{% /table %}

## IEEE floating-point numbers

{% table %}

- **Type**
- **W3C spec**
- **Description**

---

- `xsd:double`
- [W3C](https://www.w3.org/TR/xmlschema-2/#double)
- 64-bit floating point numbers incl. ∓Inf, ∓0, NaN

---

- `xsd:float`
- [W3C](https://www.w3.org/TR/xmlschema-2/#float)
- 32-bit floating point numbers incl. ∓Inf, ∓0, NaN

{% /table %}

## Time and date

{% table %}

- **Type**
- **W3C spec**
- **Description**

---

- `xsd:date`
- [W3C](https://www.w3.org/TR/xmlschema-2/#date)
- Dates (yyyy-mm-dd) with or without timezone

---

- `xsd:time`
- [W3C](https://www.w3.org/TR/xmlschema-2/#time)
- Times (`hh:mm:ss.sss`) with or without timezone

---

- `xsd:dateTime`
- [W3C](https://www.w3.org/TR/xmlschema-2/#dateTime)
- Date and time with or without timezone

---

- `xsd:dateTimeStamp`
- [W3C](https://www.w3.org/TR/xmlschema-2/#dateTimeStamp)
- Date and time with required timezone

{% /table %}

## Recurring and partial dates

{% table %}

- **Type**
- **W3C spec**
- **Description**

---

- `xsd:gYear`
- [W3C](https://www.w3.org/TR/xmlschema-2/#gYear)
- Gregorian calendar year

---

- `xsd:gMonth`
- [W3C](https://www.w3.org/TR/xmlschema-2/#gMonth)
- Gregorian calendar month

---

- `xsd:gDay`
- [W3C](https://www.w3.org/TR/xmlschema-2/#gDay)
- Gregorian calendar day of the month

---

- `xsd:gYearMonth`
- [W3C](https://www.w3.org/TR/xmlschema-2/#gYearMonth)
- Gregorian calendar year and month

---

- `xsd:gMonthDay`
- [W3C](https://www.w3.org/TR/xmlschema-2/#gMonthDay)
- Gregorian calendar month and day

---

- `xsd:duration`
- [W3C](https://www.w3.org/TR/xmlschema-2/#duration)
- Duration of time

---

- `xsd:yearMonthDuration`
- [W3C](https://www.w3.org/TR/xmlschema-2/#yearMonthDuration)
- Duration of time (months and years only)

---

- `xsd:dayTimeDuration`
- [W3C](https://www.w3.org/TR/xmlschema-2/#dayTimeDuration)
- Duration of time (days, hours, minutes, seconds only)

{% /table %}

## Limited-range integer numbers

{% table %}

- **Type**
- **W3C spec**
- **Description**

---

- `xsd:byte`
- [W3C](https://www.w3.org/TR/xmlschema-2/#byte)
- \-128…+127 (8 bit)

---

- `xsd:short`
- [W3C](https://www.w3.org/TR/xmlschema-2/#short)
- \-32768…+32767 (16 bit)

---

- `xsd:int`
- [W3C](https://www.w3.org/TR/xmlschema-2/#int)
- \-2147483648…+2147483647 (32 bit)

---

- `xsd:long`
- [W3C](https://www.w3.org/TR/xmlschema-2/#long)
- \-9223372036854775808…+9223372036854775807 (64 bit)

---

- `xsd:unsignedByte`
- [W3C](https://www.w3.org/TR/xmlschema-2/#unsignedByte)
- 0…255 (8 bit)

---

- `xsd:unsignedShort`
- [W3C](https://www.w3.org/TR/xmlschema-2/#unsignedShort)
- 0…65535 (16 bit)

---

- `xsd:unsignedInt`
- [W3C](https://www.w3.org/TR/xmlschema-2/#unsignedInt)
- 0…4294967295 (32 bit)

---

- `xsd:unsignedLong`
- [W3C](https://www.w3.org/TR/xmlschema-2/#unsignedLong)
- 0…18446744073709551615 (64 bit)

---

- `xsd:positiveInteger`
- [W3C](https://www.w3.org/TR/xmlschema-2/#positiveInteger)
- Integer numbers >0

---

- `xsd:nonNegativeInteger`
- [W3C](https://www.w3.org/TR/xmlschema-2/#nonNegativeInteger)
- Integer numbers ≥0

---

- `xsd:negativeInteger`
- [W3C](https://www.w3.org/TR/xmlschema-2/#negativeInteger)
- Integer numbers <0

---

- `xsd:nonPositiveInteger`
- [W3C](https://www.w3.org/TR/xmlschema-2/#nonPositiveInteger)
- Integer numbers ≤0

{% /table %}

## Encoded binary data

{% table %}

- **Type**
- **W3C spec**
- **Description**

---

- `xsd:hexBinary`
- [W3C](https://www.w3.org/TR/xmlschema-2/#hexBinary)
- Hex-encoded binary data

---

- `xsd:base64Binary`
- [W3C](https://www.w3.org/TR/xmlschema-2/#base64Binary)
- Base64-encoded binary data

{% /table %}

## Miscellaneous XSD types

{% table %}

- **Type**
- **W3C spec**
- **Description**

---

- `xsd:anyURI`
- [W3C](https://www.w3.org/TR/xmlschema-2/#anyURI)
- Absolute or relative URIs and IRIs

---

- `xsd:language`
- [W3C](https://www.w3.org/TR/xmlschema-2/#language)
- Language tags per [BCP47](https://en.wikipedia.org/wiki/IETF_language_tag)

---

- `xsd:normalizedString`
- [W3C](https://www.w3.org/TR/xmlschema-2/#normalizedString)
- Whitespace-normalized strings

---

- `xsd:token`
- [W3C](https://www.w3.org/TR/xmlschema-2/#token)
- Tokenized strings

---

- `xsd:NMTOKEN`
- [W3C](https://www.w3.org/TR/xmlschema-2/#NMTOKEN)
- XML NMTOKENs

---

- `xsd:Name`
- [W3C](https://www.w3.org/TR/xmlschema-2/#Name)
- XML Names

---

- `xsd:NCName`
- [W3C](https://www.w3.org/TR/xmlschema-2/#NCName)
- XML NCNames

{% /table %}
