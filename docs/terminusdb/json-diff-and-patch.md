---
title: JSON Diff and Patch with TerminusDB
nextjs:
  metadata:
    title: JSON Diff and Patch with TerminusDB — compare and patch JSON documents
    description: Compute structural diffs between JSON documents and apply them as patches with the TerminusDB JSON Diff and Patch API. Works standalone against any two JSON objects, or against TerminusDB's immutable history to compare two commits or layers. Includes a no-auth quickstart, HTTP endpoint reference, curl examples, JavaScript and Python client examples, and the full set of patch operation types.
    keywords: json diff, json patch, json diff and patch, terminusdb diff api, terminusdb patch api, structural json diff, json document comparison, json delta, document version comparison, json diff python, json diff javascript, json diff curl, dataversion diff, commit diff, json patch operations, swap value, copy list, modify table, terminusdb
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/json-diff-and-patch/
media: []
lastUpdated: "2026-05-18"
tags:
  - typescript
  - diff-patch
  - how-to
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running on `localhost:6363`
- An HTTP client (curl, Postman, or similar)
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to compute JSON diffs and apply patches via the HTTP API.
{% /callout %}

TerminusDB provides a **JSON Diff and Patch API** that computes structural differences between JSON documents and applies those differences as patches. Use it to compare document versions, audit changes, synchronise distributed copies, or build collaborative editing workflows.

**Diff** computes what changed between two JSON objects. **Patch** applies a previously computed diff to transform one object into another.

The API works in two modes:

- **Standalone** — compare any two JSON documents you supply directly in the request. No database required; useful for comparing snapshots from any source.
- **Versioned** — compare a stored document, or every document, between two points in TerminusDB's immutable history. The database holds the *before* state for you, so you can ask for the difference between two commits, branches, or layers without keeping copies of the old data yourself.

