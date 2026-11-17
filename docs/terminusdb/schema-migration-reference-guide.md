---
title: Schema Migration Reference Guide
nextjs:
  metadata:
    title: Schema Migration Reference Guide — TerminusDB Schema Migrations
    description: Complete reference for schema migration operations in TerminusDB — add fields, rename properties, change types, and re-key documents with worked examples.
    keywords: schema migration, terminusdb migration, database schema change, add field migration, rename property, change type, schema evolution, backward compatible
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/schema-migration-reference-guide/
media: []
tags:
  - typescript
  - schema
  - reference
  - intermediate
lastUpdated: "2026-05-01"
---

{% callout title="What you'll achieve" %}
By the end of this guide, you will know how to migrate your TerminusDB schema — add required fields with defaults, rename properties, change field types, and re-key documents — all with automatic instance data transformation.
{% /callout %}

{% prerequisites-clone /%}

## Worked example: evolve a Product schema

This example demonstrates three common migration operations in a single API call. You have a `Product` class and need to:

1. **Add a required field** (`sku`) with a default value for existing documents
2. **Rename a field** (`category` → `department`)
3. **Cast a field type** (`price` from `xsd:string` → `xsd:decimal`)

### Before migration

**Schema:**

```json
{
  "@id": "Product",
  "@type": "Class",
  "@key": {"@type": "Lexical", "@fields": ["name"]},
  "name": "xsd:string",
  "price": "xsd:string",
  "category": "xsd:string"
}
```

**Existing document:**

```json
{"@id": "Product/Widget", "@type": "Product", "name": "Widget", "price": "9.99", "category": "tools"}
```

### Run the migration

```bash
curl -u admin:root -X POST "http://localhost:6363/api/migration/admin/tdb-example-mydb" \
  -H "Content-Type: application/json" \
  -d '{
    "author": "alice@example.com",
    "message": "Evolve Product: add sku, rename category, cast price",
    "operations": [
      {
        "@type": "CreateClassProperty",
        "class": "Product",
        "property": "sku",
        "type": "xsd:string",
        "default": {"@type": "Default", "value": "UNKNOWN"}
      },
      {
        "@type": "MoveClassProperty",
        "class": "Product",
        "from": "category",
        "to": "department"
      },
      {
        "@type": "CastClassProperty",
        "class": "Product",
        "property": "price",
        "type": "xsd:decimal",
        "default": {"@type": "Default", "value": 0.0}
      }
    ]
  }'
```

**Expected response:**

```json
{"@type": "api:MigrationResponse", "api:status": "api:success"}
```

### After migration

**Schema:**

```json
{
  "@id": "Product",
  "@type": "Class",
  "@key": {"@type": "Lexical", "@fields": ["name"]},
  "name": "xsd:string",
  "price": "xsd:decimal",
  "sku": "xsd:string",
  "department": "xsd:string"
}
```

**Transformed document:**

```json
{"@id": "Product/Widget", "@type": "Product", "name": "Widget", "price": 9.99, "sku": "UNKNOWN", "department": "tools"}
```

Every existing document was transformed automatically:
- `sku` added with value `"UNKNOWN"` (the migration default)
- `category` renamed to `department` (value preserved)
- `price` cast from string `"9.99"` to decimal `9.99`

### Dry-run mode

Preview what a migration will do without applying it:

```bash
curl -u admin:root -X POST "http://localhost:6363/api/migration/admin/tdb-example-mydb?dry_run=true&verbose=true" \
  -H "Content-Type: application/json" \
  -d '{
    "author": "alice@example.com",
    "message": "Preview migration",
    "operations": [
      {"@type": "CreateClassProperty", "class": "Product", "property": "sku", "type": "xsd:string", "default": {"@type": "Default", "value": "UNKNOWN"}}
    ]
  }'
```

**Expected response:**

```json
{"@type": "api:MigrationResponse", "api:status": "api:success"}
```

The database is unchanged — dry-run validates that the operations are legal without committing.

---

## How schema migration works

Schema migration moves schema and instance data together automatically in a replayable fashion. This is essential for allowing flexible schemas to co-exist nicely with change-requests and merges.

You can perform schema operations directly on the branch of interest, or you can _target_ the schema of a given branch in another branch, allowing the migrations to be re-performed such that a new common schema is obtained.

TerminusDB can _infer_ some migrations silently when they do not impact instance data. However, operations that require instance data to change must be specified explicitly.

### Weakening vs strengthening

- **Weakening** (backward-compatible): add optional fields, add new classes, widen types. No existing data changes.
- **Strengthening** (breaking): add required fields, delete classes, narrow types. Existing data must be transformed.

The worked example above demonstrates both: `CreateClassProperty` with a default is a strengthening (it modifies instance data), while adding an `Optional` property would be a weakening.

