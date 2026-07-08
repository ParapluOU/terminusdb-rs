---
tags:
  - reference
  - access-control
  - curl
  - intermediate
title: Managing Users and Invitations in TerminusDB
nextjs:
  metadata:
    title: Managing Users and Invitations in TerminusDB
    description: Learn how to create organizations, invite users with specific roles, and manage authorization in TerminusDB using the REST API
    keywords: terminusdb, api, managing users and invitations in terminusdb
    alternates:
      canonical: https://terminusdb.org/docs/managing-users-and-invitations/
---

This guide demonstrates how to manage users, organizations, and authorization in TerminusDB. You'll learn how to invite users to organizations and data products with specific capabilities, creating a complete authorization hierarchy.

## Understanding TerminusDB Authorization

TerminusDB uses a **capability-based authorization system** with three core concepts:

- **Users**: Individual accounts that can authenticate with TerminusDB
- **Organizations**: Top-level containers that own data products
- **Roles**: Collections of actions (permissions) like `Role/admin` or `Role/consumer`
- **Capabilities**: Grant specific roles to users for a given scope (organization or data product)

### Key Roles

TerminusDB includes two default built-in roles:

**Role/admin** (referred to as `"Admin Role"` in API calls) - Full administrative access over a scope (database or organization) including:
- Create and delete databases (in an organization)
- Read and write schema, instance data, and metadata
- Manage capabilities (invite/remove users in an organization)
- Clone, fetch, push, branch, and rebase operations on a database

**Role/consumer** (referred to as `"Consumer Role"` in API calls) - Read-only access over a scope (database or organization) including:
- Read schema and instance data
- Read database metadata
- Clone and fetch operations on a database
- Class frame queries

### Understanding Access Levels

**Organization-level capabilities** grant:
- Ability to create databases
- Manage users and capabilities within the organization
- Access to organization metadata and databases

**Database-level capabilities** grant:
- Operational access: documents, queries (WOQL, GraphQL), schema
- Grants access to database metadata endpoints
- Scoped to a specific database only

For a comprehensive overview of TerminusDB's role-based access control model, see the [Access Control Tutorial](/docs/access-control-tutorial/).

## Prerequisites

Before starting, ensure you have:
- A running TerminusDB instance (default: `http://localhost:6363`) - see [Install TerminusDB as a Docker Container](/docs/install-terminusdb-as-a-docker-container/) for setup instructions
- Admin credentials (default: username `admin`, password `root`)
- `curl` and `jq` installed for making API requests

## Setup: Environment Variables

Let's set up environment variables for easier command execution:

```bash
# TerminusDB server configuration
export TERMINUSDB_URL="http://localhost:6363"
export ADMIN_USER="admin"
export ADMIN_PASS="root"

# Generate a unique suffix for this tutorial (avoids conflicts)
export TUTORIAL_ID=$(date +%s)

# Organization and user names
export ORG_NAME="acme_corp_${TUTORIAL_ID}"
export ORG_ADMIN_USER="alice_${TUTORIAL_ID}"
export ORG_ADMIN_PASS="alice_password_123"
export REGULAR_USER="bob_${TUTORIAL_ID}"
export REGULAR_PASS="bob_password_456"
export DB_USER="charlie_${TUTORIAL_ID}"
export DB_USER_PASS="charlie_password_789"
export DB_NAME="products"
```

## Part 1: Creating an Organization

Only system administrators can create organizations. Let's create an organization called "Acme Corp" using the admin account:

```bash
curl -X POST "${TERMINUSDB_URL}/api/organizations/${ORG_NAME}" \
  -u "${ADMIN_USER}:${ADMIN_PASS}" \
  -H "Content-Type: application/json" \
  -d '{}'
```

**Expected Response:**
```json
"terminusdb://system/data/Organization/acme_corp_1234567890"
```

This returns the organization's unique identifier. The organization exists but has no users yet.

