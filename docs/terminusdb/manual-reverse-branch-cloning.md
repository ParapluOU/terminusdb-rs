---
tags:
  - how-to
  - version-control
  - advanced
title: Manual reverse branch cloning
nextjs:
  metadata:
    title: Manual reverse branch cloning
    description: How to manually clone branches between data products
    keywords: terminusdb, branch, clone, commit, git for data, manual reverse branch cloning, version control
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/manual-reverse-branch-cloning/
media: []
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally
- Two databases or branches to synchronise
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have cloned data between branches using manual reverse-clone techniques.
{% /callout %}

## Reverse Branch Cloning

Reverse branch clonings is useful when the basic clone command can't be used, such as when the source TerminusDB instance is in a location that a cloud instance can't connect to. Another reason could be that just a branch should be moved between two data products and not the entire data product.

In most circumstances, the `clone` command can be used to move a data product branch from a cloud TerminusDB instance to a localhost instance, or from a cloud instance to another cloud instance or team.

How to reverse clone a data product branch from a localhost instance to a cloud instance, is described here. The baseic procedure goes like this:

1. If the cloud data product does not exist, create a new empty one
1. Setup a remote in the localhost data product from the cloud instance
1. Fetch the remote data product from the cloud instance
1. Push the relevant branch, such as `main` from localhost to the remote data product `main` branch
1. Move any additional branches that should be moved
1. Copy title, description and other attributes of the data product to the cloud version. 

Read more about [git-for-data](/docs/git-for-data-reference/) for more information on how to use TerminusDB and git-for-data.
