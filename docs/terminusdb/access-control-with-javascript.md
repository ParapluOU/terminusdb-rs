---
title: Access Control Reference Guide
nextjs:
  metadata:
    title: Access Control Reference Guide
    description: A driver to manage access control with the JS Client
    keywords: access control, javascript client
    alternates:
      canonical: https://terminusdb.org/docs/access-control-with-javascript/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
tags:
  - typescript
  - access-control
  - reference
---

**License**: Apache Version 2

## new AccessControl()

AccessControl is a driver to work with the TerminusDB and DFRNT TerminusDB cloud access control API once launched.

For credentials, you can use a JWT token, an API token or basic authentication with username and password.

**Example**

```javascript
//connect with the API token
//(to request a token create an account in  https://terminusdb.org/)
const accessControl = new AccessControl("https://servername.com",
{organization:"my_team_name",
token:"dGVybWludXNkYjovLy9kYXRhL2tleXNfYXB........"})
accessControl.getOrgUsers().then(result=>{
     console.log(result)
})

//connect with the jwt token this type of connection is only for the dashboard
//or for application integrate with our login workflow
const accessControlJwt = new AccessControl("https://servername.com",
{organization:"my_team_name",
jwt:"eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6IkpXUjBIOXYyeTFORUd........"})
accessControlJwt.getOrgUsers().then(result=>{
     console.log(result)
})

//if the jwt is expired you can change it with
accessControlJwt.setJwtToken("eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6IkpXUjBIOXYy
eTFORUd.......")

//connect with the base authentication this type of connection is only for the local installation
const accessControlLocal = new AccessControl("http://localhost:6363",
{organization:"my_team_name", user:"admin",
key:"mykey"})
accessControlLocal.getOrgUsers().then(result=>{
    console.log(result)
})
```

## getDefaultOrganization

### accessControl.getDefaultOrganization(params) ⇒ `string` | `undefined`

Get a organization from parameters.

**Returns**: `string` | `undefined` - - organization

{% table %}

- Param
- Type
- Description

---

- params
- `object`
- The parameters

{% /table %}

## setJwtToken

### accessControl.setJwtToken(jwt)

Sets the Jwt token for the object

Param

Type

Description

jwt

`string`

The jwt api token to use

## setApiToken

### accessControl.setApiToken(atokenpi)

Sets the API token for the object. Create a DFRNT TerminusDB cloud account to [get your API token](/docs/how-to-connect-dfrnt-hub/).

Param

Type

Description

atokenpi

`string`

The API token to use to connect with DFRNT TerminusDB cloud

## setApiKey

### accessControl.setApiKey(atokenpi)

Sets the API token for the object, to request a token create an account in https://terminusdb.org/

Param

Type

Description

atokenpi

`string`

The API token to use to connect with TerminusDB

## getAPIUrl

### accessControl.getAPIUrl(cloudAPIUrl) ⇒ `string`

Get a API url from cloudAPIUrl

**Returns**: `string` - apiUrl

Param

Type

Description

cloudAPIUrl

`string`

The base url for cloud

## customHeaders

### accessControl.customHeaders(customHeaders) ⇒ `object`

add extra headers to your request

Param

Type

customHeaders

`object`

## getOrganization

### accessControl.getOrganization(organization) ⇒ `object`

\-- TerminusDB API --- Get an organization from the TerminusDB API.

**Returns**: `object` - - organization

Param

Type

Description

organization

`string`

The organization

## getAllOrganizations

### accessControl.getAllOrganizations() ⇒ `Promise`

\-- TerminusDB API --- This end point works in basic authentication, admin user Get list of organizations

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

## createOrganization

### accessControl.createOrganization(orgName) ⇒ `Promise`

\-- TerminusDB API --- This end point works in basic authentication, admin user Create an organization

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

orgName

`string`

The organization name to create

**Example**

