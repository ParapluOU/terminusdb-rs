---
tags:
  - explanation
  - knowledge-graph
  - version-control
  - vectorlink
  - curl
title: A Knowledge Graph for AI Agents with Git-style Branching
nextjs:
  metadata:
    title: Knowledge Graph for AI Agents — Branch, Diff and Merge
    description: Use TerminusDB as a knowledge graph for AI agents. Give each agent run its own branch, review the diff, merge it, and query past facts at any commit.
    keywords: knowledge graph for ai agents, graph database for ai agents, agent memory, git-style branching, temporal memory, agent workflows, branch per agent, diff and merge, time travel queries, invalidate outdated facts, versioned search, immutable database, terminusdb
    openGraph:
      images: https://assets.terminusdb.com/docs/terminusdb-knowledge-graph-database-og.png
    alternates:
      canonical: https://terminusdb.com/docs/knowledge-graph-for-ai-agents/
media: []
---

AI agents need somewhere to keep what they learn. They also need a safe place to try changes, and a way for you to check those changes before they count. TerminusDB is a [knowledge graph database](/docs/knowledge-graph-database/) with [git-like version control](/docs/graph-database-with-git-like-version-control/), so it covers these needs with features it already has: branches, commits, diffs, merges and time travel.

This page explains how these features fit an agent workflow. Then it shows a short example with the HTTP API.

## Why agents need version control

An agent writes facts: "service X is owned by team A", "this customer prefers email", "function `parse()` lives in `utils.py`". Some of these facts are wrong. Some become outdated. And often several agents, or several runs of one agent, write at the same time.

A plain database or vector store makes this hard. A write replaces the old value, and the old value is gone. You cannot see what the agent changed in one run. You cannot undo one run without undoing others. And you cannot ask what the agent knew last Tuesday.

TerminusDB keeps [every write as an immutable commit](/docs/immutability-explanation/). This gives you the same tools for agent memory that git gives you for code.

## How the parts fit together

### A branch per agent run

You can [create a branch](/docs/branch-howto/) for each agent run or each task. The agent reads and writes only on its branch. The `main` branch does not change until you merge.

A new branch is cheap. It shares all history with the branch it came from, and it stores only the new changes. If a run goes wrong, you [delete the branch](/docs/branch-howto/), and nothing on `main` is affected.

### Diff and merge what the agent changed

When the run is done, you ask for a [diff](/docs/diff-and-patch-operations/) between `main` and the agent's branch. The diff lists each document the agent added, removed or changed, down to the field. A person, a test, or another agent can review it.

If the change is good, you [merge it](/docs/merge-howto/) into `main`. TerminusDB uses a three-way merge. If two branches changed the same field of the same document, the merge stops and reports the conflict. It does not silently pick one value.

### Commit history as temporal memory

Every commit has an author, a message and a timestamp. You can read the [commit log](/docs/version-control-operations/) to see who changed what, and when. With [time travel queries](/docs/graph-database-with-time-travel-queries/) you can read the graph as it was at any commit, with the same API you use for live data.

For an agent, this is a simple kind of temporal memory: "what did we believe at this point?" is a normal query.

### Replace outdated facts without losing them

When a fact is no longer true, the agent writes the new value. That creates a new commit. Queries on the branch now return the new value. The old value is not deleted from history. It stays readable at the earlier commit. When something becomes invalid you delete it, history keeps it.

So you do not need extra "valid until" fields to keep old facts. The commit history keeps them for you. If a fact was wrong, you can [restore the earlier state](/docs/time-travel-howto/) or [reset a branch](/docs/undo-reset-howto/).

One note: commit history records when a fact was *written*. It does not record when something was true in the real world. If you need that, model it in your schema, for example with [date and interval fields](/docs/time-tutorial-intervals/), into a bi-temporal knowledge graph.

### Search that follows the branch, for every commit

In TerminusDB 12.1 (release candidate), [Versioned Search](/docs/versioned-search/) adds semantic, full-text and hybrid search inside the database. When the server and the database have [automatic indexing turned on](/docs/versioned-search-automatic-indexing/), new commits are indexed by themselves.

