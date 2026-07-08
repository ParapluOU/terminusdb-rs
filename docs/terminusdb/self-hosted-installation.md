---
tags:
  - how-to
  - installation
  - curl
  - self-hosted
title: Self-Hosted Installation Guide
nextjs:
  metadata:
    title: Self-Hosted TerminusDB — Docker, Docker Compose, and Bare Metal
    description: How to deploy TerminusDB on your own infrastructure with production-ready configuration. Docker single container, Docker Compose, and bare-metal options.
    keywords: self-hosted, Docker, Docker Compose, deployment, production, installation
    alternates:
      canonical: https://terminusdb.org/docs/self-hosted-installation/
---

{% callout type="note" %}
**Prerequisites**
- A Linux server or virtual machine
- Docker or the ability to compile from source
- Network access to port 6363
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have a self-hosted TerminusDB instance running and accessible.
{% /callout %}

This guide covers three ways to run TerminusDB on your own infrastructure, from simplest to most customisable. For a quick local setup, see the [quickstart](/docs/get-started/). This page is for production or shared-team deployments.

## Option 1: Docker (single container)

The quickest path to a production-ready instance. A single Docker command with persistent storage:

```bash
docker run -d \
  --name terminusdb \
  --restart unless-stopped \
  -p 127.0.0.1:6363:6363 \
  -v terminusdb_storage:/app/terminusdb/storage \
  -e TERMINUSDB_SERVER_PORT=6363 \
  -e TERMINUSDB_AUTOLOGIN=false \
  terminusdb/terminusdb-server:v12
```

### Production flags explained

| Flag | Purpose |
|------|---------|
| `--restart unless-stopped` | Automatically restart on crash or host reboot |
| `-p 127.0.0.1:6363:6363` | Bind to localhost only — put a reverse proxy in front for external access |
| `-v terminusdb_storage:/app/terminusdb/storage` | Named volume for data persistence across container recreations |
| `-e TERMINUSDB_AUTOLOGIN=false` | Disable auto-login — require password for all connections |

### Change the default password

After first start, change the admin password immediately:

```bash
curl -s -u admin:root -X PUT http://localhost:6363/api/users \
  -H "Content-Type: application/json" \
  -d '{"name": "admin", "password": "your-secure-password"}'
```

### Expose to the network

To make the instance accessible to other machines, bind to `0.0.0.0` and place a TLS reverse proxy (nginx, Caddy, or Traefik) in front:

```bash
docker run -d \
  --name terminusdb \
  --restart unless-stopped \
  -p 0.0.0.0:6363:6363 \
  -v terminusdb_storage:/app/terminusdb/storage \
  -e TERMINUSDB_AUTOLOGIN=false \
  terminusdb/terminusdb-server:v12
```

{% callout type="warning" title="Always use TLS" %}
TerminusDB transmits credentials as Basic Auth headers. Without TLS, passwords are sent in cleartext. Always put a reverse proxy with TLS termination in front of TerminusDB when exposing it to a network.
{% /callout %}

## Option 2: Docker Compose

For teams that want a reproducible setup or need to run TerminusDB alongside other services:

```yaml
# docker-compose.yml
services:
  terminusdb:
    image: terminusdb/terminusdb-server:v12
    container_name: terminusdb
    restart: unless-stopped
    ports:
      - "127.0.0.1:6363:6363"
    volumes:
      - terminusdb_data:/app/terminusdb/storage
    environment:
      TERMINUSDB_SERVER_PORT: "6363"
      TERMINUSDB_AUTOLOGIN: "false"
      TERMINUSDB_ENABLE_DASHBOARD: "false"

volumes:
  terminusdb_data:
    driver: local
```

Start with:

```bash
docker compose up -d
```

### Adding a reverse proxy

Extend the compose file with Caddy for automatic TLS:

```yaml
services:
  terminusdb:
    image: terminusdb/terminusdb-server:v12
    restart: unless-stopped
    volumes:
      - terminusdb_data:/app/terminusdb/storage
    environment:
      TERMINUSDB_SERVER_PORT: "6363"
      TERMINUSDB_AUTOLOGIN: "false"

  caddy:
    image: caddy:2
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile
      - caddy_data:/data
    depends_on:
      - terminusdb

volumes:
  terminusdb_data:
  caddy_data:
```

