---
tags:
  - how-to
  - documents
  - curl
  - rdf
  - intermediate
title: Language-Tagged RDF Strings with curl
nextjs:
  metadata:
    title: Language-Tagged RDF Strings with curl
    description: How to store and retrieve multilingual text in TerminusDB using rdf:langString and the HTTP API with curl.
    keywords: rdf:langString, language string, multilingual, curl, HTTP API, i18n, @language
    alternates:
      canonical: https://terminusdb.org/docs/language-strings-with-curl/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to store and query multilingual (language-tagged) strings via the HTTP API.
{% /callout %}

TerminusDB supports language-tagged strings through the `rdf:langString` data type. This lets you attach a language tag to any text value, which is useful for multilingual content, localized labels, or any field where the language of the text matters.

This guide walks through the full lifecycle: creating a schema with language-tagged fields, inserting documents with language values, and retrieving them — all using curl against the HTTP API.

## Prerequisites

- A running TerminusDB instance on `localhost:6363`
- Default credentials `admin:root` (adjust to match your setup)
- `curl` installed

## What you will build

A `Greeting` class with two language-tagged fields, then insert and retrieve a multilingual greeting document.

---

## Step 1: Create a database

```bash
curl -u admin:root -X POST "http://localhost:6363/api/db/admin/greetings" \
  -H "Content-Type: application/json" \
  -d '{"label": "Greetings DB", "comment": "Multilingual greetings"}'
```

You should see a success response:

```json
{"@type":"api:DbCreateResponse", "api:status":"api:success", ...}
```

## Step 2: Define the schema

The schema needs two things to support `rdf:langString`:

- **Declare the `rdf` and `rdfs` prefixes** in the `@context` so TerminusDB can resolve the type and property IRIs.
- **Use `"rdf:langString"` as the field type** for any property that should hold language-tagged text.

Since we are replacing the default context document with one that includes custom prefixes, we need `full_replace=true`.

```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/greetings?graph_type=schema&author=admin&message=Add+schema&full_replace=true" \
  -H "Content-Type: application/json" \
  -d '[
    {
      "@type": "@context",
      "@base": "terminusdb:///data/",
      "@schema": "terminusdb:///schema#",
      "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
      "rdfs": "http://www.w3.org/2000/01/rdf-schema#"
    },
    {
      "@type": "Class",
      "@id": "Greeting",
      "rdfs:label": "rdf:langString",
      "message": "rdf:langString"
    }
  ]'
```

This creates a `Greeting` class with two language-tagged fields:

- **`rdfs:label`** — a standard RDF label, useful for display names
- **`message`** — a custom field for the greeting text

Both use `rdf:langString`, which means every value must include a language tag.

## Step 3: Insert a document

