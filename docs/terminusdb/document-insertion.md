---
title: Document API Reference Guide
nextjs:
  metadata:
    title: Document API Reference Guide
    description: A reference guide for the TerminusDB document API interface.
    keywords: terminusdb, api, document, document api reference guide, document database, documents, javascript, json-ld
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/document-insertion/
media: []
tags:
  - typescript
  - documents
  - reference
---

This page is the HTTP API reference for TerminusDB's document interface — the REST endpoints you use to create, read, update, and delete documents and schema. If you are using a client SDK, you won't call these endpoints directly, but understanding them helps you debug and write advanced integrations.

TerminusDB exposes two endpoints:

- **`/api/document/`** — CRUD operations on instance documents and schema documents. All documents are validated against the schema on every write.
- **`/api/schema/`** — A convenience endpoint for reading schema information with inheritance resolved (shows all properties available on a class, including inherited ones).

## The document endpoint

### Getting documents

All document retrieval is done through GET requests on the following endpoint:

```http
GET /api/document/<resource path>
```

Where resource path is the usual strings like `admin/foo` for database foo, or `_system` for the system graph, or `admin/foo/_meta` for the metadata graph of the foo database, etc.

By default, this will return a stream of all documents to be found at this location. What exactly is returned can be modified using parameters, which are to be provided as query parameters.

#### Parameters

{% table %}

- parameter
- default
- explanation

--- 

- graph\_type
-
- either instance or schema. Used to switch between getting documents from the instance or the schema graph.

---

- type
-
- If given, only documents of the given type are returned.

---

- id
- 
- If given, only the document with the given ID is returned.

---

- ids
- 
- A JSON array of document IDs to retrieve. Example: `["Person/Alice","Person/Bob"]`. Takes precedence over `type`.

---

- compress\_ids
- true
- If true (the default), compress all IDs and type names using the prefixes defined in the database. Replaces the deprecated `prefixed` parameter.

---

- minimized
- true
- If true (the default), forego pretty printing, and return the documents with very little whitespace. Each json document will be on its own line.

---

- unfold
- true
- If true (the default), any subdocuments contained in the returned document are returned too. If false, these are referred to by their ID instead.

---

- skip
- 0
- How many results to skip

---

- count
- 
- How many results to return. If this option is absent, all results are returned.

---

- as\_list
- false
- If true, don't return a stream of json objects, but a list. This makes parsing the json easier in some environments.

---

- format
- json
- Output serialization format. `json` is available in all editions. `jsonld`, `rdfxml`, and `turtle` are enterprise-only and return HTTP 400 on community editions. The format can also be requested via the `Accept` header; the query parameter takes precedence.

{% /table %}

#### Alternate query mechanism

The above table shows parameters that are supposed to be provided as query parameters. There's however another mechanism, where instead, the parameters are passed in as a posted JSON document. In this calling style, an additional parameter is allowed, `"query"`, by which the returned documents are filtered by matching against some template.

The alternative method uses a POST rather than a get, specifies the header `X-HTTP-Method-Override: GET`, and posts a JSON document with the various query parameters instead:

```json
{
    "@rdf:type": "Person",
    "count": 10,
    "query": { "age": 42 },
}
```

The above example would find the first 10 documents of class `Person`, whose age is 42.

This may provide a more convenient style for querying from a library, especially when a (large) query document has to be provided for filtering purposes. However, unlike a pure GET request with query parameters, a POST with a method override does not result in a page that can be bookmarked in a browser. If that is desirable, the GET style is better.

### Posting documents

All new document submission is done through POST requests on the following endpoint:

```http
POST /api/document/<resource path>
```

Where resource path is the usual strings like `admin/foo` for database foo, or `_system` for the system graph, or `admin/foo/_meta` for the metadata graph of the foo database, etc.

The documents to be submitted are given as post data. Multiple documents can be specified at once, either as a stream of JSON objects or as a JSON list containing the documents to be inserted. If a document is specified that already exists, and overwrite is false (the default), an error is returned.

#### Parameters

{% table %}

- parameter
- default
- explanation

---

- author
- 
- The commit author

---

- message
- 
- The commit message

---

- graph\_type
- instance
- either instance or schema. Used to switch between submitting to the instance or the schema graph.

---

- full\_replace
- false
- If true, all existing documents are deleted before inserting the posted documents. This allows the full replacement of the contents of a database. This is especially useful for replacing the schema.

---

- raw\_json
- false
- If true, the input documents are treated as raw JSON , inserted as type `sys:JSONDocument` and are not subject to schema restrictions.

---

