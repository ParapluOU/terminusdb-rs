---
tags:
  - how-to
  - vectorlink
  - version-control
title: History & Branching in Versioned Search
nextjs:
  metadata:
    title: History & Branching in Versioned Search
    description: Per-commit snapshots, branch-out with block reuse, reassignment, and staleness handling
    keywords: terminusdb, versioned search, history, branching, snapshots, staleness, commit, vectorlink
    openGraph:
      images: https://assets.terminusdb.com/docs/vectorlink-semantic-cms.png
    alternates:
      canonical: https://terminusdb.com/docs/versioned-search-history/
media: []
---

{% callout type="note" title="Release Candidate — TerminusDB 12.1" %}
This page documents functionality in the upcoming TerminusDB 12.1 release. Details may change before the final release.
{% /callout %}

The examples on this page use the `admin/star_wars` database created in [TerminusDB Push Indexing](/docs/versioned-search-indexing/). If you have not already run those steps, start there first — this page picks up where the indexing walkthrough left off.

All requests go through TerminusDB on port `:6365`. TerminusDB authorises each call and forwards it to the search engine internally — you never talk to the engine directly.

This is what makes Versioned Search more than a vector index: every commit is an independent, reproducible snapshot, and branching shares a parent's vectors instead of recomputing them.

## Per-commit snapshots are reproducible

The indexing walkthrough already demonstrated this: after updating Yoda's bio in commit 3, searching for "Jedi teacher on Dagobah" at HEAD returns the updated snippet, while searching at commit 1 returns the original short bio without "Dagobah". Let's recap the key points.

Every document insert, update, or delete creates a new commit in TerminusDB. The `post_commit_hook` pushes only the delta to the engine — the changed documents. Unchanged documents are reused from the parent commit's vectors, never re-embedded.

First, list the commits to find the ID of commit 1 (the initial document insert). Commit IDs are generated fresh when you create the database, so yours will differ from the examples below. Run this command and find the `identifier` for the "Add Yoda and Luke" commit:

```bash publishes="commit_identifiers" publishColumn="identifier" publishLabel="message"
curl -u admin:root 'http://localhost:6365/api/log/admin/star_wars?count=10'
```

The response lists each commit with its `identifier` and `message`:

| identifier | message |
| --- | --- |
| `o4x1v8xu2o0r8sl7dke8mkbdivmu39l` | Update Yoda bio |
| `6qd6132h27ijljrfags2r1mpvbjbmxx` | Add Mon Calamari |
| `xy918u5vxlmz3ocqrs859ocheaaiuqj` | Add Yoda and Luke |
| `2p0ufdnea9u4vebc4gmsmxgv2187z5y` | Add schema |

Click the "Add Yoda and Luke" identifier in the result table above to copy it, then replace `COMMIT_ID` in the command below. At commit 1, Yoda's bio is the original short version — no mention of Dagobah:

```bash
curl -u admin:root "http://localhost:6365/api/search/admin/star_wars/local/commit/COMMIT_ID?q=Jedi+teacher+on+Dagobah&snippet=true"
# [
#   {
#     "id": "Character/Yoda",
#     "distance": 0.3186,
#     "chunk": { "snippet": "Yoda. A wise old Jedi master, small and green." }
#   },
#   {
#     "id": "Character/Luke%20Skywalker",
#     "distance": 0.3486,
#     "chunk": { "snippet": "Luke Skywalker. A farm boy from Tatooine who became a Jedi knight." }
#   }
# ]
```

Compare this with searching at HEAD (the latest commit), where Yoda's bio has been updated to include Dagobah:

```bash
curl -u admin:root 'http://localhost:6365/api/search/admin/star_wars?q=Jedi+teacher+on+Dagobah&snippet=true'
# [
#   {
#     "id": "Character/Yoda",
#     "distance": 0.3582,
#     "chunk": { "snippet": "Yoda. A wise old Jedi master, small and green. Trained Jedi for over 800 years on Dagobah." }
#   }
# ]
```

At commit 1, Yoda's snippet is the original short bio — the embedding cannot match "Dagobah" or "teacher" because those words are absent. At HEAD, the updated bio includes both concepts and Yoda is the sole result. The old snapshot is frozen; later indexing does not change it.

Only Yoda was re-embedded for the update commit. Mon Calamari and Luke were reused from the parent, not recomputed.

## Branch-out shares the parent's vectors

Create a branch from `main`:

```bash
curl -u admin:root -X POST 'http://localhost:6365/api/branch/admin/star_wars/local/branch/experiment' \
  -H 'Content-Type: application/json' -d '{}'
# {"@type":"api:BranchResponse","api:status":"api:success"}
```

The branch is created, but its schema graph starts empty. Propagate the schema from `main` so the branch knows about the `Character` and `Species` classes and their embedding metadata:

```bash
curl -u admin:root -X POST \
  'http://localhost:6365/api/document/admin/star_wars/local/branch/experiment?graph_type=schema&author=admin&message=Add+schema&full_replace=true' \
  -H 'Content-Type: application/json' \
  -d '[
    {"@type":"@context","@base":"terminusdb:///data/","@schema":"terminusdb:///schema#","@metadata":{"terminusdb":{"options":["store_indices"]}}},
    {"@id":"Character","@type":"Class","@key":{"@type":"Lexical","@fields":["name"]},"name":"xsd:string","bio":"xsd:string","@metadata":{"embedding":{"query":"query($id: ID){ Character(id: $id) { name bio } }","template":"{{name}}. {{bio}}"}}},
    {"@id":"Species","@type":"Class","@key":{"@type":"Lexical","@fields":["name"]},"name":"xsd:string","description":"xsd:string","@metadata":{"embedding":{"query":"query($id: ID){ Species(id: $id) { name description } }","template":"{{name}}. {{description}}"}}}
  ]'
# ["Character", "Species"]
```

