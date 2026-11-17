---
tags:
  - how-to
  - access-control
  - beginner
title: How to Use the Admin Console
nextjs:
  metadata:
    title: How to Use the Admin Console — TerminusDB Administration Guide
    description: Step-by-step guide to managing teams, users, branches, grants, and data products using the TerminusDB Admin Console (TDB Admin) that ships with TerminusDB Community v12.0.6+.
    keywords: terminusdb admin console, tdb admin, manage teams, manage users, grants permissions, data products, branch management, terminusdb gui
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/admin-console/
media:
  - /images/admin-console/01-login.png
  - /images/admin-console/02-team-membership.png
  - /images/admin-console/03-branches.png
  - /images/admin-console/04-grants.png
  - /images/admin-console/05-data-products.png
lastUpdated: "2026-05-19"
---

{% callout type="note" title="Release Candidate — TerminusDB 12.1" %}
This page documents functionality in the upcoming TerminusDB 12.1 release. Details may change before the final release.
{% /callout %}

{% callout type="note" title="What you'll achieve" %}
By the end of this guide, you will know how to sign in to the Admin Console and manage teams, users, branches, permissions, and data products through a web interface — no command line required.
{% /callout %}

The Admin Console (TDB Admin) is a web-based administration interface bundled with TerminusDB Community Edition. It provides a graphical way to manage your TerminusDB instance: organise users into teams, control access with role-based grants, create and manage data products (databases), and work with branches.

{% callout type="warning" title="Prerequisites" %}
- **TerminusDB Community v12.0.6 or later** — the Admin Console ships by default with this version. If you are running an earlier version, [upgrade your installation](/docs/install-terminusdb-as-a-docker-container/).
- The Admin Console is available at your TerminusDB host on the `/dashboard/` path (e.g. `http://localhost:6363/dashboard/`).
- You need the `admin` username and the password you set when starting TerminusDB.
{% /callout %}

## Sign in

1. Open your browser and navigate to the Admin Console URL — by default, `http://localhost:6363/dashboard/`.
2. Enter your **Username** (typically `admin` for the initial setup).
3. Enter your **Password** (the password configured when you started TerminusDB).
4. Click **Sign in**.

![The TDB Admin login screen showing username and password fields](/images/admin-console/01-login.png)

{% callout type="note" title="Default credentials" %}
If you started TerminusDB with Docker using the default configuration, the username is `admin` and the password is `root`. Change this immediately in production — see [Password Reset](#reset-a-users-password) below.
{% /callout %}

## Manage teams and members

Teams are the primary way to organise users and control which data products they can access. Every user belongs to at least one team, and each team has its own set of data products.

### Create a team

1. Click **Teams** in the left sidebar.
2. Click **Create Team** in the top-right corner.
3. Enter a team name (e.g. `Engineering`) and confirm.

### View and manage team members

1. Click **Teams** in the left sidebar.
2. Click a team name to view its members.
3. Each member row shows the username, their assigned role, and a **Remove** action.

![Team membership view showing users with their assigned roles](/images/admin-console/02-team-membership.png)

### Add a member to a team

1. Navigate to the team page.
2. Click **Grant Permissions** in the top-right corner.
3. Select the user, choose a role (e.g. Admin Role or Consumer Role), and confirm.

### Remove a member from a team

1. Navigate to the team page.
2. Click **Remove** next to the member you want to remove.
3. Confirm the removal.

## Manage data products

Data products are TerminusDB databases, grouped by team. Each data product has a name, label, description, and one or more branches.

### View data products

1. Click **Data Products** in the left sidebar.
2. Data products are listed grouped by team. Each entry shows the database name, label, and description.

![Data products view showing databases grouped by team](/images/admin-console/05-data-products.png)

### Create a data product

1. Click **Data Products** in the left sidebar.
2. Click **New Data Product** in the top-right corner.
3. Select the owning team, enter a name, label, and description.
4. Click **Create**.

### Delete a data product

1. Navigate to **Data Products**.
2. Click **Delete** next to the data product you want to remove.
3. Confirm the deletion.

{% callout type="warning" title="Destructive action" %}
Deleting a data product permanently removes the database and all its branches, commits, and data. This action cannot be undone.
{% /callout %}

## Manage branches

Branches let you work on data changes in isolation before merging them back. The Admin Console shows all branches for a data product and lets you create or delete them.

### View branches

1. Click **Data Products** in the left sidebar.
2. Click a data product name to open it.
3. The **Branches** section lists all branches. The default branch (`main`) is marked with a badge and cannot be deleted.

![Branch management view for a data product showing main and feature branches](/images/admin-console/03-branches.png)

### Create a branch

1. Open a data product.
2. Click **Create Branch** in the top-right corner.
3. Enter a branch name and confirm. The new branch is created from the current state of `main`.

### Delete a branch

1. Open a data product.
2. Click **Delete** next to the branch you want to remove.
3. Confirm the deletion. The `main` branch cannot be deleted.

## Manage grants and permissions

Grants control what users can do within your TerminusDB instance. A grant connects three things: a **user**, a **role** (which defines capabilities), and a **resource** (the team or data product the role applies to).

### View grants

1. Click **Grants** in the left sidebar.
2. Use the tabs to switch between **Team Grants** and **Data Product Grants**.
3. Use the **Team** dropdown to filter grants by team.

![Grants view showing user-role-resource assignments with team and data product tabs](/images/admin-console/04-grants.png)

### Grant access

1. Click **Grants** in the left sidebar.
2. Click **Grant Access** in the top-right corner.
3. Select the user, role, and resource (team or data product).
4. Confirm the grant.

### Revoke access

1. Navigate to **Grants**.
2. Click **Revoke** next to the grant you want to remove.
3. Confirm the revocation.

{% callout type="note" title="Built-in roles" %}
TerminusDB ships with two default roles:
- **Admin Role** — full read/write access, can manage users and grants.
- **Consumer Role** — read-only access to data products within the granted scope.

You can create custom roles via the **Roles** section in the sidebar or the [Capabilities API](/docs/capabilities-api-modes/).
{% /callout %}

## Reset a user's password

1. Click **Password Reset** in the left sidebar.
2. Select or type the username.
3. Enter the new password and confirm.

{% callout type="warning" title="Change the default password" %}
If you are running TerminusDB in production, change the default `admin` password immediately after first sign-in. Anyone with the admin password has full control of your instance.
{% /callout %}

## Manage users

### Create a user

1. Click **Users** in the left sidebar.
2. Click **Create User**.
3. Enter the username and password, then confirm.

### Delete a user

1. Click **Users** in the left sidebar.
2. Click **Delete** next to the user you want to remove.
3. Confirm the deletion.

## Next steps

- [Access control explanation](/docs/access-control/) — understand the TerminusDB permission model in depth.
- [Branch your database](/docs/branch-howto/) — work with branches using the HTTP API and client libraries.
- [Capabilities API modes](/docs/capabilities-api-modes/) — programmatic access to roles and grants.
- [Install TerminusDB](/docs/install-terminusdb-as-a-docker-container/) — set up a fresh TerminusDB instance with Docker.