Language-tagged values use the `@lang` and `@value` keys. The `@lang` value must be a valid [IANA language tag](https://www.iana.org/assignments/language-subtag-registry/) such as `en`, `fr`, `de`, or `es`.

```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/greetings?author=admin&message=Add+greeting" \
  -H "Content-Type: application/json" \
  -d '{
    "@type": "Greeting",
    "@id": "Greeting/hello",
    "rdfs:label": {"@lang": "en", "@value": "Hello World"},
    "message": {"@lang": "fr", "@value": "Bonjour le monde"}
  }'
```

The response returns the full document ID:

```json
["terminusdb:///data/Greeting/hello"]
```

### What happens with invalid language tags?

TerminusDB validates language tags against the IANA registry. If you try a tag like `"foobar"`, the insert will fail with an `unknown_language_tag` error. Standard subtags like `en`, `en-GB`, `fr`, `de`, `zh-Hans` all work.

## Step 4: Retrieve the document

```bash
curl -u admin:root \
  "http://localhost:6363/api/document/admin/greetings?id=Greeting/hello&minimized=false"
```

The response preserves the language tags:

```json
{
  "@id": "Greeting/hello",
  "@type": "Greeting",
  "rdfs:label": {
    "@lang": "en",
    "@value": "Hello World"
  },
  "message": {
    "@lang": "fr",
    "@value": "Bonjour le monde"
  }
}
```

## Step 5: Update the document

To change the language or text, replace the full document with PUT. Every field must be included — omitted fields are removed.

```bash
curl -u admin:root -X PUT \
  "http://localhost:6363/api/document/admin/greetings?author=admin&message=Update+greeting" \
  -H "Content-Type: application/json" \
  -d '{
    "@type": "Greeting",
    "@id": "Greeting/hello",
    "rdfs:label": {"@lang": "de", "@value": "Hallo Welt"},
    "message": {"@lang": "es", "@value": "Hola Mundo"}
  }'
```

## Step 6: Retrieve all documents

To list all `Greeting` documents, omit the `id` parameter:

```bash
curl -u admin:root \
  "http://localhost:6363/api/document/admin/greetings?type=Greeting&as_list=true&minimized=false"
```

---

## Multiple languages per field

The schema above uses mandatory (single-value) fields, so each field holds exactly one language-tagged string. If you need to store the same field in several languages — for example, a label in English, French, and German — use a `Set` cardinality instead.

### Update the schema to use Set

```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/greetings?graph_type=schema&author=admin&message=Update+schema&full_replace=true" \
  -H "Content-Type: application/json" \
  -d '[
    {
      "@type": "@context",
      "@base": "terminusdb:///data/",
      "@schema": "terminusdb:///schema#",
      "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
      "rdfs": "http://www.w3.org/2000/01/rdf-schema#"
    },
    {
      "@type": "Class",
      "@id": "Greeting",
      "rdfs:label": {"@type": "Set", "@class": "rdf:langString"},
      "message": "rdf:langString"
    }
  ]'
```

Now `rdfs:label` accepts an array of language-tagged values while `message` remains single-valued.

### Insert a document with multiple labels

```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/greetings?author=admin&message=Add+multilingual" \
  -H "Content-Type: application/json" \
  -d '{
    "@type": "Greeting",
    "@id": "Greeting/world",
    "rdfs:label": [
      {"@lang": "en", "@value": "Hello World"},
      {"@lang": "fr", "@value": "Bonjour le monde"},
      {"@lang": "de", "@value": "Hallo Welt"}
    ],
    "message": {"@lang": "en", "@value": "A greeting in many languages"}
  }'
```

### Retrieve and inspect

```bash
curl -u admin:root \
  "http://localhost:6363/api/document/admin/greetings?id=Greeting/world&minimized=false"
```

The Set field comes back as an array:

```json
{
  "@id": "Greeting/world",
  "@type": "Greeting",
  "rdfs:label": [
    {"@lang": "en", "@value": "Hello World"},
    {"@lang": "fr", "@value": "Bonjour le monde"},
    {"@lang": "de", "@value": "Hallo Welt"}
  ],
  "message": {
    "@lang": "en",
    "@value": "A greeting in many languages"
  }
}
```

### Cardinality options for language strings

| Cardinality | Schema syntax | Multiple values? | Retrieval shape |
|---|---|---|---|
| Mandatory | `"rdf:langString"` | No — array is rejected | Single `{@lang, @value}` object |
| Optional | `{"@type":"Optional","@class":"rdf:langString"}` | No | Single object or absent |
| Set | `{"@type":"Set","@class":"rdf:langString"}` | Yes (unordered) | Array of objects |
| List | `{"@type":"List","@class":"rdf:langString"}` | Yes (ordered) | Array preserving insertion order |

Use **Set** when you want translations in any order, or **List** when insertion order matters (for example, a priority-ranked list of translations).

## Cleanup

When you are done experimenting, delete the database:

```bash
curl -u admin:root -X DELETE "http://localhost:6363/api/db/admin/greetings"
```

---

## Key points

- **Schema**: Declare `rdf` and `rdfs` prefixes in `@context`, use `"rdf:langString"` as the field type
- **Values**: Use `{"@lang": "<tag>", "@value": "<text>"}` for each language-tagged string
- **Validation**: Language tags are validated against the IANA registry — invalid tags are rejected
- **Round-trip**: Language tags are preserved exactly through insert and retrieval

## Further reading

- [Data Types Reference](/docs/data-types/) — full list of supported types including `rdf:langString`
- [Document API Reference](/docs/document-insertion/) — complete HTTP API parameter reference
- [Curl Quickstart](/docs/quickstart-example/) — broader introduction to the HTTP API
- [Schema Reference](/docs/schema-reference-guide/) — how to define classes and properties