```javascript
accessControl.createOrganization("my_org_name").then(result=>{
     console.log(result)
})
```

## deleteOrganization

### accessControl.deleteOrganization(orgName) ⇒ `Promise`

\-- TerminusDB API --- Delete an Organization

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

orgName

`string`

The organization name to delete

**Example**

```javascript
accessControl.createOrganization("my_org_name").then(result=>{
     console.log(result)
})
```

## createRole

### accessControl.createRole(\[name\], \[actions\]) ⇒ `Promise`

\--TerminusDB API --- basic authentication, admin user. Create a new role in the system database.

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

\[name\]

`string`

The role name.

\[actions\]

`typedef.RolesActions`

A list of actions

**Example**

```javascript
accessControl.createRole("Reader",[ACTIONS.INSTANCE_READ_ACCESS]).then(result=>{
 console.log(result)
})
```

## deleteRole

### accessControl.deleteRole(\[name\]) ⇒ `Promise`

\-- TerminusDB API --- basic Authentication, admin user. Delete role in the system database, (this api is enabled only in the local installation)

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

\[name\]

`string`

The role name.

**Example**

```javascript
accessControl.deleteRole("Reader").then(result=>{
 console.log(result)
})
```

## getAllUsers

### accessControl.getAllUsers() ⇒ `Promise`

\-- TerminusDB API --- basic Authentication, admin user. Return the list of all the users (this api is enabled only in the local installation)

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.  
**Example**

```javascript
accessControl.getAllUsers().then(result=>{
 console.log(result)
})
```

## createUser

### accessControl.createUser(name, \[password\]) ⇒ `Promise`

\-- TerminusDB API --- basic Authentication, admin user. Add the user into the system database

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

name

`string`

the user name

\[password\]

`string`

you need the password for basic authentication

**Example**

```javascript
accessControl.deleteUser(userId).then(result=>{
 console.log(result)
})
```

## deleteUser

### accessControl.deleteUser(userId) ⇒ `Promise`

\-- TerminusDB API --- basic Authentication, admin user. Remove the user from the system database.

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

userId

`string`

the document user id

**Example**

```javascript
accessControl.deleteUser(userId).then(result=>{
 console.log(result)
})
```

## manageCapability

### accessControl.manageCapability(userName, resourceName, rolesArr, operation, scopeType) ⇒ `Promise`

\-- TerminusDB API --- Grant/Revoke Capability

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

userName

`string`

the document user id

resourceName

`string`

the name of a (database or team)

rolesArr

`array`

the roles name list

operation

`typedef.CapabilityCommand`

grant/revoke operation

scopeType

`typedef.ScopeType`

the resource type (database or organization)

**Example**

```javascript
//we add an user to an organization and manage users' access
//the user myUser can  access the Organization and all the database under the organization with "reader" Role
client.manageCapability(myUser,myteam,[reader],"grant","organization").then(result=>{
 console.log(result)
})

//the user myUser can  access the database db__001 under the organization myteam
//with "writer" Role
client.manageCapability(myUser,myteam/db__001,[writer],"grant","database").then(result=>{
 consol.log(result)
})
```

## getAccessRoles

### accessControl.getAccessRoles() ⇒ `Promise`

\--TerminusDB and TerminusDB API --- Get all the system database roles types.

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

## getOrgUsers

### accessControl.getOrgUsers(\[orgName\]) ⇒ `Promise`

\-- TerminusDB and TerminusDB API -- Get all the organization's users and roles,

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

\[orgName\]

`string`

The organization name.

**Example**

