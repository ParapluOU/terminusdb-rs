---
tags:
  - explanation
  - schema
  - rdf
  - advanced
title: IRIs, Prefixes, and Minting
nextjs:
  metadata:
    title: IRIs, Prefixes, and Minting
    description: How TerminusDB treats IRIs as identity, context prefixes as presentation, and why migrations expand, operate, and contract rather than reasoning in compressed space
    keywords: IRI, URI, prefix, @base, @schema, minting, schema migration, RDF
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/iris-prefixes-and-minting/
media: []
---

Every document, class, property, and enum member in TerminusDB is identified by an **IRI** (internationalized URI, often looks like a URL) — a full, expanded identifier such as `terminusdb:///data/Person/sg9k2x`. Everything you see in a shorter form, like `Person/sg9k2x` or `myclass:Person`, is that IRI in compressed form. The IRI is the identity; the short form is presentation based on the prefixes in the `@context` section of the schema graph.

This distinction sounds cosmetic until you change the prefixes. Then it becomes important: queries, links, and migrations all hang on which form you are actually holding.

## Context as a dictionary of prefixes

The `@context` document declares, among other things, two minting prefixes:

- `@schema` — where new *class* IRIs are minted, and the prefix used to compress schema IRIs (class names, property names, enum member nodes), used by default for schema classes,
- `@base` — where new *instance* IRIs are minted, and the prefix used to compress document IRIs, used by default for documents.

Both do double duty: one side steers minting, the other steers how stored IRIs render on the way out. That is all they are. A class created as `Person` mints `@schema + "Person"` — say `terminusdb:///schema#Person` — and renders back as `Person` while that context is in force. The IRI is what was stored; the name is what you are shown.

Prefixes can also be declared explicitly. A class named `myclass:Person` expands through the `myclass` prefix binding and lives happily outside the `@schema` namespace — classes are not required to live under `@schema`, it is simply the default minting namespace.

## Expand, operate, contract

Because a compressed name only means something relative to the context *right now*, reasoning about identity in compressed space can be unsafe. `"F"` under `@schema` `http://old#` and `"F"` under `@schema` `http://new#` are different IRIs — but could be for the same class at different points in time. Any comparison, rewrite, or lookup done on compressed names silently depends on what the context looks like.

The engine therefore works IRI-first:

1. **Expand** every identifier through the current context to its full IRI.
2. **Operate** on the expanded IRI — match, rewrite, compare.
3. **Contract** the result back through the context for display.

Storage is the expanded form. When you write `Person/alice`, the triple stored on disk carries `terminusdb:///data/Person/alice`; when the same triple comes back out, the current context decides whether you see `Person/alice`, a prefixed form, or the full IRI. Two API responses holding the same document can render it differently under different contexts — the bytes underneath never change.

Migrations follow the same rule. A migration that moved data in compressed space would corrupt the moment a prefix changed meaning — the same string would rewrite two different IRIs, or fail to find an IRI that was compressed differently at write time. Operating on the expanded IRI is what makes a migration *timeless*: it means the same thing whenever it runs.

## A minted IRI never moves

Once an IRI is minted it is immutable — even across class changes, property moves, or context swaps. A document minted as `terminusdb:///data/F/1` stays there forever; it does not chase its class through renames, and it does not rebase when `@base` changes.

This is deliberate. Links point at IRIs, not at names — a document referenced from three other documents, a branch history, and an external system cannot be renamed without breaking all of them. Since several classes may share one `@base`, an IRI under a base does not even prove which class minted it. The only safe rewrite is no rewrite: new identifiers go to the new place, old identifiers stay valid. The one explicit exception is `RebaseIRIPrefix`, covered below.

## How new IRIs are minted (created)

Instance minting builds the new IRI from the class's mint base:

- The class IRI is compressed under `@schema` — if it compresses to `F`, instances mint `@base + "F" + "/" + suffix`.
- If the class IRI lives outside the `@schema` namespace — a prefixed class, or a class minted before the context changed — it cannot compress, so the class IRI itself is the mint root: `terminusdb:///schema#F/<random>`.