### Verify Organization Creation

```bash
curl -s -X GET "${TERMINUSDB_URL}/api/organizations/${ORG_NAME}" \
  -u "${ADMIN_USER}:${ADMIN_PASS}" \
  -H "Content-Type: application/json" | jq
```

**Expected Response:**
```json
{
  "@id": "Organization/acme_corp_1234567890",
  "@type": "Organization",
  "name": "acme_corp_1234567890"
}
```

## Part 2: Creating Users

Now let's create three users who will have different roles in our organization:

### Create Alice (Future Organization Admin)

```bash
curl -X POST "${TERMINUSDB_URL}/api/users" \
  -u "${ADMIN_USER}:${ADMIN_PASS}" \
  -H "Content-Type: application/json" \
  -d "{
    \"name\": \"${ORG_ADMIN_USER}\",
    \"password\": \"${ORG_ADMIN_PASS}\"
  }"
```

**Expected Response:**
```json
"terminusdb://system/data/User/alice_1234567890"
```

### Create Bob (Future Organization Admin via Invitation)

```bash
curl -X POST "${TERMINUSDB_URL}/api/users" \
  -u "${ADMIN_USER}:${ADMIN_PASS}" \
  -H "Content-Type: application/json" \
  -d "{
    \"name\": \"${REGULAR_USER}\",
    \"password\": \"${REGULAR_PASS}\"
  }"
```

### Create Charlie (Future Data Product User)

```bash
curl -X POST "${TERMINUSDB_URL}/api/users" \
  -u "${ADMIN_USER}:${ADMIN_PASS}" \
  -H "Content-Type: application/json" \
  -d "{
    \"name\": \"${DB_USER}\",
    \"password\": \"${DB_USER_PASS}\"
  }"
```

### Verify Users Have No Capabilities Yet

Let's check Alice's current capabilities, try manually with the other roles (switch environment variable in the command):

```bash
curl -s -X GET "${TERMINUSDB_URL}/api/users/${ORG_ADMIN_USER}?capability=true" \
  -u "${ADMIN_USER}:${ADMIN_PASS}" \
  -H "Content-Type: application/json" | jq
```

**Expected Response:**
```json
{
  "@id": "User/alice_1234567890",
  "@type": "User",
  "name": "alice_1234567890",
  "capability": []
}
```

Notice the `capability` array is empty - Alice has no permissions yet.

## Part 3: Granting Organization Admin to Alice

As system admin, let's grant Alice administrative access to the Acme Corp organization. With this permission, Alice can manage the organization (also known as a "team" or "project"):

```bash
curl -X POST "${TERMINUSDB_URL}/api/capabilities" \
  -u "${ADMIN_USER}:${ADMIN_PASS}" \
  -H "Content-Type: application/json" \
  -d "{
    \"operation\": \"grant\",
    \"scope\": \"${ORG_NAME}\",
    \"user\": \"${ORG_ADMIN_USER}\",
    \"roles\": [\"Admin Role\"],
    \"scope_type\": \"organization\"
  }"
```

**Expected Response:**
```json
{
  "@type": "api:CapabilityResponse",
  "api:status": "api:success"
}
```

### Verify Alice's New Capabilities

```bash
curl -s -X GET "${TERMINUSDB_URL}/api/users/${ORG_ADMIN_USER}?capability=true" \
  -u "${ADMIN_USER}:${ADMIN_PASS}" \
  -H "Content-Type: application/json" | jq
```

**Expected Response:**
```json
{
  "@id": "User/alice_1234567890",
  "@type": "User",
  "name": "alice_1234567890",
  "capability": [
    {
      "@id": "Capability/...",
      "@type": "Capability",
      "role": [
        {
          "@id": "Admin Role",
          "@type": "Role",
          "name": "Admin Role",
          "action": [
            "create_database",
            "delete_database",
            "instance_read_access",
            "instance_write_access",
            "schema_read_access",
            "schema_write_access",
            "manage_capabilities",
            ...
          ]
        }
      ],
      "scope": {
        "@id": "Organization/acme_corp_1234567890",
        "@type": "Organization",
        "name": "acme_corp_1234567890"
      }
    }
  ]
}
```

