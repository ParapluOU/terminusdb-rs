---
tags:
  - tutorial
  - documents
  - curl
  - beginner
  - self-hosted
title: Getting Started
nextjs:
  metadata:
    title: Getting Started
    description: Technical documentation for TerminusDB and the DFRNT TerminusDB cloud.
    keywords: terminusdb, document, document database, documents, getting started, json-ld
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/get-started/
media: []
---

## Getting started with TerminusDB in less than 15 minutes

TerminusDB is a database with version history. It stores a graph of documents as database records. Documents can either have an enforced structure or be unstructured JSON.

If you are not familiar with the command line, we recommend to get started with the [DFRNT](https://dfrnt.com/hypergraph-content-studio/) cloud environment instead. It provides a complete web based user interface for TerminusDB, that you can even use the cloud solution with your local Docker TerminusDB instance!

### Let's get started!

In this quickstart tutorial, we assume you have docker installed already. Use the [Docker installation guide](https://docs.docker.com/get-docker/) if you don't have Docker installed already.

You can tick the boxes to help track progress through our tutorials. They are stored in your browser's localStorage, so you can come back to them later.

Copy and paste each command into a Linux/macOS terminal. If you have docker and curl on Windows, the steps are similar.

We will build a structured and unstructured JSON document repository in TerminusDB. The documents form your Knowledge Graph. It is a graph of documents where the contents can be traversed as nodes and edges, this is what we will accomplish on this page:

1. Create a database
2. Build unstructured JSON store
3. Build structure-enforced JSON Linked Data store (core strength)

{% task-heading id="get-started-install-docker" number="1" %}
Start TerminusDB with Docker
{% /task-heading %}

This command will download the TerminusDB docker image, start a container with TerminusDB running, and open port 6363. Your data will be stored in a volume called `terminusdb_storage`.

```bash
# Download and run latest TerminusDB docker image on port 6363
export TERMINUSDB_ADMIN_PASS=root
docker run --pull always -d -p 127.0.0.1:6363:6363 -e TERMINUSDB_ADMIN_PASS -v terminusdb_storage:/app/terminusdb/storage --name terminusdb terminusdb/terminusdb-server:v12
```

To stop the container again, just run `docker stop terminusdb`. Your data is stored in a `terminusdb_storage` volume on your computer, so it will persist even if the container is stopped. To restart it again, just hit `docker restart terminusdb`. Make sure to make a note of your password, it will be set only when the storage is initialized.

{% task-heading id="get-started-create-database" number="3" %}
Create a database called 'starter'
{% /task-heading %}

We connect to TerminusDB and create a new database called 'starter' for our work. It is stored in the `terminusdb_storage` volume.

```bash
# Create a new database called 'starter'
curl -u admin:root -X POST "http://127.0.0.1:6363/api/db/admin/starter" \
  -H "Content-Type: application/json" \
  -d '{ "label": "Starter DB", "comment": "" }'
```

You will receive a database creation status message with database metadata and a resource identifier.

Learn more: [Document API Reference Guide](/docs/http-documents-api/)

{% task-heading id="get-started-insert-first-json-document" number="4" %}
Insert a named (unstructured) JSON document 
{% /task-heading %}

Note the `@id` field, it is used to identify the document.

```bash
curl -u admin:root -X POST "http://127.0.0.1:6363/api/document/admin/starter?raw_json=true&author=me&message=First+document" \
  -H "Content-Type: application/json" \
  -d '{ "@id":"JSONDocument/hello-world", "hello": "world" }'
```

You will get a fully qualified id back as a response. It is `terminusdb:///data/JSONDocument/hello-world`. You can use just the part `JSONDocument/hello-world` as an id if you don't change the schema context information.

Learn more: [Document API Reference Guide](/docs/document-insertion/)

{% task-heading id="get-started-retrieve-json-document" number="5" %}
Retrieve the document using the `@id` field
{% /task-heading %}

We query the document by id. Note that we don't need to use the fully qualified id, just the part around the last `/`.

```bash
curl -u admin:root -X GET "http://127.0.0.1:6363/api/document/admin/starter?id=JSONDocument/hello-world"
```

{% task-heading id="get-started-update-json-document" number="6" %}
Update the document
{% /task-heading %}

Here we write a new version of the JSON document.

```bash
curl -u admin:root -X PUT "http://127.0.0.1:6363/api/document/admin/starter?raw_json=true&author=me&message=Update+document" \
  -H "Content-Type: application/json" \
  -d '{"@id":"JSONDocument/hello-world","hello":"world2"}'
```

You will receive the id back as a response.

{% task-heading id="get-started-delete-json-document" number="7" %}
Delete the document again
{% /task-heading %}

No need to keep this test document, let's delete it.

```bash
curl -u admin:root -X DELETE "http://127.0.0.1:6363/api/document/admin/starter?id=JSONDocument/hello-world&author=me&message=Delete+document"
```

You will receive an empty response (HTTP 204) which means success.

{% task-heading id="get-started-create-structured-document" number="8" %}
Create a structured document
{% /task-heading %}

A structured document has a schema that enforces the document shape. Let's create a schema "Person" name, that can be connected to other Person documents into a graph of documents.

```bash
curl -u admin:root -X POST "http://127.0.0.1:6363/api/document/admin/starter?graph_type=schema&author=me&message=Add+Person+schema" \
  -H "Content-Type: application/json" \
  -d '[{  
    "@type": "Class",
    "@id": "Person",
    "name": "xsd:string",
    "manages": { "@type": "Set", "@class": "Person" }
  }]'
```

{% task-heading id="get-started-insert-first-structured-document" number="9" %}
Insert a Person document
{% /task-heading %}

```bash
curl -u admin:root -X POST "http://127.0.0.1:6363/api/document/admin/starter?author=me&message=Add+person" \
  -H "Content-Type: application/json" \
  -d '{
    "@id": "Person/Alice",
    "@type": "Person",
    "name": "Alice",
    "manages": []
  }'
```

You will receive `["terminusdb:///data/Person/Alice"]` as a response, which is equivalent to `Person/Alice` based on the default context document.

{% task-heading id="get-started-retrieve-structured-document" number="10" %}
Retrieve the document using the `@id` field
{% /task-heading %}

```bash
curl -u admin:root -X GET "http://127.0.0.1:6363/api/document/admin/starter?id=Person/Alice"
```

You will receive the structured document as a response.

{% task-heading id="get-started-update-structured-document" number="11" %}
Update the document again
{% /task-heading %}


```bash
curl -u admin:root -X PUT "http://127.0.0.1:6363/api/document/admin/starter?author=me&message=Add+person" \
  -H "Content-Type: application/json" \
  -d '{
    "@id": "Person/Alice",
    "@type": "Person",
    "name": "Alice Johnson",
    "manages": []
  }'
```

Check that the document was updated with the previous GET request.

{% task-heading id="get-started-delete-structured-document" number="12" %}
Delete the document 
{% /task-heading %}

```bash
curl -u admin:root -X DELETE "http://127.0.0.1:6363/api/document/admin/starter?id=Person/Alice&author=me&message=Delete+person"
```

## Done!

You have now completed the very initial steps of using TerminusDB. You can now explore the rest of the documentation from here.

The next step now is to install a Javascript or Python client that will help you interact effectively with TerminusDB. You can use the HTTP API directly, and construct WOQL by hand, but we recommend all users to start with a Javascript or Python client, unless using TerminusDB as a pure JSON document store.

Continue here (Javascript is the primary client):

* [Javascript Client](/docs/connect-with-the-javascript-client/).
* [Python Client](/docs/connect-with-python-client/)


## Other alternatives

You can continue with querying languages from here:
* [GraphQL](/docs/graphql-basics/),
* [REST](/docs/document-insertion/),
* [WOQL](/docs/woql-explanation/).