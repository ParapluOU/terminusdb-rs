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
      canonical: https://terminusdb.org/docs/patch-endpoint/
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

Use `client.patchResource(patch, message)` to apply patches directly to documents stored in the current branch. Each patch object must include an `@id` field identifying the target document.

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
    "@id": "Obj/id2",
    "name": { "@op": "SwapValue", "@before": "foo", "@after": "bar" },
  },
]

const patchResult = await client.patchResource(patch, "apply patch to mybranch")
console.log(patchResult)
// ["Obj/id1", "Obj/id2"]
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
