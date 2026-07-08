---
tags:
  - how-to
  - version-control
  - dashboard
  - intermediate
title: Squash Commits with the TerminusDB Dashboard
nextjs:
  metadata:
    title: Squash Commits with the TerminusDB Dashboard
    description: A guide to show how to squash the commits of a branch or main into one large commit using the TerminusDB dashboard.
    keywords: terminusdb, branch, clean history, commit, compress, dashboard, dfrnt hub, git for data
    openGraph:
      images: https://github.com/terminusdb/terminusdb-web-assets/blob/master/docs/squashed-branch.png?raw=true
    alternates:
      canonical: https://terminusdb.org/docs/squash/
media:
  - alt: Branch options with the ability to squash the branch of the database
    caption: ""
    media_type: Image
    title: Branch options with the ability to squash the branch of the database
    value: https://assets.terminusdb.com/docs/branch-options.png
  - alt: A squashed branch combines all commits into one big one
    caption: ""
    media_type: Image
    title: A squashed branch combines all commits into one big one
    value: https://assets.terminusdb.com/docs/squashed-branch.png
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- A database with multiple commits on a branch
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have squashed multiple commits into one.
{% /callout %}

To squash a branch of a database, or indeed main, navigate to the project home page, the first icon on the left that looks like a database.

Scroll down to the `Manage Branches` section and selected `Branches`.

Next to the branch you want to squash, select the ellipses symbol to see the branch options.

![Branch options with the ability to squash the branch of the database](https://assets.terminusdb.com/docs/branch-options.png)

Choose the `Squash` button.

Give the operation a description and press the `Squash Branch` button.

![A squashed branch combines all commits into one big one](https://assets.terminusdb.com/docs/squashed-branch.png)

> Be wary as squashing a project will result in the commit history being lost