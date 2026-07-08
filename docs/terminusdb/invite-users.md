---
tags:
  - how-to
  - access-control
  - dfrnt-cloud
  - beginner
title: Invite Users to Your Team
nextjs:
  metadata:
    title: Invite Users to Your Team
    description: A how-to guide for inviting collaborators to your team using the TerminusDB dashboard.
    keywords: terminusdb, invite users to your team
    openGraph:
      images: https://assets.terminusdb.com/docs/manage-your-projects-add-a-new-user.png
    alternates:
      canonical: https://terminusdb.org/docs/invite-users/
media:
  - alt: add a new user to your team
    caption: ""
    media_type: Image
    title: add a new user to your team
    value: https://assets.terminusdb.com/docs/manage-your-projects-add-a-new-user.png
---

{% callout type="note" %}
**Prerequisites**
- A DFRNT Hub account with admin privileges
- A team already created
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have invited users to your DFRNT Hub team.
{% /callout %}

{% callout type="note" %}
**TerminusCMS is now DFRNT Hub**
TerminusCMS has been renamed to **DFRNT Hub**. All features described on this page are available at [dfrnt.com](https://dfrnt.com/hypergraph-content-studio/). The TerminusDB open source database remains unchanged.
{% /callout %}


To invite team members and manage the team, do the following:

1.  Click the arrow next to your profile icon in the top right corner.
2.  Select 'Team Members'.
3.  In the following screen, select 'Invite a Member'.
![add a new user to your team](https://assets.terminusdb.com/docs/manage-your-projects-add-a-new-user.png)5.  In the pop-up window, enter the user’s email address and select one of the following access permissions - \_this will be applied to team-level permissions so will apply to all data products within your team. If you want to only give read-write access to a specific data product, it makes sense to give the user low-level permissions and assign higher permissions for that data product only - we will explain this next

*   Admin - can add and remove users and permissions and has total access to data products.
*   Collaborator - Able to access data products.
*   Data Updater - Read and write access to data products.
*   Data Reader - Read-only access to data products.
*   Info Reader - Schema-level access but not data-level access.

7.  The user will be sent an email with a link they need to click (if they don’t receive it, tell them to check their spam folder).
8.  When the user has accepted the invitation, their details will display within the Team Members section.

## **Editing & Removing Users**

To edit the role given to a user:

1.  Navigate to the Team Members section from the profile dropdown.
2.  Find the user to change from the list and click on the second icon.
3.  From the pop-up window, select the new role to give them.

To delete a user from a team, do the same as above, but select the third icon in red.

> Hover over the icons for information about what they do.

## **Granular Permissions**

Grant different permissions for different data products/projects. To do this do the following:

1.  Navigate to the Team Members section from the profile dropdown.
2.  Find the user to add specific data product permissions for and click on the first icon.
3.  The resulting table below will list all of the data products in the team.
4.  Choose the data product to change the user’s role for by clicking on the edit permissions icon.
5.  Choose from the list of permissions

> Users can only have permissions higher than the team permissions, so if someone needs read/write permissions for only one data product, ensure that the team permissions are set at a lower level.

## Manage Access Control with the JS Client

[Access Control with the JavaScript Client, Reference Guide](/docs/js-access-control/)
