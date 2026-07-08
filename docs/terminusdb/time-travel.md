---
tags:
  - how-to
  - version-control
  - dashboard
  - intermediate
title: Time Travel using the DFRNT TerminusDB cloud dashboard
nextjs:
  metadata:
    title: Time Travel using the DFRNT TerminusDB cloud dashboard
    description: A guide to show how to time travel to any previous commit using the DFRNT TerminusDB cloud dashboard.
    keywords: terminusdb, branch, commit, dashboard, dfrnt hub, git for data, history, point in time
    openGraph:
      images: https://assets.terminusdb.com/docs/time-travel.png
    alternates:
      canonical: https://terminusdb.org/docs/time-travel/
media:
  - alt: Time travel to any previous commit using the DFRNT TerminusDB cloud dashboard
    caption: ""
    media_type: Image
    title: Time travel to any previous commit using the DFRNT TerminusDB cloud dashboard
    value: https://assets.terminusdb.com/docs/time-travel.png
  - alt: See what data used to look like in a previou commit
    caption: ""
    media_type: Image
    title: See what data used to look like in a previou commit
    value: https://assets.terminusdb.com/docs/travel-back-in-time.png
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- A database with commit history
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to query your database at any previous point in time.
{% /callout %}

It is possible in the DFRNT TerminusDB cloud dashboard to time travel to any previous commit to examine the data and schema. This is particularly useful if you want to see what something looked like at a particular date, or if something has broken and you want to see when the last stable state was so you can revert back to it.

Time travel is each to do. From any screen when viewing a project (apart from the project home page), you will see a `stopwatch symbol` in the top bar.

Select this and a panel will appear from the right.

![Time travel to any previous commit using the DFRNT TerminusDB cloud  dashboard](https://assets.terminusdb.com/docs/time-travel.png)

Each commit in the list features the comments from the merged change request.

Travel back in time by selecting the button next to the commit you want to go back to.

![See what data used to look like in a previou commit](https://assets.terminusdb.com/docs/travel-back-in-time.png)

The dashboard informs you that you are not on the latest version.

You can then go an inspect the data in your project and the schema to see what's changed.