Now insert a document on the branch — Boba Fett, who only exists on `experiment`:

```bash
curl -u admin:root -X POST \
  'http://localhost:6365/api/document/admin/star_wars/local/branch/experiment?author=admin&message=Add+Boba+Fett' \
  -H 'Content-Type: application/json' \
  -d '[{"@type": "Character", "name": "Boba Fett", "bio": "A feared bounty hunter in Mandalorian armour."}]'
# ["Character/Boba%20Fett"]
```

The `post_commit_hook` fires on the branch automatically. The engine forks the branch from `main`'s latest indexed version, sharing its stored vectors — Yoda, Luke, and Mon Calamari are inherited without re-embedding. Only Boba Fett (the delta) is embedded.

Check the branch indexing status:

```bash
curl -u admin:root 'http://localhost:6365/api/index/admin/star_wars/local/branch/experiment'
# {
#   "branch": "experiment",
#   "status": "completed",
#   "last_indexed_commit": "6ij9pq212hha38o0hcc1v1r3wnhpiyq",
#   "engine": { "searchable_documents": 4, "text_segments_indexed": 4, ... },
#   "error": null
# }
```

Now observe the three key properties of branch indexing.

**Block reuse** — the branch sees main's documents (Yoda, Luke, Mon Calamari) without re-indexing them:

```bash
curl -u admin:root 'http://localhost:6365/api/search/admin/star_wars/local/branch/experiment?q=wise+old+man&snippet=true'
# [
#   {
#     "id": "Character/Yoda",
#     "distance": 0.3717,
#     "chunk": { "snippet": "Yoda. A wise old Jedi master, small and green. Trained Jedi for over 800 years on Dagobah." }
#   }
# ]
```

**Branch-only documents** — Boba Fett exists only on the `experiment` branch:

```bash
curl -u admin:root 'http://localhost:6365/api/search/admin/star_wars/local/branch/experiment?q=bounty+hunter&snippet=true'
# [
#   {
#     "id": "Character/Boba%20Fett",
#     "distance": 0.3412,
#     "chunk": { "snippet": "Boba Fett. A feared bounty hunter in Mandalorian armour." }
#   }
# ]
```

**Branch isolation** — main is untouched by the branch. Boba Fett does not appear on `main`:

```bash
curl -u admin:root 'http://localhost:6365/api/search/admin/star_wars?q=bounty+hunter&snippet=true'
# []
```

The branch was created by sharing `main`'s stored vectors, not copying them. Only Boba Fett (the delta) was embedded. Appends on `experiment` never touch `main`.

## Branch from anywhere

A branch can fork from **any** commit, regardless of which branch originally indexed it. The engine resolves a commit's snapshot globally per domain, so a branch forked at commit `C` finds `C`'s vectors whether `C` was indexed on `main` or elsewhere. TerminusDB handles the parent resolution automatically — you just create the branch and start inserting documents.

## Staleness: searching a not-yet-indexed commit

Indexing is asynchronous, so search can lag the write head. If you search a commit that isn't indexed yet, the engine **serves the nearest indexed ancestor immediately** (never blocks) and tells you what it actually served via the **`TerminusDB-Data-Version`** response header.

To see this in action, first find the latest commit on `main` and save its ID:

```bash
HEAD_COMMIT=$(curl -s -u admin:root 'http://localhost:6365/api/log/admin/star_wars?count=1' | python3 -c "import sys,json; print(json.load(sys.stdin)[0]['identifier'])")
echo $HEAD_COMMIT
# o4x1v8xu2o0r8sl7dke8mkbdivmu39l
```

Now search at that commit with the `-D -` flag so curl prints response headers. The `TerminusDB-Data-Version` header tells you which commit the engine actually served:

```bash
curl -u admin:root -D - "http://localhost:6365/api/search/admin/star_wars/local/commit/${HEAD_COMMIT}?q=wise+old+man"
# HTTP/1.1 200 OK
# TerminusDB-Data-Version: commit:o4x1v8xu2o0r8sl7dke8mkbdivmu39l
# [ ... results ... ]
```

When the requested commit is already indexed, the served data-version matches what you asked for. But if you insert a new document and immediately search at the new HEAD — before the `post_commit_hook` finishes indexing — the engine serves the nearest indexed ancestor instead. The `TerminusDB-Data-Version` header will show a different commit than the one you requested, telling you the result is **stale**.

Because the served data-version differs from what you asked for, the caller can detect staleness and trigger a reindex to catch up:

```bash
curl -u admin:root -X POST 'http://localhost:6365/api/index/admin/star_wars'
# {"@type":"api:IndexResponse","api:status":"api:success"}
```

The engine never serves a descendant of the requested commit. It only serves the requested commit itself or a member of the supplied ancestor window — never the branch tip just because it happens to be the latest indexed commit. This prevents leaking newer data the requested snapshot never had.

If a branch has **no** indexed ancestor at all, search returns `404` with a `SearchNotIndexed` error. A background index is triggered automatically — retry shortly.

## Snapshot isolation in practice

The combination of per-commit snapshots and ancestor-only resolution gives you a guarantee: searching commit `C` returns results that reflect exactly the state of the world at commit `C`. Not more, not less. This is what makes Versioned Search safe for audit, reproducibility, and branch-isolated experimentation.

---

Next: [Entity Resolution](/docs/versioned-search-entity-resolution/).
