---
tags:
  - python
  - data-import
  - how-to
  - intermediate
title: Import Data with the Python Client
nextjs:
  metadata:
    title: Import Data with the Python Client
    description: A guide to show how to import CSV data into TerminusDB using the Python Client
    keywords: terminusdb, csv, data loading, import, import data with the python client, python, terminusdb python client
    openGraph:
      images: https://assets.terminusdb.com/docs/python-client-use-import-data.png
    alternates:
      canonical: https://terminusdb.org/docs/import-data-with-python-client/
media: []
---

{% callout type="note" %}
**Prerequisites**
- TerminusDB running locally — see [Docker setup](/docs/get-started/) for instructions
- The TerminusDB Python client installed ([installation guide](/docs/install-terminusdb-js-client/))
- A database with a schema defined
- Data to import (CSV, JSON, or DataFrame)
{% /callout %}

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have imported data from an external source into your TerminusDB database using the Python client.
{% /callout %}

This how-to assumes that you are already connected to a database and have a schema that matches the CSV you want to import.

## Importing a CSV file

You can import CSV files easily by importing them into dictionaries using Python's built-in libraries. Those dictionary objects can be inserted into the database using the `insert_document` function.

```python
import csv
objects = []
with open('test.csv', 'r') as f:
    csv_reader = csv.DictReader(f)
    objects = list(csv_reader)

# Add @type to each row to match your schema class
for obj in objects:
    obj['@type'] = 'YourClassName'

client.insert_document(objects)
```

{% callout type="note" %}
If your database has only one document class, TerminusDB can infer the `@type` automatically. When you have multiple classes, you must set `@type` on each document explicitly.
{% /callout %}

## Next steps

- [**Get documents**](/docs/get-documents/) — verify your imported data by retrieving documents
- [**Add a schema**](/docs/add-a-schema/) — define your document types before importing
- [**WOQL queries**](/docs/run-woql-query/) — query and transform imported data with Datalog
- [**Branch your database**](/docs/branch-howto/) — import data on a branch, then merge once validated