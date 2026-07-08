---
tags:
  - how-to
  - installation
  - curl
  - self-hosted
  - beginner
title: Install TerminusDB with Docker
nextjs:
  metadata:
    title: Install TerminusDB with Docker
    description: Get TerminusDB running locally in under 2 minutes with Docker Compose.
    keywords: terminusdb, docker, getting started, install, docker compose, quick start, setup
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/install-terminusdb-as-a-docker-container/
media: []
---

{% callout type="note" %}
**Prerequisites**
- Docker installed on your system ([get Docker](https://docs.docker.com/get-docker/))
- A terminal or command prompt
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have TerminusDB running in a Docker container on your machine — ready to accept queries on `localhost:6363`.
{% /callout %}

## Quick start (2 minutes)

Clone the repository and start the container:

```bash
git clone https://github.com/terminusdb/terminusdb
cd terminusdb
docker compose up
```

That's it. TerminusDB is now running at **http://localhost:6363**.

### Verify it works

Open the dashboard in your browser:

```bash
http://127.0.0.1:6363/dashboard/
```

Or test with curl:

```bash
curl -u admin:root http://localhost:6363/api/info
```

You should see a JSON response with the server version.

---

## What's included

The Docker Compose stack gives you:

| Service | URL | Purpose |
|---------|-----|---------|
| TerminusDB | `http://localhost:6363` | Database server + HTTP API |
| Dashboard | `http://localhost:6363/dashboard/` | Visual UI for data modelling and queries |
| GraphiQL | `http://localhost:6363/api/graphiql/admin/{db}` | Interactive GraphQL query browser |

Default credentials: **admin** / **root**

---

## Stop and restart

```bash
# Stop
docker compose down

# Restart (data persists in Docker volumes)
docker compose up
```

{% callout type="warning" %}
Adding `-v` to `docker compose down -v` **deletes all data** by removing Docker volumes. Only use this if you want a fresh start.
{% /callout %}

---

## Optional: AI indexing

To enable AI-powered vector indexing, create a `.env` file before starting:

```bash
OPENAI_KEY=your-openai-key-here
BUFFER_AMOUNT=120000
```

The OpenAI key is optional — all database features work without it.

---

## Windows users

On Windows, the default Docker memory is **2 GB**. TerminusDB benefits from more memory — increase it in Docker Desktop settings.

For a detailed Windows guide, see [Install TerminusDB with Docker on Windows](/docs/install-terminusdb-docker-windows/).

---

## Next steps

**Tutorials (pick one):**

- [JSON Version Control in 5 Minutes](/docs/version-controlled-json/) — store schemaless JSON with branches, diffs, and patches
- [Your First 10 Minutes](/docs/get-started/) — clone a dataset and explore it (recommended first tutorial)
- [Your First 15 Minutes](/docs/first-15-minutes/) — build a schema, insert documents, branch and merge

**Reference:**

- [Create a Database](/docs/create-a-database/) — your first database in under a minute
- [TypeScript Quickstart](/docs/connect-with-the-javascript-client/) — connect and start querying
- [Python Quickstart](/docs/connect-with-python-client/) — connect with the Python client
- [Advanced Docker Configuration](/docs/docker-advanced-configuration/) — production deployment, CLI access, migration from bootstrap
