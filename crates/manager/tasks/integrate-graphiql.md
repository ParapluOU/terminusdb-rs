# integrate the graphiql web query editor inside the app.
TerminusDB provides the capability of running GraphQL queries _per database_. We have a entrypoint for this in the TerminusDBHttpClient; i think its called .execute_graphql().

The Graphiql frontend app can be integrated as direted here:
https://github.com/graphql/graphiql/tree/main/packages/graphiql#readme

Since we have multiple instances, per node, per database, we probably have to use the namespacing feature.

We want two ways for users to access the functionality:

## from node overview
- right click a node
- click context menu item ":magnifying_glass_icon Query..."
- in the database selector modal, choose the DB to query
- the almost-fullscreen modal that mounts Graphiql should render

## from database modal
-