```javascript
accessControl.getOrgUsers().then(result=>{
 console.log(result)
})

//this function will return an array of capabilities with users and roles
//-- TerminusDB --  response array example
//[{capability: "Capability/3ea26e1d698821c570afe9cb4fe81a3......"
//     email: {@type: "xsd:string", @value: "user@terminusdb.com"}
//     picture: {@type: "xsd:string",…}
//     role: "Role/dataReader"
//     scope: "Organization/my_org_name"
//     user: "User/auth0%7C613f5dnndjdjkTTT"}]
//
//
// -- Local Installation -- response array example
//[{ "@id":"User/auth0%7C615462f8ab33f4006a6bee0c",
//  "capability": [{
//   "@id":"Capability/c52af34b71f6f8916ac0115ecb5fe0e31248ead8b1e3d100852015...",
//   "@type":"Capability",
//  "role": [{
//    "@id":"Role/admin",
//    "@type":"Role",
//    "action": ["instance_read_access"],
//     "name":"Admin Role"
//     }],
//  "scope":"Organization/@team"}]]
```

## getTeamUserRoles

### accessControl.getTeamUserRoles(\[userName\], \[orgName\]) ⇒ `Promise`

\-- TerminusDB and TerminusDB API -- Get the user roles for a given organization or the default organization,

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

\[userName\]

`string`

The organization name.

\[orgName\]

`string`

The organization name.

**Example**

```javascript
accessControl.getTeamUserRole("myUser").then(result=>{
 console.log(result)
})

//response object example
{
 "@id": "User/myUser",
  "capability": [
        {
          "@id":"Capability/server_access",
          "@type":"Capability",
          "role": [{
             "@id":"Role/reader",
              "@type":"Role",
             "action": [
                "instance_read_access",
             ],
              "name":"reader"
            }],
          "scope":"Organization/myteam"
        }
      ],
  "name": "myUser"
}
```

## ifOrganizationExists

### accessControl.ifOrganizationExists(orgName) ⇒ `Promise`

\-- TerminusDB API --- Check if the organization exists. it is a Head call . IMPORTANT This does not work with the API-TOKEN.

**Returns**: `Promise` - A promise that returns the call status object, 200: if the organization exists and 404: if the organization does not exist

Param

Type

Description

orgName

`string`

The organization name to check if exists.

## createOrganizationRemote

### accessControl.createOrganizationRemote(orgName) ⇒ `Promise`

\-- TerminusDB API ---

IMPORTANT This does not work with the API-TOKEN. Create an organization

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

orgName

`string`

The organization name to create

**Example**

```javascript
accessControl.createOrganization("my_org_name").then(result=>{
     console.log(result)
})
```

## getPendingOrgInvites

### accessControl.getPendingOrgInvites(\[orgName\]) ⇒ `Promise`

\-- TerminusDB API ---

Get the pending invitations list.

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

\[orgName\]

`string`

The organization name.

**Example**

```javascript
const invitationList = accessControl.getPendingOrgInvites().then(result=>{
   console.log(invitationList)

})
//this will return an array of invitations object like this
//[{@id: "Organization/my_team_name/invitations/Invitation/7ad0c9eb82b6175bcda9c0dfc2ac51161ef5ba
cb0988d992c4bce82b3fa5d25"
//      @type: "Invitation"
//      creation_date: "2021-10-22T11:13:28.762Z"
//      email_to: "new_user@terminusdb.com"
//      invited_by: "User/auth0%7C6162f8ab33567406a6bee0c"
//      role: "Role/dataReader"
//      status: "needs_invite"}]
```

## sendOrgInvite

### accessControl.sendOrgInvite(userEmail, role, \[note\], \[orgName\]) ⇒ `Promise`

\-- TerminusDB API ---

Send a new invitation

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

userEmail

`string`

The email of user.

role

`string`

The role for user. (the document @id role like Role/collaborator)

\[note\]

`string`

The note to send with the invitation.

\[orgName\]

`string`

The organization name.

**Example**

```javascript
accessControl.sendOrgInvite("new_user@terminusdb.com","Role/admin",
"please join myteam").then(result=>{
   console.log(result)
})
```

## getOrgInvite

### accessControl.getOrgInvite(inviteId, \[orgName\]) ⇒ `Promise`

