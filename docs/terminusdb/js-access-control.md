---
title: Access Control with the JavaScript Client Reference Guide
nextjs:
  metadata:
    title: Access Control with the JavaScript Client Reference Guide
    description: A reference guide to help understand TerminusDB access control using the JavaScript Client to manage teams, users and database access
    keywords: terminusdb, access control, javascript, terminusdb javascript client, typescript
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/js-access-control/
media: []
tags:
  - typescript
  - access-control
  - reference
---

**License**: Apache Version 2


## new AccessControl()

The AccessControl is a driver to work with TerminusDB and TerminusDB access control api for the credential you can use the JWT token, the API token or the basic authentication with username and password

**Example**

```javascript
//connect with the API token
//(to request a token create an account in  https://terminusdb.org/)
const accessContol = new AccessControl("https://servername.com",
{organization:"my_team_name",
token:"dGVybWludXNkYjovLy9kYXRhL2tleXNfYXB........"})
accessControl.getOrgUsers().then(result=>{
     console.log(result)
})
//connect with the jwt token this type of connection is only for the dashboard
//or for application integrate with our login workflow
const accessContol = new AccessControl("https://servername.com",
{organization:"my_team_name",
jwt:"eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6IkpXUjBIOXYyeTFORUd........"})
accessControl.getOrgUsers().then(result=>{
     console.log(result)
})
//if the jwt is expired you can change it with
accessControl.setJwtToken("eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6IkpXUjBIOXYy
eTFORUd.......")
//connect with the base authentication this type of connection is only for the local installation
const accessContol = new AccessControl("http://localhost:6363",
{organization:"my_team_name", user:"admin"
key:"mykey"})
accessControl.getOrgUsers().then(result=>{
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

{% table %}

- Param
- Type
- Description

---

- jwt
- `string`
- The jwt api token to use

{% /table %}

## setApiToken

### accessControl.setApiToken(atokenpi)

Sets the API token for the object, to request a token create an account in https://terminusdb.org/

{% table %}

- Param
- Type
- Description

---

- atokenpi
- `string`
- The API token to use to connect with TerminusDB

{% /table %}

## setApiKey

### accessControl.setApiKey(atokenpi)

Sets the API token for the object, to request a token create an account in https://terminusdb.org/

{% table %}

- Param
- Type
- Description

---

- atokenpi
- `string`
- The API token to use to connect with TerminusDB

{% /table %}

## getAPIUrl

### accessControl.getAPIUrl(cloudAPIUrl) ⇒ `string`

Get a API url from cloudAPIUrl

**Returns**: `string` - apiUrl

{% table %}

- Param
- Type
- Description

---

- cloudAPIUrl
- `string`
- The base url for cloud

{% /table %}

## customHeaders

### accessControl.customHeaders(customHeaders) ⇒ `object`

add extra headers to your request

{% table %}

- Param
- Type

---

- customHeaders
- `object`

{% /table %}

## getOrganization

### accessControl.getOrganization(organization) ⇒ `object`

\-- TerminusDB API --- Get an organization from the TerminusDB API.

**Returns**: `object` - - organization

{% table %}

- Param
- Type
- Description

---

- organization
- `string`
- The organization

{% /table %}

## getAllOrganizations

### accessControl.getAllOrganizations() ⇒ `Promise`

\-- TerminusDB API --- This end point works in basic authentication, admin user Get list of organizations

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

## createOrganization

### accessControl.createOrganization(orgName) ⇒ `Promise`

\-- TerminusDB API --- This end point works in basic authentication, admin user Create an organization

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- orgName
- `string`
- The organization name to create

{% /table %}

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

{% table %}

- Param
- Type
- Description

---

- orgName
- `string`
- The organization name to delete

{% /table %}

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

{% table %}

- Param
- Type
- Description

---

- \[name\]
- `string`
- The role name.

---

- \[actions\]
- `typedef.RolesActions`
- A list of actions

{% /table %}

**Example**

```javascript
accessControl.createRole("Reader",[ACTIONS.INSTANCE_READ_ACCESS]).then(result=>{
 console.log(result)
})
```

## deleteRole

### accessControl.deleteRole(\[name\]) ⇒ `Promise`

\-- TerminusdDB API --- basic Authentication, admin user. Delete role in the system database, (this api is enabled only in the local installation)

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- \[name\]
- `string`
- The role name.

{% /table %}

**Example**

```javascript
accessControl.deleteRole("Reader").then(result=>{
 console.log(result)
})
```

## getAllUsers

### accessControl.getAllUsers() ⇒ `Promise`

\-- TerminusdDB API --- basic Authentication, admin user. Return the list of all the users (this api is enabled only in the local installation)

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.  
**Example**

```javascript
accessControl.getAllUsers().then(result=>{
 console.log(result)
})
```

## createUser

### accessControl.createUser(name, \[password\]) ⇒ `Promise`

\-- TerminusdDB API --- basic Authentication, admin user. Add the user into the system database

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- name
- `string`
- the user name

---

- \[password\]
- `string`
- you need the password for basic authentication

{% /table %}

**Example**

```javascript
accessControl.createUser("myuser", "my_password").then(result=>{
 console.log(result)
})
```

## deleteUser

### accessControl.deleteUser(userId) ⇒ `Promise`

\-- TerminusdDB API --- basic Authentication, admin user. Remove the user from the system database.

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- userId
- `string`
- the document user id

{% /table %}

**Example**

```javascript
accessControl.deleteUser(userId).then(result=>{
 console.log(result)
})
```

## manageCapability

### accessControl.manageCapability(userName, resourceName, rolesArr, operation, scopeType) ⇒ `Promise`

\-- TerminusdDB API --- Grant/Revoke Capability

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- userName
- `string`
- the document user id

---

- resourceName
- `string`
- the name of a (database or team)

---

- rolesArr
- `array`
- the roles name list

---

- operation
- `typedef.CapabilityCommand`
- grant/revoke operation

---

- scopeType
- `typedef.ScopeType`
- the resource type (database or organization)

{% /table %}

**Example**

```javascript
//we add an user to an organization and manage users' access
//the user myUser can  access the Organization and all the database under the organization with "reader" Role
client.manageCapability(myUser,myteam,[reader],"grant","organization").then(result=>{
 consol.log(result)
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

{% table %}

- Param
- Type
- Description

---

- \[orgName\]
- `string`
- The organization name.

{% /table %}

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

{% table %}

- Param
- Type
- Description

---

- \[userName\]
- `string`
- The user name.

---

- \[orgName\]
- `string`
- The organization name.

{% /table %}

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

{% table %}

- Param
- Type
- Description

---

- orgName
- `string`
- The organization name to check if exists.

{% /table %}

## createOrganizationRemote

### accessControl.createOrganizationRemote(orgName) ⇒ `Promise`

\-- TerminusDB API --- IMPORTANT This does not work with the API-TOKEN. Create an organization

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- orgName
- `string`
- The organization name to create

{% /table %}

**Example**

```javascript
accessControl.createOrganization("my_org_name").then(result=>{
     console.log(result)
})
```

## getPendingOrgInvites

### accessControl.getPendingOrgInvites(\[orgName\]) ⇒ `Promise`

\-- TerminusDB API --- Get the pending invitations list.

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- \[orgName\]
- `string`
- The organization name.

{% /table %}

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

\-- TerminusDB API --- Send a new invitation

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- userEmail
- `string`
- The email of user.

---

- role
- `string`
- The role for user. (the document @id role like Role/collaborator)

---

- \[note\]
- `string`
- The note to send with the invitation.

---

- \[orgName\]
- `string`
- The organization name.

{% /table %}

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

{% table %}

- Param
- Type
- Description

---

- inviteId
- `string`
- The invite id to retrieve.

---

- \[orgName\]
- `string`
- The organization name.

{% /table %}

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

\-- TerminusDB API --- Delete an invitation

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- inviteId
- `string`
- The invite id to delete.

---

- \[orgName\]
- `string`
- The organization name.

{% /table %}

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

\-- TerminusDB API --- Accept/Reject invitation. If the invitation has been accepted we add the current user to the organization. The only user that can accept this invitation is the user registered with the invitation email. We identify the user with the jwt token.

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- inviteId
- `string`
- The invite id to update.

---

- accepted
- `boolean`
- The status of the invitation.

---

- \[orgName\]
- `string`
- The organization name.

{% /table %}

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

\-- TerminusDB API --- Get the user role for a given organization or the default organization. The user is identified by the jwt or the access token.

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- \[orgName\]
- `string`
- The organization name.

{% /table %}

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

\-- TerminusDB API -- Remove a user from an organization. Only an admin user can remove a user from an organization.

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- userId
- `string`
- The id of the user to be removed. (this is the document user's @id)

---

- \[orgName\]
- `string`
- The organization name in which the user is to be removed.

{% /table %}

**Example**

```javascript
accessControl.removeUserFromOrg("User/auth0%7C613f5dnndjdjkTTT","my_org_name").then(result=>{
 console.log(result)
})
```

## getDatabaseRolesOfUser

### accessControl.getDatabaseRolesOfUser(userId, \[orgName\]) ⇒ `Promise`

\-- TerminusDB API -- Get the user's role for every database under the organization.

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- userId
- `string`
- The user's id.

---

- \[orgName\]
- `string`
- The organization name.

{% /table %}

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

\-- TerminusDB API -- Create a user's role for a resource (organization/database).

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- userId
- `string`
- The user's id.

---

- scope
- `string`
- The resource name/id.

---

- role
- `string`
- The user role to be assigned.

---

- \[orgName\]
- `string`
- The organization name.

{% /table %}

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

\-- TerminusDB API -- Update user's role for a resource (organization/database). This API works only in TerminusDB.

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- userId
- `string`
- The user's id.

---

- capabilityId
- `string`
- The capability id.

---

- scope
- `string`
- The resource name/id.

---

- role
- `string`
- The user role to be updated.

---

- \[orgName\]
- `string`
- The organization name.

{% /table %}

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

\-- TerminusDB API -- Get all the access request list for a specified organization.

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- \[orgName\]
- `string`
- The organization name.

{% /table %}

**Example**

```javascript
accessControl.accessRequestsList().then(result=>{
 console.log(result)
})
```

## sendAccessRequest

### accessControl.sendAccessRequest(\[email\], \[affiliation\], \[note\], \[orgName\]) ⇒ `Promise`

\-- TerminusDB API -- Send an access request to join an organization.

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- \[email\]
- `string`
- The user email.

---

- \[affiliation\]
- `string`
- The user affiliation, company, university etc.

---

- \[note\]
- `string`
- The message for the team admin.

---

- \[orgName\]
- `string`
- The organization name.

{% /table %}

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

\-- TerminusDB API -- Delete an access request to join your team. Only an admin user can delete it.

**Returns**: `Promise` - A promise that returns the call response object, or an Error if rejected.

{% table %}

- Param
- Type
- Description

---

- \[orgName\]
- `string`
- The organization name.

{% /table %}

**Example**

```javascript
accessControl.deleteAccessRequest("djjdshhsuuwewueueuiHYHYYW.......").then(result=>{
 console.log(result)
})
```