Alice now has the `Admin Role` role for the organization, including the crucial `manage_capabilities` action that enables her to grant other users capabilities on the organization and their databases.

## Part 4: Alice Invites Bob as Organization Admin

Now Alice can use her organization admin privileges to invite Bob. Organization admins can grant capabilities using resource IDs (without needing system admin access):

```bash
curl -X POST "${TERMINUSDB_URL}/api/capabilities" \
  -u "${ORG_ADMIN_USER}:${ORG_ADMIN_PASS}" \
  -H "Content-Type: application/json" \
  -d "{
    \"operation\": \"grant\",
    \"scope\": \"Organization/${ORG_NAME}\",
    \"user\": \"User/${REGULAR_USER}\",
    \"roles\": [\"Role/admin\"]
  }"
```

**Key Points:**
- Alice authenticates using her credentials (not system admin)
- Uses resource IDs: `Organization/{name}`, `User/{name}`, `Role/admin`
- No `scope_type` parameter needed when using IDs
- This demonstrates delegated capability management

**Expected Response:**
```json
{
  "@type": "api:CapabilityResponse",
  "api:status": "api:success"
}
```

### Verify Bob's Capabilities

Alice can now verify Bob's capabilities using the organization-scoped endpoint (organization admins can query users about the capabilities they hold in their organization):

```bash
curl -s -X GET "${TERMINUSDB_URL}/api/organizations/${ORG_NAME}/users/${REGULAR_USER}" \
  -u "${ORG_ADMIN_USER}:${ORG_ADMIN_PASS}" | jq '.capability[0].role[0].name'
```

**Expected Response:**
```json
"Admin Role"
```

**Key Points:**
- Organization admins can query capabilities for users **within their organization** using `/api/organizations/{org}/users/{user}`
- The global `/api/users/{user}?capability=true` endpoint requires system admin access (reads from SystemDatabase)
- This allows Alice to manage and audit capabilities she has granted

Bob now has organization admin privileges, granted by Alice (not the system admin).

## Part 5: Bob Creates a Data Product (Database)

Now let's demonstrate that Bob can exercise his organization admin privileges by creating a database. Bob has the `create_database` permission from the Admin Role that Alice granted him:

```bash
curl -X POST "${TERMINUSDB_URL}/api/db/${ORG_NAME}/${DB_NAME}" \
  -u "${REGULAR_USER}:${REGULAR_PASS}" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "Products Database",
    "comment": "Database for product information",
    "schema": true
  }'
```

**Expected Response:**
```json
{
  "@type": "api:DbCreateResponse",
  "api:status": "api:success"
}
```

### Verify Database Creation

```bash
curl -s -X GET "${TERMINUSDB_URL}/api/db/${ORG_NAME}/${DB_NAME}" \
  -u "${ORG_ADMIN_USER}:${ORG_ADMIN_PASS}" \
  -H "Content-Type: application/json" | jq
```

## Part 6: Alice Invites Charlie to the Data Product

Now Alice will grant Charlie access to just the `products` database (not the entire organization) using the ID-based API.

### Get the Database Resource ID

First, Alice needs to get the database's resource ID:

```bash
DB_RESOURCE_ID=$(curl -s -X GET "${TERMINUSDB_URL}/api/db/${ORG_NAME}/${DB_NAME}?verbose=true" \
  -u "${ORG_ADMIN_USER}:${ORG_ADMIN_PASS}" | jq -r '."@id"')
echo "Database Resource ID: ${DB_RESOURCE_ID}"
```

**Example Output:**
```
Database Resource ID: UserDatabase/SsGIjJtbWy0tHzBf
```

