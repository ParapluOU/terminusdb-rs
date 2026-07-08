---
title: Access Control Tutorial Source Code
nextjs:
  metadata:
    title: Access Control Tutorial Source Code
    description: Complete source code walkthrough for the TerminusDB JavaScript access control tutorial.
    keywords: access control, RBAC, JavaScript, tutorial, source code
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/access-control-tutorial-source/
media: []
tags:
  - typescript
  - access-control
  - tutorial
  - self-hosted
---

This page provides a detailed walkthrough of the access control tutorial code. The code uses the `AccessControl` class in the TerminusDB JavaScript Client library to manage user access control for Organizations and databases.

## Setup and Initialization

Import the TerminusDB client and set up the configuration:

```javascript
/* Import terminusdb */
const TerminusClient = require("@terminusdb/terminusdb-client")

/* Import the list of allowed actions */
const { ACTIONS } = TerminusClient.UTILS

/* We are using TerminusDB for Authorization 
   so we'll create all the Users with a NO_KEY password */
const NO_KEY = "NO_KEY"

/* TerminusDB server host url */
const serverHost = "http://localhost:6363"
```

Initialize the TerminusDB Client with the admin's credentials. Only the global admin can create Teams/Organizations and Users:

```javascript
const client = new TerminusClient.WOQLClient(serverHost, {user:"admin", key:"root"})
const accessControl = new TerminusClient.AccessControl(serverHost, {user:"admin", key:"root"})
```

## Define Custom Roles

Define the role names and their associated actions:

```javascript
/* Role names */
const customReaderRole = "reader"
const customWriterRole = "writer"
const customAppAdmin = "appAdmin"
const customSchemaWriter = "schema_writer"

const appAdminActions = [
    ACTIONS.CREATE_DATABASE,
    ACTIONS.DELETE_DATABASE,
    ACTIONS.SCHEMA_READ_ACCESS,
    ACTIONS.SCHEMA_WRITE_ACCESS,
    ACTIONS.INSTANCE_READ_ACCESS,
    ACTIONS.INSTANCE_WRITE_ACCESS,
    ACTIONS.COMMIT_READ_ACCESS,
    ACTIONS.COMMIT_WRITE_ACCESS,
    ACTIONS.META_READ_ACCESS,
    ACTIONS.META_WRITE_ACCESS,
    ACTIONS.CLASS_FRAME,
    ACTIONS.BRANCH,
    ACTIONS.CLONE,
    ACTIONS.FETCH,
    ACTIONS.PUSH,
    ACTIONS.REBASE
]

const readerActions = [
    ACTIONS.COMMIT_WRITE_ACCESS,
    ACTIONS.SCHEMA_READ_ACCESS, 
    ACTIONS.INSTANCE_READ_ACCESS, 
    ACTIONS.CLASS_FRAME
]

const writerActions = [
    ACTIONS.COMMIT_WRITE_ACCESS,
    ACTIONS.SCHEMA_READ_ACCESS, 
    ACTIONS.INSTANCE_WRITE_ACCESS, 
    ACTIONS.CLASS_FRAME
]

const schemaWriterActions = [ACTIONS.SCHEMA_WRITE_ACCESS]
```

## Create Custom Roles

Roles are sets of actions (permissions) that you can use to grant or restrict access to specific resources and operations. Only the global admin can create Roles:

```javascript
async function createCustomRoles() {
    try {
        // Create the appAdmin role
        const roleAppAdminId = await accessControl.createRole(customAppAdmin, appAdminActions)
        console.log(`The role ${roleAppAdminId} has been created`)

        // Create the reader role
        const roleReaderId = await accessControl.createRole(customReaderRole, readerActions)
        console.log(`The role ${roleReaderId} has been created`)

        // Create the writer role
        const roleWriterId = await accessControl.createRole(customWriterRole, writerActions)
        console.log(`The role ${roleWriterId} has been created`)

        // Create the schema_writer role
        const roleSchemaWriterId = await accessControl.createRole(customSchemaWriter, schemaWriterActions)
        console.log(`The role ${roleSchemaWriterId} has been created`)

    } catch(err) {
        const errorType = err.data && err.data["api:error"] ? err.data["api:error"]["@type"] : null
        if (errorType !== 'api:DocumentIdAlreadyExists') {
            throw err
        }
        console.log(`The Document already exists`)
    }
}
```

## Define Users, Teams, and Databases

The dataproducts (or databases) are created under a Team (or Organization). By linking a User to a Team (capability), the User has access to all the dataproducts under this Team:

```javascript
const user__01 = "User__01"
const user__02 = "User__02"
const user__03 = "User__03"

const team__01 = "Team__01"
const team__02 = "Team__02"
const team__03 = "Team__03"

const db__01 = "dataproduct__01"
const db__02 = "dataproduct__02"
```

## Create Teams

Using the admin user, we create three different teams. At this moment no user is connected with these teams:

