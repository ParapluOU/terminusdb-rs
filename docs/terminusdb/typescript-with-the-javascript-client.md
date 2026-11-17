---
title: TypeScript with the JavaScript Client
nextjs:
  metadata:
    title: TypeScript with the JavaScript Client
    description: Use generics, typed WOQL variables, and shared typedefs with the TerminusDB JavaScript client in TypeScript.
    keywords: terminusdb, typescript, javascript client, generics, typedef, woql
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/typescript-with-the-javascript-client/
tags:
  - typescript
  - how-to
  - beginner
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide you will call the TerminusDB JavaScript client from TypeScript with typed query bindings, typed documents, version-tracked responses, and a documented error shape.
{% /callout %}

The `terminusdb` package ships generated TypeScript declarations, so `import` works out of the box with no extra `@types` package.
This guide covers what those declarations give you beyond plain JavaScript: generic result types, typed WOQL variables, and the shared type definitions exported under `typedef`.
It does not add runtime validation — the type parameters describe what you expect the server to return, they do not check it.

## Prerequisites

- TerminusDB running locally or a TerminusCMS account
- Node.js 22+ installed
- The client installed and connected — see [Install JS Client](/docs/install-terminusdb-js-client/) and [Connect to JS Client](/docs/connect-with-the-javascript-client/)

## Typed WOQL variables

Wherever a WOQL parameter expects a variable, the generated types accept both the `"v:name"` string form and `Var` objects produced by `WOQL.vars(...)` (array) or `WOQL.Vars(...)` (record).
Use whichever reads better; the compiler accepts both.

```typescript
const [person, name] = TerminusClient.WOQL.vars("person", "name")
const v = TerminusClient.WOQL.Vars("person", "name") // v.person, v.name
```

## Typed query bindings

`client.query<T>` takes a type parameter describing one result row.
The resolved promise has `bindings: T[]` plus the update counters the server returns for write queries.

```typescript test-example id="ts-typed-query" fixture="docs-test" file="examples/ts-typed-query.example.ts"
```

When a call is made without a type argument the result stays `any`, so existing JavaScript-flavoured TypeScript keeps compiling unchanged.

## Typed documents

`getDocument`, `addDocument`, `updateDocument`, `deleteDocument` and `queryDocument` all take an optional document type.
Choose `T` for a single document or `T[]` when the call returns a list, for example with `as_list: true`.

```typescript test-example id="ts-typed-document" fixture="docs-test" file="examples/ts-typed-document.example.ts"
```

`getSchema<T>` resolves to `T[]` and is typed the same way for schema documents.

## Version-tracked responses

Methods with a `getDataVersion` parameter accept a second type parameter for the wrapped response: `WithDataVersion<T>` gives you `result` and the `dataVersion` token used for optimistic concurrency.

```typescript test-example id="ts-data-version" fixture="docs-test" file="examples/ts-data-version.example.ts"
```

## Error handling

Failed calls reject with a standard `Error` carrying the HTTP status on `.status` and the parsed server error body on `.data`, modelled as `TerminusDBError`.
A missing document is a catchable `404` with `api:DocumentNotFound` in the error body — an empty `as_list` result remains a successful `[]`, not an error.

```typescript test-example id="ts-error-handling" fixture="docs-test" file="examples/ts-error-handling.example.ts"
```

## Shared type definitions

All reusable shapes — `WOQLQueryResult`, `WithDataVersion`, `APIErrorBody`, `TerminusDBError`, and the `DocParams*` parameter objects — are importable by name from the package root and also exported under the `typedef` namespace.
See [JavaScript Client Type Definitions](/docs/javascript-client-typedefs/) for the full reference.

{% callout type="note" %}
Type parameters are checked at compile time only.
The client does not validate responses against them at runtime; a schema change server-side can still produce a shape your type does not describe.
{% /callout %}
