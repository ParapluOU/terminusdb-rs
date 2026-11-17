---
tags:
  - explanation
  - enterprise
title: Security and coordinated patch releases
nextjs:
  metadata:
    title: Security and coordinated patch releases — TerminusDB
    description: How to report a TerminusDB security issue, how customers get early access to advisories and patches, and when fixes reach the open source release.
    keywords: terminusdb security, security policy, vulnerability reporting, coordinated release, security advisory, cve
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/security/
---

We take security reports seriously. This page explains how to report an issue, and how fixes reach customers and the open source community.

## Report a security issue

Please report security issues privately through [GitHub private vulnerability reporting](https://github.com/terminusdb/terminusdb/security/advisories/new). Do not open a public issue.

Include, if you can:

- OS name and version
- TerminusDB version
- How to reproduce the issue
- Proof of concept code

We coordinate security issues through GitHub, from the first report to a published advisory if applicable.

## Supported versions

| Version        | Supported |
| -------------- | --------- |
| 12.0 and later | Yes       |
| Before 12.0    | No        |

## How a fix is released

Security fixes are released in three steps.

### 1. Early access for customers, under NDA

Customers may register to get draft advisories and early access to patches, so they can run regression tests on their own systems before the fix is public. Early access is under NDA.

### 2. Coordinated customer release

Customers get the patched release first, in a coordinated release, still under NDA. Timelines follow the policies in each customer agreement. TerminusDB on Azure releases are part of this coordinated release.

### 3. Community release

The fix is then released as an open source version of TerminusDB in a regular release. The CVE and the security advisory are published at the same time as the community release.

## Past security releases

- [TerminusDB v12.0.9](https://github.com/terminusdb/terminusdb/releases/tag/v12.0.9) — security fixes
- [Published security advisories](https://github.com/terminusdb/terminusdb/security/advisories)

## Coordinated releases for customers

Coordinated releases and early access are part of TerminusDB customer agreements, including TerminusDB on Azure. To learn more, {% cta-link href="https://gettier.ai/connect" event="security_contact" %}contact the team at Gettier{% /cta-link %}.
