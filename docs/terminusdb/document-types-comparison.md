---
title: TerminusDB Document Types Compared
nextjs:
  metadata:
    title: "Document Types: Documents, Subdocuments, and Shared Documents"
    description: "Compare TerminusDB document types - when to use regular documents, subdocuments, and shared documents for different ownership and lifecycle patterns"
    keywords: terminusdb, data model, diff, document, document database, document type, documents, javascript
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/document-types-comparison/
media: []
versionNote:
  version: "12.0.6"
  text: "The @shared document type described on this page is available from TerminusDB 12.0.6."
tags:
  - typescript
  - documents
  - schema
  - reference
---

## Document types at a glance

{% callout type="note" title="Version 12.0.6" %}
The `@shared` document type described on this page is available from TerminusDB **12.0.6**. If you are running an earlier version, only regular documents and subdocuments are available.
{% /callout %}

TerminusDB offers several RDF graph document types, each with different ownership, lifecycle, and identity semantics. Choosing the right type shapes how your linked data behaves when documents are created, shared between parents, and deleted.

This page compares the types side by side to help you choose the right one for your use case.

## Comparison table

| Property | Regular document | Shared document (`@shared`) | Subdocument (`@subdocument`) |
|----------|-----------------|----------------------------|------------------------------|
| **Identity** | Own IRI (`Class/id`) | Own IRI (`Class/id`) | Nested IRI (parent path) |
| **Key types** | Any (`Lexical`, `Hash`, `ValueHash`, `Random`) | Any (`Lexical`, `Hash`, `ValueHash`, `Random`) | Any (`Lexical`, `Hash`, `ValueHash`, `Random`) |
| **Ownership** | Independent | Independent (but lifecycle-managed) | Owned by exactly one parent document/subdocument |
| **Can be referenced by** | Any number of documents | Any number of documents | Only its containing document |
| **Lifecycle** | Manual deletion only | Automatic cascade deletion when zero references remain, or manually | Only deleted when parent is deleted |
| **Standalone creation** | Yes | Yes | No (must be nested in a parent) |
| **Standalone retrieval** | Yes (by IRI) | Yes (by IRI) | Yes (by its nested IRI) |
| **Unfolding** | By IRI reference (unless `@unfoldable`) | By IRI reference (unless `@unfoldable`) | Always unfolded inline |

## Regular documents

Regular documents are the default. They have their own IRI, can be referenced/linked by any number of other documents, and are never deleted automatically. Use them when documents have an independent lifecycle.

```json
{
    "@type"  : "Class",
    "@id"    : "Person",
    "@key"   : { "@type": "Lexical", "@fields": ["name"] },
    "name"   : "xsd:string",
    "email"  : "xsd:string"
}
```

**When to use:** The document exists independently, is shared across many contexts, and should never be automatically deleted. Examples: users, products, reference data.

## Subdocuments (`@subdocument`)

A subdocument is wholly owned by its containing document. It cannot exist independently and is always created, retrieved, and deleted together with its parent.

```json
{
    "@type"         : "Class",
    "@id"           : "Address",
    "@subdocument"  : [],
    "@key"          : { "@type": "Random" },
    "street"        : "xsd:string",
    "city"          : "xsd:string",
    "postal_code"   : "xsd:string"
}
```

**When to use:** The data is internal to a parent document and never needs to be shared or referenced from elsewhere. Examples: addresses embedded in a person, configuration nested in a service, line items in an order. All documents that belong only to one particular parent document. 

**Constraints:**
- Only one parent can point to a given subdocument instance
- Cannot be retrieved or updated independently (only through the parent)

## Shared documents (`@shared`)

A shared document is a regular document (own IRI, any key type) that participates in automatic lifecycle management. When a `@shared` document is no longer referenced by any other document, it is automatically cascade-deleted at commit time.

```json
{
    "@type"    : "Class",
    "@id"      : "Footnote",
    "@shared"  : [],
    "@key"     : { "@type": "Lexical", "@fields": ["label"] },
    "label"    : "xsd:string",
    "content"  : "xsd:string"
}
```

**When to use:** The document may be shared across multiple parents, but once nothing references it, it should be automatically cleaned up. Examples: footnotes shared between articles, tags used by multiple resources, shared configuration blocks.

**Constraints:**
- Cannot also be `@subdocument` (mutual exclusion)
- Liveness is determined at commit time by counting all triples pointing to the document
- Explicit deletion is still permitted (behaves like a regular document delete)

### How `@shared` liveness works

When any reference to a `@shared` document is removed (through parent deletion, field update, or triple removal), the engine checks at commit time whether any triples remain that point to the target. If zero references remain, the document is automatically cascade-deleted.

The liveness check counts **all** triples pointing to the target document, regardless of which field or class they originate from. Any reference from any document keeps the `@shared` document alive.

