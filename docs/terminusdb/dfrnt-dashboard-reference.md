---
tags:
  - reference
  - dashboard
  - dfrnt-cloud
title: TerminusDB Dashboard Reference Guide
nextjs:
  metadata:
    title: TerminusDB Dashboard Reference Guide
    description: A conceptual overview of the TerminusDB dashboard describing its features and their purpose.
    keywords: terminusdb, dashboard, dfrnt hub, terminuscms, terminusdb dashboard reference guide
    openGraph:
      images: https://assets.terminusdb.com/docs/document-explorer-home.png
    alternates:
      canonical: https://terminusdb.org/docs/dfrnt-dashboard-reference/
media:
  - alt: How teams, users and projects are structured.
    caption: ""
    media_type: Image
    title: How teams, users and projects are structured.
    value: https://assets.terminusdb.com/docs/teams-users-and-projects2.png
---

This page documents the DFRNT Hub cloud dashboard — the web interface for managing teams, projects, schemas, and data. Use it to model schemas visually, curate content, and administer access without writing code.

{% callout type="warning" title="Built-in Dashboard Discontinued" %}
As of TerminusDB v11.2, the built-in dashboard component has been discontinued. This page documents the cloud dashboard interface. For information about the discontinued built-in dashboard, see the [Dashboard Status page](/docs/dashboard/).
{% /callout %}

## Overview

The DFRNT Hub cloud dashboard provides:

*   Manage your teams and collaborators
*   Manage your data products/projects
*   Model schema or edit in code
*   Manual data and content curation
*   Test and build queries
*   Collaborate on other user's TerminusDB projects
*   Manage, review, and accept or reject change requests.

For details about how to achieve any of the list above, please take a look at the [getting started guide](/docs/get-started/).

TerminusDB is structured in such a way -

![How teams, users and projects are structured.](https://assets.terminusdb.com/docs/teams-users-and-projects2.png)

When you sign up for DFRNT TerminusDB cloud, it automatically generates a team for you based on your login credentials. You will be an admin user for any of your teams.

You can then invite collaborators to your team. The permissions you grant them for the team will be applied to all projects within that team.

Projects/data products created within that team will be available to you and any collaborators invited.

You can have several teams with different projects and collaborators.

## Managing Teams & Collaborators

### Teams

Teams are a way to group your projects and team members. For example you may have different teams in your organization looking after different functions, so you could have teams for Personnel, Marketing, Finance, and Operations for example. These teams would include the relevant people and projects.

[Learn how to create a team here](/docs/create-a-team/).

### Collaborators/Users

Collaborators are invited by the team admin to work with the projects within that team. Invites are sent via email. The email contains a link that takes that user to the dashboard sign up page. Upon signing up or logging in, the user can accept or reject the invitation. Once accepted that user will be able to work with the project in the team with the permissions granted by the admin.

### Permissions

There are [five permission levels](/docs/invite-users/) for users ranging from admin down to info reader. The team permissions for a user are inherited by the projects within that team.

Individual project permissions can be granted in the team management section. Project-specific permissions can only be higher than team permissions. For example, if you want a user to be a collaborator for one project, set their team privilege to a lower permission level such as info-reader or data-reader.

## Managing Projects/Data Products

When you select a team you can create new projects or manage existing ones by selecting a project from the list on the left. Once a project is created or selected you will be directed to the project/data product management screen. Here you can clone, branch, reset and squash main or any branches, and delete the project.

## Model Schema

The second icon with three circles on the left takes you to the screen to model and build the project schemas. The screen has two tabs, one for building the schema with a UI, the other for constructing it in JSON.

For more informaiton about schema modeling, visit the [how-to model schema guide](/docs/model-schema/).

## Manual Data Entry & Content Curation

The third icon, the document with a tick, takes you to the document curation section. On the left and in the main section of the screen it will list all of the documents within the project schema. Here you can click through to view, sort, and filter existing documents. Or add, edit, and delete documents.

TerminusDB automatically constructs document frames from the schema. These frames are rendered as forms in the dashboard and all users to add and edit data and content directly into the backend.

TerminusDB comes with change requests workflows for data and content curation.

### TerminusDB Change Request Workflows

Change request workflows in TerminusDB are an automated process. When a user tries to edit, delete, or add a document they are prompted to create a change request by giving it a name and description. The change request creates a new branch of the data where the user can make changes away from main. They can exit the change request and pick it up later, or submit it for review. A review can accept and merge the change request, or reject and delete it.

To ensure that other changes don't get stamped the change request workflows check the database to see if it has changed since the branch was created, and if so, update the change request with the latest changes prior to it being reviewed and merged.

## Query Playgrounds

The TerminusDB dashboards come with WOQL and GraphQL query playgrounds.

### WOQL Query Playground

WOQL is a Datalog query language. More information about WOQL can be found in the [WOQL how-to guide](/docs/woql-basics/). The query language is based on two rules, tripples and unification. This blog post explains [WOQL's methodology](https://terminusdb.org/blog/the-power-of-web-object-query-language/).

The playground features query panels to write your schema. You can have several panels open to tweak your query. To aid you in building your query, listed on the left are the documents within the schema, you can select one which expands to show the properties of each document. Clicking on one will add create a new query panel with a WOQL query for that document property.

### GraphQL Query Playground

GraphQL is typically an API language, but TerminusDB has incorporated some of WOQLs features to allow you to perform graph queries using GraphQL. The playground features a query input area and a results panel. The playground is tab based and you can have many tabs open to experiment and tweak queries.

TerminusDB automatically generates the GraphQL schema from the project's schema. This enables features such as autofill and a dropdown of classes and properties.

For more information about querying with GraphQL, visit the [GraphQL Query how-to guide](/docs/graphql-basics/).