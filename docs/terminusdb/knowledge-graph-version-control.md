---
tags:
  - explanation
  - version-control
  - knowledge-graph
  - beginner
title: Knowledge Graph Database with Git-like Version Control
nextjs:
  metadata:
    title: Knowledge Graph Database with Git-like Version Control
    description: An instructional overview of TerminusDB as a git-for-data knowledge graph database — covering commits, branches, merges, rollback, diff and patch, time travel, and the similarities and differences with Git.
    keywords: terminusdb, knowledge graph database, git for data, git-like version control, version-controlled database, commit and rollback, RDF triples, branching database, time travel database, diff and patch, data provenance, immutable database, knowledge graph version control
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/knowledge-graph-version-control/
media: []
---

TerminusDB is a **knowledge graph database with git-like version control**. It stores data as a connected graph of [RDF triples](/docs/graphs-explanation/) and treats every change to that graph the same way Git treats changes to source code: as an immutable, attributable, branchable, and reversible commit. If you have ever used Git to clone a repository, create a branch, commit a change, or roll back a mistake, you already understand most of how TerminusDB manages data.

This page introduces the core concepts of **git-for-data** version control in TerminusDB, maps each one to its Git counterpart, and points you to the deeper guides for each operation.

## Why version-control a knowledge graph?

A traditional database overwrites data in place. The previous state is lost the moment you `UPDATE`. That is fine for transactional bookkeeping, but it is a poor fit for the way modern teams actually work with data:

- Analysts need to reproduce results from last quarter, not just today's snapshot.
- Auditors need to know who changed what, and when.
- Data engineers need a safe place to try a transformation before it touches production.
- Collaborators need to merge contributions from different sources without trampling each other.

A **version-controlled knowledge graph database** solves all four. TerminusDB keeps every commit, every branch, and every author on an [immutable](/docs/immutability-explanation/) append-only store, so the full history of your data is queryable, reversible, and shareable — just like a Git repository, but for facts instead of files.

## The mental model: Git for data

If you replace "file" with "RDF triple" and "repository" with "knowledge graph", the Git mental model carries over almost line for line.

| Git concept | TerminusDB equivalent | What it means for data |
|---|---|---|
| Repository | Database | A versioned container of triples and schema |
| Working tree | Current branch HEAD | The live state your queries see |
| Commit | Commit | An atomic, signed change to the graph |
| Branch | Branch | A named line of history you can write to |
| Merge | Merge / rebase | Combine changes from two branches |
| `git diff` | Diff | A structured patch describing what changed |
| `git revert` / `reset` | Reset / undo | Move HEAD back to an earlier commit |
| `git log` | Commit log / audit | Who changed what, when, and why |
| `git clone` / `push` / `pull` | Clone / push / pull | Synchronize databases across machines |
| Remote | Remote | Another TerminusDB instance you collaborate with |

The crucial difference is **granularity**. Git versions text lines inside files. TerminusDB versions individual **subject–predicate–object** triples inside a knowledge graph. That means a TerminusDB diff is not a textual hunk that you have to re-parse — it is a structured, semantic description of which facts were added, removed, or changed. Every atomic commit records added and removed triples in records that were added, modified, or deleted (including nested objects and arrays).

## Core concepts, one by one

### Clone, push, and pull

TerminusDB databases can be copied between instances exactly the way Git repositories are. You [clone](/docs/clone-a-project/) a remote database to get a full local copy with its entire history, [pull](/docs/pull-from-project/) to fetch new commits from upstream, and [push](/docs/push-to-project/) to publish your local commits back. See [**Clone, push, and pull**](/docs/use-the-collaboration-features/) for the full collaboration model.

**Like Git:** the transfer protocol is commit-based, so you only move what is new.
**Unlike Git:** the unit being transferred is a layer of triples, not a pack of file deltas, so you can reason about the change semantically as soon as it lands.

### Commits and the immutable history

Every write to a TerminusDB database produces a [commit](/docs/commit-message-howto/) with an author, a timestamp, a message, and a content-addressed identifier. Commits are stacked into [immutable layers](/docs/immutability-explanation/), and nothing is ever overwritten — new data is appended, deletions are recorded as masks. This is what makes **commit-and-rollback** safe: rolling back is just moving a branch pointer, never destroying data.

### Branches

A branch in TerminusDB is a named pointer to a commit, exactly as in Git. You create a branch to try out a schema change, run an experimental migration, or stage a release without disturbing `main`. See [**Branch how-to**](/docs/branch-howto/) for the operations and [**Operations reference**](/docs/version-control-operations/) for the underlying API.

