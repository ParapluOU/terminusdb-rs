---
tags:
  - python
  - woql
  - tutorial
  - intermediate
title: Python WOQL Customer Data Processing Example
nextjs:
  metadata:
    title: Python WOQL Customer Data Processing Example
    description: A guide to show how to perform basic document queries using the Python Client for customer management.
    keywords: python datalog
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/python-woql-customer-data-processing-example/
media: []
---

Use WOQL in the Python flavour for customer data processing 

Documentation contributed from the TerminusDB community by Dinis Bessa de Sousa, thank you!

## The problem

In the context of online customer development, we are storing data from purchases made by customers, where each purchase is made by a user at some (valid) time (that might be different from the time it was stored) through an online platform.

The attributes associated with a purchase include metrics closely related to the purchase itself, such as the number of articles bought and the total amount of the purchase, but also metrics related to the activity while on the website, such as the number of clicks, duration of time on the website, etc.

## Creating / changing a schema

In some past sections, we have seen how to set up a schema by having it defined as a class in Python. You can also do a [schema migration](/docs/schema-migration-reference-guide/) from an existing one via the [TerminusDB API](/docs/openapi/). Below, we add classes to an existing schema by using the following script:

```python
#Define the operations to alter the schema

schema = {
        "author": "admin",
        "message": "updating schema",
        "operations": [
            {
        
                "@type": "CreateClass",
                "class_document":
                {
                    "@id": "Attributes",
                    "@type": "Class",
                    "@key": 
                    {
                        "@type": "Random"
                    },
                    "@subdocument": []
                }
            }
            ,
            {
                "@type": "CreateClass",
                "class_document":
                {
                    "@id": "Purchase",
                    "@type": "Class",
                    "userId": { "@type" : "Optional", "@class" : "xsd:string" },
                    "attributes": "Attributes",
                    "at": "xsd:integer"
                }
            }
        ]
}

#Add some attributes to the Attributes subdocument

attributes = ["articles_bought", "n_clicks", "duration", "total"]
for attr_name in attributes:
    schema["operations"][0]["class_document"][attr_name] = { "@type" : "Optional", "@class" : "xsd:double" }

#POST the changes to the TerminusDB API

url = "http://localhost:6363/api/migration/admin/your_db"
headers = {
'Content-Type': 'application/json'
}
payload = json.dumps(schema)
username = os.getenv("TERMINUS_USER") #default is admin
password = os.getenv("TERMINUS_KEY") #default is root

response = requests.request("POST", url, headers=headers, data=payload, auth=HTTPBasicAuth(username, password))
```