### Grant Charlie Access Using Resource ID

Now use the resource ID to grant Charlie admin access to the database:

```bash
curl -X POST "${TERMINUSDB_URL}/api/capabilities" \
  -u "${ORG_ADMIN_USER}:${ORG_ADMIN_PASS}" \
  -H "Content-Type: application/json" \
  -d "{
    \"operation\": \"grant\",
    \"scope\": \"${DB_RESOURCE_ID}\",
    \"user\": \"User/${DB_USER}\",
    \"roles\": [\"Role/admin\"]
  }"
```

**Key Points:**
- Database resource IDs are hashes (e.g., `UserDatabase/SsGIjJtbWy0tHzBf`), not database names
- Get the resource ID from the database metadata endpoint with `?verbose=true`
- User format: `User/{username}` (not hashed)
- Role format: `Role/admin` (built-in role names)
- No `scope_type` parameter (only system admin can use that)

**Expected Response:**
```json
{
  "@type": "api:CapabilityResponse",
  "api:status": "api:success"
}
```

### Alice Audits Database Access

Alice can audit which users have access to databases in her organization by listing all organization users. This shows all capabilities including database-level grants:

```bash
curl -s -X GET "${TERMINUSDB_URL}/api/organizations/${ORG_NAME}/users" \
  -u "${ORG_ADMIN_USER}:${ORG_ADMIN_PASS}" | jq
```

**Expected Response:**
Alice can see all users and their capabilities. Charlie's entry will show database-level access:
```json
[
  {
    "@id": "User/charlie_...",
    "name": "charlie_...",
    "capability": [
      {
        "@id": "Capability/...",
        "scope": "UserDatabase/...",
        "role": [
          {
            "name": "Admin Role",
            "action": ["instance_read_access", "schema_read_access", ...]
          }
        ]
      }
    ]
  }
]
```

**Key Points:**
- Organization admins can see **all users and their capabilities** in the organization
- This includes both organization-level and database-level grants
- Allows auditing who has access to specific databases
- Can be filtered using `jq` to find users with access to a specific database

## Part 6.5: Creating Custom Roles

While TerminusDB provides built-in roles (`Admin Role` and `Consumer Role`), you can create custom roles tailored to specific use cases. Let's create a custom "Database Analyst" role for users who need read-only access plus metadata viewing.

### Create a Custom Role

Custom roles allow you to grant precisely the permissions your users need. Remember that creating roles require system administrator access:

```bash
curl -X POST "${TERMINUSDB_URL}/api/roles" \
  -u "${ADMIN_USER}:${ADMIN_PASS}" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Database Analyst",
    "action": [
      "instance_read_access",
      "schema_read_access",
      "meta_read_access",
      "commit_read_access",
      "class_frame"
    ]
  }'
```

**Expected Response:** Role ID
```json
"terminusdb://system/data/Role/Database%20Analyst"
```

### Understanding Role Permissions

The custom "Database Analyst" role includes:

- **`instance_read_access`**: Required to read documents and query instance data
- **`schema_read_access`**: Access to schema definitions  
- **`meta_read_access`**: Access to database metadata
- **`commit_read_access`**: View commit history
- **`class_frame`**: Query class structure

**Note:** The `/api/db/{org}/{db}` metadata endpoint checks for `instance_read_access` permission. However, at the database level (not organization level), capabilities grant **operational access** to documents and queries, but metadata endpoints typically require organization-level permissions for full access when users are not granted instance access to database contents.

### Bob Changes Charlie's Role

Now Bob exercises his organization admin privileges to change Charlie's access from Admin Role to the custom Database Analyst role, providing more restricted database-level access.

First, Bob gets the database resource ID:

```bash
DB_RESOURCE_ID=$(curl -s -X GET "${TERMINUSDB_URL}/api/db/${ORG_NAME}/${DB_NAME}?verbose=true" \
  -u "${REGULAR_USER}:${REGULAR_PASS}" | jq -r '."@id"')
echo "Database Resource ID: ${DB_RESOURCE_ID}"
```