If you want to see it running before reading further, jump to the [Quickstart](#quickstart-try-it-without-installation).

## How diff and patch work

### Diff: identify what changed

A **diff** takes two JSON objects and presents any differences between them. Diff has several uses. A key use is displaying a clear summary of differences between large objects, enhancing the visibility of changes. This enables manual, user-interface assisted, or client actions to resolve differences. Actions include:

*   Retain the original object.
*   Change to the new (or latest) version of the object.
*   Create a new version of the object.

### Patch: apply a change

A **patch** applies a diff to two objects to obtain a new object with any differences highlighted. A patch is applied individually or in bulk to a patch endpoint that will apply the patch to the specified data product.

## Quickstart — try it without installation

The diff and patch endpoints are available anonymously on the public data server at `data.terminusdb.org` — no authentication required:

{% callout type="warning" %}
The endpoint at data.terminusdb.org is for non-production and exploratory use only. It is deployed with very limited memory to prevent abuse and is provided without any guarantees whatsoever. Do not rely on it for production applications. Availability is not guaranteed. It also scales to zero to save energy and can take a few seconds to spin back up when idle.
{% /callout %}

```bash test-example id="diff-public-demo" file="examples/diff-public-demo.example.sh"
```

Expected output:
```json
{"name": {"@after":"Bob", "@before":"Alice", "@op":"SwapValue"}}
```

```bash test-example id="patch-public-demo" file="examples/patch-public-demo.example.sh"
```

Expected output:
```json
{"name": "Bob"}
```

## Diff and Patch Endpoints

The Patch and Diff endpoints are available on any TerminusDB instance. Use your local server endpoint for each operation:

**JSON Diff**

```text 
http://localhost:6363/api/diff
```

**JSON Patch**

```text
http://localhost:6363/api/patch
```

> **Note:** The former cloud endpoints (`cloud.terminusdb.org/jsondiff` and `cloud.terminusdb.org/jsonpatch`) are no longer available. Use your own TerminusDB instance or [create a DFRNT account](https://dfrnt.com/create-account) for hosted access.

### Diff

The diff endpoint takes a POST of two JSON documents, _before_, and _after_. This endpoint then returns a 200 and a patch which takes _before_ to _after_ if applied using the patch interface.

The payload is structured as a JSON document with one of the following forms:

*   With `"before"` and `"after"`, pointing to the documents you would like to diff.
*   With `"before_data_version"`, `"after"` and `"document_id"`, specifying the data version or commit ID with which to compare the given _after_ document.
*   With `"before_data_version"`, `"after_data_version"` and `"document_id"` specifying the data version or commit ID with which to compare the document given by `"document_id"`
*   With `"before_data_version"`, `"after_data_version"`, meaning that we would like to get a diff for _all_ documents between the two specified data versions.

There are also two options:

*   `keep`: A dictionary which has keys which need to be copied
*   `copy_value`: Which specifies that we should make _explicit_ which values existed during a list copy.

An example of the payload:

```json
{ "before" : { "@id" : "Person/Jane", "@type" : "Person", "name" : "Jane"},
  "after" :  { "@id" : "Person/Jane", "@type" : "Person", "name" : "Janine"}}
```

Which would result in the following patch:

```json
{ "name" : { "@op" : "SwapValue", "@before" : "Jane", "@after": "Janine" }}
```

An example of a payload comparing commits or dataversions:

```json
{ "before_data_version" : "branch:s7dde27gyj8ezat3itw5nr3peu1lymh",
  "document_id" : "terminusdb:///data/test/665df8a9c3a58be6db622be4b37a76bea46c3e5e3cd2db923e708e574d1566be",
  "after" :  { "@id" : "Person/Jane", "@type" : "Person", "name" : "Janine"}}
```

An example of a payload comparing only dataversions:

```json
{ "before_data_version" : "branch:s7dde27gyj8ezat3itw5nr3peu1lymh",
  "after_data_version" : "branch:jb81rgx9lzow35r3pkrsvdf5l75kaq",
  "document_id" : "terminusdb:///data/test/665df8a9c3a58be6db622be4b37a76bea46c3e5e3cd2db923e708e574d1566be"}
```

#### Diff examples using curl

```bash test-example id="diff-swap-with-keep" file="examples/diff-swap-with-keep.example.sh"
```

Expected output:
```json
{
  "asdf": {"@after":"bar", "@before":"foo", "@op":"SwapValue"},
  "fdsa":"bar"
}
```

```bash test-example id="diff-array-object-swap" file="examples/diff-array-object-swap.example.sh"
```

Expected output:
```json
[{"asdf": {"@after":"bar", "@before":"foo", "@op":"SwapValue"}}]
```

```bash test-example id="diff-list-append" file="examples/diff-list-append.example.sh"
```

Expected output:
```json
{
  "@op":"CopyList",
  "@rest": {
    "@after": [3 ],
    "@before": [],
    "@op":"SwapList",
    "@rest": {"@op":"KeepList"}
  },
  "@to":3
}
```

```bash test-example id="diff-list-append-copy-value" file="examples/diff-list-append-copy-value.example.sh"
```

Expected output:
```json
{
  "@op":"CopyList",
  "@rest": {
    "@after": [3 ],
    "@before": [],
    "@op":"SwapList",
    "@rest": {"@op":"KeepList", "@value": []}
  },
  "@to":3,
  "@value": [0, 1, 2 ]
}
```

```bash test-example id="diff-nested-object-swap" file="examples/diff-nested-object-swap.example.sh"
```

Expected output:
```json
{
  "asdf": {"fdsa": {"@after":"quuz", "@before":"quux", "@op":"SwapValue"}}
}
```

### Patch

Patch takes a POST with a _before_ document and a _patch_ and produces an _after_ document.

```json
{ "before" : { "@id" : "Person/Jane", "@type" : "Person", "name" : "Jane"}
  "patch" : {"name" : { "@op" : "ValueSwap", "@before" : "Jane", "@after": "Janine" }}}
```

Resulting in the following document:

```json
{ "@id" : "Person/Jane", "@type" : "Person", "name" : "Janine"}
```

#### Patch examples using curl

Apply a patch that swaps a nested field value:

```bash test-example id="patch-nested-object" file="examples/patch-nested-object.example.sh"
```

Expected output:
```json
{"alpha":1, "asdf": {"fdsa":"quuz"}}
```

Append an element to a list using a `CopyList` + `SwapList` patch:

```bash test-example id="patch-list-append" file="examples/patch-list-append.example.sh"
```

Expected output:
```json
[0, 1, 2, 3]
```

## Use the DFRNT hosted endpoint

The diff and patch endpoints can be used directly as an API without running your own TerminusDB instance, using the [dfrnt.com](https://dfrnt.com/hypergraph-content-studio/) cloud hosting.

Use the hosted endpoint for each operation. Have your API token ready to run the commands below.

**JSON Diff example**

Take your token and your username — both available in the profile section — and use them with the example below, which calls the cloud diff operation.

```bash
TOKEN=01234567-0123-0123-0123...
DFRNT_USER=00000000-0000-0000...
curl -H "Authorization: Token $TOKEN" -X POST -H "Content-Type: application/json" "https://dfrnt.com/api/hosted/${DFRNT_USER}/api/diff" -d \
  '{ "before" : { "asdf" : "foo", "fdsa" : "bar"}, "after" : { "asdf" : "bar", "fdsa" : "bar"}, "keep" : { "fdsa" : true}}'
```

Result:
```json
{"asdf": {"@after":"bar", "@before":"foo", "@op":"SwapValue"},"fdsa":"bar"}
```

**JSON Patch example**

Take your token and your username — both available in the profile section — and use them with the example below, which calls the cloud patch operation.

```bash
TOKEN=01234567-0123-0123-0123...
DFRNT_USER=00000000-0000-0000...
curl -H "Authorization: Token $TOKEN" -X POST -H "Content-Type: application/json" "https://dfrnt.com/api/hosted/${DFRNT_USER}/api/patch" -d \
  '{ "before" : { "alpha" : 1, "asdf" : { "fdsa" : "quux"}}, "patch" : {
      "asdf": {"fdsa": {"@after":"quuz", "@before":"quux", "@op":"SwapValue"}}
}}'
```

Result:
```json
{"alpha":1, "asdf": {"fdsa":"quuz"}}
```

See [Diff and Patch Endpoints](#diff-and-patch-endpoints) for the full request specification, and examples of [diff](#diff-examples-using-curl) and [patch](#patch-examples-using-curl) using curl.

## Use a TerminusDB client library

Use JSON Diff and Patch from a TerminusDB JavaScript or Python client to find and handle changes in TerminusDB schemas and documents, JSON schemas, and other document databases such as MongoDB.

### Requirements

Install a [JavaScript](/docs/install-terminusdb-js-client/) or [Python](/docs/install-terminusdb-js-client/) TerminusDB client.

### Get started

Follow the simple steps below.

> If using **TerminusDB with Python**, connect to your TerminusDB cloud instance first — see [Connect with the Python Client](/docs/connect-with-python-client/) for instructions if required.

1.  [Create an endpoint](#create-an-endpoint)

2.  [Compute a diff](#compute-a-diff)

3.  [Review the patch](#review-the-patch)

4.  [Apply the patch](#apply-the-patch)
    

### Create an endpoint

Create a client endpoint with `WOQLClient`.

#### Create an endpoint with the JavaScript Client

```javascript
const TerminusClient = require("@terminusdb/terminusdb-client");

var client = new TerminusClient.WOQLClient("http://localhost:6363")
```

#### Create an endpoint with the Python Client

```python
from terminusdb_client import WOQLClient

client = WOQLClient("http://localhost:6363/")
```

### Compute a diff

Compute the difference between two hypothetical documents — `Doc1` and `Doc2` — to produce a patch.

#### Compute a diff — JS

Use `getJSONDiff`:

```javascript
let result_patch = await client.getJSONDiff(Doc1, Doc2)
```

#### Compute a diff — Python

Use `diff`:

```text
result_patch = client.diff(Doc1, Doc2)
```

### Review the patch

Print the contents of a patch.

#### Review — JS

```javascript
console.log(result_patch)
```

#### Review — Python

Example uses `pprint` (`from pprint import pprint`):

```python
pprint(result_patch.content)
```

### Apply the patch

Apply the patch to `Doc1`.

#### Apply — JS

```javascript
let after_patch = await client.patch(Doc1, result_patch);
```

#### Apply — Python

```python
after_patch = client.patch(Doc1, result_patch)
```

## Patch operation types

A patch is a JSON document made up of one or more **operation entries**. Each entry tells the patch engine what to do at a given location — swap a value, copy a list segment, modify a table region, and so on. The sections below catalogue every operation type the API understands.

Diff itself accepts two parameters that influence which operations appear in the output: `keep`, a document describing which fields *must* be copied in the final object, and a `copy_value` boolean flag that specifies whether to record the exact value in a copy operation.

### Copy

Copy is implicit. All properties which are not specifically mentioned will be considered part of an implicit copy. This will make patches more compressed and easier to specify by hand.

### Mandatory

`@before`/`@after` instructions contain objects specified as tightly as required to obtain ids, or as ids.

```typescript
{ '@id' : "Person/jim",
  'date_of_birth' : { '@op' : 'SwapValue',
                      '@before' : "1928-03-05",
                      '@after' : "1938-03-05"
                    }}
```

### Optional

Optional diffs also contain `@before`/`@after` designations, but potentially `null` fields to describe missing elements.

```typescript
{ '@id' : "Object/my_object",
  'name' : { '@op' : 'SwapValue',
             '@before' : null,
             '@after' : "Jim" }}
```

### Set / Cardinality

Set requires the ability to explicitly remove or add elements — we can do this by maintaining a `@before`/`@after` with a list of those which exist *only* on the left, and *only* on the right.

### List

The list diff requires swaps at a position. We use `@copy`, `@swap` and `@keep`.

#### Copy List

Copy the previous list from `From_Position` to `To_Position`.

```typescript
{ "@op" : "CopyList",
  "@to" : To_Position,
  "@rest" : Diff }
```

#### Swap List

Swap out the list starting from the current point from `Previous` to `Next`. This can be used to extend, or drop elements as well as do full replacement.

```typescript
{ "@op" : "SwapList",
  "@before" : Previous,
  "@after" : Next,
  "@rest" : Diff }
```

#### Patch List

Patch the list starting from the current point with the patch list in `"@patch"`. The patch must be less than or equal to the length of the list.

```typescript
{ "@op" : "PatchList",
  "@patch" : Patch,
  "@rest" : Diff }
```

#### List operation example

```javascript
var Patch =
{ '@id' : "TaskList/my_tasks",
  'tasks' : { '@op' : "CopyList",                      // Replace List
              '@to' : 2,
              '@rest' : { '@op' : "PatchList",
                          '@patch' : [{ '@op' : "SwapValue",
                                        '@before' : "Task/shopping",
                                        '@after' : "Task/climbing"},
                                      { '@op' : "SwapValue",
                                        '@before' : "Task/cleaning",
                                        '@after' : "Task/dining"},
                                      { '@op' : "SwapValue",
                                        '@before' : "Task/fishing",
                                        '@after' : "Task/travelling"}],
                          '@rest' : { '@op' : "KeepList" } } }}
var Before =
{ '@id' : "TaskList/my_tasks",
  'tasks' : ["Task/driving", "Task/reading", "Task/shopping",
             "Task/cleaning","Task/fishing", "Task/arguing"] }
var After =
{ '@id' : "TaskList/my_tasks",
  'tasks' : ["Task/driving", "Task/reading", "Task/climbing",
             "Task/dining", "Task/travelling", "Task/arguing"] }
```

### Array

Arrays allow index swapping or "shrink" and "grow".

### Force

A **force** patch (`ForceValue`) sets the value of a location regardless of the current read-state. This is a potentially unsafe operation as there is no guarantee we are seeing the object state version we think we are.

```typescript
{ '@id' : "Employee/012" ,
  'name' : { '@op' : 'ForceValue',
             '@after' : "Jake" }}
```

### Table

A table diff specifies the differences and similarities between two tables. The tables *need not* have the same dimensions. To describe these differences, we use a `ModifyTable` patch. The `ModifyTable` patch is comprised of:

- **`copies`** — sections of the table which can be copied verbatim.
- **`deletes`** — segments which are to be removed from the original.
- **`inserts`** — segments which are to be inserted into the new table.
- **`moves`** — segments that are the same in both tables, but have moved location. This is particularly useful as moving rows and columns is a typical operation in a table (such as a CSV or Excel document).

#### Table operation example

Given the following table:

```typescript
[['Job Title','Company','Location','Company Size','Company Industry'],
 ['Sr. Mgt.','Boeing','USA','Large','Aerospace'],
 ['Data Architect','Airbus','France','Large','Aerospace'],
 ['Founder','Ellie Tech','Sweden','Startup','AI'],
 ['Platform Engineer','Adidas','Germany','Large','Apparel']]
```

And a sorted version of the same (sorting on the first column):

```typescript
[['Job Title','Company','Location','Company Size','Company Industry'],
 ['Data Architect','Airbus','France','Large','Aerospace'],
 ['Founder','Ellie Tech','Sweden','Startup','AI'],
 ['Platform Engineer','Adidas','Germany','Large','Apparel'],
 ['Sr. Mgt.','Boeing','USA','Large','Aerospace']]
```

We have the following patch resulting from the diff:

```typescript
{'@op':"ModifyTable",
 dimensions:{'@after':[5,5],'@before':[5,5]},
 deletes:[],
 inserts:[],
 copies:[{'@at':{'@height':1,'@width':5,'@x':0,'@y':0},'@value':[['Job Title','Company','Location','Company Size','Company Industry']]}],
 moves:[{'@from':{'@height':1,'@width':5,'@x':0,'@y':1},
         '@to':{'@height':1,'@width':5,'@x':0,'@y':4},
         '@value':[['Sr. Mgt.','Boeing','USA','Large','Aerospace']]},
        {'@from':{'@height':1,'@width':5,'@x':0,'@y':2},
         '@to':{'@height':1,'@width':5,'@x':0,'@y':1},
         '@value':[['Data Architect','Airbus','France','Large','Aerospace']]},
        {'@from':{'@height':1,'@width':5,'@x':0,'@y':3},
         '@to':{'@height':1,'@width':5,'@x':0,'@y':2},
         '@value':[['Founder','Ellie Tech','Sweden','Startup','AI']]},
        {'@from':{'@height':1,'@width':5,'@x':0,'@y':4},
         '@to':{'@height':1,'@width':5,'@x':0,'@y':3},
         '@value':[['Platform Engineer','Adidas','Germany','Large','Apparel']]}]}
```

## Further Reading

**Client libraries:**

- JavaScript client [diff](/docs/javascript/#getjsondiffbeforeafteroptions) and [patch](/docs/javascript/#patchbeforepatch)
- Python client [diff](/docs/python/#diff) and [patch](/docs/python/#patchbeforepatch)

**Related guides:**

- [Git-like Version Control for JSON](/docs/version-controlled-json/) — Tutorial using diff and patch in a branching workflow
- [Patch Endpoint](/docs/patch-endpoint/) — Apply patches to documents via the HTTP API
- [Diff and Patch Operations](/docs/diff-and-patch-operations/) — API reference for diff and patch endpoints
- [How to Time-Travel](/docs/time-travel-howto/) — Query historical data and compare points in time
- [How to Branch](/docs/branch-howto/) — Create branches and compare them with diff
- [How to Merge](/docs/merge-howto/) — Merge branches with conflict handling
- [Version Control Operations](/docs/version-control-operations/) — Complete reference for all versioning endpoints