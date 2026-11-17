---
title: JavaScript Client Type Definitions
nextjs:
  metadata:
    title: JavaScript Client Type Definitions
    description: Reference for the typedefs exported by the TerminusDB JavaScript client — WOQLQueryResult, WithDataVersion, APIErrorBody, TerminusDBError and the DocParams objects.
    keywords: terminusdb, typescript, javascript client, typedef, types, reference
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/javascript-client-typedefs/
tags:
  - typescript
  - reference
---

The `terminusdb` package exports its reusable type definitions from the package root, so they can be imported alongside the client and referenced in annotations, casts, and generic type arguments. The same types are also reachable under the `typedef` namespace.

```typescript
import TerminusClient, {
  type WOQLQueryResult,
  type WithDataVersion,
} from "terminusdb"

const wrapped: WithDataVersion<WOQLQueryResult<MyRow>> = response
```

This page documents the shapes the client defines.
It describes compile-time types only — none of them are validated at runtime, and fields marked optional may simply be absent on a given response.

## Result shapes

| Type | Definition | Used by |
| --- | --- | --- |
| `WOQLQueryResult<T>` | `{ bindings: T[]; inserts?: number; deletes?: number; transaction_retry_count?: number }` | `client.query()` — one `bindings` row per solution |
| `WithDataVersion<T>` | `{ result: T; dataVersion: string }` | Any call with `getDataVersion: true` |
| `APIErrorBody` | `{ [key: string]: any }` — common fields: `@type`, `api:status`, `api:message`, `api:error` (whose own `@type` identifies the error, e.g. `api:DocumentNotFound`) | `err.data` on rejected calls |
| `TerminusDBError` | `{ message: string; name?: string; stack?: string; data?: APIErrorBody; status?: number }` | All rejected promises from `dispatch`; client-side validation failures are plain `Error`s without `data`/`status` |

## Document interface parameters

| Type | Used by | Purpose |
| --- | --- | --- |
| `DocParamsGet` | `getDocument`, `queryDocument` | GET parameters: `query`, `graph_type`, `type`, `id`, `prefixed`, `minimized`, `unfold`, `skip`, `count`, `as_list` |
| `DocParamsPost` | `addDocument` | POST parameters: `raw_json`, `graph_type`, `full_replace`, `overwrite`, `merge_repeats` |
| `DocParamsPut` | `updateDocument` | PUT parameters: `raw_json`, `create`, `graph_type`, `merge_repeats` |
| `DocParamsDelete` | `deleteDocument` | DELETE parameters: `graph_type`, `id`, `nuke` |

## Connection and resource types

| Type | Definition | Purpose |
| --- | --- | --- |
| `ParamsObj` | Connection parameters | `new WOQLClient(url, params)` |
| `CredentialObj` | `{ type: 'basic' \| 'jwt' \| 'apikey'; user?: string \| boolean; key: string }` | `localAuth`, `remoteAuth` credentials |
| `DbDetails` | `{ label: string; comment?: string; public?: boolean; schema?: boolean }` | `createDatabase` details |
| `DbDoc` | Database record shape | `getDatabase`, `getDatabases` results |
| `RolesObj` | `{ agent_name: string; database_name?: string; organization_name?: string; actions?: array; invitation?: string }` | Role management calls |
| `NamedResourceData` | `{ filename: string; data: Buffer \| string }` | CSV resources supplied to `query()` |

## Enumerations

| Type | Values |
| --- | --- |
| `GraphType` | `"instance" \| "schema"` |
| `GraphRef` | `"schema/main" \| "instance/main" \| string` |
| `ResourceType` | `"commits" \| "meta" \| "branch" \| "ref" \| "repo" \| "db"` |
| `RepoType` | `"local" \| "remote"` |
| `ScopeType` | `"database" \| "organization"` |
| `ActionType` | API actions: `'graph'`, `'db'`, `'clone'`, `'triples'`, `'woql'`, `'fetch'`, `'pull'`, `'rebase'`, `'branch'`, `'reset'`, `'push'`, `'squash'` |
| `DataFormatObj` | `{ type?: "csv" \| "turtle"; format_header?: string }` |
| `FuntionType` | WOQL update verbs: `"add_quad"`, `"delete_quad"`, `"add_triple"`, `"delete_triple"`, `"quad"`, `"triple"` |

## Query-building types

| Type | Definition | Purpose |
| --- | --- | --- |
| `LocalizeResult` | `[function(WOQLQuery=): WOQLQuery, Object.<string, Var>]` | Return of `WOQLQuery.localize()` — localizing wrapper plus unique variables |
| `WOQLJson` | `any` | JSON-LD representation of a WOQL query, as produced by `query.json()` |

{% callout type="note" %}
The generated declaration files under `dist/typescript/` in the `terminusdb` package are the source of truth; this page mirrors their intent for convenience.
The [TypeScript guide](/docs/typescript-with-the-javascript-client/) shows how the generic methods consume these types.
{% /callout %}
