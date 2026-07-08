---
tags:
  - reference
  - documents
  - curl
  - rdf
  - enterprise
title: Turtle Support
nextjs:
  metadata:
    title: Turtle Support
    description: How TwinfoxDB Enterprise serializes and parses Turtle (TTL) documents with prefix declarations, typed literals, subdocument nesting, and round-trip integrity.
    keywords: turtle, ttl, rdf, prefix, typed literals, subdocument, enterprise, serialization
    alternates:
      canonical: https://terminusdb.org/docs/enterprise-turtle/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
---

Enterprise adds full Turtle serialization and parsing to the document endpoint. Documents stored in TerminusDB can be retrieved as well-formed Turtle, and Turtle documents can be submitted for import — both validated against the database schema.

Turtle (Terse RDF Triple Language) is a compact, human-readable RDF serialization. It uses prefixed names, semicolons for predicate grouping, and square brackets for blank nodes — making it considerably more readable than RDF/XML for the same data.

## Turtle output

Request Turtle output with the `format=turtle` query parameter or the `Accept: text/turtle` header:

```bash
curl -s -H "$AUTH" "$SERVER/api/document/$DB?type=Product&format=turtle"
```

The output is a valid Turtle document with `@base` and `@prefix` declarations followed by the document triples:

```turtle
@base <terminusdb:///data/> .
@prefix schema: <terminusdb:///schema#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .

<terminusdb:///data/Product/SKU-1001>
  a schema:Product ;
  schema:name "Industrial Sensor Module" ;
  schema:price "249.99"^^xsd:decimal ;
  schema:active "true"^^xsd:boolean ;
  schema:category <terminusdb:///data/Category/ELEC> ;
  schema:unit [
    a schema:UnitOfMeasure ;
    schema:symbol "pcs" ;
    schema:name "pieces"
  ] .
```

### Prefix declarations

Prefix declarations are generated from the database context. The standard RDF and XSD namespaces are always included. The `@base` declaration provides the base IRI for resolving relative references. Schema-specific namespaces (`terminusdb:///schema#`) appear as the prefix used for types and properties.

### Typed literals

Typed values carry `^^` datatype annotations matching their XSD types:

- **`xsd:decimal`** for decimal numbers (e.g., prices, weights)
- **`xsd:boolean`** for boolean values
- **`xsd:integer`** for integer values
- **`xsd:dateTime`** for date-time values
- **Strings** are plain double-quoted literals without a datatype annotation

### Document references

References to other documents use full IRIs in angle brackets:

```turtle
schema:category <terminusdb:///data/Category/ELEC> .
```

### Subdocument nesting

Subdocuments (classes with `@subdocument`) are serialized as blank node blocks using the `[ ... ]` syntax rather than flat references. This preserves the hierarchical structure and keeps related data together:

```turtle
schema:unit [
    a schema:UnitOfMeasure ;
    schema:symbol "pcs" ;
    schema:name "pieces"
  ] .
```

The blank node syntax is the Turtle equivalent of nested child elements in RDF/XML or embedded objects in JSON — it expresses the same containment relationship in a compact, readable form.

## Turtle input

Submit Turtle documents with the `Content-Type: text/turtle` header:

```bash
echo '@prefix schema: <terminusdb:///schema#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

<terminusdb:///data/Product/SKU-3001>
  a schema:Product ;
  schema:sku "SKU-3001" ;
  schema:name "Calibration Standard" ;
  schema:category <terminusdb:///data/Category/ELEC> ;
  schema:price "425.00"^^xsd:decimal ;
  schema:weight_kg "0.15"^^xsd:decimal ;
  schema:unit [
    a schema:UnitOfMeasure ;
    schema:symbol "pcs" ;
    schema:name "pieces"
  ] ;
  schema:active "true"^^xsd:boolean .' | \
curl -s -X POST \
  -H "Content-Type: text/turtle" \
  -H "$AUTH" \
  --data-binary @- \
  "$SERVER/api/document/$DB?author=admin&message=import"
```