**Like Git:** branches are cheap, named, and movable.
**Unlike Git:** because branches live inside a database engine, you can query any branch with [WOQL](/docs/woql-explanation/) or [GraphQL](/docs/graphql-basics/) directly — no `checkout` required.

### Merge

When two branches diverge, you reconcile them with a [merge](/docs/merge-howto/). TerminusDB replays the commits of one branch onto another, detecting conflicts at the triple level rather than the line level. Because conflicts are described as competing facts, they are usually easier to resolve than textual merge conflicts in code.

### Diff and patch

A [diff](/docs/diff-and-patch-operations/) between two commits, branches, or documents produces a structured patch — a JSON description of which triples or document fields were inserted, deleted, or modified. You can apply that patch to another branch, send it over the wire, or store it as a record of intent. The full grammar is documented in the [**JSON diff and patch reference**](/docs/json-diff-and-patch/). You can even use the powerful json diff and patch engine on two submitted documents without storing the data in TerminusDB first.

**Like Git:** patches can be inspected, transported, and replayed.
**Unlike Git:** patches are semantic JSON, not textual hunks, so they survive reformatting, renaming, and reordering without spurious conflicts.

### Time travel

Because history is immutable, every past commit is still a live, queryable database. [**Time-travel queries**](/docs/time-travel-howto/) let you ask "what did the graph look like at commit X?" or "what changed between Monday and Friday?" with no replay, no restore, and no extra storage strategy.

### Undo and reset

Mistakes happen. [**Undo and reset**](/docs/undo-reset-howto/) walks through how to move a branch back to a previous commit when you need to discard the last few changes — the data-graph equivalent of `git reset --hard`. Because the discarded commits remain in the immutable store, "undo" is reversible: you can re-point the branch forward again if you change your mind.

### Recover data

Even when a branch has moved past a commit, the underlying layers are still there. The [**recovery tutorial**](/docs/recovery-tutorial/) shows how to find and restore data that looks lost — typically a one-step operation, because nothing was ever truly deleted.

### Audit changes

Every commit carries author, timestamp, and message metadata. The [**audit tutorial**](/docs/audit-tutorial/) shows how to turn that metadata into a compliance-ready change log: who modified which entity, when, and why. This is the same information `git log` and `git blame` give you for code, but applied to facts in your knowledge graph.

### Reset, squash, and rebase

For day-to-day cleanup, you can [reset a branch](/docs/reset-a-project/) to a specific commit and [squash](/docs/squash-projects/) a long chain of small commits into a single tidy one before sharing. These operations behave just like their Git namesakes.

## Similarities and differences with Git, in one paragraph

TerminusDB borrows Git's **commit graph**, **branching model**, **clone/push/pull workflow**, and **immutable object store**, and applies all of it to a [knowledge graph](/docs/graphs-explanation/) of RDF triples instead of a tree of text files. 

The differences are mostly upgrades for a database setting: changes are tracked at the **triple level** rather than the line level, every historical commit is **immediately queryable** via the [REST document API](/docs/http-documents-api/), [WOQL](/docs/woql-explanation/) datalog language and [GraphQL](/docs/graphql-basics/), diffs are **semantic JSON patches** rather than textual hunks, transactions are [**ACID**](/docs/acid-transactions-explanation/), and the underlying [immutable layer architecture](/docs/immutability-and-concurrency/) gives you lock-free concurrency on top of full history.

## Where to go next

- [**Clone, push, and pull**](/docs/use-the-collaboration-features/) — the full collaboration workflow
- [**Merge how-to**](/docs/merge-howto/) — combine branches and resolve conflicts
- [**Time-travel how-to**](/docs/time-travel-howto/) — query the past
- [**Undo and reset how-to**](/docs/undo-reset-howto/) — move HEAD safely
- [**Diff and patch**](/docs/diff-and-patch-operations/) — structured, semantic change records
- [**Recover data**](/docs/recovery-tutorial/) — restore from immutable history
- [**Audit changes**](/docs/audit-tutorial/) — turn commit metadata into compliance evidence
- [**Operations reference**](/docs/version-control-operations/) — every version-control API call
- [**Git-for-data reference**](/docs/git-for-data-reference/) — the canonical reference for the model

If you are new to TerminusDB, the fastest way to feel the Git-like version-control model in practice is the [**first 15 minutes quickstart**](/docs/first-15-minutes/), which walks you through commits, branches, and a merge on a fresh database.
