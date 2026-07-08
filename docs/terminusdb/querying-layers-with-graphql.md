---
tags:
  - graphql
  - how-to
  - version-control
title: Querying Layers with GraphQL
nextjs:
  metadata:
    title: Querying Layers with GraphQL
    keywords: GraphQL, Layer, Repository, Commit Graph, Meta Graph
    description: Learn how to query TerminusDB layer objects using GraphQL to navigate version control history and repository structure.
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/querying-layers-with-graphql/
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- A database with a schema and data
- Familiarity with GraphQL basics ([getting started](/docs/graphql-basics/))
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to query TerminusDB layers using the GraphQL interface.
{% /callout %}

TerminusDB stores version control information in internal system graphs that can be queried using GraphQL. This guide explains how to access and query layer objects, which represent specific versions of your data and schema at different points in time.

## Understanding Layers

Layers are the fundamental storage units in TerminusDB's version control system. Each layer represents an immutable snapshot of your data or schema at a specific point in time. Layers are content-addressed using a SHA-1 hash identifier, which allows TerminusDB to deduplicate storage and efficiently track changes.

### Layer Properties

The `layer:Layer` class has the following structure:

- **`layer_identifier`** - The SHA-1 hash that uniquely identifies the layer content (40 hexadecimal characters)

Note: In GraphQL queries, the field is named `layer_identifier` because it derives from the `layer:identifier` property in the RDF schema.

## Accessing Layers through System Graphs

Layers can be queried through three different system graph endpoints:

### Meta Graph

To access repository and layer information for a specific data product:

```url
http://localhost:6363/api/graphql/ORG/DATA_PRODUCT/_meta
```

### Commit Graph

To access commit history, branches, and their associated layers:

```url
http://localhost:6363/api/graphql/ORG/DATA_PRODUCT/local/_commits
```

### System Graph

For instance, to get _only_ system graph access, you can use the following endpoint:

```url
http://localhost:6363/api/graphql/_system
```

## Querying Layers through Repositories

The most common way to access layer information is through repository objects. Repositories have a `head` property that points to the current layer.

### Query Repository Head Layer

Here's how to query repository head layers using `curl` with basic authentication:

```bash
curl -X POST "http://localhost:6363/api/graphql/admin/MyDatabase/_meta" \
  -u admin:root \
  -H "Content-Type: application/json" \
  -d '{"query":"query { Local { name head { layer_identifier } } }"}'
```

Replace `MyDatabase` with your actual database name.

The equivalent GraphQL query is:

```graphql
query {
  Local {
    name
    head {
      layer_identifier
    }
  }
}
```

This query returns the current head layer for all local repositories:

```json
{
  "data": {
    "Local": [
      {
        "name": "local",
        "head": {
          "layer_identifier": "2da4448a794d8f60bee0ffbca7330c0f2288461d"
        }
      }
    ]
  }
}
```

The `name` field typically returns `"local"` for the local repository, and the `layer_identifier` is the SHA-1 hash of the current head layer.

### Understanding the Identifier

The `layer_identifier` field contains the layer's SHA-1 hash. This is the same identifier used internally by TerminusDB for storage. The hash is computed from the layer's content, making it:

- **Content-addressable** - The same content always produces the same identifier
- **Immutable** - Once created, a layer's identifier never changes
- **Verifiable** - The integrity of the layer can be cryptographically verified

This identifier is what you might see in other parts of TerminusDB, such as:
- Storage file names (`.larch` files)
- Pack files used for remote synchronization
- Commit objects that reference schema and instance layers

## Querying Layers through Commits

Commits contain references to both schema and instance layers. This allows you to see exactly which versions of your schema and data existed at any commit.

### Query Commit Layers

```graphql
query {
  Commit {
    identifier
    message
    author
    timestamp
    schema {
      layer_identifier
    }
    instance {
      layer_identifier
    }
  }
}
```

Example response:

```json
{
  "data": {
    "Commit": [
      {
        "identifier": "abc123...",
        "message": "Added new person records",
        "author": "admin",
        "timestamp": "1701234567.890",
        "schema": {
          "layer_identifier": "d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3"
        },
        "instance": {
          "layer_identifier": "e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4"
        }
      }
    ]
  }
}
```

### Schema vs Instance Layers

TerminusDB maintains separate layers for schema and instance data:

- **Schema Layer** - Contains your class definitions, properties, and relationships
- **Instance Layer** - Contains your actual document data

This separation allows schema changes without affecting instance data, and vice versa. Both layers are optional in a commit - a commit might only update the schema while keeping the same instance layer.

## Filtering Layer Queries

You can filter commits to find layers from specific points in time or with specific characteristics.

### Find Layers by Author

```graphql
query {
  Commit(filter: { author: { eq: "admin" } }) {
    identifier
    author
    schema {
      layer_identifier
    }
    instance {
      layer_identifier
    }
  }
}
```

### Find Latest Commits

```graphql
query {
  Commit(orderBy: { timestamp: DESC }, limit: 5) {
    identifier
    message
    timestamp
    schema {
      layer_identifier
    }
    instance {
      layer_identifier
    }
  }
}
```

## Navigating Layer History

You can traverse commit history to see how layers evolved over time.

### Query Parent Commits

```graphql
query {
  Commit {
    identifier
    message
    schema {
      layer_identifier
    }
    parent {
      identifier
      message
      schema {
        layer_identifier
      }
    }
  }
}
```