- require\_migration
- false
- If true, require that a schema migration is specified when the schema changes.

---

- merge\_repeats
- false
- If true, duplicate document IDs in the same request are silently merged instead of causing an error.

---

- overwrite
- false
- If true, existing documents with the same ID will be overwritten instead of causing an error. This enables upsert behavior.

---

- allow\_destructive\_migration
- false
- If true, allow schema migrations that delete or change existing data.

{% /table %}

#### Result

After a successful post, the result is a list of the IDs of the inserted documents.

### Replacing documents

Existing documents can be replaced through a PUT request on the following endpoint:

```http
PUT /api/document/<resource path>
```

Where resource path is the usual strings like `admin/foo` for database foo, or `_system` for the system graph, or `admin/foo/_meta` for the metadata graph of the foo database, etc.

The documents to be submitted are given as post data. Multiple documents can be specified at once, either as a stream of JSON objects or as a JSON list containing the documents to be replaced. If a document is specified that does not exist in the database, an error is returned unless `create` is set to `true` in which case it is inserted.

#### Parameters

{% table %}

- parameter
- default
- explanation

---

- author
- 
- The commit author

---

- message
- 
- The commit message

---

- graph\_type
- instance
- either instance or schema. Used to switch between submitting to the instance or the schema graph.

---

- create
- false
- insert if the document was not already in the database.

---

- raw\_json
- false
- If true, the replaced documents are treated as raw JSON , they must be replacing a document of type `sys:JSONDocument` and they are not subject to schema restrictions.

---

- require\_migration
- false
- If true, require that a schema migration is specified when the schema changes.

---

- merge\_repeats
- false
- If true, duplicate document IDs in the same request are silently merged instead of causing an error.

---

- allow\_destructive\_migration
- false
- If true, allow schema migrations that delete or change existing data.

{% /table %}

### Deleting documents

Existing documents can be deleted through a DELETE request on the following endpoint:

```http
DELETE /api/document/<resource path>
```

Where resource path is the usual strings like `admin/foo` for database foo, or `_system` for the system graph, or `admin/foo/_meta` for the metadata graph of the foo database, etc.

#### Parameters

{% table %}

- parameter
- default
- explanation

---

- author
- 
- The commit author

---

- message
- 
- The commit message

---

- graph\_type
- instance
- either instance or schema. Used to switch between submitting to the instance or the schema graph.

---

- id
- 
- If given, the document to delete. If not given, it is expected that the post data will contain a list of ids to delete.

---

- nuke
- false
- If true, delete everything at this resource location (dangerous!).

{% /table %}

#### Specifying what documents to delete

As shown above, deleting a single document can be done through query parameters alone. If multiple documents are to be deleted at once, a document has to be posted of the following format:

```json
[ "..id 1..",
  "..id 2..",
  ...
]
```

In other words, a JSON list of document IDs.

### ID Capture for Doc Insert & Replace

When inserting or replacing several documents at once, it may occur that some of these documents need to refer to each other. However, at insertion time, you may not know what the IDs of the new documents are going to be. This is especially the case for document types that generate their identifier randomly, but even for non-random key types, it may be convenient to rely on the server's ID generation algorithm, rather than trying to predict what IDs will get generated. Therefore, in order to support cross-references between inserted documents, the document interface allows you to capture generated document IDs in a variable, and then refer to that variable later in other documents.

#### Capturing an identifier into a variable

When inserting or replacing a document that you want to refer to in another document inserted in the same operation, use a `@capture` key in the document to associate the generated identifier with a variable. For example,

```json
{ "@type": "Person",
  "@capture": "Id_Tom",
  "name": "Tom"
}
```

This stores the generated ID in a variable called `ID_Tom` for the duration of the document insert/replace operation.

It is allowed to capture an ID and then never actually refer to it.

It is an error to capture the same variable twice. Doing so will result in a `api:CaptureIdAlreadyBound` error with the following shape:

```json
{"@type": "api:CaptureIdAlreadyBound",
 "api:capture": "..capture id..",
 "api:document": {..document where previously captured variable was captured again..}
```

#### Referring to an identifier using a variable

When inserting or replacing a document that needs to refer to another document inserted in the same operation, you can use a json dictionary of the form `{"@ref": "..id.."}` in place of an ordinary id. For example,

```json
{ "@type": "Person",
  "name": "Jerry",
  "rival": {"@ref": "Id_Tom"}
}
```

It is an error to refer to a variable that is never captured. Doing so will result in a `api:NotAllCapturesFound` error of the following shape:

```json
{ "@type": "api:NotAllCapturesFound",
  "api:captures": [..list of capture ids that were referenced but not found..]
}
```