```javascript
async function createTeams() {
    const team_id01 = await accessControl.createOrganization(team__01)
    console.log("team__01 has been created", team_id01)
    
    const team_id02 = await accessControl.createOrganization(team__02)
    console.log("team__02 has been created", team_id02)
    
    const team_id03 = await accessControl.createOrganization(team__03)
    console.log("team__03 has been created", team_id03)
}
```

## Create Users

We create users with the same password `NO_KEY` since we are using TerminusDB for authorization:

```javascript
async function createUsers() {
    const user_id01 = await accessControl.createUser(user__01, NO_KEY)
    console.log("user__01 has been created", user_id01)
    
    const user_id02 = await accessControl.createUser(user__02, NO_KEY)
    console.log("user__02 has been created", user_id02)
    
    const user_id03 = await accessControl.createUser(user__03, NO_KEY)
    console.log("user__03 has been created", user_id03)
}
```

## Connect Teams with Users

The TerminusDB administrator (admin) can create capabilities that let users access specific resources. A capability is a connection between Roles and Resources (teams or dataproducts):

```javascript
async function connectTeamsWithUserAppAdminRole() {
    /* User_01 is the appAdmin of team__01, team__03 
       and the databases controlled by the team */
    const link01 = await accessControl.manageCapability(
        user__01, team__01, ["appAdmin"], "grant", "organization"
    )
    console.log(`The user ${user__01} has been added to ${team__01} with appAdmin role`)
    
    const link03 = await accessControl.manageCapability(
        user__01, team__03, ["appAdmin"], "grant", "organization"
    )
    console.log(`The user ${user__01} has been added to ${team__03} with appAdmin role`)

    /* User_02 is the appAdmin of team__02 
       and the databases controlled by the team */
    const link02 = await accessControl.manageCapability(
        user__02, team__02, ["appAdmin"], "grant", "organization"
    )
    console.log(`The user ${user__02} has been added to ${team__02} with appAdmin role`)
}

async function connectTeam01WithOtherUsers() {
    /* Connect User__02 with reader role to access team__01 resources */
    const link04 = await accessControl.manageCapability(
        user__02, team__01, ["reader"], "grant", "organization"
    )
    console.log(`The user ${user__02} has been added to ${team__01} with reader role`)

    /* Connect User__03 with reader and writer roles */
    const link05 = await accessControl.manageCapability(
        user__03, team__01, ["reader", "writer"], "grant", "organization"
    )
    console.log(`The user ${user__03} has been added to ${team__01} with reader and writer roles`)
}
```

## Create Databases

Create a new TerminusDB client instance for User__01. The User can create a database under a team only if they have a Role that allows `create_database`:

```javascript
async function createDB() {
    const clientTeam01 = new TerminusClient.WOQLClient(serverHost, {
        user: user__01, 
        key: NO_KEY, 
        organization: team__01
    })
    
    await clientTeam01.createDatabase(db__01, {
        label: db__01, 
        comment: "add db", 
        schema: true
    })
    console.log(`The dataproduct ${db__01} has been created by ${user__01}`)
    
    await clientTeam01.createDatabase(db__02, {
        label: db__02, 
        comment: "add db", 
        schema: true
    })
    console.log(`The dataproduct ${db__02} has been created by ${user__01}`)
}
```

## Test Permission Restrictions

Test that User__02 does not have permission to create databases under team__01:

```javascript
async function errorCreateDatabase() {
    try {
        const clientTeamUser02 = new TerminusClient.WOQLClient(serverHost, {
            user: user__02, 
            key: NO_KEY, 
            organization: team__01
        })
        const accessControlUser02 = new TerminusClient.AccessControl(serverHost, {
            user: user__02, 
            key: NO_KEY, 
            organization: team__01
        })

        /* We can see the list of actions allowed for the connected user */
        const roles = await accessControlUser02.getTeamUserRoles(user__02)
        console.log("user_02 roles", JSON.stringify(roles, null, 4))
        
        /* This user does not have the "create_database" level of access */
        await clientTeamUser02.createDatabase("test_db", {
            label: "test_db", 
            comment: "add db", 
            schema: true
        })
    } catch(err) {
        const message = err.data ? err.data["api:message"] : err.message
        console.log("user__02 does not have permission to create databases", message)
    }
}
```

## Add Roles at Database Level

The user has no specific permissions at data product level, but each data product inherits the team access level. You can increase permissions for a User to access a specific dataproduct:

```javascript
async function addCapabilityRolesForDataproduct() {
    /* For database resources, pass the team/dataproduct path */
    const link06 = await accessControl.manageCapability(
        user__02, 
        `${team__01}/${db__02}`, 
        ["writer", "schema_writer"], 
        "grant", 
        "database"
    )
    console.log(`The user ${user__02} has been added to ${team__01}/${db__02} with writer and schema_writer roles`)
    
    const accessControl01 = new TerminusClient.AccessControl(serverHost, {
        user: user__02, 
        key: NO_KEY, 
        organization: team__01
    })
    
    /* We can see the updated roles for user__02 */
    const roles = await accessControl01.getTeamUserRoles(user__02)
    console.log("user_02 roles", JSON.stringify(roles, null, 4))
}
```

