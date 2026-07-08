---
tags:
  - reference
  - documents
  - curl
  - rdf
  - enterprise
title: RDF/XML Support
nextjs:
  metadata:
    title: RDF/XML Support
    description: How TwinfoxDB Enterprise serializes and parses RDF/XML documents with proper namespace declarations, typed literals, subdocument nesting, and round-trip integrity.
    keywords: rdf xml, rdf, xml, namespace, typed literals, subdocument, enterprise, serialization
    alternates:
      canonical: https://terminusdb.org/docs/enterprise-rdfxml/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
---

Enterprise adds full RDF/XML serialization and parsing to the document endpoint. Documents stored in TerminusDB can be retrieved as well-formed RDF/XML, and RDF/XML documents can be submitted for import — both validated against the database schema.

## RDF/XML output

Request RDF/XML output with the `format=rdfxml` query parameter or the `Accept: application/rdf+xml` header:

```bash
curl -s -H "$AUTH" "$SERVER/api/document/$DB?type=Product&format=rdfxml"
```

The output is a complete RDF/XML document with proper XML declarations and namespace bindings:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF
    xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
    xmlns:xsd="http://www.w3.org/2001/XMLSchema#"
    xmlns:schema="terminusdb:///schema#"
    xml:base="terminusdb:///data/"
>
<schema:Product rdf:about="terminusdb:///data/Product/SKU-1001">
  <schema:name>Industrial Sensor Module</schema:name>
  <schema:price rdf:datatype="http://www.w3.org/2001/XMLSchema#decimal">249.99</schema:price>
  <schema:active rdf:datatype="http://www.w3.org/2001/XMLSchema#boolean">true</schema:active>
  <schema:category rdf:resource="terminusdb:///data/Category/ELEC"/>
  <schema:unit>
    <schema:UnitOfMeasure rdf:about="terminusdb:///data/Product/SKU-1001/unit/UnitOfMeasure/pcs">
      <schema:symbol>pcs</schema:symbol>
      <schema:name>pieces</schema:name>
    </schema:UnitOfMeasure>
  </schema:unit>
</schema:Product>
</rdf:RDF>
```

### Namespace declarations

Namespace prefixes are generated from the database context. The standard RDF and XSD namespaces are always included. Schema-specific namespaces (`terminusdb:///schema#`) appear as the prefix used for types and properties.

### Typed literals

Typed values carry `rdf:datatype` attributes matching their XSD types:

- **`xsd:decimal`** for decimal numbers (e.g., prices, weights)
- **`xsd:boolean`** for boolean values
- **`xsd:integer`** for integer values
- **`xsd:dateTime`** for date-time values
- **Strings** omit the datatype attribute (the RDF default)

### Document references

References to other documents use `rdf:resource` with the full IRI:

```xml
<schema:category rdf:resource="terminusdb:///data/Category/ELEC"/>
```

### Subdocument nesting

Subdocuments (classes with `@subdocument`) are serialized as nested child elements rather than flat references. This preserves the hierarchical structure and makes the XML more natural to process with XPath or XSLT:

```xml
<schema:unit>
  <schema:UnitOfMeasure rdf:about="terminusdb:///data/Product/SKU-1001/unit/UnitOfMeasure/pcs">
    <schema:symbol>pcs</schema:symbol>
    <schema:name>pieces</schema:name>
  </schema:UnitOfMeasure>
</schema:unit>
```

## RDF/XML input

Submit RDF/XML documents with the `Content-Type: application/rdf+xml` header:

```bash
curl -s -X POST \
  -H "Content-Type: application/rdf+xml" \
  -H "$AUTH" \
  -d '<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF
    xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
    xmlns:xsd="http://www.w3.org/2001/XMLSchema#"
    xmlns:schema="terminusdb:///schema#"
    xml:base="terminusdb:///data/"
>
<schema:Product rdf:about="terminusdb:///data/Product/SKU-3003">
  <schema:sku>SKU-3003</schema:sku>
  <schema:name>Calibration Standard</schema:name>
  <schema:price rdf:datatype="http://www.w3.org/2001/XMLSchema#decimal">425.00</schema:price>
  <schema:active rdf:datatype="http://www.w3.org/2001/XMLSchema#boolean">true</schema:active>
  <schema:category rdf:resource="terminusdb:///data/Category/ELEC"/>
  <schema:unit>
    <schema:UnitOfMeasure>
      <schema:name>pieces</schema:name>
      <schema:symbol>pcs</schema:symbol>
    </schema:UnitOfMeasure>
  </schema:unit>
  <schema:weight_kg rdf:datatype="http://www.w3.org/2001/XMLSchema#decimal">0.12</schema:weight_kg>
</schema:Product>
</rdf:RDF>' \
  "$SERVER/api/document/$DB?author=admin&message=import"
```

The parser reads namespace declarations, resolves `rdf:about` URIs to document IDs, maps `rdf:datatype` to schema types, and inserts documents — all validated against the schema.

### Supported reference styles

RDF/XML input supports multiple ways to reference documents and subdocuments:

- **Nested elements** — subdocuments as inline child elements (hierarchical style)
- **`rdf:resource`** — IRI references to other top-level documents
- **`rdf:nodeID`** — blank node references for subdocuments defined elsewhere in the same document
- **Element order independence** — referenced subdocuments can appear before or after the referencing parent

## Context and namespace relationship

RDF/XML namespace declarations serve the same purpose as JSON-LD `@context` — they map short prefixes to full IRIs. The key difference is in syntax:

{% table %}

- Concept
- JSON-LD
- RDF/XML

---

- Prefix mapping
- `"@context": {"schema": "http://schema.org/"}`
- `xmlns:schema="http://schema.org/"`

---

- Base IRI
- `"@base": "terminusdb:///data/"`
- `xml:base="terminusdb:///data/"`

---

- Type declaration
- `"@type": "Product"`
- `<schema:Product rdf:about="...">`

---

- Property
- `"name": "Alice"`
- `<schema:name>Alice</schema:name>`

{% /table %}

When a database has a custom schema `@context`, it affects JSON-LD output only. RDF/XML output always generates `xmlns:` declarations from the resolved namespace IRIs regardless of the schema `@context` setting.

## Use cases for RDF/XML

RDF/XML is the right format choice when integrating with:

- **Regulatory systems** that require RDF/XML submissions (pharmaceutical, financial, government)
- **SKOS taxonomy** exchange between classification systems
- **OWL ontology** alignment and federation
- **Legacy triple stores** and SPARQL endpoints that accept RDF/XML as their ingest format
- **XSLT pipelines** that transform RDF/XML into other XML formats for downstream systems

## Further reading

- [Document Formats & Content Negotiation](/docs/enterprise-document-formats/) — how to select formats
- [Turtle Support](/docs/enterprise-turtle/) — the compact, human-readable RDF counterpart
- [JSON-LD Context Processing](/docs/enterprise-jsonld-context/) — the JSON-LD counterpart
- [Multi-Format Document API Tutorial](/docs/document-format-api-curl-tutorial/) — hands-on round-trip walkthrough
