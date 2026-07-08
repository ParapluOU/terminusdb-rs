---
tags:
  - troubleshooting
  - installation
  - curl
title: Connection Failures — TerminusDB Troubleshooting
nextjs:
  metadata:
    title: Connection Failures — TerminusDB Troubleshooting
    description: Diagnose and fix connection failures when connecting to TerminusDB, including Docker networking issues, wrong ports, browser fetch errors, and timeouts.
    keywords: terminusdb, client, connect, connection, connection failures — terminusdb troubleshooting, debug, docker, error
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/troubleshooting-connection/
media: []
---

# Connection Failures

This page covers errors you encounter when your client cannot reach the TerminusDB server — the server is not running, the port is wrong, Docker networking is misconfigured, the browser blocks the request due to CORS, or the connection times out.

## Symptoms

- `ECONNREFUSED` or `Connection refused` in your terminal
- `fetch failed` or `NetworkError` in the browser console
- `CORS policy: No 'Access-Control-Allow-Origin' header` error in the browser
- Request hangs indefinitely and eventually times out
- `getaddrinfo ENOTFOUND` when using a hostname

## Common causes

### Server not running

**Error message:** `connect ECONNREFUSED 127.0.0.1:6363`

**Cause:** TerminusDB is not started, or the Docker container has exited.

**Fix:**

1. Check if the container is running:
   ```bash
   docker ps | grep terminusdb
   ```
2. If nothing appears, start the container:
   ```bash
   docker start terminusdb
   ```
3. If the container does not exist, create it:
   ```bash
   docker run -d --name terminusdb \
     -p 6363:6363 \
     -v terminusdb_storage:/app/terminusdb/storage \
     terminusdb/terminusdb-server
   ```

### Wrong port or host

**Error message:** `connect ECONNREFUSED 127.0.0.1:6364` (or any non-6363 port)

**Cause:** Your client is pointing to a port that TerminusDB is not listening on, or you are using `localhost` when the server is on a different host.

**Fix:**

1. Confirm the port mapping:
   ```bash
   docker port terminusdb
   ```
   You should see `6363/tcp -> 0.0.0.0:6363`.
2. Update your client connection URL to match the mapped port.
3. If running inside another container, use the container name or Docker network IP — not `localhost`.

### Docker networking (container-to-container)

**Error message:** `getaddrinfo ENOTFOUND localhost` or `ECONNREFUSED` from within another container

**Cause:** When two containers need to communicate, `localhost` inside a container refers to that container itself, not the host machine.

**Fix:**

1. Put both containers on the same Docker network:
   ```bash
   docker network create terminusdb-net
   docker network connect terminusdb-net terminusdb
   docker network connect terminusdb-net your-app
   ```
2. Use the container name as the hostname:
   ```
   http://terminusdb:6363
   ```
3. Alternatively, on Docker Desktop, use `host.docker.internal` to reach the host:
   ```
   http://host.docker.internal:6363
   ```

### CORS errors from the browser

**Error message:** `Access to fetch at 'http://localhost:6363' from origin 'http://localhost:3000' has been blocked by CORS policy`

**Cause:** TerminusDB unconditionally allows all cross-origin requests — it reflects back whatever `Origin` header is sent. If you are seeing a genuine CORS error, the most common causes are: a browser extension blocking requests, a proxy stripping CORS headers, or connecting to a different host than you think (e.g., the wrong port).

**Fix:**

1. Confirm TerminusDB is actually running and reachable at the expected URL:
   ```bash
   curl -s http://localhost:6363/api/ok
   ```
   You should see `"ok"`. If not, the server is not running — see [Server not running](#server-not-running) above.
2. Check your browser's network tab for the actual request URL and response headers. If `Access-Control-Allow-Origin` is missing, the request never reached TerminusDB (firewall, proxy, or wrong URL).
3. There is no `TERMINUSDB_CORS` environment variable — do not set one. See [Calling TerminusDB from the Browser](/docs/browser-cors-howto) for a working fetch example.

### Connection timeout

**Error message:** `timeout of 5000ms exceeded` or request hangs with no response

**Cause:** A firewall, VPN, or misconfigured network is preventing packets from reaching the server. The server may also be under heavy load.

**Fix:**

1. Verify basic connectivity:
   ```bash
   curl -s -o /dev/null -w "%{http_code}" http://localhost:6363/api/info
   ```
   A healthy server returns `200`.
2. Check firewall rules are not blocking port 6363.
3. If behind a corporate VPN, ensure local traffic is not being routed through the tunnel.
4. Increase the client timeout as a temporary workaround:
   ```javascript
   const client = new TerminusClient.WOQLClient("http://localhost:6363", {
     user: "admin",
     key: "root",
   })
   ```

## Still stuck?

- [Open an issue](https://github.com/terminusdb/terminusdb/issues) with the full error message and your Docker/environment details
- Check the [Installation guide](/docs/install-terminusdb-as-a-docker-container) for Docker setup instructions
- Check the [TerminusDB CLI reference](/docs/terminusdb-cli-commands) for server management commands