For a step-by-step example of changing a class on a branch and then targeting that schema from `main`, plus a pattern for deprecating classes and properties, see [Ontology Versioning](/docs/ontology-versioning/).

### Inferred migrations on schema writes (`require_migration`)

Schema documents can also be written through the document API (`POST/PUT /api/document` with `graph_type=schema`). Passing `require_migration=true` asks the engine to _infer_ the migration that transforms the old schema into the submitted one, instead of submitting operations to `/api/migration` by hand.

The inference contract:

- **Inferred operations are weakening-or-declared.** A schema edit that reads as a weakening — optional properties added, enum values added, metadata or documentation changes, context `@base`/`@schema` changes, class `@base` changes — is inferred and applied. Edits that would destroy or transform data (dropped properties or classes, narrowed types) are rejected with a `weakening_failure` error unless the request also sets `allow_destructive_migration=true`.
- **Weakening operations never write instance data.** Every inferrable weakening op performs zero instance operations — context, metadata, documentation, `@base`, optional-property and enum-expansion changes leave the instance graph untouched. A context replacement swaps only the context document; stored IRIs are immutable, so a minted document or class IRI never moves. The new `@base`/`@schema` steer future IRI minting only.
- **Failures are loud.** When a submitted schema cannot be migrated without violating the contract, the commit fails with a typed `api:` error and a witness naming the offending class or property — no silent data loss and no partial writes. To rewrite stored IRIs deliberately, use the explicit operation surface (`/api/migration`), never an inferred schema write.

---

## Schema migration operations

Pass an ordered list of operations to `POST /api/migration/{path}`. Operations are order-dependent — different orderings can produce different instance data transformations.

Operations marked **weakening** never alter existing instance data. Those marked **strengthening** require instance data changes and will fail without appropriate defaults.

## DeleteClass

The `DeleteClass` operation will remove a class from a schema. This does not change the _range_ of properties, so these properties must first be dropped before deleting a class is possible.

Due to the fact that existing instance data of this class will be deleted, this is not a _weakening_ operation.

```typescript
{ "@type" : "DeleteClass",
  "class" : <ClassName> }
```

An example of the operation would be:

```typescript
{ "@type" : "DeleteClass",
  "class" : "Person" }
```

Which would take the schema:

```typescript
{ "@id" : "Dog",
  "@type" : "Class",
  "name" : "xsd:string"}
{ "@id" : "Person",
  "@type" : "Class",
  "name" : "xsd:string" }
```

to:

```typescript
{ "@id" : "Dog",
  "@type" : "Class",
  "name" : "xsd:string"}
```

## CreateClass

The `CreateClass` operation specifies the entire class to be created. This operation is always a _weakening_ operation.

```typescript
{ "@type" : "CreateClass",
  "class_document" : <ClassDocument> }
```

### Example

The migration:

```typescript
{ "@type" : "CreateClass",
  "class_document" :
  { "@id" : "Person",
    "@type" : "Class",
    "name" : "xsd:string" } }
```

Would take the schema:

```typescript
{ "@id" : "Dog",
  "@type" : "Class",
  "name" : "xsd:string" }
```

to:

```typescript
{ "@id" : "Dog",
  "@type" : "Class",
  "name" : "xsd:string"}
{ "@id" : "Person",
  "@type" : "Class",
  "name" : "xsd:string" }
```

## MoveClass

The `MoveClass` operation renames a class and all of the URIs of instance data associated with that class. Due to the side-effects on instance data, this is not a _weakening_ operation.

```typescript
{ "@type" : "MoveClass",
  "from" : <FromClassName>,
  "to" : <ToClassName> }
```

### Example

```typescript
{ "@type" : "MoveClass",
  "from" : "Person",
  "to" : "Dog" }
```

Would take the schema:

```typescript
{ "@id" : "Person",
  "@type" : "Class",
  "name" : "xsd:string"}
```

to:

```typescript
{ "@id" : "Dog",
  "@type" : "Class",
  "name" : "xsd:string"}
```

## ReplaceClassMetadata

The `ReplaceClassMetadata` operation replaces the metadata on a class (if it exists). This operation is always a _weakening_ operation and has no effect on instance data.

```typescript
{ "@type" : "ReplaceClassMetadata",
  "class" : <ClassName>
  "metadata" : <Metadata> }
```

### Example

The operation:

```typescript
{ "@type" : "ReplaceClassMetadata",
  "class" : "Person",
  "metadata" : { "ui_preferences" : { "colour" : "blue" } } }
```

Would take the schema:

```typescript
{ "@id" : "Person",
  "@type" : "Class",
  "@metadata" : { "ui_preferences" : { "colour" : "red" } },
  "name" : "xsd:string"}
```

to:

```typescript
{ "@id" : "Person",
  "@type" : "Class",
  "@metadata" : { "ui_preferences" : { "colour" : "blue" } },
  "name" : "xsd:string" }
```

