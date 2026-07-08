---
title: Diff an Object or a Database Branch with the JS Client
nextjs:
  metadata:
    title: Diff an Object or a Database Branch with the JS Client
    description: A guide to show how to use the JS Client to diff an object or a database branch.
    keywords: terminusdb, branch, compare, delta, diff, javascript, patch, terminusdb javascript client
    openGraph:
      images: https://github.com/terminusdb/terminusdb-web-assets/blob/master/docs/js-client-collaboration-diff-patch.png?raw=true
    alternates:
      canonical: https://terminusdb.org/docs/diff-and-patch-operations/
media: []
lastUpdated: "2026-05-18"
tags:
  - typescript
  - diff-patch
  - how-to
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- A database with data (two states to compare)
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will know how to compute diffs between database states and apply patches.
{% /callout %}

TerminusDB provides **structural diff and patch** operations — compare any two documents, branches, or commits to see exactly what changed at the field level, then apply those changes programmatically. This is git-like version control for data, not line-based text diff.

## Diff an object

Return the diff from two objects

```javascript
const diffObjects = async () => {
   const before = { "@id" : "Person/Jane", "@type" : "Person", "name" : "Jane"}
   const after = { "@id" : "Person/Jane", "@type" : "Person", "name" : "Janine"}
   const options = {keep:{ "@id" : true }}

   //in the options you can list the properties that you would like to always include in the diff result.
   const diffResult = await client.getJSONDiff(before, after, options)

   console.log("the diff result ", JSON.stringify(diffResult,null,4))
}
```

Here is an example of a diff result between two objects

```json
{
   "name":{
      "@op":"ValueSwap",
      "@before":"Jane",
      "@after":"Janine"
   },
   "@id":"Person/Jane"
}
```

## Get the patch of differences between branches or commits.

```javascript
const diffDocsVersion = async () => {
   const beforeVersion = "a73ssscfx0kke7z76083cgswszdxy6l"
   const afterVersion =  "73rqpooz65kbsheuno5dsayh71x7wf4"
   const options = {keep:{ "@id" : true, "name" : true }}

   const diffResult = await client.getVersionDiff(beforeVersion, afterVersion, null, options)

   console.log("the diff result ", JSON.stringify(diffResult,null,4))
}
```

Here is the example result

```json
[
   {
      "@id":"Person/Jane",
      "@type":"Person",
      "name" : "Jane"
      "age":{
         "@after":23,
         "@before":22,
         "@op":"SwapValue"
      }
   },
   {
      "@id":"Person/Tom",
      "@type":"Person",
      "name" : "Tom"
      "age":{
         "@after":10,
         "@before":null,
         "@op":"SwapValue"
      }
   }
]
```

## Get the patch of difference between a document and an object.

```javascript
const diffDocToObject = async () => {
   const jsonObject = { "@id" : "Person/Jane", "@type" : "Person", "name" : "Jannet"}
   const options = {keep:{ "@id" : true, "name" : true }}

   //in the options you can list the properties that you would like to see in the diff result.
   const diffResult = await client.getVersionObjectDiff("main", jsonObject, "Person/Jane", options)

   console.log("the diff result ", JSON.stringify(diffResult,null,4))
}
```

## Next steps

- [**JSON Diff & Patch reference**](/docs/json-diff-and-patch/) — HTTP API reference for diff and patch endpoints
- [**Merge branches**](/docs/merge-howto/) — apply diffs between branches with conflict detection
- [**Time travel**](/docs/time-travel-howto/) — query historical states to compare snapshots
- [**Branch your database**](/docs/branch-howto/) — create isolated workspaces for safe experimentation