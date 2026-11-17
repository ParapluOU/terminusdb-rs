---
tags:
  - reference
  - schema
  - woql
  - rdf
title: QName and SQName Data Types
nextjs:
  metadata:
    title: "QName and SQName Data Types: Qualified Names for XBRL and RDF"
    description: Reference for xsd:QName and xbrl:SQName in TerminusDB — Clark notation storage, lexical prefix rendering, typecasting with prefix maps, and XBRL OIM usage.
    keywords: qname, sqname, xbrl, oim, xbrl-json, expanded name, clark notation, prefix, ncname, xsd datatypes
    alternates:
      canonical: https://terminusdb.com/docs/qname-sqname-data-types/
---

TerminusDB supports two qualified-name datatypes: `xsd:QName` from XML Schema and `xbrl:SQName` from the XBRL Open Information Model (OIM). Both store expanded names — a namespace IRI plus a local name — and render the familiar `prefix:localname` form at the edges. SQName relaxes the local-name grammar for OIM identifiers such as `aspect:core:entity`.

For the general type list, see [XSD Data Types](/docs/xsd-data-types/). For prefix bindings, see [Prefix Management](/docs/prefix-management/).

## The Two Types

{% table %}
* Type
* IRI
* Local-name grammar
---
* `xsd:QName`
* `http://www.w3.org/2001/XMLSchema#QName`
* Must be an XML NCName
---
* `xbrl:SQName`
* `https://xbrl.org/2021SQName`
* Any non-whitespace string — extra colons, `{`, `}` are legal (OIM Common §3.1)
{% /table %}

The prefix `xbrl` is bound globally to `https://xbrl.org/2021` (no trailing separator — OIM prefix maps expand by bare concatenation, and since it's a QName and not an IRI, `xbrl:SQName` is `{https://xbrl.org/2021}SQName` in clark notation). Note that a clark notation QName is not an IRI, it only contains an IRI for the namespace. 

## Storage: Clark Notation

Both types store values in expanded **Clark notation**:

```
{namespace-IRI}localname
```

The pair `{namespace, localname}` is the identity. Two different prefixes bound to the same namespace produce the same stored value — prefix choice never affects equality, ordering, or joins.

## Lexical Input

At ingest, values are written in lexical form `prefix:localname`, split on the **first** colon only:

- The **prefix** must be a valid NCName and bound in the applicable prefix map — the document `@context`, or an explicit map passed to a cast. An unbound prefix is an error, never a guess.
- The **local name** must be non-empty and contain no whitespace. `xsd:QName` additionally requires it to be a valid NCName.
- **Clark notation is also accepted** at ingest, so rendered values always round-trip even when no prefix matches the namespace.

## Lexical Rendering

On output, a stored Clark value is contracted to `prefix:localname` when the active prefix map contains the namespace. With no matching prefix, the Clark form is returned verbatim — never silently dropped or mangled. Contraction matches the full namespace IRI exactly; substring matches never fire.

## Typecasting

Casts between `xsd:string`, `xsd:QName`, and `xbrl:SQName` use the standard `typecast` WOQL instruction:

{% table %}
* From → To
* Behaviour
---
* `xsd:string` → `xsd:QName`
* Parse `prefix:local` or Clark input; validate prefix binding and NCName local name
---
* `xsd:string` → `xbrl:SQName`
* Same, with the relaxed OIM local-name grammar
---
* `xsd:QName` → `xbrl:SQName`
* Retag — always valid (QName locals satisfy the SQName grammar)
---
* `xbrl:SQName` → `xsd:QName`
* Retag only when the local name is a valid NCName — an SQName like `a:b:c` cannot become a QName
{% /table %}

### The `prefixes` parameter

`typecast` accepts an optional `prefixes` map that overrides or augments the database `@context` for the cast. It takes either a literal dictionary or a variable bound earlier in the query (for example via `dot` on a document's `namespaces` field — exactly how xBRL-JSON reports carry their own prefix maps):

```javascript
// Literal prefix map
WOQL.typecast("xbrli:NLE", "xsd:QName", "v:Concept", { xbrli: "http://www.xbrl.org/2003/instance" })

// Variable-bound map from a document field
WOQL.and(
  WOQL.dot("v:Report", "namespaces", "v:Ns"),
  WOQL.typecast("v:Raw", "xbrl:SQName", "v:Dim", "v:Ns")
)
```

Omit `prefixes` to use the database `@context`.

## Worked OIM Example

An xBRL-JSON report declares its own `namespaces` dictionary. Storing a concept dimension as an SQName keeps the value expanded and comparable across reports that use different prefixes for the same namespace:

```javascript
// Schema
{
  "@type": "Class",
  "@id": "Dimension",
  "concept": "xbrl:SQName",
  "value": "xsd:string"
}

// Instance — prefix resolved from the report's namespace map via a cast
WOQL.typecast("sobd:WholeClaim", "xbrl:SQName", "v:C", { sobd: "https://example.com/sobd" })
// stores: {https://example.com/sobd}WholeClaim
```

Querying by Clark value or by any equivalent prefix finds the same literal — the stored form is prefix-independent by construction.

## See Also

- [XSD Data Types](/docs/xsd-data-types/) — the full datatype reference
- [Prefix Management](/docs/prefix-management/) — how `@context` bindings work
- [WOQL Class Reference](/docs/woql-class-reference-guide/) — `typecast` and related instructions
