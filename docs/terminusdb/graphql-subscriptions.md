---
tags:
  - graphql
  - reference
  - intermediate
title: GraphQL Subscriptions Reference
nextjs:
  metadata:
    title: GraphQL Subscriptions Reference | TerminusDB
    description: Reference for GraphQL subscriptions over SSE Server-Sent Events in TerminusDB, including the connected event extension, next and complete events, error handling, and finite operations over SSE.
    keywords: graphql, subscriptions, sse, server-sent events, realtime, streaming, terminusdb, graphql-sse protocol
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/graphql-subscriptions/
media: []
---

{% callout type="note" title="Release Candidate — TerminusDB 12.1" %}
This page documents functionality in the upcoming TerminusDB 12.1 release. Details may change before the final release.
{% /callout %}

GraphQL subscriptions let you stream real-time updates from TerminusDB over Server-Sent Events (SSE). When a document is inserted, updated, or deleted, the server pushes an event to every active subscriber matching that change. This page is the complete reference for the subscription protocol, event formats, error handling, and client integration.

## How subscriptions work

TerminusDB implements GraphQL subscriptions using the [graphql-sse protocol](https://github.com/enisdenjo/graphql-sse/blob/master/PROTOCOL.md) in distinct connections mode. Each subscription request opens a long-lived HTTP connection (with keepalive) that the server keeps open and writes SSE events to as data changes.

The flow is straightforward:

1. Your client sends a `POST` request with `Accept: text/event-stream` and a GraphQL subscription query in the body.
2. The server validates the query, resolves the target data product, and registers the subscription.
3. The server sends a `connected` event (a TerminusDB extension — see below).
4. When a commit matching the subscription occurs, the server sends a `next` event with the result payload.
5. When the subscription ends — either because the server closes the stream or a validation error occurs — the server sends a `complete` event.

With Websockets, you often get into issues where authentication needs to be passed within messages, which is often messy in practice, including firewall piercing which is often less than trivial and requires special configuration. 

SSE is often easier to work with than websockets in these scenarios, even if you may need to ensure that the path is not buffering to get the best performance.

## Endpoint URL

Subscriptions use the same GraphQL endpoint as regular queries, but with a different `Accept` header:

```url
SERVERNAME/api/graphql/ORG/DATAPRODUCT
```

For a local instance with a data product named `admin/people`:

```url
http://localhost:6363/api/graphql/admin/people
```

The difference is in the request headers. A subscription request looks like this:

```http
POST /api/graphql/admin/people HTTP/1.1
Host: localhost:6363
Content-Type: application/json
Accept: text/event-stream
Authorization: Basic YWRtaW46cm9vdA==

{"query": "subscription { Person_added { _id name } }"}
```

## Authentication

Subscriptions use the same authentication as regular GraphQL requests. See [Connecting to GraphQL](/docs/connecting-to-graphql-reference/) for details on Basic Auth and TerminusDB Cloud token authentication.

## Subscription queries

A subscription query starts with the `subscription` keyword followed by a selection set. TerminusDB generates subscription fields from your schema — one field per document type per change type.

### Change types

Each document class generates three subscription fields:

- **`<Type>_added`** — fires when a new document of that type is inserted
- **`<Type>_updated`** — fires when an existing document of that type is replaced
- **`<Type>_deleted`** — fires when a document of that type is deleted

For example, with a `Person` class in your schema:

```graphql
subscription { Person_added { _id name } }
```

This subscription streams an event every time a new `Person` document is inserted. The selection set (`_id name`) determines which fields are included in each event payload.

### Filtering

Subscription fields accept a `filter` argument, using the same filter types as regular GraphQL queries:

```graphql
subscription {
  Person_added(filter: { name: { eq: "Alice" } }) {
    _id
    name
  }
}
```

Only inserts where `name` equals `"Alice"` will produce events. See [Filter with GraphQL](/docs/filter-with-graphql/) for the full filter syntax.

### Commit metadata

Each event includes a `_CommitMetadata` object when explicitly requested in the selection set:

```graphql
subscription {
  Person_added {
    _id
    name
    _CommitMetadata {
      _id
      _timestamp
      _datetime
      _change_type
    }
  }
}
```

The `_CommitMetadata` field provides:

- **`_id`** — the commit identifier
- **`_timestamp`** — the commit timestamp as a Unix epoch value
- **`_datetime`** — the commit timestamp as an ISO 8601 string
- **`_change_type`** — the type of change (`added`, `updated`, or `deleted`)

### Subclass matching

Subscriptions respect inheritance. If `Dog` inherits from `Animal`, subscribing to `Animal_added` will receive events for both `Animal` and `Dog` documents. This applies to per-document subscriptions and `_ChangeSet` alike.

For `_ChangeSet` subscriptions, you can control this behavior with the `include_children` argument. It defaults to `true`, meaning subclass documents are included. Set it to `false` at the top level to exclude subclass documents from all fields:

```graphql
subscription {
  _ChangeSet(include_children: false) {
    Animal_added { _id name }
  }
}
```

With `include_children: false`, only direct `Animal` documents appear in `Animal_added` — `Dog` instances are excluded. You can also override the top-level setting on individual fields:

```graphql
subscription {
  _ChangeSet(include_children: false) {
    Animal_added(include_children: true) { _id name }
    Dog_added { _id name }
  }
}
```

Here `Animal_added` includes subclass documents (per-field override), while `Dog_added` respects the top-level `false`.

## `_ChangeSet` subscriptions

Per-document subscriptions (`Person_added`, `Person_deleted`, etc.) fire once per changed document. When a single commit touches many documents, that means many events. `_ChangeSet` solves this by firing **once per commit** with all changes batched into a single payload.

### Query syntax

```graphql
subscription {
  _ChangeSet {
    Person_added { _id name }
    Person_changed { _id name }
    Person_deleted { _id name }
    _CommitMetadata { _id _timestamp _datetime _change_type }
  }
}
```

Each field under `_ChangeSet` follows the same `{Class}_{operation}` naming as per-document subscriptions, but returns a **list** of documents instead of a single document. The selection set on each field determines which fields are returned per document — same resolution mechanism, same security guarantees.

### Event shape

```json
{
  "data": {
    "_ChangeSet": {
      "Person_added": [
        {"_id": "Person/alice", "name": "Alice"},
        {"_id": "Person/bob", "name": "Bob"}
      ],
      "Person_changed": [],
      "Person_deleted": [
        {"_id": "Person/charlie", "name": "Charlie"}
      ],
      "_CommitMetadata": {
        "_id": "commit123",
        "_timestamp": 1722528000.0,
        "_datetime": "2024-08-01T16:00:00Z",
        "_change_type": "commit"
      }
    }
  }
}
```

The `_change_type` for `_ChangeSet` events is `"commit"` (not `added`/`changed`/`deleted`) because the event represents the entire commit, not an individual document change.

### Deleted document resolution

Deleted documents are resolved from the **pre-commit layer** — the state of the database before the commit was applied. This means you get the full document data (all fields that existed before deletion), not just the `_id`. The server obtains the parent layer internally; no extra configuration is needed.

### On-demand querying

`_ChangeSet` is also available on the Query root, so you can request the changes of the current commit on demand:

```graphql
query {
  _ChangeSet {
    Person_added { _id name }
  }
}
```

This returns the changes from the current transaction context. Note that deleted documents may not resolve fully on the Query root since they no longer exist in the current instance layer — full deleted resolution works only in the subscription context where the pre-commit layer is available.

### Coexistence with per-document subscriptions

`_ChangeSet` and per-document subscriptions are independent. You can subscribe to both on the same connection (or separate connections) and receive events from both. `_ChangeSet` fires once with the batch; per-document fields fire once per document. Neither interferes with the other.

### When to use `_ChangeSet` vs per-document

| Use case | Recommended subscription |
|----------|------------------------|
| React to individual document changes in real time | Per-document (`Person_added`) |
| Batch processing — process all changes from a commit at once | `_ChangeSet` |
| Audit log — record every change with commit metadata | `_ChangeSet` with `_CommitMetadata` |
| Selective — only care about one type | Per-document with filter |
| Multiple types in one event | `_ChangeSet` |

## SSE event types

TerminusDB sends three types of SSE events (plus one non-standard extension). Each event follows the standard SSE format with `event:` and `data:` fields separated by a blank line.

### `connected` (TerminusDB extension)

After a subscription is accepted, the server sends a `connected` event before any `next` events. This is not part of the graphql-sse protocol — it is a TerminusDB extension.

```
event: connected
data: null

```

The purpose is to signal subscription readiness. Without it, clients cannot know when it is safe to trigger data operations that should produce subscription events, creating a race condition between subscribing and the first mutation.

Strict SSE clients ignore unknown event types per the SSE specification, so this event is safe to ignore if your client does not need it.

### `next`

A `next` event carries a subscription result — the data payload for a matching change. The `data` field contains a JSON object with the selected fields:

```
event: next
data: {"Person_added":{"_id":"Person/alice","name":"Alice"}}

```

A `_ChangeSet` event looks like:

```
event: next
data: {"_ChangeSet":{"Person_added":[{"_id":"Person/alice","name":"Alice"}],"_CommitMetadata":{"_id":"commit123","_change_type":"commit"}}}

```

Multiple `next` events can be sent over the lifetime of a single subscription, one for each matching commit.

### `complete`

A `complete` event signals that the subscription is done and no more events will follow. The `data` field is empty:

```
event: complete
data:

```

After sending `complete`, the server closes the SSE stream. This event is sent when:

- A validation error occurs (the query fails to parse)
- A finite operation (query or mutation) is executed over SSE
- The server initiates a stream close during cleanup

### Error events

Validation errors are reported as `next` events containing a GraphQL errors object, followed by a `complete` event. The HTTP status code is always `200` with `text/event-stream` content type — errors are never sent as HTTP 4xx responses for SSE connections.

```
event: next
data: {"errors":[{"message":"Failed to parse subscription query"}]}

event: complete
data:

```

This follows the graphql-sse protocol, which requires validation errors to be reported through an accepted SSE connection rather than as HTTP error responses.

## Finite operations over SSE

TerminusDB also supports executing regular queries and mutations over an SSE connection. If you send a query or mutation (rather than a subscription) with `Accept: text/event-stream`, the server executes it, returns the result as a single `next` event, and then sends `complete`:

```
event: next
data: {"data":{"Person":[{"_id":"Person/alice","name":"Alice"}]}}

event: complete
data:

```

This is useful when you want a uniform SSE-based interface for all GraphQL operations — subscriptions and finite operations alike. The server detects the operation type by checking whether the query starts with `subscription`. Queries and mutations are executed with the same authentication and access control as regular GraphQL requests.

## NDJSON mode

TerminusDB also supports NDJSON (newline-delimited JSON) as an alternative to SSE. To use NDJSON, send `Accept: application/x-ndjson` instead of `text/event-stream`:

```http
POST /api/graphql/admin/people HTTP/1.1
Host: localhost:6363
Content-Type: application/json
Accept: application/x-ndjson
Authorization: Basic YWRtaW46cm9vdA==

{"query": "subscription { Person_added { _id name } }"}
```

In NDJSON mode, events are sent as plain JSON lines without `event:` prefixes:

- **`connected`** → `null`
- **`next`** → `{"Person_added":{"_id":"Person/alice","name":"Alice"}}` or `{"_ChangeSet":{"Person_added":[...]}}`
- **`complete`** → (empty line)

## System database subscriptions

You can subscribe to changes in the system database to monitor database lifecycle events. The system database endpoint is `/api/graphql/_system`:

```http
POST /api/graphql/_system HTTP/1.1
Host: localhost:6363
Content-Type: application/json
Accept: text/event-stream
Authorization: Basic YWRtaW46cm9vdA==

{"query": "subscription { UserDatabase_added { _id name } }"}
```

This streams an event whenever a new database is created. System database subscriptions require admin authentication.

## CORS

SSE endpoints support standard CORS headers. The server reflects the `Origin` request header in `Access-Control-Allow-Origin` and responds to `OPTIONS` preflight requests with `204 No Content`. See [Browser CORS](/docs/browser-cors-howto/) for details.

## Client integration

### Using fetch and EventSource

The most direct way to consume SSE subscriptions in JavaScript is with the `fetch` API and a streaming body reader:

```javascript
const response = await fetch('http://localhost:6363/api/graphql/admin/people', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Accept': 'text/event-stream',
    'Authorization': 'Basic ' + btoa('admin:root'),
  },
  body: JSON.stringify({
    query: 'subscription { Person_added { _id name } }',
  }),
})

const reader = response.body.getReader()
const decoder = new TextDecoder()
let buffer = ''

while (true) {
  const { done, value } = await reader.read()
  if (done) break
  buffer += decoder.decode(value, { stream: true })
  // Split on double newline to get SSE event blocks
  const blocks = buffer.split('\n\n')
  buffer = blocks.pop()
  for (const block of blocks) {
    if (!block.trim()) continue
    const event = parseSSEBlock(block)
    if (event) console.log(event)
  }
}

function parseSSEBlock(block) {
  let eventType = 'message'
  let dataLine = null
  for (const line of block.split('\n')) {
    if (line.startsWith('event: ')) eventType = line.slice(7).trim()
    else if (line.startsWith('data: ')) dataLine = line.slice(6)
    else if (line.startsWith('data:')) dataLine = line.slice(5)
  }
  if (dataLine === null) return null
  if (dataLine.trim() === '') return { _eventType: eventType, data: null }
  try {
    const parsed = JSON.parse(dataLine)
    if (typeof parsed === 'object' && parsed !== null) parsed._eventType = eventType
    return { _eventType: eventType, data: parsed }
  } catch { return null }
}
```

### Using the graphql-sse client

The [`graphql-sse`](https://www.npmjs.com/package/graphql-sse) npm package provides a ready-made client for the graphql-sse protocol. Since TerminusDB sends a non-standard `connected` event, configure the client to ignore unknown event types (which is the default behavior per the SSE specification):

```javascript
import { createClient } from 'graphql-sse'

const client = createClient({
  url: 'http://localhost:6363/api/graphql/admin/people',
  headers: {
    Authorization: 'Basic ' + btoa('admin:root'),
  },
})

const unsubscribe = client.subscribe(
  { query: 'subscription { Person_added { _id name } }' },
  {
    next: (data) => console.log('Event:', data),
    error: (err) => console.error('Error:', err),
    complete: () => console.log('Subscription closed'),
  }
)
```

### Using Apollo Client

Apollo Client supports subscriptions via WebSocket by default. To use SSE instead, you can use the [`graphql-sse` transport](https://www.apollographql.com/docs/react/data/subscriptions/) with a custom link. See [Connect with Apollo Client](/docs/connect-with-apollo-client/) for setup instructions.

## Protocol reference

### Request format

| Field | Value |
|-------|-------|
| Method | `POST` |
| URL | `/api/graphql/ORG/DATAPRODUCT` |
| `Content-Type` | `application/json` |
| `Accept` | `text/event-stream` or `application/x-ndjson` |
| `Authorization` | Basic Auth or TerminusDB Cloud token |
| Body | `{"query": "subscription { ... }"}` |

### Response headers

| Header | Value |
|--------|-------|
| `Content-Type` | `text/event-stream` (SSE) or `application/x-ndjson` (NDJSON) |
| `Cache-Control` | `no-cache` |
| `Connection` | `keep-alive` |
| `X-Accel-Buffering` | `no` |
| `Access-Control-Allow-Origin` | Reflected from `Origin` request header |

### SSE event format

Each event is a block of lines separated by a blank line (`\n\n`). Within a block:

| Line | Description |
|------|-------------|
| `event: <type>` | Event type: `connected`, `next`, or `complete` |
| `data: <json>` | JSON payload (empty for `complete`) |

### NDJSON event format

Each event is a single JSON line followed by a blank line:

| Event | Line |
|-------|------|
| `connected` | `null` |
| `next` | `{"<Type>_<change>": {...}}` |
| `complete` | (empty line) |

## Versions

* v12.1 onwards support GraphQL subscriptions with SSE (Server-Sent Events)

## See also

- [**GraphQL Basics**](/docs/graphql-basics/) — write your first queries with worked examples
- [**GraphQL Mutations**](/docs/graphql-mutations/) — insert, replace, and delete documents via GraphQL
- [**Connecting to GraphQL**](/docs/connecting-to-graphql-reference/) — endpoint URL and authentication setup
- [**GraphQL Query Reference**](/docs/graphql-query-reference/) — full reference for types, arguments, and fields
- [**Filter with GraphQL**](/docs/filter-with-graphql/) — filter syntax for queries and subscriptions
- [**Browser CORS**](/docs/browser-cors-howto/) — CORS configuration for browser-based clients
