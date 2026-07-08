---
tags:
  - how-to
  - typescript
  - self-hosted
title: Call TerminusDB from the Browser
nextjs:
  metadata:
    title: Call TerminusDB from the Browser — CORS How-To
    description: How to make HTTP requests to the TerminusDB API from browser JavaScript. No CORS configuration needed — TerminusDB allows all origins by default.
    keywords: CORS, browser, fetch, JavaScript, Angular, React, cross-origin
    alternates:
      canonical: https://terminusdb.org/docs/browser-cors-howto/
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally on `localhost:6363`
- A web application making cross-origin requests to TerminusDB
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have configured CORS to allow browser-based access to your TerminusDB instance.
{% /callout %}

Make HTTP requests to the TerminusDB API from browser JavaScript without CORS errors.

**TerminusDB allows all CORS origins by default.** The server reflects the request's `Origin` header back as `Access-Control-Allow-Origin`, so any browser origin can make requests without additional configuration. There is no environment variable to set and no allow-list to configure.

## Step 1 — Verify CORS is working

Open your browser's developer console and run:

```javascript
const response = await fetch("http://localhost:6363/api/info", {
  headers: { "Authorization": "Basic " + btoa("admin:root") }
});
const data = await response.json();
console.log(data);
```

If you see a JSON response with `"authority": "admin"`, CORS is working. No server configuration needed.

## Step 2 — Fetch documents from your application

A minimal example using the Fetch API (works in any framework):

```javascript test-example file="examples/browser-fetch.example.ts"
```

## Step 3 — Insert a document from the browser

```javascript
async function insertDocument(database, doc, message) {
  const response = await fetch(
    `${SERVER}/api/document/admin/${database}?author=admin&message=${encodeURIComponent(message)}&raw_json=true`,
    {
      method: "POST",
      headers: {
        "Authorization": AUTH,
        "Content-Type": "application/json"
      },
      body: JSON.stringify(doc)
    }
  );
  if (!response.ok) throw new Error(`${response.status}: ${await response.text()}`);
  return response.json();
}

// Usage
const ids = await insertDocument("MyDatabase",
  { "@id": "terminusdb:///data/item-1", "name": "Widget", "price": 9.99 },
  "Add widget from browser app"
);
console.log("Inserted:", ids);
```

## Framework examples

### Angular (HttpClient)

```typescript
import { HttpClient, HttpHeaders } from "@angular/common/http";

const headers = new HttpHeaders({
  "Authorization": "Basic " + btoa("admin:root"),
  "Content-Type": "application/json"
});

this.http.get("http://localhost:6363/api/document/admin/MyDatabase?type=Product&as_list=true",
  { headers }).subscribe(data => console.log(data));
```

No proxy configuration required — TerminusDB handles CORS natively.

### React / Next.js (fetch or axios)

Use the Fetch API examples above directly. If using axios:

```typescript
import axios from "axios";

const client = axios.create({
  baseURL: "http://localhost:6363",
  headers: { "Authorization": "Basic " + btoa("admin:root") }
});

const { data } = await client.get("/api/document/admin/MyDatabase?type=Product&as_list=true");
```

{% callout type="warning" title="Still getting CORS errors?" %}
Check that: (1) TerminusDB is actually running on the URL you are requesting — a connection refused is not a CORS error, even though browsers sometimes report it as one; (2) You are not using a browser extension that strips CORS headers; (3) If you are using a reverse proxy in front of TerminusDB, ensure it forwards the `Origin` header and does not strip the CORS response headers.
{% /callout %}

## How it works

TerminusDB's CORS response headers:

```text
Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS
Access-Control-Allow-Credentials: true
Access-Control-Max-Age: 1728000
Access-Control-Allow-Headers: Authorization, Authorization-Remote, Accept, Accept-Encoding, Accept-Language, Host, Origin, Referer, Content-Type, Content-Length, Content-Range, Content-Disposition, Content-Description, X-HTTP-METHOD-OVERRIDE
Access-Control-Allow-Origin: <reflected from request Origin>
```

The `Authorization` header is explicitly listed in `Access-Control-Allow-Headers`, so Basic auth works from browsers without preflight failures. The `Access-Control-Allow-Credentials: true` header means cookies and auth headers are permitted in cross-origin requests.

## See also

- [TypeScript Quickstart](/docs/connect-with-the-javascript-client/) — connect using the official JS client
- [HTTP Document API](/docs/http-documents-api/) — full API reference for document operations
- [Self-Hosted Installation](/docs/self-hosted-installation/) — deploy TerminusDB with a reverse proxy