Revoke Charlie's Admin Role on the database:

```bash
curl -X POST "${TERMINUSDB_URL}/api/capabilities" \
  -u "${REGULAR_USER}:${REGULAR_PASS}" \
  -H "Content-Type: application/json" \
  -d "{
    \"operation\": \"revoke\",
    \"scope\": \"${DB_RESOURCE_ID}\",
    \"user\": \"User/${DB_USER}\",
    \"roles\": [\"Role/admin\"]
  }"
```

Now grant the Database Analyst role to Charlie:

```bash
curl -X POST "${TERMINUSDB_URL}/api/capabilities" \
  -u "${REGULAR_USER}:${REGULAR_PASS}" \
  -H "Content-Type: application/json" \
  -d "{
    \"operation\": \"grant\",
    \"scope\": \"${DB_RESOURCE_ID}\",
    \"user\": \"User/${DB_USER}\",
    \"roles\": [\"Role/Database%20Analyst\"]
  }"
```

**Key Points:**
- Bob authenticates with his organization admin credentials
- Gets database resource ID first (databases use hashed IDs)
- Revokes Admin Role before granting Database Analyst (role transition)
- Uses ID-based format: `${DB_RESOURCE_ID}`, `User/{username}`, `Role/{rolename}`
- Role name is URL-encoded: `Database%20Analyst`
- No `scope_type` parameter needed
- Demonstrates Bob can manage database-level access

**Expected Response:**
```json
{
  "@type": "api:CapabilityResponse",
  "api:status": "api:success"
}
```

### Bob Verifies Custom Role Assignment

Bob can verify Charlie's new capability using the organization users endpoint:

```bash
curl -s -X GET "${TERMINUSDB_URL}/api/organizations/${ORG_NAME}/users/${DB_USER}" \
  -u "${REGULAR_USER}:${REGULAR_PASS}" | jq '.capability[] | {role: .role[].name, scope: .scope}'
```

**Expected Response:**
```json
{
  "role": "Database Analyst",
  "scope": "products"
}
```

### View All Available Roles

System administrators can list all roles (built-in and custom) in the system. Other users must know the specific role identifiers or names. The system administrators can list the roles using the /api/roles endpoint. Ask your system administrator to get the list of roles in your system:

```bash
curl -s -X GET "${TERMINUSDB_URL}/api/roles" \
  -u "${ADMIN_USER}:${ADMIN_PASS}" | jq '.[] | {name: .name, actions: .action}'
```

This returns all roles with their associated permissions, helping you choose or design the right role for each user.

## Part 7: Testing Authorization Boundaries

Now let's verify that users can only perform actions they're authorized for.

### Test 1: Charlie Cannot Create Databases in the Organization

Charlie has admin access to the `products` database but NOT the organization. He should **not** be able to create new databases:

```bash
curl -X POST "${TERMINUSDB_URL}/api/db/${ORG_NAME}/unauthorized_db" \
  -u "${DB_USER}:${DB_USER_PASS}" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "Unauthorized Database",
    "schema": true
  }'
```

**Expected Response:** Error 403 (Forbidden)
```json
{
  "@type": "api:DbCreateErrorResponse",
  "api:status": "api:not_found",
  "api:error": {
    "@type": "api:UnknownOrganization",
    "api:organization_name": "acme_corp_1234567890"
  }
}
```

This demonstrates that Charlie's capabilities are limited to the specific database scope.

### Test 2: Charlie Cannot Invite Users to the Organization

Charlie should not be able to grant capabilities at the organization level:

```bash
curl -X POST "${TERMINUSDB_URL}/api/capabilities" \
  -u "${DB_USER}:${DB_USER_PASS}" \
  -H "Content-Type: application/json" \
  -d "{
    \"operation\": \"grant\",
    \"scope\": \"${ORG_NAME}\",
    \"user\": \"${REGULAR_USER}\",
    \"roles\": [\"Consumer Role\"],
    \"scope_type\": \"organization\"
  }"
```

**Expected Response:** Error 403 (Forbidden)

Charlie doesn't have `manage_capabilities` permission at the organization level.

### Test 3: Charlie CAN Access His Authorized Database

Charlie should be able to access database operations (documents and queries) for the products database:

**Test 3a: Access Document Schema**
```bash
curl -s -X GET "${TERMINUSDB_URL}/api/document/${ORG_NAME}/${DB_NAME}?graph_type=schema" \
  -u "${DB_USER}:${DB_USER_PASS}" \
  -H "Content-Type: application/json" | jq
```

**Expected Response:** Schema context
```json
{
  "@base": "terminusdb:///data/",
  "@schema": "terminusdb:///schema#",
  "@type": "@context"
}
```

**Test 3b: Access via GraphQL**
```bash
curl -s -X POST "${TERMINUSDB_URL}/api/graphql/${ORG_NAME}/${DB_NAME}" \
  -u "${DB_USER}:${DB_USER_PASS}" \
  -H "Content-Type: application/json" \
  -d '{"query": "{ __schema { types { name } } }"}' | jq '.data.__schema.types[0].name'
```

**Expected Response:** Returns schema type name
```json
"GraphType"
```

### Verify Charlie Can List His Database

Let's verify Charlie can now see the organization's database he was invited, in the list of databases he has access to:

```bash
curl -s -X GET "${TERMINUSDB_URL}/api/db" \
  -u "${DB_USER}:${DB_USER_PASS}" | jq
```

**Expected Response:**
Charlie should see the database he has access to:
```json
[
  {
    "path": "acme_corp_1234567890/products"
  }
]
```

### Verify Charlie Can Access Database Metadata

Charlie can also access the metadata endpoint for his database. By default, it returns minimal information:

```bash
curl -s -X GET "${TERMINUSDB_URL}/api/db/${ORG_NAME}/${DB_NAME}" \
  -u "${DB_USER}:${DB_USER_PASS}" | jq
```

**Expected Response:**
```json
{
  "path": "acme_corp_1234567890/products"
}
```

To see full metadata (label, comment, creation date), add the `verbose=true` parameter:

```bash
curl -s -X GET "${TERMINUSDB_URL}/api/db/${ORG_NAME}/${DB_NAME}?verbose=true" \
  -u "${DB_USER}:${DB_USER_PASS}" | jq
```

**Expected Response:**
```json
{
  "@id": "UserDatabase/...",
  "@type": "UserDatabase",
  "comment": "Database for product information",
  "creation_date": "2026-01-04T11:16:08.219Z",
  "label": "Products Database",
  "name": "products",
  "path": "acme_corp_1234567890/products",
  "state": "finalized"
}
```

**What Charlie Can and Cannot Do:**

With database-level `instance_read_access`, Charlie can:
- List his accessible database in the organization
- View metadata for his database
- Read documents and query data
- Read schema definitions
- Cannot see or access other databases in the organization
- Cannot create new databases (requires organization-level permissions)

Compare this to Alice and Bob (organization admins) who can see and access **all** databases in the organization.

### Test 4: Bob (Org Admin) CAN Create Databases

Bob has organization admin privileges, so he should be able to create databases:

```bash
curl -X POST "${TERMINUSDB_URL}/api/db/${ORG_NAME}/inventory" \
  -u "${REGULAR_USER}:${REGULAR_PASS}" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "Inventory Database",
    "schema": true
  }'
```

**Expected Response:**
```json
{
  "@type": "api:DbCreateResponse",
  "api:status": "api:success"
}
```

### Test 5: Bob CAN Invite Users

Bob should be able to grant capabilities since he's an organization admin. First, get the inventory database resource ID:

```bash
INVENTORY_RESOURCE_ID=$(curl -s -X GET "${TERMINUSDB_URL}/api/db/${ORG_NAME}/inventory?verbose=true" \
  -u "${REGULAR_USER}:${REGULAR_PASS}" | jq -r '."@id"')
echo "Inventory Database Resource ID: ${INVENTORY_RESOURCE_ID}"
```

Now Bob grants Charlie the consumer role on the inventory database:

```bash
curl -X POST "${TERMINUSDB_URL}/api/capabilities" \
  -u "${REGULAR_USER}:${REGULAR_PASS}" \
  -H "Content-Type: application/json" \
  -d "{
    \"operation\": \"grant\",
    \"scope\": \"${INVENTORY_RESOURCE_ID}\",
    \"user\": \"User/${DB_USER}\",
    \"roles\": [\"Role/consumer\"]
  }"
```

**Key Points:**
- Bob uses his organization admin credentials
- Gets inventory database resource ID first (databases use hashed IDs)
- ID-based format: `${INVENTORY_RESOURCE_ID}`, `User/{username}`, `Role/consumer`
- No `scope_type` parameter (only system admin can use that)

**Expected Response:**
```json
{
  "@type": "api:CapabilityResponse",
  "api:status": "api:success"
}
```

## Part 8: Understanding Permission Hierarchy

Let's understand how organization-level and database-level permissions interact.

### Verify Bob's Write Access Still Works

Bob has organization-level Admin Role. Let's verify he still has write access to the products database:

```bash
curl -s -X GET "${TERMINUSDB_URL}/api/organizations/${ORG_NAME}/users/${REGULAR_USER}" \
  -u "${ORG_ADMIN_USER}:${ORG_ADMIN_PASS}" | jq '.capability[] | {role: .role[].name, scope: .scope}'
```

**Expected Response:**
```json
{
  "role": "Admin Role",
  "scope": "Organization/acme_corp_1234567890"
}
```

Bob's organization-level Admin Role includes `instance_write_access`, which applies to **all** databases in the organization.

**Important:** Organization-level permissions are **broader** than database-level permissions. If a user has organization-level access, database-level grants don't reduce their permissions—they already have access to all databases.

### Consumer Role Demonstration with Charlie

To properly demonstrate read-only Consumer Role access, let's verify Charlie's access with his Database Analyst role (which includes read and write permissions):

## Part 9: Listing Users in an Organization

Alice and Bob can see all users with capabilities in their organization:

```bash
curl -s -X GET "${TERMINUSDB_URL}/api/organizations/${ORG_NAME}/users/" \
  -u "${ORG_ADMIN_USER}:${ORG_ADMIN_PASS}" \
  -H "Content-Type: application/json" | jq
```

**Expected Response:**
```json
[
  {
    "@id": "User/alice_1234567890",
    "name": "alice_1234567890",
    "capability": [...]
  },
  {
    "@id": "User/bob_1234567890",
    "name": "bob_1234567890",
    "capability": [...]
  },
  {
    "@id": "User/charlie_1234567890",
    "name": "charlie_1234567890",
    "capability": [...]
  }
]
```

## Part 10: Revoking Access

Alice can revoke Charlie's access to the products database using the ID-based API.

First, get the products database resource ID (if continuing from Part 6, this is already in `DB_RESOURCE_ID`):

```bash
DB_RESOURCE_ID=$(curl -s -X GET "${TERMINUSDB_URL}/api/db/${ORG_NAME}/${DB_NAME}?verbose=true" \
  -u "${ORG_ADMIN_USER}:${ORG_ADMIN_PASS}" | jq -r '."@id"')
echo "Database Resource ID: ${DB_RESOURCE_ID}"
```

Now revoke Charlie's Database Analyst role:

