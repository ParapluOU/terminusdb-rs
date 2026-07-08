---
title: JSON-LD Context Processing
nextjs:
  metadata:
    title: JSON-LD Context Processing
    description: How TwinfoxDB Enterprise handles JSON-LD @context for document expansion, compaction, and round-trip context bridging using the Rust json-ld crate.
    keywords: json-ld, context, expansion, compaction, schema.org, linked data, w3c, enterprise
    alternates:
      canonical: https://terminusdb.org/docs/enterprise-jsonld-context/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
tags:
  - typescript
  - rdf
  - reference
  - enterprise
---

Enterprise processes JSON-LD `@context` using the W3C JSON-LD 1.1 specification, powered by the Rust `json-ld` crate. This gives you standards-compliant expansion and compaction without writing any conversion code.

## How context processing works

TerminusDB stores documents internally using an internal context — `terminusdb:///schema#` for types and properties, `terminusdb:///data/` for document IDs. When documents arrive or leave with a different `@context`, the JSON-LD processor bridges between the two:

**Input pipeline:** expand (resolve using document's `@context`) → compact (rewrite using internal context) → store

**Output pipeline:** load (internal context) → expand (resolve to absolute IRIs) → compact (rewrite using output `@context`) → stream

This means you can submit documents authored with any valid JSON-LD context and retrieve them in any other context, and the data round-trips without loss.

## Supported @context forms

The `@context` value in a JSON-LD document can take several forms, and Enterprise handles all of them.

### Inline object

The most common form — a JSON object mapping prefixes and keywords:

```json
{
  "@context": {
    "@base": "terminusdb:///data/",
    "@vocab": "terminusdb:///schema#"
  },
  "@type": "Person",
  "@id": "Person/alice",
  "name": "Alice"
}
```

### Remote URL

A URL pointing to a context definition hosted elsewhere. Enterprise fetches and caches these automatically:

```json
{
  "@context": "https://schema.org/",
  "type": "Person",
  "id": "https://example.com/person/alice",
  "name": "Alice"
}
```

Schema.org is pre-seeded in the [context cache](/docs/enterprise-context-cache/), so it resolves instantly without network access.

### Array of contexts

Multiple context entries in an array. Later entries take precedence for conflicting definitions:

```json
{
  "@context": [
    {"@base": "https://mydomain.com/data/Person/"},
    "https://schema.org/"
  ],
  "@type": "Person",
  "@id": "bob",
  "name": "Bob"
}
```

This is useful when you need to combine a remote vocabulary with local overrides — for instance, setting a custom `@base` for document IDs while using schema.org for property names.

### Mixed URL and inline

Combine a remote URL with inline overrides in an array:

```json
{
  "@context": [
    "https://schema.org/",
    {
      "@base": "terminusdb:///data/",
      "name": "terminusdb:///schema#name",
      "age": "terminusdb:///schema#age"
    }
  ],
  "@type": "Person",
  "@id": "Person/remote_1",
  "name": "Remote Context",
  "age": 28
}
```

The inline entries override schema.org's definitions for the listed properties, letting you map schema.org-style documents into your internal schema.

## Schema-level @context

You can set a custom `@context` on the database schema itself. This controls what context appears in JSON-LD output for all documents in that database.

### Setting a schema @context

Include a `@context` document in a schema update:

```json
[
  {
    "@type": "@context",
    "@schema": "http://schema.org/",
    "@base": "terminusdb:///data/",
    "@context": "https://schema.org/"
  },
  {
    "@type": "Class",
    "@id": "Person",
    "name": "xsd:string",
    "description": "xsd:string"
  }
]
```

After this, when you retrieve documents as JSON-LD, the output `@context` will be `"https://schema.org/"` and property names will be compacted using schema.org's vocabulary.

### How schema @context affects output

Without a schema `@context`, output uses the default internal context:

```json
{
  "@context": {
    "@base": "terminusdb:///data/",
    "@vocab": "terminusdb:///schema#"
  },
  "@type": "Person",
  "name": "Alice"
}
```

With a schema.org `@context`, the same document comes out as:

```json
{
  "@context": "https://schema.org/",
  "type": "Person",
  "name": "Alice",
  "id": "https://mydomain.com/data/Person/alice"
}
```

Notice that `@type` becomes `type` and `@id` becomes `id` — these are aliases defined by the schema.org context.

### Schema array @context

The schema `@context` can also be an array, giving you the same multi-tranche control for output:

```json
{
  "@type": "@context",
  "@schema": "terminusdb:///schema#",
  "@base": "terminusdb:///data/",
  "@context": [
    {"ex": "http://example.org/", "title": "ex:title"},
    {"@vocab": "terminusdb:///schema#"}
  ]
}
```

## Input without @context

Documents submitted without any `@context` are treated as plain JSON using the database's default internal context. Property names map directly to `terminusdb:///schema#` and IDs to `terminusdb:///data/`:

```json
{
  "@type": "Person",
  "@id": "Person/alice",
  "name": "Alice"
}
```

This is the standard TerminusDB behavior and works identically in both community and enterprise editions.

## Context and RDF/XML

RDF/XML uses XML namespace declarations instead of JSON-LD `@context`, but the underlying semantics are identical. When you set a schema `@context`, it affects JSON-LD output only — RDF/XML output always generates proper `xmlns:` declarations from the resolved namespace IRIs.

However, the context processing pipeline is the same for both formats. A document submitted as RDF/XML with `xmlns:schema="http://schema.org/"` goes through the same IRI resolution as a JSON-LD document with `"@context": "https://schema.org/"`.

See [RDF/XML Support](/docs/enterprise-rdfxml/) for details on namespace handling.

## Further reading

- [Context Cache](/docs/enterprise-context-cache/) — how remote contexts are cached locally
- [Document Formats & Content Negotiation](/docs/enterprise-document-formats/) — format selection for input and output
- [Turtle Support](/docs/enterprise-turtle/) — the compact, human-readable RDF counterpart
- [Configuration Reference](/docs/enterprise-configuration/) — environment variables that control context behavior
