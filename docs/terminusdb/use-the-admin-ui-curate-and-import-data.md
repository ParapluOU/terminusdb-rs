---
tags:
  - how-to
  - documents
  - dashboard
  - dfrnt-cloud
title: Use the DFRNT TerminusDB cloud Dashboard to Curate Data & Content
nextjs:
  metadata:
    title: Use the DFRNT TerminusDB cloud Dashboard to Curate Data & Content
    description: A how-to guide describing how to use the DFRNT TerminusDB cloud dashboard to add, edit, and delete content and data
    keywords: terminusdb, csv, dashboard, data loading, dfrnt hub, document database, documents, import
    openGraph:
      images: https://assets.terminusdb.com/docs/document-explorer-home.png
    alternates:
      canonical: https://terminusdb.org/docs/use-the-admin-ui-curate-and-import-data/
media:
  - alt: TerminusDB document explorer to edit content via the UI
    caption: ""
    media_type: Image
    title: TerminusDB document explorer to edit content via the UI
    value: https://assets.terminusdb.com/docs/document-explorer-home.png
  - alt: Filtering documents based on its properties in the document explorer section of the TerminusDB cloud dashboard
    caption: ""
    media_type: Image
    title: Filtering documents based on its properties in the document explorer section of the TerminusDB cloud dashboard
    value: https://assets.terminusdb.com/docs/document-explorer-filter-view.png
  - alt: create a new change request to make changes to content.
    caption: ""
    media_type: Image
    title: create a new change request to make changes to content.
    value: https://assets.terminusdb.com/docs/create-a-new-change-request.png
  - alt: Manage change requests in the TerminusDB cloud dashboard
    caption: ""
    media_type: Image
    title: Manage change requests in the TerminusDB cloud dashboard
    value: https://assets.terminusdb.com/docs/change-request-screen-open.png
  - alt: Change requests ready for review are listed in chronological order
    caption: ""
    media_type: Image
    title: Change requests ready for review are listed in chronological order
    value: https://assets.terminusdb.com/docs/change-request-waiting-for-review.png
  - alt: accept or reject a change request and leave a comment to explain why.
    caption: ""
    media_type: Image
    title: accept or reject a change request and leave a comment to explain why.
    value: https://assets.terminusdb.com/docs/change-request-review-comment.png
  - alt: change requests feature a diff viewer to make review processes quicker.
    caption: ""
    media_type: Image
    title: change requests feature a diff viewer to make review processes quicker.
    value: https://assets.terminusdb.com/docs/change-request-review-diff.png
  - alt: A message to update the change request to the latest database.
    caption: ""
    media_type: Image
    title: A message to update the change request to the latest database.
    value: https://assets.terminusdb.com/docs/change-request-out-of-date-message.png
  - alt: Filtering documents based on its properties in the document explorer section of the TerminusDB cloud dashboard
    caption: ""
    media_type: Image
    title: Filtering documents based on its properties in the document explorer section of the TerminusDB cloud dashboard
    value: https://assets.terminusdb.com/docs/document-explorer-filter-view.png
  - alt: Edit or delete a document from within TerminusDB's Document Explorer
    caption: ""
    media_type: Image
    title: Edit or delete a document from within TerminusDB's Document Explorer
    value: https://assets.terminusdb.com/docs/document-explorer-edit-or-delete.png
---

{% callout type="note" %}
**Prerequisites**
- A DFRNT Hub account
- A data product with a schema defined
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have imported and curated data using the admin UI.
{% /callout %}

