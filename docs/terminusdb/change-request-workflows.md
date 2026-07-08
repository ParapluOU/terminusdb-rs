---
tags:
  - explanation
  - collaboration
  - dfrnt-cloud
title: Change Request Workflows - TerminusDB Tour
nextjs:
  metadata:
    title: Change Request Workflows - TerminusDB Tour
    description: This page details how change request workflows function in TerminusDB to enable safe collaboration
    keywords: terminusdb, change request workflows - terminusdb tour
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/change-request-workflows/
media:
  - alt: Opening a change request
    caption: ""
    media_type: Image
    title: Opening a change request
    value: https://assets.terminusdb.com/docs/open-change-request.png
  - alt: In a change request
    caption: ""
    media_type: Image
    title: In a change request
    value: https://assets.terminusdb.com/docs/in-change-request.png
  - alt: TerminusDB change request screen
    caption: ""
    media_type: Image
    title: TerminusDB change request screen
    value: https://assets.terminusdb.com/docs/change-request-screen.png
  - alt: Change Request diff viewer
    caption: ""
    media_type: Image
    title: Change Request diff viewer
    value: https://assets.terminusdb.com/docs/cr-diff.png
  - alt: See messages in a change request for context
    caption: ""
    media_type: Image
    title: See messages in a change request for context
    value: https://assets.terminusdb.com/docs/cr-review-messages.png
  - alt: List of change requests for review
    caption: ""
    media_type: Image
    title: List of change requests for review
    value: https://assets.terminusdb.com/docs/cr-review-home.png
  - alt: Change request screen
    caption: ""
    media_type: Image
    title: Change request screen
    value: https://assets.terminusdb.com/docs/cr-review-page.png
---

{% callout type="warning" title="Unmaintained feature" %}
This feature is not actively maintained. The documentation is preserved for historical reference. Community contributions and pull requests are welcome, but no active development is planned.
{% /callout %}

The change request workflow enables safe collaboration by ensuring that changes to data and content are reviewed before being applied. It can be run locally with TerminusDB.

## Creating a Change Request

When someone goes to make a change to content and data, TerminusDB automatically opens a change request. This is a branch of the database that does not impact main. Users can make one or many changes within the change request -

![Opening a change request](https://assets.terminusdb.com/docs/open-change-request.png)

## In a Change Request

Users will see when they are in a change request. A banner with various options is included at the top of the page and there is also a notice on the left informing the user what they are connected to -

![In a change request](https://assets.terminusdb.com/docs/in-change-request.png)

> It is important to submit edits or additions in a change request. Without hitting the submit button, changes will not be saved to the change request.

When in a change request a user has three options -

1.  Continue to make edits, deletions, and additions to the change request.
2.  Exit the change request - This leaves the change request open and available to come back to at a later stage.
3.  Submit the change request for review.

## Change Request Admin

Change requests are managed from the change request screen.

![TerminusDB change request screen](https://assets.terminusdb.com/docs/change-request-screen.png)

The screen has four tabs -

1.  Open - Open change requests that can be continued with or submitted for review.
2.  Review - Change requests that have been submitted for review.
3.  Merged - Previous approved commits to the database.
4.  Rejected - Rejected change requests.

## Review Change Requests

On the change request screen, select 'review'. Change requests that users have submitted for review are listed in chronological order.

![Change Request diff viewer](https://assets.terminusdb.com/docs/cr-diff.png)

_Change requests feature a diff viewer to see what's changed_

![See messages in a change request for context](https://assets.terminusdb.com/docs/cr-review-messages.png)

_Write messages to add more context_

![List of change requests for review](https://assets.terminusdb.com/docs/cr-review-home.png)

![Change request screen](https://assets.terminusdb.com/docs/cr-review-page.png)

To review a change request, do the following -

*   Click the 'review' button for the corresponding change request.
*   The next screen has all of the details of the change request with two tabs -

*   The first is the diff view with drop-down options to display the changes that have been made
*   The second is the messages tab, this displays the messages entered when creating and submitting the change request and can provide context.

*   Users can either accept or reject a change request and leave a message explaining their reasoning behind each
*   Accepted change requests will move the change request into the merged tab - users are able to view the diff to see the changes of past commits
*   Rejected change requests move into the rejected tab on the change request home screen

## Conflicts & Collaboration

In order to avoid changes being squashed by other users when multiple people make changes to data and content, TerminusDB checks the database to see if there have been changes made before a user reviews and merges a change request.

In order to proceed. The user must rebase their change request to incorporate the latest changes into their own change request. A prompt tells the user what to do.