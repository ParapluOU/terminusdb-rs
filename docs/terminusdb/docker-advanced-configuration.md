---
tags:
  - how-to
  - installation
  - self-hosted
  - advanced
title: Advanced Docker Configuration
nextjs:
  metadata:
    title: Advanced Docker Configuration — TerminusDB
    description: Production deployment, environment configuration, CLI access, and migration options for TerminusDB Docker containers.
    keywords: terminusdb, docker, production, deployment, environment variables, cli, migration, terminusdb-bootstrap, nginx, https
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/docker-advanced-configuration/
media: []
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally via Docker — see [Install with Docker](/docs/install-terminusdb-as-a-docker-container/)
{% /callout %}

This page covers production deployment, environment configuration, CLI access, and migration from legacy setups.

## Environment configuration

The container uses environment variables with default values. Override them by creating a `.env` file in the cloned `terminusdb` directory:

```bash
# AI indexing (optional)
OPENAI_KEY=your-openai-key-here
BUFFER_AMOUNT=120000

# Server configuration
TERMINUSDB_SERVER_PORT=6363
TERMINUSDB_ADMIN_PASS=root
```

Pass the file to Docker Compose:

```bash
docker compose --env-file .env up
```

---

## Using the CLI

Access the TerminusDB command-line interface from the Docker container:

```bash
# When container is not running:
docker compose run terminusdb-server ./terminusdb

# When container is already running:
docker compose exec terminusdb-server ./terminusdb
```

For full CLI documentation, see [CLI Reference](/docs/terminusdb-cli-commands/).

---

## Server deployment

### Local deployment (default)

By default, the Docker container binds to IP `127.0.0.1`. This prevents insecure deployments and ensures the TerminusDB server is accessible on the local machine only.

### Remote / production deployment

To deploy TerminusDB on a remote server:

1. **Enable HTTPS** with a reverse proxy (Nginx, Caddy, or similar)
2. **Do not** use the `X-Forward-Header` ENV variables unless you fully understand the security implications
3. Consider the [Self-Hosted (Production)](/docs/self-hosted-installation/) guide for a comprehensive production setup

---

## Migrating from terminusdb-bootstrap

If you previously used `terminusdb-bootstrap` and want to keep your existing data, run Docker Compose with the bootstrap storage overlay:

```bash
docker compose -f docker-compose.yml -f distribution/docker-compose/bootstrap_storage.yaml up
```

This mounts the data volumes from your previous installation into the new container setup.

---

## Related guides

- [Install with Docker](/docs/install-terminusdb-as-a-docker-container/) — quick start (2 minutes)
- [Docker on Windows](/docs/install-terminusdb-docker-windows/) — Windows-specific guide
- [Self-Hosted (Production)](/docs/self-hosted-installation/) — full production deployment
- [Kubernetes](/docs/install-on-kubernetes/) — orchestrated deployment
- [Build from Source](/docs/install-terminusdb-from-source-code/) — compile TerminusDB yourself