\-- TerminusDB API --- Get the invitation info

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

inviteId

`string`

The invite id to retrieve.

\[orgName\]

`string`

The organization name.

**Example**

```javascript
const fullInviteId="Organization/my_team_name/invitations/Invitation/7ad0c9eb82b6175bcda9c0dfc
2ac51161ef5ba7cb0988d992c4bce82b3fa5d25"
accessControl.getOrgInvite(fullInviteId).then(result=>{
 console.log(result)
})
```

## deleteOrgInvite

### accessControl.deleteOrgInvite(inviteId, \[orgName\]) ⇒ `Promise`

\-- TerminusDB API ---

Delete an invitation

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

inviteId

`string`

The invite id to delete.

\[orgName\]

`string`

The organization name.

**Example**

```javascript
const fullInviteId="Organization/my_team_name/invitations/Invitation/7ad0c9eb82b6175bcda9
c0dfc2ac51161ef5ba7cb0988d992c4bce82b3fa5d25"
accessControl.deleteOrgInvite(fullInviteId).then(result=>{
     console.log(result)
})
```

## updateOrgInviteStatus

### accessControl.updateOrgInviteStatus(inviteId, accepted, \[orgName\]) ⇒ `Promise`

\-- TerminusDB API ---

Accept /Reject invitation. if the invitation has been accepted we add the current user to the organization.

The only user that can accept this invitation is the user registered with the invitation email, we indentify the user with the JWT token

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

inviteId

`string`

The invite id to updated.

accepted

`boolean`

The status of the invitation.

\[orgName\]

`string`

The organization name.

**Example**

```javascript
const fullInviteId="Organization/my_team_name/invitations/Invitation/7ad0c9eb82b6175bcda9
c0dfc2ac51161ef5ba7cb0988d992c4bce82b3fa5d25"
accessControl.updateOrgInviteStatus(fullInviteId,true).then(result=>{
  console.log(result)
})
```

## getTeamUserRole

### accessControl.getTeamUserRole(\[orgName\]) ⇒ `Promise`

\-- TerminusDB API ---

Get the user role for a given organization or the default organization The user is identified by the jwt or the access token

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

\[orgName\]

`string`

The organization name.

**Example**

```javascript
accessControl.getTeamUserRole().then(result=>{
 console.log(result)
})

//response object example
{"userRole":"Role/admin"}
```

## removeUserFromOrg

### accessControl.removeUserFromOrg(userId, \[orgName\]) ⇒ `Promise`

\-- TerminusDB API -- Remove an user from an organization, only an admin user can remove an user from an organization

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

userId

`string`

