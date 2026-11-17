---
tags:
  - release-notes
  - access-control
  - diff-patch
  - time-processing
ai: assisted
title: 'TerminusDB 12.0.9: Security Fixes and Patch for Raw JSON Documents'
release:
  version: 12.0.9
  date: 2026-10-28
  line: '12.0'
  edition: open-source
  github: https://github.com/terminusdb/terminusdb/releases/tag/v12.0.9
nextjs:
  metadata:
    title: 'Security Fixes and Raw JSON Patch | TerminusDB 12.0.9'
    description: 'TerminusDB 12.0.9 is a security release. It fixes access checks for the apply API and JWT claims, and adds apply and patch for raw JSON documents.'
    keywords: terminusdb 12.0.9, terminusdb security release, terminusdb apply api, jwt claim validation, raw json patch, terminusdb release notes
    alternates:
      canonical: https://terminusdb.com/docs/releases/12-0-9-security-fixes-raw-json-patch/
---

TerminusDB 12.0.9 is a security release. All users are recommended to update to v12.0.9 or later. It was released on **28 October 2026**.

- [**Security fixes**](#security-fixes) — stricter access checks for the apply, patch and diff APIs, and harder JWT claim validation.
- [**Apply and patch for raw JSON**](#apply-and-patch-for-raw-json) — schemaless `raw_json` documents now work with apply and patch.
- [**Date intervals**](#date-intervals) — canonical ISO 8601 interval semantics, timezone offsets and typecasting with `xdd:dateRange`.

## Security fixes

### What changed

- Fixed an issue (CVSS 7.5/6.5) that allowed users with read access to a database to use the apply API. Reported by @Santoshkumarpuppala. See the advisory [GHSA-pg23-jx8g-vq9w](https://github.com/terminusdb/terminusdb/security/advisories/GHSA-pg23-jx8g-vq9w).
- The path-less `api_patch/6` and `api_diff/6` endpoints now require read access on at least one database. This prevents anonymous resource exhaustion.
- Hardened JWT claim validation against type confusion (CVE-2026-25537) ([#2597](https://github.com/terminusdb/terminusdb/pull/2597)).

### Why it matters

If several users share your server, and some of them have read-only access, the apply API fix makes sure those users cannot use apply. If your server is reachable from the internet, the patch and diff change stops anonymous callers from using these endpoints. If you use JWT authentication, the claim validation is now stricter.

We recommend that you upgrade, especially for multi-user or internet-exposed servers.

**Read more:** [Access control](/docs/access-control/) · [Troubleshooting authentication](/docs/troubleshooting-auth/)

## Apply and patch for raw JSON

Apply and patch now work on schemaless (`raw_json`) documents ([#2571](https://github.com/terminusdb/terminusdb/pull/2571)).

**Read more:** [Patch endpoint](/docs/patch-endpoint/) · [JSON diff and patch](/docs/json-diff-and-patch/)

## Date intervals

TerminusDB now uses canonical ISO 8601 interval semantics, with timezone offsets and typecasting with `xdd:dateRange` ([#2574](https://github.com/terminusdb/terminusdb/pull/2574)).

**Read more:** [xdd:dateTimeInterval semantics](/docs/datetime-interval-semantics/) · [WOQL interval algebra](/docs/woql-interval-algebra/)

## Fixes

- Intervals handling improvements ([#2569](https://github.com/terminusdb/terminusdb/pull/2569))
- Denied JWT-scoped authorization now returns 403 `access_not_authorised` instead of crashing. The caller falls back to anonymous capabilities.

Also:

- Removed a dead anonymous guard from `api_info`. Community serves the info payload to any caller, including anonymous. Enterprise redacts it for anonymous callers.
- Pinned terminus-store to v0.21.8 for forward-compatible pack import, to prepare for faster intervals ([#2568](https://github.com/terminusdb/terminusdb/pull/2568))
- Quieter GC plugin logging and JWT opt-in at runtime ([#2573](https://github.com/terminusdb/terminusdb/pull/2573))
- Dependency bumps ([#2581](https://github.com/terminusdb/terminusdb/pull/2581), [#2598](https://github.com/terminusdb/terminusdb/pull/2598), [#2599](https://github.com/terminusdb/terminusdb/pull/2599), [#2600](https://github.com/terminusdb/terminusdb/pull/2600))

## Upgrade

```bash
docker pull terminusdb/terminusdb-server:v12.0.9
```

[Docker install guide](/docs/install-terminusdb-as-a-docker-container/) · [Full changelog on GitHub](https://github.com/terminusdb/terminusdb/compare/v12.0.8...v12.0.9)

---

**Previous:** [TerminusDB 12.0.8: UUID v7 Keys and Faster JSON Loading](/docs/releases/12-0-8-uuid-v7-faster-json-loading/) · **All 12.0 releases:** [TerminusDB 12.0 changelog](/docs/releases/12-0-changelog/) · [Release index](/docs/releases/)
