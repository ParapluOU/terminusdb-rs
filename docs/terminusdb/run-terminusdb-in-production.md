---
tags:
  - how-to
  - installation
  - self-hosted
  - advanced
title: Run TerminusDB in Production
nextjs:
  metadata:
    title: Run TerminusDB in Production | Self-Hosted Database Guide
    description: Run TerminusDB in production with Docker and a storage volume, JSON logs, JWT authentication, and the auto-optimizer that keeps reads fast with no vacuuming.
    keywords: terminusdb production, run terminusdb in production, self-hosted graph database, docker volume backup, jwt authentication, log_format json, auto-optimizer, maintenance free database
    alternates:
      canonical: https://terminusdb.com/docs/run-terminusdb-in-production/
media: []
---

TerminusDB needs very little to run in production: a container, a persistent volume, and a few environment variables. There is no daemon to babysit and no maintenance window to plan, because the storage engine keeps itself fast (see [the auto-optimizer](#the-auto-optimizer-keeps-reads-fast)).

This page collects the production-relevant settings in one place. For install paths, see [Self-Hosted Installation](/docs/self-hosted-installation/) or [Install on Kubernetes](/docs/install-on-kubernetes/).

## The short version

- **Run the official image** with a named volume on `/app/terminusdb/storage`.
- **Set `TERMINUSDB_LOG_FORMAT=json`** so log pipelines can parse output.
- **Change the admin password** — authentication is always required.
- **Put a TLS proxy in front.** TerminusDB uses Basic Auth or JWT; without TLS, credentials travel in cleartext.
- **Use `/api/ok` for health checks.**
- **Back up by snapshotting the volume** or with `terminusdb db bundle`.
- **Do nothing else.** The auto-optimizer rolls up layers continuously, so there is no vacuum job, no compaction cron, and no index rebuild to schedule.

## Run with Docker

On a single box, one container with a named volume is a complete production deployment:

```bash
docker run -d \
  --name terminusdb \
  --restart unless-stopped \
  -p 127.0.0.1:6363:6363 \
  -v terminusdb_storage:/app/terminusdb/storage \
  -e TERMINUSDB_ADMIN_SET_PASS=<strong-password> \
  -e TERMINUSDB_LOG_FORMAT=json \
  terminusdb/terminusdb-server:v12
```

Two details matter here:

- **The volume gives you both persistence and speed.** A named volume on fast local disk keeps layer files close to the engine and survives container recreation. Bind-mounting a host directory (`-v /srv/terminusdb:/app/terminusdb/storage`) works too, and makes the files directly accessible for backup tools.
- **Bind to localhost** (`127.0.0.1`) and terminate TLS in a reverse proxy (nginx, Caddy, Traefik). The same pattern works on `0.0.0.0` inside a private network.

Kubernetes uses the same building blocks: a single replica, a persistent volume claim on the storage path, `TERMINUSDB_LOG_FORMAT=json`, and a liveness probe on `/api/ok`. The full deployment and service YAML is in [Install on Kubernetes](/docs/install-on-kubernetes/).

## Logging

```bash
-e TERMINUSDB_LOG_FORMAT=json   # one JSON object per line
-e TERMINUSDB_LOG_LEVEL=INFO    # ERROR, WARNING, NOTICE, INFO or DEBUG
```

`json` is the right choice for anything that ships logs to a collector — each line parses directly in Loki, Datadog, CloudWatch or journald. The default `text` format is fine for local development.

## Authentication

### Basic Auth over TLS

TerminusDB authenticates with HTTP Basic Auth over the API — authentication is always on, there is no flag to disable it. In production, set the admin password explicitly:

```bash
-e TERMINUSDB_ADMIN_SET_PASS=<strong-password>   # applied on every container start
```

`TERMINUSDB_ADMIN_SET_PASS` sets the admin password each time the container starts — use it for initial provisioning and for rotation by restarting with a new value. The older `TERMINUSDB_ADMIN_PASS` only applies when the store is initialized on first boot and is ignored after that.

### Reset the admin password without a restart

To change the admin password on a running container, run the CLI inside it:

```bash
docker exec -it terminusdb /app/terminusdb/terminusdb user password admin
# Password: <prompts for the new password>
```

The `-it` flags give the command a terminal so it can prompt without echoing. The non-interactive form works too — but the password lands in your shell history:

```bash
docker exec terminusdb /app/terminusdb/terminusdb user password admin --password 'new-password'
```

Same approach on bare metal: run `terminusdb user password admin` as the service user.

### JWT / OIDC

If your organization already issues tokens — through Keycloak, Auth0, Entra ID or another identity provider — TerminusDB can validate them instead of (or alongside) passwords:

```bash
-e TERMINUSDB_JWT_ENABLED=true \
-e TERMINUSDB_SERVER_JWKS_ENDPOINT=https://your-idp.example.com/.well-known/jwks.json \
-e TERMINUSDB_JWT_ISSUER=https://your-idp.example.com/ \
-e TERMINUSDB_JWT_AUDIENCE=your-api-audience \
-e TERMINUSDB_JWT_AGENT_NAME_PROPERTY=sub
```

| Variable | Default | Purpose |
|---|---|---|
| `TERMINUSDB_JWT_ENABLED` | `false` | Turn JWT validation on |
| `TERMINUSDB_SERVER_JWKS_ENDPOINT` | — | JWKS URL for signature verification. Must be `https://` (`http://localhost` allowed for testing) |
| `TERMINUSDB_OIDC_ISSUER_URL` | — | Alternative to a raw JWKS URL: the issuer's OIDC base URL, from which keys are discovered |
| `TERMINUSDB_JWT_ISSUER` | — | If set, tokens must carry this `iss` |
| `TERMINUSDB_JWT_AUDIENCE` | — | If set, tokens must carry this `aud` |
| `TERMINUSDB_JWT_AGENT_NAME_PROPERTY` | `sub` | Token claim used as the TerminusDB user identity |
| `TERMINUSDB_JWT_CLOCK_TOLERANCE` | `60` | Skew allowance in seconds for token expiry checks |
| `TERMINUSDB_JWT_SCOPES_ENABLED` | `false` | Map token scopes to TerminusDB roles |
| `TERMINUSDB_JWT_SCOPES_CLAIM` | `scope` | Claim your IdP uses for scopes; required when scopes are enabled |

Two safety rules the server enforces at startup: JWT without a key source (JWKS endpoint or OIDC issuer URL) fails loudly rather than accepting every request, and scope-based authorization refuses to start until `TERMINUSDB_JWT_SCOPES_CLAIM` is set explicitly, so users can't self-assign roles.

## The auto-optimizer keeps reads fast

Every commit in TerminusDB appends a new layer to the database — that is what makes the history immutable and queries time-travelable. Left alone, long layer chains would slow reads down over time, the same problem other databases solve with `VACUUM` or compaction jobs.

TerminusDB solves it in the background. The **auto-optimizer** (enabled by default since 12.0.0) rolls up layers continuously as you write: schema layers are squashed into single layers, and instance graphs get an exponential rollup strategy, so a branch that sees thousands of commits still opens in a handful of layer reads. Since 12.0.5 it runs only on write and only when a database has parent layers — there is no wasted work on cold data.

The practical consequence for operations:

- **No vacuuming.** No scheduled maintenance jobs, no dead-tuple cleanup, no index rebuilds.
- **No performance cliff.** Query performance stays roughly flat as commit count grows, because the rollup keeps the layer chain short.
- **Manual control exists.** `terminusdb optimize <db>` and `POST /api/optimize/<path>` trigger a rollup on demand, scoped to a branch, repository, database or the system graph.

## Data consistency

Every commit is a full ACID transaction: a failed write applies nothing, so a crash mid-commit never leaves partial data behind. There is no separate consistency model to configure or repair procedures to run. See [ACID Transactions Explanation](/docs/acid-transactions-explanation/) for how TerminusDB guarantees this.

## Health checks and monitoring

```bash
curl -s http://localhost:6363/api/ok      # "ok" — liveness probe, no auth needed
curl -s -u admin:root http://localhost:6363/api/info   # version, storage version, auth — deeper check
```

Use `/api/ok` for load-balancer and Kubernetes liveness probes (the Kubernetes page wires it at 180-second intervals). Use `/api/info` when you want to verify the server can authenticate and read the system database.

Every response also carries correlation headers (since 12.0.0), which helps when tracing requests through a proxy chain.

## Backups

Two patterns cover most needs:

- **Volume snapshot.** All database state lives under `/app/terminusdb/storage`. Stop the container, archive the volume or host directory, restart. This is why running the storage on a plain Docker volume or host path pays off — any standard filesystem backup tool works.
- **`terminusdb db bundle`.** Produces a single portable file per database, including history and branches, restorable on another instance of the same version:

```bash
docker exec terminusdb ./terminusdb db bundle admin/MyDB --output /app/terminusdb/storage/MyDB.bundle
```

Commands and restore examples are in [Backups](/docs/self-hosted-installation/#backups). Scheduled, incremental and point-in-time recovery are covered by [Enterprise Backup and Restore](/docs/enterprise-backup-restore/).

## Production checklist

- [ ] `--restart unless-stopped` (or your orchestrator's equivalent)
- [ ] Named volume or host path on `/app/terminusdb/storage`, on local disk rather than network storage
- [ ] Strong admin password via `TERMINUSDB_ADMIN_SET_PASS`
- [ ] TLS terminated at a reverse proxy; Basic Auth never crosses the network in cleartext
- [ ] `TERMINUSDB_LOG_FORMAT=json` if logs go to a collector
- [ ] `TERMINUSDB_ENABLE_DASHBOARD=false` if users shouldn't reach the UI
- [ ] Liveness probe on `/api/ok`
- [ ] A backup job: volume snapshot or `db bundle`, tested with a restore
- [ ] `TERMINUSDB_SERVER_WORKERS` / `TERMINUSDB_WORKER_POOL_SIZE` left at defaults unless profiling says otherwise (see [worker threads](/docs/self-hosted-installation/#worker-thread-configuration))

## What you don't have to do

- No vacuum, analyze, or compaction jobs
- No index maintenance — storage indexes rebuild themselves through rollups
- No replica configuration to get started — a single node is a complete system

**Read more:** [Self-Hosted Installation](/docs/self-hosted-installation/) · [Install on Kubernetes](/docs/install-on-kubernetes/) · [Advanced Docker Configuration](/docs/docker-advanced-configuration/) · [TerminusDB internals](/docs/terminusdb-internals/)
