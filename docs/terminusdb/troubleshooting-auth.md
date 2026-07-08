---
tags:
  - troubleshooting
  - access-control
  - curl
title: Authentication Errors — TerminusDB Troubleshooting
nextjs:
  metadata:
    title: Authentication Errors — TerminusDB Troubleshooting
    description: Fix authentication errors with TerminusDB including wrong credentials, password vs key parameter confusion, API key format issues, and permission denied errors.
    keywords: terminusdb, api, debug, error, troubleshooting
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/troubleshooting-auth/
media: []
---

# Authentication Errors

This page covers errors that occur after your client reaches the server but fails to authenticate — wrong credentials, incorrect parameter names, malformed API keys, or insufficient permissions for the requested operation.

{% prerequisites-clone /%}

## Symptoms

- HTTP `401 Unauthorized` response
- HTTP `403 Forbidden` response
- `"Permission denied"` in the response body
- `"Invalid credentials"` or `"Authentication failed"` message
- Python client raises `DatabaseError` with status 401

## Common causes

### Wrong username or password (local install)

**Error message:** `HTTP 401: Unauthorized`

**Cause:** The credentials supplied do not match the admin password set when the container was created.

**Fix:**

1. The default credentials for a local TerminusDB install are:
   - **Username:** `admin`
   - **Password:** `root` (unless you set `TERMINUSDB_ADMIN_PASS`)
2. Verify with curl:
   ```bash
   curl -u admin:root http://localhost:6363/api/info
   ```
3. If you set a custom password via `TERMINUSDB_ADMIN_PASS`, use that value instead.
4. If you have forgotten the password, recreate the container with a new password:
   ```bash
   docker rm -f terminusdb
   docker run -d --name terminusdb \
     -p 6363:6363 \
     -v terminusdb_storage:/app/terminusdb/storage \
     -e TERMINUSDB_ADMIN_PASS=mynewpassword \
     terminusdb/terminusdb-server
   ```

### Python client: `password` vs `key` parameter

**Error message:** `TypeError: __init__() got an unexpected keyword argument 'password'` or silent 401 failures

**Cause:** The TerminusDB Python client v1.1+ renamed the `password` parameter to `key`. Code written for older versions will either raise a TypeError or silently fail authentication.

**Fix:**

```python
# WRONG (pre-v1.1 API — no longer works)
from terminusdb_client import Client
client = Client("http://localhost:6363")
client.connect(user="admin", password="root")

# CORRECT (v1.1+ API)
from terminusdb_client import Client
client = Client("http://localhost:6363")
client.connect(user="admin", key="root")
```

If you are using an older version of the client library, upgrade:
```bash
pip install --upgrade terminusdb-client
```

### API key format (DFRNT Cloud)

**Error message:** `HTTP 401: Unauthorized` when connecting to DFRNT Cloud

**Cause:** The API token is malformed, expired, or passed with the wrong header format.

**Fix:**

1. Generate a fresh API token from [DFRNT Cloud](https://studio.dfrnt.com) under your team settings.
2. Use the correct `Authorization` header format:
   ```bash
   curl -H "Authorization: Token YOUR_API_TOKEN" \
     https://dfrnt.com/api/hosted/TEAM/api/info
   ```
3. In the JavaScript client:
   ```javascript
   const client = new TerminusClient.WOQLClient(
     "https://dfrnt.com/api/hosted/MyTeam",
     { user: "myuser", token: "YOUR_API_TOKEN" }
   )
   ```
4. Do not include a trailing newline in the token (common if reading from a file):
   ```bash
   # WRONG — cat may include trailing newline
   TOKEN=$(cat token.txt)

   # CORRECT — trim whitespace
   TOKEN=$(cat token.txt | tr -d '[:space:]')
   ```

### Permission denied (insufficient privileges)

**Error message:** `"api:ErrorMessage": "Permission denied"` with HTTP 403

**Cause:** The authenticated user does not have the required capability (read, write, or manage) on the target database or organisation.

**Fix:**

1. Check what capabilities your user has:
   ```bash
   curl -u admin:root http://localhost:6363/api/document/_system?type=Capability
   ```
2. Grant the necessary role. For example, to grant full access to a database:
   ```bash
   curl -u admin:root -X POST \
     -H "Content-Type: application/json" \
     -d '{"operation": "grant", "scope": "admin/tdb-example-mydb", "role": ["admin"], "user": "User/targetuser"}' \
     http://localhost:6363/api/capabilities
   ```
3. See the [Access Control documentation](/docs/access-control-with-javascript) for full details on managing roles and capabilities.

### Token expired (DFRNT Cloud)

**Error message:** `HTTP 401: Unauthorized` on a request that previously worked, or `"token_expired"` in the response body

**Cause:** API tokens issued by DFRNT Cloud have an expiry. Once expired, all requests using that token fail with 401 — even though the token format is valid and the user has the correct permissions.

**Fix:**

1. Generate a new token from [DFRNT Cloud](https://studio.dfrnt.com) under your team settings.
2. If you use a long-running automation or CI pipeline, implement token refresh logic:
   ```bash
   # Check token validity before making requests
   RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" \
     -H "Authorization: Token $TOKEN" \
     https://dfrnt.com/api/hosted/MyTeam/api/info)

   if [ "$RESPONSE" = "401" ]; then
     echo "Token expired — regenerate from DFRNT Cloud dashboard"
     exit 1
   fi
   ```
3. For service accounts, contact your DFRNT Cloud team admin about long-lived tokens.

### CORS preflight rejected (browser-based requests)

**Error message:** Browser console shows `Access to XMLHttpRequest ... has been blocked by CORS policy` or `Preflight request doesn't pass access control check`

**Cause:** A genuine CORS block from TerminusDB itself is extremely unlikely — TerminusDB unconditionally allows all cross-origin requests (it reflects any `Origin` header back as `Access-Control-Allow-Origin`). There is no `TERMINUSDB_CORS` environment variable and no CORS allow-list to configure. When you see this error, the most common causes are:
- TerminusDB is not reachable at all, and the browser misreports a connection failure as a CORS error
- A reverse proxy (nginx, Caddy, Traefik) between the browser and TerminusDB is stripping or not forwarding CORS headers
- A browser extension (ad blocker, privacy tool) is blocking the request

**Fix:**

1. First confirm TerminusDB is reachable:
   ```bash
   curl -s http://localhost:6363/api/ok
   ```
   If this returns an error (not `"ok"`), TerminusDB is not running — the CORS error is a red herring. See [Connection Failures](/docs/troubleshooting-connection).

2. If using a reverse proxy, ensure it forwards the TerminusDB CORS response headers (`Access-Control-Allow-Origin`, `Access-Control-Allow-Methods`, `Access-Control-Allow-Headers`) rather than stripping or overriding them.

3. Disable browser extensions temporarily to rule out extension interference.

4. This is NOT an authentication error — it occurs before credentials are sent. Once the request reaches TerminusDB, authentication errors (401) will surface separately.

See [Calling TerminusDB from the Browser](/docs/browser-cors-howto) for a working browser fetch example.

## Still stuck?

- [Open an issue](https://github.com/terminusdb/terminusdb/issues) with the full error message (redact your credentials)
- Check the [JavaScript Client connection guide](/docs/connect-with-the-javascript-client) for auth setup
- Check the [Python Client connection guide](/docs/connect-with-python-client) for auth setup