This script modifies the schema by adding an Attributes class and a Purchase class. These two classes are linked, as the Attributes class is said to be a subdocument of Purchase. A [subdocument](/docs/documents-explanation/#subdocuments) can only be pointed to by its containing document, it's a kind of fieldset. Its information is internal to the identity of the document and not intended to be shared. This designation enables deeply nested JSON documents that are self-contained and retrievable using the TerminusDB document interface.

## Querying in WOQL with the Python Client

The main information need for this setting is to get all the purchases made by a certain customer (represented by a certain identifier) `userId` up to a certain date (represented by a `timestamp`).

Using the Python client, we can do that using the following query:

```python

from terminusdb_client import WOQLQuery as wq
from terminusdb_client import WOQLClient

(purchase, user_id, attributes_id, attributes, at) = wq().vars('purchase', 'user_id', 'attributes_id', 'attributes', 'at')

client = WOQLClient("http://localhost:6363/")
client.connect(db="...")

customer_id = "greatUserId" # String for a given userId
timestamp = 1735732800 # Wednesday, 1 January 2025 12:00:00 GMT+00:00

query = wq().select(attributes, at, 
            wq().woql_and(
                wq().select(attributes_id, at,
                    wq().woql_and(
                        wq().triple(purchase, "rdf:type", "@schema:Purchase"),
                        wq().triple(purchase, "userId", wq().string(customer_id)),
                        wq().triple(purchase, "attributes", attributes_id),
                        wq().triple(purchase, "at", at),
                        wq().woql_not(
                            wq().greater(at, timestamp)
                        ),
                    ),
                ),
                wq().read_document(attributes_id,  attributes),
                wq().order_by(at, order="asc")
            )
        )

result = client.query(query)

purchases = None

if result["bindings"]:
    purchases = result["bindings"]
else:
    print("Cannot find result.")
```

Now, to better understand what all this means, let's break it down into smaller parts!

> Note: WOQL allows fluent and functional (and even mixed) style of querying. In the functional style, subqueries are arguments for other queries, and triples can be linked by the `woql_and` function. However, we have noted that the fluent style may not always work as expected, so the functional style is advised. Learn more about WOQL and its styles and more [here](/docs/woql-explanation/).

### Initialise variables

In WOQL, variables and other strings have to have a prefix before them, to indicate what their use is. For example, all variables must be written as `v:variable_name` and if you are referencing a class from your schema you should write `@schema:ClassName`. [Reference](https://github.com/terminusdb/terminusdb-tutorials/blob/main/getting_started/python-client/lesson_7.md#woqlquery---making-logical-queries-with-triples).

The WOQL Query library makes it possible to use variables such as `v:purchase`, by using a proper variable and by using the `vars()` function, both work well.

```python
(purchase, user_id, attributes_id, attributes, at) = wq().vars('purchase', 'user_id', 'attributes_id', 'attributes', 'at')
```

### Filtering and Selecting

Firstly, we have to filter the purchases based on the `timestamp` and `customer_id`, both defined by us. The userId must be equal to `customer_id`, and the purchase must have happened before the given `timestamp`.

Since the attributes are stored as a subdocument fieldset, we first retrieve their document IDs before extracting details. This inner query is used then both to filter purchases and select the id of the corresponding attribute subdocuments and the time of purchase.

```python
customer_id = "greatUserId" # String for a given userId
timestamp = 1735732800 # Wednesday, 1 January 2025 12:00:00 GMT+00:00

(...)
wq().select(attributes_id, at,
    wq().woql_and(
        wq().triple(purchase, "rdf:type", "@schema:Purchase"),
        wq().triple(purchase, "userId", wq().string(customer_id)),
        wq().triple(purchase, "attributes", attributes_id),
        wq().triple(purchase, "at", at),
        wq().woql_not(
            wq().greater(at, timestamp)
        )
    ),
),
```

The `triple()` function is used to query triples from the information in the database. Note that it uses the unification approach to variables, borrowed from the logical Prolog engine you may already know. This means that the following approaches are equivalent in WOQL:

**Extract and check if equal:**

```python
wq().triple(purchase, "userId", user_id),
wq().eq(user_id, wq().string(customer_id))
```

**Do both in one single command:**

```python
wq().triple(purchase, "userId", wq().string(customer_id))
```

Then we filter by the time, using the WOQL query functions for comparisons. We can retrieve all that happened before a certain timestamp as timestamps that are not greater than it (or that are less than or equal to it).

**Not greater than:**

```python
wq().woql_not(
    wq().greater(at, timestamp)
),
```

**Less than or equal to:**

```python
wq().woql_or(
    wq().less(at, timestamp),
    wq().equal(at, timestamp)
),
```

Now, we have successfully collected all the IDs for the attribute subdocuments of the purchases, as well as the time they were made.

### Reading documents and ordering

The query is completed by reading the attributes given the attribute\_id and ordering the purchases by the time they were made:

```python
query = wq().select(attributes, at, 
            wq().woql_and(
                wq().select(attributes_id, at,
                    wq().woql_and(
                        wq().triple(purchase, "rdf:type", "@schema:Purchase"),
                        wq().triple(purchase, "userId", wq().string(customer_id)),
                        wq().triple(purchase, "attributes", attributes_id),
                        wq().triple(purchase, "at", at),
                        wq().woql_not(
                            wq().greater(at, timestamp)
                        ),
                    ),
                ),
                wq().read_document(attributes_id,  attributes),
                wq().order_by(at, order="asc")
            )
        )
```

All of the attributes from the attribute subdocument can be retrieved with the `read_document()` function. This function fetches the entire subdocument for a given attribute ID, allowing us to access all stored details of the purchase in this case.

Use the `order_by()` function to order the results in order to some variable, supporting both ascending (asc) and descending (desc) ordering.

```python
#Order by one attribute, ascending.
wq().order_by(at, order="asc")

#Order by more than one attribute, with different orderings.
wq().order_by(at, other_attribute, order=["asc", "desc"])
```

### Running the query and using the results

Now that the query is constructed and we have covered all of its parts, we only have to run the query!

```python
query = ...

result = client.query(query) #or result = query.execute(client)

purchases = None

if result["bindings"]:
    purchases = result["bindings"]
else:
    print("Cannot find result.")
```

Below there is an example of a response document, where the resulting documents are found on the `bindings` attribute of the respose document.

```json
{   
    '@type': 'api:WoqlResponse', 
    'api:status': 'api:success', 
    'api:variable_names': ['attributes', 'at'], 
    'bindings': [
                {
                    'at': {
                        '@type': 'xsd:integer',
                        '@value': 1700732800
                        }, 
                    'attributes': {
                        '@id': 'Purchase/f4edb522-9814-4658-9c24-0e49eb3bcdfa/attributes/Attributes/oBhtnc9pSmTvCSfN', 
                        '@type': 'Attributes', 
                        'articles_bought': 1.0, 
                        'n_clicks': 30.0,
                        'total': 5.0
                        }
                }, 
                {
                    'at': {
                        '@type': 'xsd:integer',
                        '@value': 1725732800
                        }, 
                    'attributes': {
                        '@id': 'Purchase/f4edb522-9814-4658-9c24-0e49eb3bcdfa/attributes/Attributes/oBhtnc9pSmTvCSfN',
                        '@type': 'Attributes',
                        'articles_bought': 1.0,
                        'duration': 400.2,
                        'total': 10.0,
                        }
                }, 
                ], 
    'deletes': 0, 
    'inserts': 0, 
    'transaction_retry_count': 0
}

```

> Note 2: The TerminusDB Python client's documentation is still under development. You can contribute to it by creating a pull request. In the meantime, check out the [TerminusDB Python client's repo](https://github.com/terminusdb/terminusdb-client-python/tree/main), were some more useful information can be found!