## Cleanup: Delete Resources

{% callout type="warning" title="Important Deletion Order" %}
You must be careful when removing dataproducts, teams, and users. For removing a team, you have to first remove all the databases under that team and revoke all user capabilities.
{% /callout %}

```javascript
async function deleteDatabase(db) {
    try {
        const clientTeam01 = new TerminusClient.WOQLClient(serverHost, {
            user: user__01, 
            key: NO_KEY, 
            organization: team__01
        })
        await clientTeam01.deleteDatabase(db)
        console.log(`The dataproduct ${db} has been deleted`)
    } catch(err) {
        console.log(`The dataproduct ${db} doesn't exist`)
    }
}

async function deleteUsersAndTeamsIfExists() {
    try {
        /* Important: delete the databases before the User with appAdmin role.
           If no appAdmin user is related with the Organization 
           you cannot remove the databases */
        await deleteDatabase(db__01)
        await deleteDatabase(db__02)

        /* The organization cannot be removed as it is referred to by a capability.
           We have to revoke the capability to the organization before removing.
           The Users still exist, just not related with the Organization anymore */
        await accessControl.manageCapability(
            user__02, team__01, ["reader"], "revoke", "organization"
        )
        console.log(`${user__02} has been removed from ${team__01}`)
        
        await accessControl.manageCapability(
            user__03, team__01, ["reader", "writer"], "revoke", "organization"
        )
        console.log(`${user__03} has been removed from ${team__01}`)
        
        await accessControl.manageCapability(
            user__01, team__01, ["appAdmin"], "revoke", "organization"
        )
        console.log(`${user__01} has been removed from ${team__01}`)

        /* Now we can remove team__01 */
        await accessControl.deleteOrganization(team__01)
        console.log(`${team__01} has been deleted`)

        await accessControl.manageCapability(
            user__02, team__02, ["appAdmin"], "revoke", "organization"
        )
        console.log(`${user__02} has been removed from ${team__02}`)
        
        await accessControl.manageCapability(
            user__01, team__03, ["appAdmin"], "revoke", "organization"
        )
        console.log(`${user__01} has been removed from ${team__03}`)

        await accessControl.deleteOrganization(team__02)
        console.log(`${team__02} has been deleted`)
        
        await accessControl.deleteOrganization(team__03)
        console.log(`${team__03} has been deleted`)

        await accessControl.deleteUser(user__01)
        console.log(`${user__01} has been deleted`)
        
        await accessControl.deleteUser(user__02)
        console.log(`${user__02} has been deleted`)
        
        await accessControl.deleteUser(user__03)
        console.log(`${user__03} has been deleted`)
    } catch(err) {
        console.log(err.message)
    }
}

async function removeRoles() {
    await accessControl.deleteRole(customAppAdmin)
    console.log(`The ${customAppAdmin} role has been deleted`)

    await accessControl.deleteRole(customWriterRole)
    console.log(`The ${customWriterRole} role has been deleted`)

    await accessControl.deleteRole(customReaderRole)
    console.log(`The ${customReaderRole} role has been deleted`)

    await accessControl.deleteRole(customSchemaWriter)
    console.log(`The ${customSchemaWriter} role has been deleted`)
}
```

## Run the Tutorial

The main function orchestrates all the steps:

```javascript
async function run() {
    try {
        /* Create custom roles */
        await createCustomRoles()
        console.log("The Roles have been created...")
        
        /* Create all the teams */
        await createTeams()
        console.log("The Teams have been created...")
        
        /* Create all the users */
        await createUsers()
        console.log("The Users have been created...")

        /* Connect users with teams assigning specific roles */
        await connectTeamsWithUserAppAdminRole()
        await connectTeam01WithOtherUsers()
        
        /* Create dataproducts */
        await createDB()
        console.log("The dataproducts have been created...")

        /* Check permission - demonstrates no permission for user_02 */
        await errorCreateDatabase()
        console.log("Demonstrated: no permission for user_02 to create database...")

        /* Update permission for dataproduct
           The User has a role access level for the team and all databases under it.
           The admin can assign a different role for a specific database.
           The role at database level works only if it is higher than team access level */
        await addCapabilityRolesForDataproduct()
        console.log("The permission for a dataproduct has been granted...")

        /* Cleanup */
        await deleteUsersAndTeamsIfExists()
        console.log("Dataproducts, teams and users have been deleted...")
        
        await removeRoles()
        console.log("Roles have been deleted...")

    } catch(err) {
        const data = err.data || {}
        console.log(err.message)
        if (data.message) console.log(data.message)
    }
}

run()
```

## Next Steps

- Learn more about [WOQL queries](/docs/woql-explanation/) for data operations
- Explore the [JavaScript Client Reference](/docs/javascript/) for all available methods
- Check out the [Schema Reference Guide](/docs/schema-reference-guide/) for designing your data models