## ReplaceClassDocumentation

The `ReplaceClassDocumentation` operation replaces the documentation on a class (if it exists). This operation is always a _weakening_ operation and has no effect on instance data.

```typescript
{ "@type" : "ReplaceClassDocumentation",
  "class" : <ClassName>
  "documentation" : <Documentation> }
```

### Example

The operation:

```typescript
{ "@type" : "ReplaceClassDocumentation",
  "class" : "Person",
  "documentation" : { "@comment" : "This is a person class",
                      "@properties" : { "name" : { "@comment" : "The name of a person",
                                                    "@label" : "name" } },
                      "@label" : "Person" } }
```

Would take the schema:

```typescript
{ "@id" : "Person",
  "@type" : "Class",
  "@documentation" : { "@comment" : "A Person",
                       "@properties" : { "name" : { "@comment" : "Name of a person",
                                                    "@label" : "name" } },
                       "@label" : "Person" },
  "name" : "xsd:string"}
```

to:

```typescript
{ "@id" : "Person",
  "@type" : "Class",
  "@documentation" : { "@comment" : "This is a person class",
                      "@properties" : { "name" : { "@comment" : "The name of a person",
                                                   "@label" : "name" } },
                      "@label" : "Person" },
  "name" : "xsd:string"}
```

## ReplaceClassBase

The `ReplaceClassBase` operation sets or replaces the `@base` on a class. This operation is always a _weakening_ operation and has no effect on instance data: minted IRIs are immutable, so the new base only steers where future instance IRIs for that class are minted.

```typescript
{ "@type" : "ReplaceClassBase",
  "class" : <ClassName>,
  "base" : <BaseIRI> }
```

### Example

The operation:

```typescript
{ "@type" : "ReplaceClassBase",
  "class" : "Person",
  "base" : "https://example.org/people/" }
```

Would take the schema:

```typescript
{ "@id" : "Person",
  "@type" : "Class",
  "@base" : "https://example.org/humans/",
  "name" : "xsd:string" }
```

to:

```typescript
{ "@id" : "Person",
  "@type" : "Class",
  "@base" : "https://example.org/people/",
  "name" : "xsd:string" }
```

Existing `Person` documents keep their IRIs under `https://example.org/humans/`; only documents minted afterwards root at the new base.

## DeleteClassBase

The `DeleteClassBase` operation removes the `@base` from a class, reverting it to the default minting derived from the context. This operation is always a _weakening_ operation and has no effect on instance data.

```typescript
{ "@type" : "DeleteClassBase",
  "class" : <ClassName> }
```

## ReplaceContext

The `ReplaceContext` operation will update the context object, which will change how URIs are compressed when returning data and where newly minted identifiers are placed.

Changing `@base` or `@schema` affects only future IRI creation. Minted IRIs are immutable: a document, class, enum member, or property keeps its existing IRI forever, even after the context or the class itself changes. Multiple classes may share one `@base`, so rebasing stored identifiers is never attempted — instance data is rewritten only when an operation requires it precisely. Fault intolerance has priority: stored references always remain valid.

```typescript
{ "@type" : "ReplaceContext",
  "context" : <Context> }
```

## RebaseIRIPrefix

The `RebaseIRIPrefix` operation rewrites every stored IRI that starts with `from` so that it starts with `to` instead — in the instance, schema and inference graphs, at subject, predicate and object positions. It is the explicit, opt-in counterpart of the rule that minted IRIs are immutable: where `ReplaceContext` only changes future minting, `RebaseIRIPrefix` moves existing data.

```typescript
{ "@type" : "RebaseIRIPrefix",
  "from" : <PrefixIRI>,
  "to" : <PrefixIRI> }
```

`from` and `to` may be full IRI prefixes or names of prefixes declared in the context — they are resolved under the current context before matching. Rewriting `from` to the same value is an error.

Because the operation rewrites stored identifiers, it is **destructive** and requires `allow_destructive_migration: true`; without the flag the migration fails before any triple moves. Whole-document replacements are never involved: every affected triple is rewritten in place, so subdocuments, captured backlinks and cons cells move with the rest of the graph atomically in a single transaction.

Covered positions include:

- `rdf:type` references, property predicates and document links in the instance graph — including subdocuments, which keep their own minted `@id` paths unless the data prefix itself is rebased.
- Schema triples: class identifiers, `@inherits` and `@oneOf`/TaggedUnion member references, enum member nodes, key and type-spec node identifiers.
- Prefix declarations and `@base`/`@schema` minting prefixes — the namespace move carries its own minting declarations so new identifiers land under the new namespace. Pair the operation with `ReplaceContext` when the compression vocabulary should change with it.

Literal values are never rewritten — a stored string or a saved WOQL query that happens to mention the old prefix is data, not vocabulary.

