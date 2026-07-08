---
title: Squash Commits on a Branch
nextjs:
  metadata:
    title: Squash Commits on a Branch
    description: How to squash multiple commits into one using the JavaScript client, Python client, or HTTP API.
    keywords: terminusdb, squash, compress commits, clean history, branch, javascript client, python client, http api, curl
    alternates:
      canonical: https://terminusdb.org/docs/squash-projects/
    openGraph:
      images: https://assets.terminusdb.com/docs/js-client-collaboration-squash.png
tags:
  - typescript
  - python
  - version-control
  - how-to
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally ([install guide](/docs/install-terminusdb-as-a-docker-container/))
- A branch with multiple commits to squash
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have squashed multiple commits on a branch into a single commit — producing a clean, readable history.
{% /callout %}

Squashing combines multiple commits into a single commit. This is useful for cleaning up history after a series of incremental changes, before merging a feature branch back to main.

{% prerequisites-clone /%}

## Squash a branch

{% code-tabs %}
{% code-tab label="TypeScript" %}
```typescript
const squashBranch = async () => {
  const branchName = "mybranch"
  const message = "Squash: combine all feature commits"
  await client.squashBranch(branchName, message)

  // Verify — should show a single commit
  const commits = await client.query(
    TerminusClient.WOQL.lib().commits("mybranch")
  )
  console.log("Commits after squash:", JSON.stringify(commits.bindings, null, 2))
}
```

After squashing, the branch has a single commit with your message:

```json
[
  {
    "Author": { "@type": "xsd:string", "@value": "admin" },
    "Commit ID": { "@type": "xsd:string", "@value": "vn7l94v4broiaz28346mdhwtgxvvy6p" },
    "Message": { "@type": "xsd:string", "@value": "Squash: combine all feature commits" },
    "Parent ID": null
  }
]
```
{% /code-tab %}
{% code-tab label="Python" %}
```python
client.branch = "mybranch"
message = "Squash: combine all feature commits"
result = client.squash(message)
```

The result contains the new commit path. Reset the branch HEAD to it:

```python
client.reset(result, use_path=True)
```

Verify:

```python
commits = client.log(count=5)
print("Commits after squash:", commits)
```
{% /code-tab %}
{% code-tab label="HTTP" %}
```bash
curl -u admin:root -X POST \
  "http://localhost:6363/api/squash/admin/tdb-example-mydb/local/branch/mybranch" \
  -H "Content-Type: application/json" \
  -d '{"commit_info": {"author": "admin", "message": "Squash: combine all feature commits"}}'
```
{% /code-tab %}
{% /code-tabs %}

{% callout type="note" %}
After squashing, the branch contains only the final state of your data in a single commit. The individual intermediate commits are no longer accessible on this branch.
{% /callout %}

## Next steps

- [Branch](/docs/branch-howto/) to create a feature branch before squashing main
- [Reset](/docs/reset-a-project/) to undo a squash if needed (before garbage collection)
- [Clone](/docs/clone-a-project/) to share your cleaned-up branch with collaborators