```
Article A ──notes──► Footnote/1 (@shared)
Article B ──notes──► Footnote/1

Delete Article A → Footnote/1 still alive (Article B references it)
Delete Article B → Footnote/1 has zero references → cascade-deleted
```

### Cascade is recursive

If a cascade-deleted `@shared` document itself has fields pointing to other `@shared` documents, those targets are checked in turn. A visited set prevents infinite loops on circular references.

If two `@shared` documents reference only each other (a circular island) and all external references are removed, both are cascade-deleted.

### Standalone creation

`@shared` documents can be created independently with no parent referencing them. They are not immediately deleted upon creation; the liveness check only fires when a reference is **removed**, not when a document has never been referenced.

### Worked example: full lifecycle

Using the Footnote/Article schema from above, this example walks through creation, sharing, and cascade deletion.

**Step 1 — Define the schema:**

```json
{ "@type": "Class", "@id": "Footnote", "@shared": [],
  "@key": { "@type": "Lexical", "@fields": ["label"] },
  "label": "xsd:string", "content": "xsd:string" }

{ "@type": "Class", "@id": "Article",
  "@key": { "@type": "Lexical", "@fields": ["title"] },
  "title": "xsd:string",
  "notes": { "@type": "Set", "@class": "Footnote" } }
```

**Step 2 — Create a `@shared` footnote standalone (no parent yet):**

```json
{ "@type": "Footnote", "label": "fn1", "content": "See appendix A." }
```

Result: `Footnote/fn1` exists. No cascade check fires — it has never been referenced.

**Step 3 — Insert two articles that reference the footnote:**

```json
{ "@type": "Article", "title": "intro", "notes": ["Footnote/fn1"] }
{ "@type": "Article", "title": "appendix", "notes": ["Footnote/fn1"] }
```

Result: `Footnote/fn1` is now referenced by two parents.

**Step 4 — Delete one parent:**

```json
DELETE Article/intro
```

Result: `Article/intro` is deleted. The engine checks `Footnote/fn1` — `Article/appendix` still references it. **Target survives.**

**Step 5 — Delete the second parent:**

```json
DELETE Article/appendix
```

Result: `Article/appendix` is deleted. The engine checks `Footnote/fn1` — zero references remain. **Target is cascade-deleted automatically.**

After step 5, querying for `Footnote/fn1` returns nothing — it was cleaned up without an explicit delete call.

## Unfoldable documents (`@unfoldable` / `@unfold`)

Unfolding is orthogonal to lifecycle and ownership. It controls how a document appears **on retrieval** - whether it is returned as a full inline object or as an IRI reference.

| Annotation | Level | Purpose |
|-----------|-------|---------|
| `@unfoldable` | Class-level | All references to this class unfold inline |
| `@unfold` | Field-level | Only this specific field unfolds its target inline |

Unfolding can be combined with any document type. A `@shared` document may also be `@unfoldable` if you want it to appear inline when retrieved. The annotations address different concerns:

- **Lifecycle:** `@shared`, `@subdocument` (when is the document deleted?)
- **Retrieval:** `@unfoldable`, `@unfold` (how is the document represented in API responses?)

See the [Document Unfolding Reference](/docs/document-unfolding-reference/) for details on cycle detection and performance.

## Decision flowchart

Use the following questions to choose the right document type:

1. **Does this data belong exclusively to one parent?**
   - Yes → **Subdocument** (`@subdocument`)
   - No → Continue

2. **Should multiple documents reference this data?**
   - No → **Regular document** (or subdocument if exclusively owned)
   - Yes → Continue

3. **Should the document be automatically deleted when nothing references it?**
   - Yes → **Shared document** (`@shared`)
   - No → **Regular document**

4. **Should the document appear inline when retrieved (regardless of type)?**
   - Yes → Add `@unfoldable` to the class, or `@unfold` to specific fields

## Migration notes

### Adding `@shared` to an existing class

No data migration is needed. Adding `@shared` to a class definition enables the cascade semantics from that point forward. Existing documents are unaffected until a reference to them is removed.

### Removing `@shared` from a class

The cascade semantics stop immediately. Previously-shared documents become regular documents with no automatic lifecycle management. No data changes are required.

### Moving from `@subdocument` to `@shared`

This is a breaking schema change. Subdocuments have nested IRIs and restricted key types; shared documents have independent IRIs. Migration requires:

1. Create new shared document instances with their own IRIs
2. Update all parent references to point to the new IRIs
3. Remove the old subdocument instances
4. Update the schema (remove `@subdocument`, add `@shared`)

This cannot currently be done in a single transaction without data restructuring.

## Further reading

- [**The document model**](/docs/documents-explanation/) — how documents, subdocuments, and references work in the graph
- [**Schema reference guide**](/docs/schema-reference-guide/) — full syntax for `@subdocument`, `@shared`, and key strategies
- [**Add a schema**](/docs/add-a-schema/) — define document types with practical examples
- [**Schema migration**](/docs/schema-migration-reference-guide/) — change document types with data transformation