A class can also carry its own `@base`, which replaces the context `@base` for that class's instances. Changing it is a free, weakening operation — it affects only future mints, never stored data. A `Person` class with `@base` `people/` mints `terminusdb:///data/people/<random>` while the default context `@base` is `terminusdb:///data/`; set it to `https://example.org/people/` and new instances mint there instead — the documents already stored do not budge.

## What ReplaceContext does — and does not do

Given the model above, `ReplaceContext` is a small, precise operation: it swaps the context document's `@base`/`@schema` values. That changes where *future* identifiers mint and how IRIs compress on output — and nothing else. No class IRI moves, no document IRI moves, no stored reference breaks.

The tempting alternative — "rewrite everything to the new prefix" — is the compressed-space mistake at database scale: it assumes every stored IRI under a prefix *means* that prefix, which is exactly backwards. A real prefix rebase, when one is truly wanted, is an explicit operation of its own — destructive, deliberate, and never implied by a context change.

## RebaseIRIPrefix: the opt-in escape hatch (since 12.1.0-rc2)

Sometimes a namespace genuinely has to move — a project outgrows `terminusdb:///schema#`, an organization adopts its own canonical prefix, or data arrives minted under a prefix that conflicts with yours. Since **12.1.0-rc2**, that is what `RebaseIRIPrefix` is for:

```typescript
{ "@type" : "RebaseIRIPrefix",
  "from" : "terminusdb:///schema#",
  "to" : "http://example.com/schema#" }
```

The operation rewrites every stored IRI that starts with `from` so it starts with `to` — subjects, predicates and objects across the instance, schema and inference graphs, in one atomic transaction. Class identifiers, `@inherits` and `@oneOf` references, enum member nodes, `rdf:type` assertions, property predicates and links all move together, so nothing is left pointing at the old namespace. `from` and `to` may be full IRI prefixes or names of prefixes declared in the context.

Three properties make it safe to have at all:

- **It is opt-in.** The request fails with `api:DestructiveMigrationRequiresOptIn` unless `allow_destructive_migration` is set to `true` — there is no accidental rebase. `from` equal to `to` is rejected up front as `api:RebasePrefixUnchanged`.
- **It operates on expanded IRIs.** Matching happens on the stored form, never on compressed names, so the current context cannot skew what gets rewritten.
- **It leaves literals alone.** A stored string or saved query mentioning the old prefix is data, not vocabulary — only IRI positions move. Document IRIs under `@base` likewise stay put unless the data prefix itself is the one being rebased.

Pair it with `ReplaceContext` when the minting prefixes should change to match: the rebase already carries the context's own `@schema`/`@base` declarations and each class's `@base`, so new identifiers mint under the new namespace and compress again.

## A worked example

Start with a class `F` and a document minted as `terminusdb:///data/F/1`, then replace the context with `@base` `http://a/` and `@schema` `http://a#`. Afterwards:

- The class is still `terminusdb:///schema#F` — it renders expanded now because `http://a#` cannot compress it.
- The document is still `terminusdb:///data/F/1`, with every property intact.
- A *new* class mints under `http://a#`; a *new* document of an `http://a#` class mints under `http://a/`.
- A *new* document of the old class mints `terminusdb:///schema#F/<random>` — the class IRI is its mint root, since it no longer compresses under `@schema`.
- Queries, WOQL bindings, and links to `terminusdb:///data/F/1` resolve exactly as before — the identifier in the graph is unchanged.

Nothing moved, nothing broke, and the only thing that changed is where the next identifiers go.
If any step of that list surprises you, the compressed forms were doing the thinking —
expand them and the behavior falls out of the model.

## Summary

| Concept | Meaning |
| ------- | ------- |
| IRI | The identity — stored, linked, immutable once minted |
| `@schema` | Minting + compression prefix for class-namespace IRIs |
| `@base` | Minting + compression prefix for instance IRIs (per-class overridable) |
| Compressed name | Presentation — valid only relative to the current context |
| `ReplaceContext` | Swaps the context; steers future mints; touches nothing stored |
| `RebaseIRIPrefix` | Explicit opt-in prefix rebase (12.1.0-rc2+) — never a side-effect of context change |

Keep the two layers separate in your head and most migration questions answer themselves: identities live in the graph and never move, prefixes live in the context and are free to change. When in doubt: expand, operate, contract.