The `echo | --data-binary @-` pattern is necessary because Turtle starts with `@prefix`, and curl interprets a leading `@` as a filename reference. Using `--data-binary` (rather than `-d`) also preserves newlines, which Turtle syntax relies on.

The parser reads prefix declarations, resolves prefixed names to full IRIs, maps `^^` datatype annotations to schema types, and inserts documents — all validated against the schema.

### Supported reference styles

Turtle input supports multiple ways to reference documents and subdocuments:

- **Blank node blocks** — subdocuments as inline `[ ... ]` blocks (hierarchical style)
- **Full IRI references** — `<terminusdb:///data/...>` references to other top-level documents
- **Blank node labels** — `_:label` references for subdocuments defined elsewhere in the same document
- **Statement order independence** — referenced subdocuments can appear before or after the referencing parent

## Context and prefix relationship

Turtle `@prefix` declarations serve the same purpose as JSON-LD `@context` and RDF/XML `xmlns:` — they map short prefixes to full IRIs. The key difference is in syntax:

{% table %}

- Concept
- JSON-LD
- RDF/XML
- Turtle

---

- Prefix mapping
- `"@context": {"schema": "http://schema.org/"}`
- `xmlns:schema="http://schema.org/"`
- `@prefix schema: <http://schema.org/> .`

---

- Base IRI
- `"@base": "terminusdb:///data/"`
- `xml:base="terminusdb:///data/"`
- `@base <terminusdb:///data/> .`

---

- Type declaration
- `"@type": "Product"`
- `<schema:Product rdf:about="...">`
- `a schema:Product`

---

- Property
- `"name": "Alice"`
- `<schema:name>Alice</schema:name>`
- `schema:name "Alice"`

---

- Typed literal
- `"price": 249.99`
- `<schema:price rdf:datatype="...#decimal">249.99</schema:price>`
- `schema:price "249.99"^^xsd:decimal`

{% /table %}

When a database has a custom schema `@context`, it affects JSON-LD output only. Turtle output always generates `@prefix` declarations from the resolved namespace IRIs regardless of the schema `@context` setting.

## Turtle compared to RDF/XML

Turtle and RDF/XML express exactly the same RDF graph — they are alternative serializations of the same data model. The choice between them is primarily about readability and tooling:

{% table %}

- Aspect
- Turtle
- RDF/XML

---

- Readability
- Compact, line-oriented, easy to scan
- Verbose, requires XML familiarity

---

- Editability
- Easy to hand-edit in any text editor
- Error-prone without XML tooling

---

- Tooling
- Text processing, grep, diff
- XSLT, XPath, XML Schema validation

---

- Blank nodes
- `[ ... ]` inline or `_:label`
- `rdf:nodeID` or nested elements

---

- Multiple values
- Repeat the predicate or use `,` separator
- Repeat the element

---

- Typical use
- Human review, version control diffs, debugging
- XSLT pipelines, regulatory XML submissions

{% /table %}

## Use cases for Turtle

Turtle is the right format choice when working with:

- **Human review workflows** where data stewards need to read and verify RDF content
- **Version control** where line-oriented diffs make changes easy to review in pull requests
- **SPARQL tooling** that accepts Turtle as a data exchange format
- **Academic and research** data exchange where Turtle is the de facto standard
- **Debugging and diagnostics** where compact, readable output helps trace data issues
- **Configuration-as-code** pipelines where RDF data is committed alongside application code

## Further reading

- [Document Formats & Content Negotiation](/docs/enterprise-document-formats/) — how to select formats
- [RDF/XML Support](/docs/enterprise-rdfxml/) — the XML-based counterpart
- [JSON-LD Context Processing](/docs/enterprise-jsonld-context/) — the JSON-LD counterpart
- [Multi-Format Document API Tutorial](/docs/document-format-api-curl-tutorial/) — hands-on round-trip walkthrough