The search index follows your branches. A branch [shares its parent's vectors](/docs/versioned-search-history/) and only embeds what changed. Search on an agent's branch sees the agent's new documents, and search on `main` does not. You can also search a specific commit and get the same results each time. This is useful when you need to repeat what an agent saw.

## A short example

This example uses the HTTP API on a local server at `localhost:6363` with the default `admin:root` login. The calls are the same as in [How to Branch](/docs/branch-howto/), [How to Merge](/docs/merge-howto/) and [How to Time-Travel](/docs/time-travel-howto/).

Using schema you get all the benefit of both data governance, graphql capabilities, automated embeddings management and more. You can also use TerminusDB with raw JSON, with natural limitations on how well agents can understand your data, rules and semantics. 

### 1. Create a database and a simple schema

```bash
curl -u admin:root -X POST http://localhost:6363/api/db/admin/agent_memory \
  -H "Content-Type: application/json" \
  -d '{"label": "Agent memory", "comment": "Facts written by agents"}'

curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/agent_memory?graph_type=schema&author=admin&message=Add+schema" \
  -H "Content-Type: application/json" \
  -d '{"@id": "Fact", "@type": "Class", "@key": {"@type": "Lexical", "@fields": ["name"]},
       "name": "xsd:string", "statement": "xsd:string"}'
```

### 2. Add a fact on main

```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/agent_memory/local/branch/main?author=admin&message=Add+owner" \
  -H "Content-Type: application/json" \
  -d '{"@type": "Fact", "name": "auth-service-owner", "statement": "The auth service is owned by team A."}'
```

### 3. Note the head of main, then give the agent a branch

The head commit of `main` is the point where the branch starts. You need it later for the merge.

```bash
curl -u admin:root "http://localhost:6363/api/log/admin/agent_memory/local/branch/main?count=1"
# Copy the "identifier" value, for example abc123def456

curl -u admin:root -X POST http://localhost:6363/api/branch/admin/agent_memory/local/branch/agent-run-42 \
  -H "Content-Type: application/json" \
  -d '{"origin": "admin/agent_memory/local/branch/main"}'
```

### 4. The agent updates one fact and adds another

```bash
curl -u admin:root -X PUT \
  "http://localhost:6363/api/document/admin/agent_memory/local/branch/agent-run-42?author=agent&message=Owner+changed" \
  -H "Content-Type: application/json" \
  -d '{"@id": "Fact/auth-service-owner", "@type": "Fact", "name": "auth-service-owner",
       "statement": "The auth service is owned by team B."}'

curl -u admin:root -X POST \
  "http://localhost:6363/api/document/admin/agent_memory/local/branch/agent-run-42?author=agent&message=Add+repo+fact" \
  -H "Content-Type: application/json" \
  -d '{"@type": "Fact", "name": "auth-service-repo", "statement": "The auth service code is in the auth repository."}'
```

### 5. Review the diff

```bash
curl -u admin:root -X POST http://localhost:6363/api/diff/admin/agent_memory \
  -H "Content-Type: application/json" \
  -d '{"before_data_version": "main", "after_data_version": "agent-run-42"}'
```

The response lists a `SwapValue` for the changed `statement` and an `Insert` for the new fact.

### 6. Merge into main

Use the commit from step 3 as `before_commit`, because it is the common ancestor of the two branches.

```bash
curl -u admin:root -X POST http://localhost:6363/api/apply/admin/agent_memory/local/branch/main \
  -H "Content-Type: application/json" \
  -d '{
    "before_commit": "commit:abc123def456",
    "after_commit": "agent-run-42",
    "commit_info": {"author": "reviewer", "message": "Merge agent run 42"}
  }'
```

### 7. Read the old fact

`main` now says team B. The old statement is still there at the earlier commit:

```bash
curl -u admin:root \
  "http://localhost:6363/api/document/admin/agent_memory/local/commit/abc123def456?id=Fact/auth-service-owner"
```

You can do the same steps with the [TypeScript and Python clients](/docs/use-the-clients/). The [merge guide](/docs/merge-howto/) shows `client.apply()` in both.

## When this is a good fit

- Several AI agents, or several runs, write transactionally to the same knowledge graph database.
- You want a person or a test to check the AI agent changes before they go live.
- You need to see what an AI agent knew at a given time, for debugging or audit.
- You want structured facts with a schema, not only text chunks, and automatic high quality embeddings.

It is a weaker fit if you only need a short-lived chat buffer, or high-rate event logs. History also uses storage, as described in the [time travel limits](/docs/graph-database-with-time-travel-queries/#limits).

## See also

- [Knowledge Graph Database](/docs/knowledge-graph-database/) — what a knowledge graph database is, and how TerminusDB does it
- [Knowledge Graph Version Control](/docs/knowledge-graph-database-version-control/) — the git-for-data concepts, one by one
- [Time Travel Queries](/docs/graph-database-with-time-travel-queries/) — read the graph at any commit
- [Versioned Search](/docs/versioned-search/) — semantic and hybrid search per commit and branch
- [Immutability](/docs/immutability-explanation/) — why TerminusDB never changes data in place
- [Get started](/docs/get-started/) — includes a prompt to set up TerminusDB with an AI coding agent
