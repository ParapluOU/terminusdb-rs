---
tags:
  - explanation
  - beginner
title: Who maintains TerminusDB
nextjs:
  metadata:
    title: "Who Maintains TerminusDB? Gettier and the Core Maintainers"
    keywords: terminusdb maintainers, is terminusdb maintained, terminusdb gettier, terminusdb open source, terminusdb releases
    description: TerminusDB is actively maintained by Gettier and the core maintainers. See recent releases, the open-source license, and how Gettier uses TerminusDB for financial reporting.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/about-gettier/
media: []
---

TerminusDB is actively maintained. [Gettier](https://gettier.ai) and the core maintainers look after the project, and have done so since 2025. We ship regular releases of the open-source community version, with new features, speed improvements, and security fixes.

## Recent releases

Here is what shipped in the 12.0 series:

- [12.0.0](/docs/releases/12-0-0-precise-numbers-free-form-json/) (December 2025) — precise numbers and free-form JSON documents.
- [12.0.1](/docs/releases/12-0-1-graphql-count-woql-set-operators/) (December 2025) — GraphQL `_count` and WOQL set operators.
- [12.0.4](/docs/releases/12-0-4-range-queries-time-intervals/) (February 2026) — fast range queries and interval algebra.
- [12.0.5](/docs/releases/12-0-5-streaming-history-diffs/) (March 2026) — streaming history with diffs.
- [12.0.6](/docs/releases/12-0-6-shared-documents-cascade-delete/) (June 2026) — shared documents and Handlebars text rendering.
- [12.0.7](/docs/releases/12-0-7-insert-delete-patches-folded-diffs/) (August 2026) — resolve merge conflicts with insert and delete patches.
- [12.0.8](/docs/releases/12-0-8-uuid-v7-faster-json-loading/) (October 2026) — UUID v7 keys and faster raw JSON loading.
- [12.0.9](/docs/releases/12-0-9-security-fixes-raw-json-patch/) (October 2026) — security fixes and a patch for raw JSON documents.

The full list is in the [12.0 changelog](/docs/releases/12-0-changelog/). You can follow the code on [GitHub](https://github.com/terminusdb/terminusdb).

## Open source stays open source

TerminusDB is open source under the Apache 2.0 license. The community version keeps getting releases and security patches. You can run it yourself for free with [Docker](/docs/install-terminusdb-as-a-docker-container/).

For production, managed hosting and TerminusDB Enterprise are available through [TerminusDB Cloud](/docs/cloud/) and [Azure](/docs/deploy-on-azure/).

## How Gettier uses TerminusDB

Gettier builds a platform for finance teams, from record to report, where every number is traced to its source. TerminusDB is the open-source foundation under it.

Financial reporting needs the same things TerminusDB is built for:

- **Every change is tracked.** You can see who changed a value, when, and in which commit.
- **Data is checked before it counts.** The schema checks every document when it is written.
- **History is kept.** You can go back to any earlier commit and compare it with today.

Learn more about [Gettier](https://gettier.ai) and try today.

## Next steps

- New to TerminusDB? Start with [What is TerminusDB?](/docs/terminusdb-explanation/) and the [Quickstart](/docs/get-started/).
- Want to talk about your use case, enterprise support, or managed hosting? {% cta-link href="https://gettier.ai/connect" event="about_contact" %}Talk to us{% /cta-link %}.
