---
tags:
  - release-notes
  - schema
  - dashboard
draft: true
status: draft
ai_generated: true
title: 'TerminusDB 10.1.4: Multilingual Schemas and Class Metadata'
release:
  version: 10.1.4
  date: 2022-08-24
  line: '10.1'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v10.1.4
nextjs:
  metadata:
    title: 'Multilingual Schemas, Class Metadata | TerminusDB 10.1.4'
    description: 'TerminusDB 10.1.4 lets you add language-tagged strings and free JSON metadata to your schema, so one model serves many locales. Also: a perf fix.'
    keywords: terminusdb 10.1.4, multilingual schema, language tagged strings, class metadata json, schema annotations, terminusdb release notes
    robots: noindex, nofollow
    alternates:
      canonical: https://terminusdb.com/docs/releases/10-1-4-multilingual-schemas-metadata/
---

TerminusDB 10.1.4 was released on **24 August 2022**. It brings two schema capabilities: language-tagged strings for multilingual models, and free-form JSON `@metadata` on classes for annotations that travel with the schema.

- [**Multilingual schemas**](#multilingual-schemas) — labels and documentation in several languages.
- [**Attach metadata to classes**](#attach-metadata-to-classes) — arbitrary JSON on a class, readable by tooling.

## Multilingual schemas

### What it does

Schema elements accept language-tagged strings, so a class or property can carry its label and documentation in more than one language. A single schema can now serve a French UI and an English UI from the same definitions.

### Why it matters

Support for schema in multiple languages allows for better internationalization and localization of schemas, making it easier to serve schema-aware dynamic user interfaces in different languages where the application is fixed but the model fluid.

**Read more:** [Language strings with curl](/docs/language-strings-with-curl/)

## Attach metadata to classes

### What it does

A class can carry free, untyped JSON under `@metadata`. The value is stored with the schema and returned on schema retrieval, so tools and UIs can read hints, display settings or integration flags straight from the class definition.

```json
{
  "@id": "Person",
  "@type": "Class",
  "@metadata": { "render": "markdown", "icon": "person" },
  "name": "xsd:string"
}
```

### Why it matters

Metadata annotations on schema classes provide a way to annotate the behaviors associated with the schema in a machine readable way and provide processing instructions for the instance documents. These get used by tools and UIs to read hints, display settings, or integration flags directly from the class definition.

**Read more:** [@metadata in the schema guide](/docs/schema-reference-guide/#metadata)

## Fixes

- A serious performance regression is fixed ([#1401](https://github.com/terminusdb/terminusdb/pull/1401)).
- `JSONDocument` is linkable again from `Optional`, `Set`, `List` and `Array` properties.

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v10.1.4
```

No data migration is needed. [Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [GitHub release](https://github.com/terminusdb/terminusdb/releases/tag/v10.1.4)

---

**Previous:** [TerminusDB 10.1.3](/docs/releases/10-1-3-cast-strings-sys-top/) · **Next:** [TerminusDB 10.1.7](/docs/releases/10-1-7-dashboard-time-travel/) · [TerminusDB 10.1 changelog](/docs/releases/10-1-changelog/) · [Release index](/docs/releases/)
