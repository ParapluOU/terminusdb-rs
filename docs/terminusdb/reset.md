---
tags:
  - how-to
  - version-control
  - dashboard
  - beginner
title: Reset a Project with the TerminusDB Dashboard
nextjs:
  metadata:
    title: Reset a Project with the TerminusDB Dashboard
    description: A guide to show how to reset to a particular commit of a branch or main using the TerminusDB dashboard.
    keywords: terminusdb, branch, commit, dashboard, dfrnt hub, git for data, reset, reset a project with the terminusdb dashboard
    openGraph:
      images: https://github.com/terminusdb/terminusdb-web-assets/blob/master/docs/reset-a-branch.png?raw=true
    alternates:
      canonical: https://terminusdb.org/docs/reset/
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
    value: https://assets.terminusdb.com/docs/reset-a-branch.png
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- A database with commit history
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have reset a branch to a previous state.
{% /callout %}

To reset a branch of a database, or indeed main, navigate to the project home page, the first icon on the left that looks like a database.

Scroll down to the `Manage Branches` section and selected `Branches`.

Next to the branch you want to reset, select the ellipses symbol to see the branch options.

![Branch options with the ability to squash the branch of the database](https://assets.terminusdb.com/docs/branch-options.png)

Choose the `Reset` button.

Choose the commit you would like to reset to and copy the commit ID by selecting the clipboard icon. _You can inspect commits using the [time travel feature](/docs/time-travel/)._

Paste the commit ID and press the `Reset Branch` button.

![A squashed branch combines all commits into one big one](https://assets.terminusdb.com/docs/reset-a-branch.png)