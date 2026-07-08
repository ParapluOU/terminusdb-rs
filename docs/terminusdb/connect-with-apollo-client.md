---
title: Connect with Apollo Client to use GraphQL with TerminusDB
nextjs:
  metadata:
    title: Connect with Apollo Client to use GraphQL with TerminusDB
    description: A reference guide to get you up and running with TerminusDB & TerminusDB  using GraphQL and Apollo Client
    keywords: terminusdb, client, connect, connection, graphql, javascript, terminusdb graphql, terminusdb javascript client
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/connect-with-apollo-client/
media: []
tags:
  - typescript
  - graphql
  - how-to
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- Node.js and npm installed
- An existing database with a schema
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have connected Apollo Client to TerminusDB's GraphQL endpoint.
{% /callout %}

{% callout type="note" %}
**TerminusCMS is now DFRNT Hub**
TerminusCMS has been renamed to **DFRNT Hub**. All features described on this page are available at [dfrnt.com](https://dfrnt.com/hypergraph-content-studio/). The TerminusDB open source database remains unchanged.
{% /callout %}


1.  Install dependencies

```bash
npm install @apollo/client graphql
```

2.  Initialize ApolloClient and Connect with TerminusDB

Import the required dependencies needed -

```javascript
import { ApolloClient, InMemoryCache, ApolloProvider, gql, HttpLink, ApolloLink, concat } from '@apollo/client';
```

Or

```javascript
const Apollo =  require( '@apollo/client');

const { ApolloClient, InMemoryCache, concat, gql,HttpLink,ApolloLink } = Apollo
```

Initialize ApolloClient by passing its constructor with a configuration object with the TerminusDB server endpoint, user credentials and cache fields.

> Extra information about the Apollo client cache can be found on their [website](https://www.apollographql.com/docs/react/caching/overview)

## Connect with TerminusDB Local

```javascript
const orgName = "myOrganizationName"
const dbName = "myDBname"
const myBranch = "main"

const user = "admin"
const password = "mypass"
const userPassEnc = btoa(`${user}:${password}`)

const terminusdbURL = `http://localhost:6363/api/graphql/${orgName}/${dbName}/local/branch/${myBranch}/`

const httpLink = new HttpLink({ uri: terminusdbURL });
const authMiddleware = new ApolloLink((operation, forward) => {
    // add the authorization to the headers
    operation.setContext(({ headers = {} }) => ({
    headers: {
        ...headers,
        authorization: `Basic ${userPassEnc}`}
    }));
    return forward(operation);
})

const cache = new InMemoryCache({
    addTypename: false
});

const value = concat(authMiddleware, httpLink)

const apolloClient = new ApolloClient({
    cache:cache,
    link: value,       
});

// Query your database

apolloClient
  .query({
    query: gql`
     query{
        Person{
        _id
        name
        }
    }
    `,
  })
  .then((result) => console.log(result.data))
  .catch(err =>console.log(err.message));
```

## Connect with DFRNT TerminusDB cloud

> You will need to [get your API key](/docs/how-to-connect-dfrnt-hub/) to connect with DFRNT TerminusDB cloud

```javascript
const orgName = "myOrganizationName"
const dbName = "myDBname"
const myBranch = "main"

const myAPIToken = 'replaceYourToken'

const terminusdbURL = `https://dfrnt.com/api/hosted/${orgName}/api/graphql/${orgName}/${dbName}/local/branch/${myBranch}/`

const httpLink = new HttpLink({ uri: terminusdbURL });
const authMiddleware = new ApolloLink((operation, forward) => {
    // add the authorization to the headers
    operation.setContext(({ headers = {} }) => ({
    headers: {
        ...headers,
        authorization: `Token ${myAPIToken}`}
    }));
    return forward(operation);
})

const cache = new InMemoryCache({
    addTypename: false
});

const value = concat(authMiddleware, httpLink)

const apolloClient = new ApolloClient({
    cache:cache,
    link: value,       
});

// Query your database

apolloClient
  .query({
    query: gql`
     query{
        Person{
        _id
        name
        }
    }
    `,
  })
  .then((result) => console.log(result.data))
  .catch(err =>console.log(err.message));
```