The id of the user to be removed. (this is the document user's @id)

\[orgName\]

`string`

The organization name in which the user is to be removed.

**Example**

```javascript
accessControl.removeUserFromOrg("User/auth0%7C613f5dnndjdjkTTT","my_org_name").then(result=>{
 console.log(result)
})
```

## getDatabaseRolesOfUser

### accessControl.getDatabaseRolesOfUser(userId, \[orgName\]) ⇒ `Promise`

\-- TerminusDB API --

Get the user's role for every databases under the organization

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

userId

`string`

The user's id.

\[orgName\]

`string`

The organization name.

**Example**

```javascript
accessControl.getDatabaseRolesOfUser('User/auth0%7C61790e366377Yu6596a').then(result=>{
     console.log(result)
})

//this is a capabilities list of databases and roles
//[ {capability: "Capability/b395e8523d509dec6b33aefc9baed3b2e2bfadbd4c79d4ff9b20dce2b14e2edc"
//if there is an id we have a user specific capabality for this database
   // name: {@type: "xsd:string", @value: "profiles_test"}
   // role: "Role/dataUpdater"
   // scope: "UserDatabase/7ebdfae5a02bc7e8f6d79sjjjsa4e179b1df9d4576a3b1d2e5ff3b4859"
   // user: "User/auth0%7C61790e11a3966d006906596a"},

//{ capability: null
// if the capability id is null the user level of access for this database is the
same of the team
  //name: {@type: "xsd:string", @value: "Collab002"}
  //role: "Role/dataReader"
  // scope: "UserDatabase/acfcc2db02b83792sssb15239ccdf586fc5b176846ffe4878b1aea6a36c8f"
  //user: "User/auth0%7C61790e11a3966d006906596a"}]
```

## createUserRole

### accessControl.createUserRole(userId, scope, role, \[orgName\]) ⇒ `Promise`

\-- TerminusDB API --

Create a user's a role for a resource (organization/database)

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

userId

`string`

The user's id.

scope

`string`

The resource name/id.

role

`string`

The user role to be assigned.

\[orgName\]

`string`

The organization name.

**Example**

```javascript
const dbId = "UserDatabase/acfcc2db02b83792sssb15239ccdf586fc5b176846ffe4878b1aea6a36c8f"
accessControl.assignUserRole('User/auth0%7C61790e11a3966d006906596a',dbId,
"Role/collaborator").then(result=>{
     console.log(result)

})
```

## updateUserRole

### accessControl.updateUserRole(userId, capabilityId, scope, role, \[orgName\]) ⇒ `Promise`

\-- TerminusDB API --

Update user's a role for a resource (organization/database), (this api works only in TerminusDB)

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

userId

`string`

The user's id.

capabilityId

`string`

The capability id.

scope

`string`

The resource name/id.

role

`string`

The user role to be updated.

\[orgName\]

`string`

The organization name.

**Example**

```javascript
const dbId = "UserDatabase/acfcc2db02b83792sssb15239ccdf586fc5b176846ffe4878b1aea6a36c8f"
const capId= "Capability/b395e8523d509dec6b33aefc9baed3b2e2bfadbd4c79d4ff9b20dce2b14e2edc"
accessControl.updateUserRole('User/auth0%7C61790e11a3966d006906596a',capId,dbId,
"Role/dataUpdater").then(result=>{
     console.log(result)

})
```

## accessRequestsList

### accessControl.accessRequestsList(\[orgName\]) ⇒ `Promise`

\-- TerminusDB API --

Get all the access request list for a specify organization

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

\[orgName\]

`string`

The organization name.

**Example**

```javascript
accessControl.accessRequestsList().then(result=>{
 console.log(result)
})
```

## sendAccessRequest

### accessControl.sendAccessRequest(\[email\], \[affiliation\], \[note\], \[orgName\]) ⇒ `Promise`

\-- TerminusDB API --

Get all the access request list for a specify organization

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

\[email\]

`string`

the user email.

\[affiliation\]

`string`

the user affiliation, company, university etc..

\[note\]

`string`

the message for the team admin

\[orgName\]

`string`

The organization name.

**Example**

```javascript
accessControl.sendAccessRequest("myemail@terminusdb.com",
 "my_company",
 "please add me to your team"
).then(result=>{
 console.log(result)
})
```

## deleteAccessRequest

### accessControl.deleteAccessRequest(\[orgName\]) ⇒ `Promise`

\-- TerminusDB API --

Delete an access request to join your team, only an admin user can delete it

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

Param

Type

Description

\[orgName\]

`string`

The organization name.

**Example**

```javascript
accessControl.deleteAccessRequest("djjdshhsuuwewueueuiHYHYYW.......").then(result=>{
 console.log(result)
})
```

## Further reading

- [Access Control Reference](/docs/access-control/) — HTTP API reference for users, roles, and capabilities
- [RBAC Tutorial](/docs/access-control-tutorial/) — step-by-step walkthrough of setting up roles, users, and permissions
- [Tutorial Source Code](/docs/access-control-tutorial-source/) — full JavaScript example with custom roles and multiple users
- [Troubleshooting Authentication Errors](/docs/troubleshooting-auth/) — fix 401/403 issues