With a `Caddyfile`:

```
terminusdb.example.com {
    reverse_proxy terminusdb:6363
}
```

## Option 3: Bare metal / binary

TerminusDB can run directly on a host without Docker.

### Download the binary

Pre-built binaries are available from [GitHub Releases](https://github.com/terminusdb/terminusdb/releases). Download the binary for your platform:

```bash
# Linux x86_64
curl -L -o terminusdb \
  https://github.com/terminusdb/terminusdb/releases/latest/download/terminusdb-amd64-linux

chmod +x terminusdb
```

### Run the server

```bash
export TERMINUSDB_SERVER_DB_PATH="./storage"
export TERMINUSDB_SERVER_PORT=6363
export TERMINUSDB_AUTOLOGIN=false

./terminusdb serve
```

The server stores all data in the path specified by `TERMINUSDB_SERVER_DB_PATH`. Back up this directory for disaster recovery.

### Supported platforms

| Platform | Architecture | Status |
|----------|-------------|--------|
| Linux | x86_64, arm64 | Fully supported |
| macOS | x86_64, arm64 (Apple Silicon) | Fully supported |
| Windows | x86_64 | Via WSL2 or Docker |

### Build from source

For custom builds or unsupported platforms, see [Build from Source](/docs/install-terminusdb-from-source-code/).

## Environment Variables

Key configuration variables for production deployments:

| Variable | Default | Description |
|----------|---------|-------------|
| `TERMINUSDB_SERVER_PORT` | `6363` | HTTP port the server listens on |
| `TERMINUSDB_SERVER_NAME` | `127.0.0.1` | Bind address |
| `TERMINUSDB_SERVER_WORKERS` | `8` | Number of worker threads |
| `TERMINUSDB_SERVER_DB_PATH` | `./storage/db` | Data storage location |
| `TERMINUSDB_AUTOLOGIN` | `true` | Skip auth for local access (disable in production) |
| `TERMINUSDB_JWT_ENABLED` | `false` | Enable JWT authentication |
| `TERMINUSDB_ENABLE_DASHBOARD` | `true` | Serve the web dashboard UI |
| `TERMINUSDB_SERVER_MAX_TRANSACTION_RETRIES` | `3` | Max retry count for conflicting transactions |

## Health checks

Use the `/api/ok` endpoint for load balancer health checks:

```bash
curl -s http://localhost:6363/api/ok
# Returns: "ok"
```

For a more detailed check that includes authentication:

```bash
curl -s -u admin:root http://localhost:6363/api/info
```

## Backups

TerminusDB provides two approaches for backup: the CLI bundle commands for community use, and enterprise-grade backup tooling for production deployments.

### CLI bundle and unbundle

The TerminusDB CLI includes `db bundle` and `db unbundle` commands for creating portable backups of individual databases:

```bash
# Create a bundle backup
terminusdb db bundle admin/MyDatabase --output MyDatabase-backup.bundle

# Restore from a bundle
terminusdb db unbundle admin/MyDatabase MyDatabase-backup.bundle
```

Bundles capture all commits, branches, and data as a single file. They can be transferred between TerminusDB instances of the same version.

### Storage volume backup

For a full instance backup (all databases), stop the container and archive the storage volume:

```bash
docker compose stop terminusdb
tar -czf terminusdb-backup-$(date +%Y%m%d).tar.gz \
  $(docker volume inspect terminusdb_data --format '{{.Mountpoint}}')
docker compose start terminusdb
```

### Enterprise backup tooling

For production deployments with automated scheduling, point-in-time recovery, incremental backups, and cross-instance replication, see the [Enterprise Backup and Restore](/docs/enterprise-backup-restore/) documentation.

## Next steps

- [Access Control](/docs/access-control/) — Set up users and permissions
- [TypeScript Quickstart](/docs/connect-with-the-javascript-client/) — Connect your application
- [CLI Reference](/docs/terminusdb-cli-commands/) — Manage databases from the command line