```bash
curl -X POST "${TERMINUSDB_URL}/api/capabilities" \
  -u "${ORG_ADMIN_USER}:${ORG_ADMIN_PASS}" \
  -H "Content-Type: application/json" \
  -d "{
    \"operation\": \"revoke\",
    \"scope\": \"${DB_RESOURCE_ID}\",
    \"user\": \"User/${DB_USER}\",
    \"roles\": [\"Role/Database%20Analyst\"]
  }"
```

**Expected Response:**
```json
{
  "@type": "api:CapabilityResponse",
  "api:status": "api:success"
}
```

### Verify Revocation

Charlie should no longer be able to access documents in the products database:

```bash
curl -s -X GET "${TERMINUSDB_URL}/api/document/${ORG_NAME}/${DB_NAME}" \
  -u "${DB_USER}:${DB_USER_PASS}" | jq
```

**Expected Response:** 403 Forbidden
```json
{
  "api:status": "api:forbidden",
  "api:message": "Access to 'UserDatabase/...' is not authorised with action 'instance_read_access'",
  "action": "'@schema':'Action/instance_read_access'"
}
```

**Note:** Charlie can still access the metadata endpoint (`GET /api/db/{org}/{db}`) because he retains organization-level Metadata Viewer role. The revocation only removed his database-level Database Analyst role.

## Summary

This tutorial demonstrated:

1. **System Admin Powers**: Only system administrators can create organizations
2. **Organization Admins**: Can create databases and invite users to the organization
3. **Capability Delegation**: Organization admins can grant capabilities without system admin involvement
4. **Scope Hierarchy**: Users can have different roles at organization vs. database level
5. **Capability Precedence**: Organization-level capabilities are broader than database-level capabilities—if a user has organization-level admin access, database-level grants don't reduce their permissions
6. **Authorization Boundaries**: Users cannot perform actions outside their granted capabilities
7. **Role Differences**: Admin role includes `manage_capabilities`, consumer role provides read-only access

### Key Takeaways

- **Capabilities** = User + Role + Scope (where scope is organization or database)
- **Organization admins** have the `manage_capabilities` action, allowing them to invite users
- **Database admins** can manage the database but cannot create new databases or invite org-level users
- **Consumer role** provides read-only access
- **Delegation works**: Org admins can grant capabilities without requiring system admin intervention

## Cleanup

To remove all resources created in this tutorial:

```bash
# Delete databases
curl -X DELETE "${TERMINUSDB_URL}/api/db/${ORG_NAME}/${DB_NAME}" \
  -u "${ADMIN_USER}:${ADMIN_PASS}"

curl -X DELETE "${TERMINUSDB_URL}/api/db/${ORG_NAME}/inventory" \
  -u "${ADMIN_USER}:${ADMIN_PASS}"

# Delete organization (must be empty first)
curl -X DELETE "${TERMINUSDB_URL}/api/organizations/${ORG_NAME}" \
  -u "${ADMIN_USER}:${ADMIN_PASS}"

# Delete users
curl -X DELETE "${TERMINUSDB_URL}/api/users/${ORG_ADMIN_USER}" \
  -u "${ADMIN_USER}:${ADMIN_PASS}"

curl -X DELETE "${TERMINUSDB_URL}/api/users/${REGULAR_USER}" \
  -u "${ADMIN_USER}:${ADMIN_PASS}"

curl -X DELETE "${TERMINUSDB_URL}/api/users/${DB_USER}" \
  -u "${ADMIN_USER}:${ADMIN_PASS}"

# Delete custom role
curl -X DELETE "${TERMINUSDB_URL}/api/roles/Database%20Analyst" \
  -u "${ADMIN_USER}:${ADMIN_PASS}"
```

## Next Steps

- Explore [TerminusDB Explanation](/docs/terminusdb-explanation/) for understanding database concepts
- Learn about [WOQL Queries](/docs/how-to-query-with-woql) for querying your databases
- Review [Access Control](/docs/access-control/) for complete list of permissions