#### Ordering of documents

ID captures and ID references can be done in any order. That means that when you are submitting several documents, you're allowed to refer to a captured ID in an earlier document. This also allows you to do cross-references, where two documents refer to each other:

```json
{ "@type": "Person",
  "@capture": "Id_Tom",
  "name": "Tom",
  "rival": {"@ref": "Id_Jerry"}
}
{ "@type": "Person",
  "@capture": "Id_Jerry",
  "name": "Jerry",
  "rival": {"@ref": "Id_Tom"}
}
```

In this example, Tom refers to Jerry, even though at that point in the submitted document stream, Jerry has not yet been processed. This is not a problem - both Tom and Jerry will get inserted referring to each other.

#### Self-reference

Using ID capture, it is possible to create a document that refers to itself:

```json
{ "@type": "Person",
  "@capture": "Captured_Id",
  "name": "Elmo",
  "friend": {"@ref": "Captured_Id"}
}
```

This will make Elmo be his own friend.

#### ID capture only works within a single operation

It is important to keep in mind that the ID capture mechanism only works within a single call to the document api. It is not possible to capture an ID in one operation, and then refer to it in a second operation. The `@capture` and `@ref` instructions do not get saved into the database. They are processed immediately and are then forgotten.

If you need to refer to a document already in the database, the only way to do so is by referring to its ID.

## The schema endpoint

The schema endpoint can be used to query information about classes in a resource. These queries happen through a GET on the following endpoint:

```http
GET /api/schema/<resource path>
```

Where resource path is the usual strings like `admin/foo` for database foo, or `_system` for the system graph, or `admin/foo/_meta` for the metadata graph of the foo database, etc.

The purpose of this endpoint is to quickly discover the supported fields of a particular type. The primary envisioned use case for this is the automatic generation of forms and other UI elements, as well as client code generation.

### Parameters

{% table %}

- parameter
- default
- explanation

---

- type
- 
- If given, the type to get information for. If omitted, information for all types is returned.

{% /table %}

#### Result

The result of this GET is a stream of documents describing all the types in a particular resource.

### Schema-checking and schemaless mode

The schema endpoint can also be used to switch between schema checking and schemaless mode.

Switching between checking or not checking does not delete the schema itself. After disabling schema checking it is still possible to update the schema or to query it through the schema endpoint. However, when disabled, it is possible to submit documents that do not match the schema. Re-enabling schema checking is only possible if all the documents in the given resource match the current schema.

```http
POST /api/schema/<resource path>
```

#### Parameters

{% table %}

- parameter
- default
- explanation

---

author

 - author
 - 
 - The commit author

---

 - message
 - 
 - The commit message

---

 - schema\_checking
 - 
 - Value should be either enabled or disabled

{% /table %}

## The apply endpoint

The apply endpoint takes the difference between any two commits and applies it to a branch. This is useful for cherry-picking changes or squash-merging between branches.

```http
POST /api/apply/<resource path>
```

Where resource path is the usual strings like `admin/foo` for database foo, or `admin/foo/local/branch/dev` for the `dev` branch of `admin/foo`.

The request body is a JSON document containing the following fields:

### Parameters

{% table %}

- parameter
- default
- explanation

---

 - before\_commit
 - 
 - The first commit to compare in order to produce a diff

---

 - after\_commit
 - 
 - The last commit to compare in order to produce a diff

---

 - commit\_info
 - 
 - A JSON document with author and message

---

 - match\_final\_state
 - false
 - If true, ignores conflicts when the final state would be the same regardless.

---

- keep
- 
- A JSON object specifying which fields to keep from the target branch on conflict. Example: `{"@id": true}`

{% /table %}

#### Result

The result of this POST request is either an updated branch with a successful application of the difference between two commits, or an error giving the reason for an unresolvable conflict.

## Version control commit id feedback

When a commit is made, the `TerminusDB-Data-Version` header is returned, which is the reference to the commit made in the [version history](/docs/immutability-explanation/). This header can be used to retrieve the exact copy of the document from the specific version in the immutable history.

To get the specific version, use the branchspec `org/dataproduct/local/commit/:commitId` where `:commitId` is the value of the right hand side of the colon separated value in the `TerminusDB-Data-Version` header.

Each commit references a specific layer version of one or more branches of a specific data product. Practically, the layer that is referenced by the commit could be part of multiple data products follwing a clone, push or pull process. 

## Further Reading

* [Documents in a knowledge graph and how to use them](/docs/documents-explanation/)
* [Immutability, version history, and revision control](/docs/immutability-explanation/)
