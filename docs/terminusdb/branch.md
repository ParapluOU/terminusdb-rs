---
tags:
  - how-to
  - version-control
  - dashboard
  - beginner
title: Branch a Project with the TerminusDB Dashboard
nextjs:
  metadata:
    title: Branch a Project with the TerminusDB Dashboard
    description: A guide to show how to branch projects using the TerminusDB dashboard.
    keywords: terminusdb, branch, branch a project with the terminusdb dashboard, commit, dashboard, dfrnt hub, git for data, terminuscms
    openGraph:
      images: https://assets.terminusdb.com/docs/branch-project.png
    alternates:
      canonical: https://terminusdb.org/docs/branch/
media:
  - alt: Branch the project
    caption: ""
    media_type: Image
    title: Branch the project
    value: https://assets.terminusdb.com/docs/branch-project.png
  - alt: Create a new project branch in TerminusDB
    caption: ""
    media_type: Image
    title: Create a new project branch in TerminusDB
    value: https://assets.terminusdb.com/docs/branch-new.png
  - alt: Branch Options
    caption: ""
    media_type: Image
    title: Branch Options
    value: https://assets.terminusdb.com/docs/branch-options.png
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- An existing database
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have created and managed branches in TerminusDB.
{% /callout %}

The DFRNT TerminusDB cloud dashboard enables you to branch projects and work with the data product in a bitemporal manner, including the revision history. To do this, choose the team and project you want to branch. You will be directed to the project home page. This is where you can branch it.

Scroll down to see the manage `branch` section.

![Branch the project](https://assets.terminusdb.com/docs/branch-project.png)

Each project can have one or more branches, the default is called main. Each branch contains a snapshot of the data as it was at the time of branching. This is useful for experimenting or providing data to other teams when you want to keep them away from main.

## Create a new branch

Click the `new branch` button.

![Create a new project branch in TerminusDB](https://assets.terminusdb.com/docs/branch-new.png)

Give the branch and ID.

You then have two choices:

1.  Branch from the current head to include all of the data
2.  Create an empty branch

Click `new branch` to create it.

You will then be switched to that branch.

## Swap between branches

From the manage branch section use the ellipsis symbol next to the branches to switch between branches and main.

![Branch Options](https://assets.terminusdb.com/docs/branch-options.png)