This query shows you the lineage of commits and their associated schema layers. By comparing `schema.layer_identifier` values between a commit and its parent, you can determine if the schema changed between commits.

### Detecting Layer Changes

If the schema identifier is the same between a commit and its parent, the schema did not change:

```json
{
  "identifier": "commit1",
  "schema": {
    "layer_identifier": "layer_abc123..."
  },
  "parent": {
    "identifier": "commit0",
    "schema": {
      "layer_identifier": "layer_abc123..."  // Same layer = no schema change
    }
  }
}
```

If the identifier differs, the schema was modified:

```json
{
  "identifier": "commit2",
  "schema": {
    "layer_identifier": "layer_def456..."  // Different layer = schema changed
  },
  "parent": {
    "identifier": "commit1",
    "schema": {
      "layer_identifier": "layer_abc123..."
    }
  }
}
```

## Querying Branches and Their Layers

Branches point to commits, which in turn reference layers. You can query branch heads to find the current layer state for any branch.

### Query Branch Head Layers

```graphql
query {
  Branch {
    name
    head {
      identifier
      schema {
        layer_identifier
      }
      instance {
        layer_identifier
      }
    }
  }
}
```

Example response:

```json
{
  "data": {
    "Branch": [
      {
        "name": "main",
        "head": {
          "identifier": "commit_xyz789...",
          "schema": {
            "layer_identifier": "layer_abc123..."
          },
          "instance": {
            "layer_identifier": "layer_def456..."
          }
        }
      },
      {
        "name": "development",
        "head": {
          "identifier": "commit_uvw456...",
          "schema": {
            "layer_identifier": "layer_ghi789..."
          },
          "instance": {
            "layer_identifier": "layer_jkl012..."
          }
        }
      }
    ]
  }
}
```

This allows you to compare layer identifiers across branches to understand how different branches have diverged. If two branches have the same `layer_identifier` values, they share identical content in those layers.

## Practical Use Cases

### Audit Layer Changes

Track when and why specific layers were created:

```graphql
query {
  Commit(orderBy: { timestamp: DESC }) {
    identifier
    message
    author
    timestamp
    schema {
      layer_identifier
    }
    instance {
      layer_identifier
    }
  }
}
```

### Find Commits Using a Specific Layer

Identify all commits that reference a particular layer identifier:

```graphql
query {
  Commit(filter: { 
    schema: { 
      layer_identifier: { eq: "a3f5e7b2c4d1f8e9a6b3c5d2e4f1a7b9c2d5e8f1" } 
    } 
  }) {
    identifier
    message
    author
    timestamp
  }
}
```

### Compare Layer States Across Branches

Determine if two branches share the same schema or instance layers:

```graphql
query {
  Branch {
    name
    head {
      schema {
        layer_identifier
      }
      instance {
        layer_identifier
      }
    }
  }
}
```

If two branches have the same `schema.layer_identifier` or `instance.layer_identifier`, they share that layer and have identical content in that graph.

## Working with Layer Identifiers

### Layer Identifier Format

Layer identifiers are always 40 hexadecimal characters (160 bits), representing a SHA-1 hash:

```
a3f5e7b2c4d1f8e9a6b3c5d2e4f1a7b9c2d5e8f1
```

### Storage Location

While you typically work with layer identifiers through GraphQL, TerminusDB stores the actual layer data in `.larch` files under the storage directory, organized by the first three characters of the identifier as a prefix:

```
storage/db_hash/a3f/a3f5e7b2c4d1f8e9a6b3c5d2e4f1a7b9c2d5e8f1.larch
```

### Content Addressing


Layer identifiers are SHA-1 hashes of the layer's logical content. TerminusDB's storage engine computes these identifiers when finalizing layers, providing:

1. **Deduplication** - Layers with identical content share the same identifier and storage
2. **Content Integrity** - The identifier serves as a cryptographic fingerprint
3. **Efficient References** - Multiple commits can reference the same layer without duplication

*Note: The identifier is computed from the layer's logical structure (RDF triples and dictionary), not from the serialized `.larch` file format.*

## Best Practices

### Performance Considerations

- Layer queries through the meta and commit graphs are generally fast because these graphs are small compared to your main data
- When querying commit history, use `limit` and `offset` to paginate results rather than loading all commits at once
- Layer identifiers are indexed, making filtering by identifier very efficient

### Using Layer Information

Layer identifiers are useful for:

- **Debugging** - Understanding what changed between versions
- **Auditing** - Tracking data and schema lineage
- **Integration** - Coordinating with external systems that need to track TerminusDB state
- **Optimization** - Identifying shared layers to understand storage efficiency

### Monitoring Repository Health

Query repository heads can be checked to know which layer is used to detect changes:

```graphql
query {
  Local {
    name
    head {
      layer_identifier
    }
  }
}
```

## Summary

Layer objects are the foundation of TerminusDB's version control system. Through GraphQL queries on the `_meta` and `_commits` system graphs, you can:

- Navigate repository structure and current state
- Traverse commit history to understand changes over time
- Compare layer identifiers to detect schema and data changes
- Track lineage across branches
- Audit when and why layers were created

By understanding how to query and interpret layer identifiers, you gain insight into TerminusDB's immutable storage model and can leverage insights from the version control metadata.

## See also

* [System Graph Interface to GraphQL](/docs/system-graph-graphql-interface-reference)