---
tags:
  - how-to
  - terminusdb-cloud
  - beginner
title: How to connect to TerminusDB
nextjs:
  metadata:
    title: How to connect to TerminusDB
    description: How to connect with TerminusDB using an API key
    keywords: terminusdb, api, client, connect, connect to terminusdb, connection
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/how-to-connect-terminusdb-cloud/
media:
  - alt: Client examples
    caption: ""
    media_type: Image
    title: Client examples
    value: https://cdn.terminusdb.com/img/terminus-cms/client_api_connection.jpeg
---

{% callout type="note" %}
**Prerequisites**
- A TerminusDB Cloud account
- An API key ([get one](/docs/get-your-api-key/))
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have connected to TerminusDB Cloud programmatically.
{% /callout %}

Hosted TerminusDB is available as [TerminusDB Cloud](/docs/cloud/), currently in early access. [Request access](/docs/cloud/) if you don't have an account yet.

TerminusDB Cloud is offered as a SaaS platform with the full TerminusDB data product API. API tokens are generated in the settings section of your cloud instance, and git-for-data actions (clone, push, pull, merge, rebase) work the same as on a self-hosted server.

[Read more about TerminusDB Cloud](/docs/cloud/).

## Next steps

You're connected to TerminusDB Cloud. Now start building:

- [**Get your API key**](/docs/get-your-api-key/) — mint a token to authenticate programmatically
- [**TypeScript quickstart**](/docs/connect-with-the-javascript-client/) — build, branch, and query with the JS client
- [**HTTP Documents API**](/docs/http-documents-api/) — CRUD operations using curl or any HTTP client
