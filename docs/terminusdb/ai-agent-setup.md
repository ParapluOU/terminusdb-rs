---
tags:
  - tutorial
  - beginner
  - self-hosted
  - vectorlink
title: Set Up TerminusDB with an AI Agent
nextjs:
  metadata:
    title: Set Up TerminusDB with an AI Agent
    description: Let Claude Code, Codex, or Cursor install TerminusDB for you — one prompt brings up the 12.1 release candidate with vector search, creates a database, and runs a first query.
    keywords: terminusdb ai agent setup, claude code terminusdb, docker compose terminusdb, vector search setup, local embeddings
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/ai-agent-setup/
media: []
lastUpdated: "2026-05-01"
---

TerminusDB is local-first: the database runs on your own machine, and with [Versioned Search](/docs/versioned-search-quickstart/) (12.1 release candidate) the embeddings can run on your own hardware too, with no API key.

If you use an AI coding agent such as Claude Code, Codex or Cursor, you can paste this prompt and let it do the setup for you. It uses the `docker-compose.yml` in the [vectorsearch folder](https://github.com/terminusdb/terminusdb/tree/main/vectorsearch) of the TerminusDB repository. That is the 12.1 release candidate, with built-in vector search and local embeddings (no API key). Read the steps before you run them, as the agent will run commands on your machine.

```text copyEvent="agent_prompt_copy"
Set up a local TerminusDB for me and check that it works. Use Docker Compose.

1. Make a new empty folder called terminusdb-local and work in it.
2. Download these three files from the terminusdb/terminusdb repository, main branch, folder vectorsearch:
   https://raw.githubusercontent.com/terminusdb/terminusdb/main/vectorsearch/docker-compose.yml
   https://raw.githubusercontent.com/terminusdb/terminusdb/main/vectorsearch/.env.example
   https://raw.githubusercontent.com/terminusdb/terminusdb/main/vectorsearch/README.md
3. Copy .env.example to .env. Set TERMINUSDB_ADMIN_PASS to a new random password.
   Leave OPENAI_KEY empty. Do not print the password in your answer.
4. Run: docker compose up -d
   Then run: docker compose ps
5. Wait until http://localhost:6363/api/info answers with HTTP 200 using basic auth
   admin:<password>. Try for up to 60 seconds.
   Then wait until the embeddings model is ready — docker compose logs embeddings-init
   should end with "Model ready." This can take a few minutes on first start.
6. Create a database:
   POST /api/db/admin/agent_demo
   body: {"label": "Agent demo", "comment": "Made by my AI agent"}
7. Add a tiny schema. The @metadata.embedding block is what makes documents
   searchable — the query fetches the fields, the template renders them into text:
   POST /api/document/admin/agent_demo?graph_type=schema&author=agent&message=Add+schema
   body: {"@type": "Class", "@id": "Person", "name": "xsd:string",
          "friends": {"@type": "Set", "@class": "Person"},
          "@metadata": {"embedding": {
            "query": "query($id: ID){ Person(id: $id) { name } }",
            "template": "{{name}}"}}}
8. Insert two documents:
   POST /api/document/admin/agent_demo?author=agent&message=Add+people
   body: [{"@type": "Person", "@id": "Person/ada", "name": "Ada", "friends": []},
          {"@type": "Person", "@id": "Person/alan", "name": "Alan", "friends": ["Person/ada"]}]
9. Run one query with GraphQL:
   POST /api/graphql/admin/agent_demo
   body: {"query": "{ Person { name friends { name } } }"}
10. Index and run one vector search:
    POST /api/index/admin/agent_demo   (empty body)
    then GET /api/search/admin/agent_demo?q=person+named+Ada&count=2
    Check the index status with GET /api/index/admin/agent_demo:
    searchable_documents should be 2, not 0.
11. Report back to me: which commands you ran, the HTTP status of each call,
    the GraphQL result, the search hits, and how to stop everything (docker compose down).
    If a step failed, show the error and stop. Do not guess.

Docs: https://terminusdb.com/docs/get-started/ and https://terminusdb.com/llms.txt
```

The 12.1 release candidate image reports `"version": "12.0.8"` in `/api/info`. That is expected. Step 10 (the manual index call) is optional. It makes the index ready before the first search. Without it, the first search starts the catch-up and may return no results at first.

{% callout type="note" title="Automatic embeddings need three things" %}
Automatic embeddings only work when:

- the server runs with `TERMINUSDB_INDEXER_BACKEND=vectorlink` (the vectorsearch `docker-compose.yml` sets this for you),
- the class has an `@metadata.embedding` block with a `query` and a `template`, as in step 7,
- and the documents were committed after both were in place. Documents that already exist are indexed on the first search. This can take a moment, so retry if the first search is empty. To do it in advance, run `POST /api/index/{org}/{db}` once.

See [How automatic indexing works](/docs/versioned-search-automatic-indexing/) for the details and a troubleshooting list.
{% /callout %}

Prefer to do it by hand? The [First 10 Minutes quickstart](/docs/get-started/) does the same kind of thing with a single `docker run` command.
