---
tags:
  - python
  - installation
  - troubleshooting
  - intermediate
title: Handle SSL Certificate Errors in the Python Client
nextjs:
  metadata:
    title: Handle SSL Certificate Errors in the Python Client
    description: How to suppress or resolve SSL certificate verification errors when connecting the TerminusDB Python client to local or self-signed TLS instances.
    keywords: python, SSL, certificate, self-signed, TLS, verify
    alternates:
      canonical: https://terminusdb.org/docs/python-certificate-issues/
---

When connecting the Python client to a TerminusDB instance using a self-signed certificate or a local TLS proxy, you may see errors like `SSLCertVerificationError` or `CERTIFICATE_VERIFY_FAILED`. This guide shows how to handle them.

## Disable certificate verification

Pass `verify=False` to skip SSL certificate validation entirely:

```python
from terminusdb_client import Client

client = Client("https://localhost:6363", verify=False)
client.connect(user="admin", key="root")
```

{% callout type="warning" title="Security risk" %}
Disabling certificate verification makes the connection vulnerable to man-in-the-middle attacks. Only use `verify=False` for local development or testing — **never in production**.
{% /callout %}

## Use a custom CA bundle

If your instance uses a self-signed certificate that you trust, point `verify` to your CA bundle file instead of disabling verification:

```python
from terminusdb_client import Client

client = Client("https://my-terminusdb.internal:6363", verify="/path/to/ca-bundle.crt")
client.connect(user="admin", key="root")
```

This validates the server certificate against your custom CA while maintaining TLS security.

## Export your self-signed certificate

If you generated a self-signed certificate for your TerminusDB instance and want to use it as a trusted CA bundle, export it:

```bash
# Extract the certificate from a running server (Linux/macOS)
openssl s_client -connect my-terminusdb.internal:6363 < /dev/null 2>/dev/null \
  | openssl x509 -outform PEM > terminusdb-cert.pem
```

Then reference the exported file:

```python
client = Client("https://my-terminusdb.internal:6363", verify="./terminusdb-cert.pem")
client.connect(user="admin", key="root")
```

## Set certificate path via environment variable

For CI/CD pipelines or containerised applications, you can set the certificate path without modifying code:

```bash
export REQUESTS_CA_BUNDLE=/path/to/ca-bundle.crt
python my_app.py
```

The Python `requests` library (used internally by the TerminusDB client) reads this variable automatically. No code changes needed.

## Troubleshooting

**Error:** `SSLCertVerificationError: certificate verify failed: unable to get local issuer certificate`

This means the server's certificate was signed by a CA that your system does not trust. Solutions:
1. Add your organisation's CA certificate to the system trust store
2. Pass the CA bundle path to `verify=`
3. Set `REQUESTS_CA_BUNDLE` environment variable

**Error:** `SSLCertVerificationError: certificate verify failed: self-signed certificate`

The server is using a self-signed certificate. Either export it and pass it to `verify=`, or use `verify=False` for development only.

**Error:** `ConnectionError: Max retries exceeded` (after disabling verify)

This is not a certificate error — the server is unreachable. Check that TerminusDB is running and the hostname/port are correct.

## When to use each approach

| Scenario | Recommendation |
|----------|---------------|
| Local Docker with `localhost` | Use `http://localhost:6363` (no TLS needed) |
| Self-signed cert in development | `verify=False` is acceptable |
| Internal CA in staging/production | Point `verify` to your CA bundle |
| CI/CD pipeline with internal CA | Set `REQUESTS_CA_BUNDLE` env var |
| Public DFRNT Cloud | No action needed — certificates are valid |

## See also

- [Python Quickstart](/docs/connect-with-python-client/) — connect to TerminusDB with the Python client
- [Self-Hosted Installation](/docs/self-hosted-installation/) — deploy with TLS reverse proxy
- [Troubleshooting Connection Failures](/docs/troubleshooting-connection/) — other connectivity issues