{% callout type="note" %}
**TerminusCMS is now DFRNT Hub**
TerminusCMS has been renamed to **DFRNT Hub**. All features described on this page are available at [dfrnt.com](https://dfrnt.com/hypergraph-content-studio/). The TerminusDB open source database remains unchanged.
{% /callout %}


The TerminusDB cloud dashboard features a data and content editing section called the Document Explorer. It is here where users can add, edit and delete content and data directly into the backend.

In this example, we've cloned the [Star Wars demo database](/docs/explore-a-real-dataset/).

First, select the Star Wars project (or an existing project of your own) and navigate to the document explorer section by selecting the third icon on the left, the document with a tick.

![TerminusDB document explorer to edit content via the UI](https://assets.terminusdb.com/docs/document-explorer-home.png)

The screen displays a list of documents that are included within the project's schema and the number of documents within the database. It also includes a list of the documents on the left with a `+` symbol to add a new one.

## View and filter documents

Click on film (from the left or on the main screen). This will produce a list of the Star Wars films.

If it is a long list, you can use the filter to narrow down the results. In this example, we have added a rule to filter the films using the director property of the document specifying it equals _George Lucas_.

![Filtering documents based on its properties in the document explorer section of the TerminusDB dashboard](https://assets.terminusdb.com/docs/document-explorer-filter-view.png)

Depending on the property you filter by, you can use these filter parameters -

*   Equals
*   Does not equal
*   Starts with
*   Contains
*   Greater than
*   Greater than or equal to
*   Less than
*   Less than or equal to

You can also group filters to combine multiple properties.

There is also a GraphQL query tab that displays how the document can be queried and the structure of any filters applied.

To view a document, click on it from the table view. This will open up that document where you can edit it.

Editing, adding, and deleting content and data will first create a change request.

## Change Requests

To edit, add, or delete a document you must first create a change request. A change request creates a branch of the database at that moment and lets you make any changes to this branch, safely away from live data. This process happens automatically. When selecting to edit, add, or delete a document you will be prompted to open a change request.

In this example, we're going to add Even Piell, a character in The Phantom Menace, to the People documents. We choose either the plus next to `People` on the left, or select `Add new People` from the top right.

You will then be prompted with a popup.

![create a new change request to make changes to content.](https://assets.terminusdb.com/docs/create-a-new-change-request.png)

Give the change request and title and description. If you're working with others, provide a good title and description so they have some context if they review the request.

Select `Start a change request`. You'll then be in the change request branch to make changes.

The screen looks like this now. You will notice on the left, and in the top bar, there is a notice that informs you that you're in a change request with the title given. We have expanded the top bar using the dropdown arrow to show the options available. You can -

*   Exit the change request - Come back at a later date to pick up your changes.
*   Ready for review - This sends the change request for review to either be accepted and merged, or rejected and deleted. _Make sure you are finished with your changes before sending it for review_.

For this how-to guide's flow, we'll skip adding the document here and continue it in the next section. We will continue with the change request and hypothetically make some changes to show you how change requests work.

### Change Request Home

For open change requests, ones sent for review, and approved or rejected change requests, you need to navigate to the change request section by clicking the last icon on the left, with the merge symbol.

![Manage change requests in the TerminusDB dashboard](https://assets.terminusdb.com/docs/change-request-screen-open.png)

There are four tabs on this screen:

*   **Open:** These are change requests that have been exited rather than submitted for review. You can click `keep editing` to carry on with your changes or `submit it for review`.
*   **Review:** Change requests submitted for review are added here.
*   **Merged:** Accepted and approved change requests go here. On this screen, you can see past change requests and view the details to see what changed.
*   **Rejected:** Change requests that have been rejected.

### Reviewing Change Requests

A user has added some documents and made some changes and submitted the change request for review.

![Change requests ready for review are listed in chronological order](https://assets.terminusdb.com/docs/change-request-waiting-for-review.png)

The requests are listed in chronological order. To review the change request, select the `Review` button.

The review screen will tell you who has made the change request and allow you to add notes and accept or reject the change request.

![accept or reject a change request and leave a comment to explain why.](https://assets.terminusdb.com/docs/change-request-review-comment.png)

Further below the comments section, there is a diff view to display all of the changes that have been made to make reviewing faster and easier.

![change requests feature a diff viewer to make review processes quicker.](https://assets.terminusdb.com/docs/change-request-review-diff.png)

To accept or reject the change request. Press the corresponding button.

### Conflict Checking

In some cases when a change request has been opened and worked on, other users may have merged change requests. This results in your change request becoming out-of-date as it was a snapshot taken before those previously merged change requests.

TerminusDB checks the commit history when you begin to review a change request to ensure it includes the latest data. If it is out-of-date it flags a message and prompts you to update it to ensure no past changes are stomped.

![A message to update the change request to the latest database.](https://assets.terminusdb.com/docs/change-request-out-of-date-message.png)

## Adding, Editing, & Deleting Documents

Adding, editing, and deleting documents is straightforward.

### Adding a document

Navigate to the Document Explorer by clicking on the document with a tick icon on the left.

![Filtering documents based on its properties in the document explorer section of the TerminusDB dashboard](https://assets.terminusdb.com/docs/document-explorer-filter-view.png)

Either -

1.  Click the plus symbol next to the document you wish to add from the left, or
2.  Click into a document and on the top right, click the `Add New Document` button.

Create a new change request, see the section above for details.

Go ahead and fill in the form. The fields of the form will provide detail about what format is required. Any field with an asterisk denotes that it is mandatory.

Submit your changes by pressing the `Submit` button.

_You will need to submit and review the change request for the changes to be applied_.

### Editing

Let's assume you're in a change request.

Find the document you want to edit and click on it.

![Edit or delete a document from within TerminusDB's Document Explorer](https://assets.terminusdb.com/docs/document-explorer-edit-or-delete.png)

Press the `Edit` button.

Make your changes.

Ensure to press `Submit` to commit the changes to the change request.

_You will need to submit and review the change request for the changes to be applied_.

### Deleting

Find the document you want to delete and click on it.

Select the `red bin icon`.

Confirm you wish to delete the document.

_You will need to submit and review the change request for the changes to be applied_.