### Example

```typescript
{ "@type" : "RebaseIRIPrefix",
  "from" : "terminusdb:///schema#",
  "to" : "http://example.com/schema#" }
```

moves every class, property, enum member and instance reference under the new namespace while instance document IRIs (minted under `@base`) stay put.

## ExpandEnum

The `ExpandEnum` operation will allow new fields to be added to an `Enum`. This operation is always a weakening operation.

```typescript
{ "@type" : "ExpandEnum",
  "enum" : <EnumName>,
  "values" : [<Value0>, ... <ValueN>] }
```

### Example

The command

```typescript
{ "@type" : "ExpandEnum",
  "enum" : "Colour",
  "values" : ["purple", "orange"] }
```

Will take a schema:

```typescript
{ "@id" : "Colour",
  "@type" : "Enum",
  "@value" : ["red", "green", "blue"] }
```

To the schema:

```typescript
{ "@id" : "Colour",
  "@type" : "Enum",
  "@value" : ["red", "green", "blue", "purple", "orange"] }
```

## DeleteClassProperty

The `DeleteClassProperty` command removes a property from the schema and deletes all associated data points in the instance graph. This is not a _weakening_ operation.

```typescript
{ "@type" : "DeleteClassProperty",
  "class" : <ClassName>
  "property" : <PropertyName> }
```

## CreateClassProperty

The `CreateClassProperty` command creates a new property of a given name and type. It is a weakening operation only if the type is within a type family which includes:

*   Cardinality including zero
*   A Set
*   An Optional
*   An Array

Notably this excludes lists and required properties. With lists it will require the addition of the empty list resulting in a _strengthening_. The operation is impossible with a required property unless a default is specified.

```typescript
{ "@type" : "CreateClassProperty",
  "class" : <ClassName>,
  "property" : <PropertyName>,
  "type" : <Type> }
```

Or

```typescript
{ "@type" : "CreateClassProperty",
  "class" : <ClassName>,
  "property" : <PropertyName>,
  "type" : <Type>,
  "default" : <DefaultValue> }
```

## MoveClassProperty

The `MoveClassProperty` command will move the name of a property from one name to another.

```typescript
{ "@type" : "MoveClassProperty",
  "class" : <ClassName>,
  "from" : <PropertyName>,
  "to" : <PropertyName> }
```

This operation is never a weakening.

## UpcastClassProperty

The `UpcastClassProperty` command will weaken a type to another type which is a supertype or inclusive type family (such as moving a required or Optional to Set).

This operation is always a weakening.

```typescript
{ "@type" : "UpcastClassProperty",
  "class" : <ClassName>
  "property" : <PropertyName>,
  "type" : <TypeSpecification> }
```

## CastClassProperty

The `CastClassProperty` command will attempt to cast a property type to another type (such as a string to a date). This operation is never a weakening operation as it requires changing the type layout of data.

```typescript
{ "@type" : "CastClassProperty",
  "class" : <ClassName>
  "property" : <PropertyName>,
  "type" : <TypeSpecification>,
  "default" : <DefaultOrError> }
```

The \`DefaultOrError document is of the form:

```typescript
{ "@type" : "Error" }
```

Which will result in an error if casting is impossible, or:

```typescript
{ "@type" : "Default",
  "value" : <Value> }
```

Where `Value` is the value within type `TypeSpecification` which will be used if casting is impossible.

## ChangeKey

Changes the key strategy for a document class. This operation re-keys all existing documents of the specified class according to the new key descriptor.

**When the key strategy type changes** (e.g., Random to Lexical), all document IDs are regenerated and any references to the old IDs are updated automatically.

**When the key strategy type stays the same** (e.g., Random to Random, or the default base key to Random), the top-level document ID is preserved and only subdocument IDs that do not conform to the expected prefix are regenerated. Conforming subdocument IDs are left intact.

This operation also works when targeting a subdocument class directly. In that case, the migration processes each root parent document that contains an instance of the targeted subdocument class.

### ChangeKey without fields (Random)

```typescript
{ "@type" : "ChangeKey",
  "class" : <ClassName>,
  "key" : "Random" }
```

### ChangeKey with fields (Lexical)

```typescript
{ "@type" : "ChangeKey",
  "class" : <ClassName>,
  "key" : "Lexical",
  "fields" : [<FieldName>, ...] }
```

### ChangeKey with fields (Hash)

```typescript
{ "@type" : "ChangeKey",
  "class" : <ClassName>,
  "key" : "Hash",
  "fields" : [<FieldName>, ...] }
```

### ChangeKey (ValueHash)

```typescript
{ "@type" : "ChangeKey",
  "class" : <ClassName>,
  "key" : "ValueHash" }
```

## ChangeParents (unimplemented)

## ChangeCollection (unimplemented)