---
tags:
  - reference
  - documents
  - curl
  - rdf
  - enterprise
title: Document Formats & Content Negotiation
nextjs:
  metadata:
    title: Document Formats & Content Negotiation
    description: How to read and write TerminusDB documents as JSON, JSON-LD, RDF/XML, or Turtle using query parameters or HTTP content negotiation.
    keywords: document api, json-ld, rdf xml, turtle, ttl, content negotiation, format, accept header, enterprise
    alternates:
      canonical: https://terminusdb.org/docs/enterprise-document-formats/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
---

The Enterprise document endpoint supports four serialization formats for the same underlying data. You choose the format per request — the storage layer is format-independent.

## Supported formats

{% table %}

- Format
- Content-Type
- Query parameter
- Use case

---

- JSON (default)
- `application/json`
- `format=json`
- Internal APIs, front-end applications, smallest payload

---

- JSON-LD
- `application/ld+json`
- `format=jsonld`
- Linked data publishing, semantic interoperability, context-aware exchange

---

- RDF/XML
- `application/rdf+xml`
- `format=rdfxml`
- Regulatory submissions, SPARQL endpoints, legacy triple stores

---

- Turtle
- `text/turtle`
- `format=turtle`
- Human-readable RDF, version control diffs, SPARQL tooling, debugging

{% /table %}

## Requesting a format (output)

There are two ways to request a specific output format when retrieving documents.

### Query parameter

Add `format=` to the GET request:

```bash
# JSON-LD
curl -s -H "$AUTH" "$SERVER/api/document/$DB?type=Product&format=jsonld"

# RDF/XML
curl -s -H "$AUTH" "$SERVER/api/document/$DB?type=Product&format=rdfxml"

# Turtle
curl -s -H "$AUTH" "$SERVER/api/document/$DB?type=Product&format=turtle"
```

### Accept header

Use standard HTTP content negotiation:

```bash
# JSON-LD via Accept header
curl -s -H "$AUTH" -H "Accept: application/ld+json" \
  "$SERVER/api/document/$DB?type=Product"

# RDF/XML via Accept header
curl -s -H "$AUTH" -H "Accept: application/rdf+xml" \
  "$SERVER/api/document/$DB?type=Product"

# Turtle via Accept header
curl -s -H "$AUTH" -H "Accept: text/turtle" \
  "$SERVER/api/document/$DB?type=Product"
```

The `format=` query parameter takes precedence if both are present.

## Submitting a format (input)

Set the `Content-Type` header when POSTing or PUTting documents:

```bash
# Submit JSON-LD
curl -s -X POST \
  -H "Content-Type: application/ld+json" \
  -H "$AUTH" \
  -d '{"@context": {"@base": "terminusdb:///data/", "@vocab": "terminusdb:///schema#"}, ...}' \
  "$SERVER/api/document/$DB?author=admin&message=import"

# Submit RDF/XML
curl -s -X POST \
  -H "Content-Type: application/rdf+xml" \
  -H "$AUTH" \
  -d '<?xml version="1.0"?><rdf:RDF ...>...</rdf:RDF>' \
  "$SERVER/api/document/$DB?author=admin&message=import"

# Submit Turtle
curl -s -X POST \
  -H "Content-Type: text/turtle" \
  -H "$AUTH" \
  -d '@prefix schema: <terminusdb:///schema#> . <terminusdb:///data/Product/SKU-1> a schema:Product ; ...' \
  "$SERVER/api/document/$DB?author=admin&message=import"
```

The server parses the body according to the declared Content-Type, resolves IRIs against the document's context, namespace declarations, or prefix declarations, validates against the schema, and inserts the documents, folding their subdocuments correctly into the complete JSON-LD documents.

Blank nodes are used to ensure the correct shapes of documents to be imported. Note that the blank nodes are not stored in the graph, and are only used to facilitate transportation of the document structure. The schema holds the shape of the documents. 

TwinfoxDB uses explicit id:s for subdocument structures that share their lifecycle with the "parent" document.

## JSON-LD output modes

JSON-LD output has two modes controlled by the `as_list` parameter:

**Stream mode** (default) — each document, if more than one, is a self-contained JSON-LD object served with its own `@context`:

```bash
curl -s -H "$AUTH" "$SERVER/api/document/$DB?type=Product&format=jsonld"
```

```json
{"@context": {"@base": "terminusdb:///data/", "@vocab": "terminusdb:///schema#"}, "@id": "Product/SKU-1001", "@type": "Product", ...}
{"@context": {"@base": "terminusdb:///data/", "@vocab": "terminusdb:///schema#"}, "@id": "Product/SKU-1002", "@type": "Product", ...}
```

**List mode** — all documents wrapped in a single `@graph` envelope with a shared `@context`:

```bash
curl -s -H "$AUTH" "$SERVER/api/document/$DB?type=Product&format=jsonld&as_list=true"
```

```json
{
  "@context": {"@base": "terminusdb:///data/", "@vocab": "terminusdb:///schema#"},
  "@graph": [
    {"@id": "Product/SKU-1001", "@type": "Product", ...},
    {"@id": "Product/SKU-1002", "@type": "Product", ...}
  ]
}
```

List mode is more efficient for batch processing since the context is not repeated.

## Schema @context for output remapping

When a database schema includes a custom `@context`, JSON-LD output uses that context instead of the default internal one. This lets you control how output documents look to consumers without changing the stored data.

A common example is to remap `@id` to `id` in the external json-ld, and `@type` to `type`, which is how schema.org aliasing is done.

Set a schema-level `@context` by including it in a schema update:

```bash
curl -s -X POST -H "Content-Type: application/json" -H "$AUTH" \
  -d '[{"@type": "@context", "@schema": "terminusdb:///schema#", "@base": "terminusdb:///data/", "@context": "https://schema.org/"}]' \
  "$SERVER/api/document/$DB?graph_type=schema&author=admin&message=set+context&full_replace=true"
```

After this, JSON-LD output for documents in this database will carry the schema.org context, and property names will be compacted using schema.org's vocabulary.

## Round-trip integrity

Data survives format conversion without loss. You can:

1. Export as Turtle or RDF/XML
2. Send it to a partner
3. Receive their edits back as JSON-LD, Turtle, or RDF/XML
4. Import — nothing is lost or distorted

All four formats describe the same underlying RDF graph. You can insert a document as Turtle, retrieve it as RDF/XML, send it to a partner who edits it as JSON-LD, and re-import — the data round-trips without loss.

Schema validation applies to every format. Whether you POST JSON, JSON-LD, RDF/XML, or Turtle, the data is checked against the schema before it is committed.

## Hands-on tutorial

For a complete walkthrough with `curl` examples covering all formats, see the [Multi-Format Document API Tutorial](/docs/document-format-api-curl-tutorial/).

## Further reading

- [JSON-LD Context Processing](/docs/enterprise-jsonld-context/) — how `@context` works for expansion and compaction
- [RDF/XML Support](/docs/enterprise-rdfxml/) — namespace handling and typed literals
- [Turtle Support](/docs/enterprise-turtle/) — prefix declarations, blank node nesting, and human-readable RDF
- [Document API Reference](/docs/document-insertion/) — full parameter reference
