---
title: "Patch Endpoint — Apply Structural Patches to Documents"
nextjs:
  metadata:
    title: "Patch Endpoint — Apply Structural Patches to Documents"
    description: "Apply JSON diff patches to documents in a TerminusDB branch. Patch a single document with a before/after comparison, or patch multiple documents in a resource by ID."
    keywords: terminusdb, branch, diff, document, javascript, patch, terminusdb javascript client, typescript
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/patch-endpoint/
media: []
lastUpdated: "2026-05-18"
tags:
  - typescript
  - diff-patch
  - reference
---

{% callout type="warning" %}
The endpoint at data.terminusdb.org is for non-production and exploratory use only. It is deployed with very limited memory to prevent abuse and is provided without any guarantees whatsoever. Do not rely on it for production applications. Availability is not guaranteed.
{% /callout %}

{% prerequisites-clone /%}

TerminusDB provides two patch operations for applying structural changes to documents:

- **`patch`** — Apply a patch to a specific document you supply (client-side before + patch)
- **`patchResource`** — Apply patches to documents already stored in a branch (server-side, by `@id`)

Both use the same [patch format](/docs/json-diff-and-patch/) produced by TerminusDB's diff operations.

*The patch endpoint was added in [TerminusDB 11.0.3](/docs/releases/11-0-3-patch-endpoint/).*

## Patch a single document

Use `client.patch(before, patch)` when you have the original document and a patch object. TerminusDB applies the patch and returns the result — no database write occurs.

```javascript
const before = {
  "@id": "Person/Jane",
  "@type": "Person",
  name: "Jane",
}

const patch = {
  name: { "@op": "SwapValue", "@before": "Jane", "@after": "Janine" },
}

const result = await client.patch(before, patch)
console.log(result)
// { "@id": "Person/Jane", "@type": "Person", "name": "Janine" }
```

This is useful for previewing what a patch would produce before committing it to a branch.

## Patch documents in a branch

Use `client.patchResource(patch, message)` to apply patches directly to documents stored in the current branch.

{% callout type="note" %}
Insert and Delete operations were added in [TerminusDB 12.0.7](/docs/releases/12-0-7-insert-delete-patches-folded-diffs/) — they are what make resolving merge conflicts through the patch endpoint possible.
{% /callout %}

The `patch` array accepts three types of operations:

- **Field-level patch**: `{"@id":"...","field":{"@op":"SwapValue","@before":"...","@after":"..."}}`
- **Insert**: `{"@op":"Insert","@insert":{"@id":"...","@type":"...","field":"value"}}`
- **Delete**: `{"@op":"Delete","@delete":{"@id":"..."}}` or `{"@op":"Delete","@delete":"Document/ID"}`

Insert and Delete operations use the same format as the diff endpoint output, enabling a diff→filter→patch workflow. Insert requires either `@id` or `@capture` to be present in the `@insert` document.

### Captures and cross-references

Insert operations support `@capture` and `@ref` for cross-referencing documents within a single patch transaction. A document with `@capture:"name"` can be referenced by another document using `{"@ref":"name"}`. This is useful for creating linked documents in one atomic operation:

```json
[
  {"@op":"Insert","@insert":{"@id":"Person/Alice","@type":"Person","@capture":"A","name":"Alice","friend":{"@ref":"B"}}},
  {"@op":"Insert","@insert":{"@id":"Person/Bob","@type":"Person","@capture":"B","name":"Bob","friend":{"@ref":"A"}}}
]
```

A document may have both `@id` and `@capture` — `@id` assigns the document ID, while `@capture` registers a name for use in `@ref` cross-references.

### @ref in field-level patches

The `@ref` mechanism also works in field-level `SwapValue` operations. A `SwapValue` `@before` or `@after` value can use `{"@ref":"name"}` to reference a capture from the same patch transaction. This allows swapping a document's reference field to a newly inserted document:

```json
[
  {"@op":"Insert","@insert":{"@id":"Person/Alice","@type":"Person","@capture":"A","name":"Alice"}},
  {"@id":"Person/Charlie","friend":{"@op":"SwapValue","@before":null,"@after":{"@ref":"A"}}}
]
```

Note that field-level patches operate on documents from the previous committed layer — you cannot patch a document that was inserted in the same patch transaction. However, `@ref` in a field-level patch can reference captures from same-transaction inserts. URI comparisons are performed on fully expanded canonical IRIs, so compressed IDs (e.g. `Person/Bob`) and full IRIs (e.g. `http://example.com/document/Person/Bob`) are treated as equal.

### Patch format

Supply an array of patch objects, each with `@id` set to the document to modify:

```json
[
  {
    "@id": "Obj/id1",
    "name": {
      "@op": "SwapValue",
      "@before": "foo",
      "@after": "bar"
    }
  },
  {
    "@id": "Obj/id2",
    "name": {
      "@op": "SwapValue",
      "@before": "foo",
      "@after": "bar"
    }
  }
]
```

### Apply the patch

```javascript
client.db("tdb-example-mydb")
client.checkout("mybranch")

const patch = [
  {
    "@id": "Obj/id1",
    "name": { "@op": "SwapValue", "@before": "foo", "@after": "bar" },
  },
  {
    "@op": "Insert",
    "@insert": { "@id": "Obj/id3", "@type": "Class", name: "baz" },
  },
  {
    "@op": "Delete",
    "@delete": { "@id": "Obj/id2" },
  },
]

const patchResult = await client.patchResource(patch, "apply patch to mybranch")
console.log(patchResult)
// ["Obj/id1", "Obj/id3"]
```

### Conflict errors

If a patch cannot be applied (for example, the `@before` value does not match the current document state), TerminusDB returns a **409 Conflict** response:

```json
{
  "@type": "api:PatchError",
  "api:status": "api:conflict",
  "api:witnesses": [
    {
      "@op": "InsertConflict",
      "@id_already_exists": "Person/Jane"
    }
  ]
}
```

## Related

- [JSON Diff and Patch](/docs/json-diff-and-patch/) — How to generate patches using diff operations
- [JavaScript Client API — `patch`](/docs/javascript/#woqlclient-patch) — Full method signature
- [JavaScript Client API — `patchResource`](/docs/javascript/#woqlclient-patchresource) — Full method signature
