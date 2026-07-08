---
tags:
  - tutorial
  - documents
  - curl
  - rdf
  - enterprise
title: Multi-Format Document API with curl
nextjs:
  metadata:
    title: Multi-Format Document API with curl
    description: A hands-on tutorial for using the TerminusDB Document API with curl to work with JSON, JSON-LD, RDF/XML, and Turtle formats. Covers a full round-trip cycle for enterprise reference data management.
    keywords: document api, curl, json-ld, rdf xml, turtle, ttl, enterprise, reference data, multi-format
    alternates:
      canonical: https://terminusdb.org/docs/document-format-api-curl-tutorial/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
---

Enterprise reference data systems rarely live in isolation. Master data feeds into downstream analytics platforms, regulatory reporting pipelines, and partner integrations — each expecting a different serialization. A product taxonomy might need to be JSON for the internal API, JSON-LD for a linked-data catalog, RDF/XML for a standards body submission, and Turtle for human review or SPARQL tooling — all from the same source of truth.

The TerminusDB Enterprise multi-format Document API lets you store reference data once and retrieve or ingest it in any of these formats without writing conversion code. This tutorial walks through a complete round-trip using only `curl`, from database creation through schema design, data entry, multi-format export, and re-import.

## What you will learn

By the end of this tutorial you will be able to:

- Create a database and schema for operational reference data
- Insert documents using the standard JSON API
- Retrieve those documents as **JSON**, **JSON-LD**, **RDF/XML**, and **Turtle**
- Understand when each format is the right choice
- Re-import data from JSON-LD, RDF/XML, and Turtle back into TerminusDB
- Verify round-trip integrity across all four formats

## Prerequisites

- A running TwinfoxDB Enterprise instance (Docker or local) on `localhost:6363`
- `curl` installed (any recent version)
- Basic familiarity with JSON and HTTP

All commands in this tutorial use basic authentication with the default `admin:root` credentials. Adjust the password if you have changed it.

## Setup: encode your credentials

Every request needs an `Authorization` header. Set it once in your shell session so the remaining commands stay readable:

```bash
export BASE64=$(echo -n 'admin:root' | base64)
export SERVER="http://localhost:6363"
export DB="admin/ProductCatalog"
export AUTH="Authorization: Basic $BASE64"
```

The database path `admin/ProductCatalog` represents the `admin` organization and a data product called `ProductCatalog` — a realistic name for a reference data system that governs product master data across an enterprise.

---

## Step 1 — Create the database

A reference data system starts with a dedicated data product. Create it with a single POST:

```bash
curl -s -X POST \
  -H "Content-Type: application/json" \
  -H "$AUTH" \
  -d '{
    "label": "Product Catalog",
    "comment": "Enterprise product reference data",
    "schema": true
  }' \
  "$SERVER/api/db/$DB"
```

**Expected response:**

```json
{"@type":"api:DbCreateResponse","api:status":"api:success"}
```

**Why this matters:** In an enterprise setting, each reference data domain — products, customers, locations — lives in its own data product. Version control, branching, and access control all operate at this level, giving data stewards an isolated workspace for each domain.

---

## Step 2 — Define the schema

A good reference data schema captures both the structure and the business rules. The schema below models a product catalog with categories and products, using subdocuments for nested structures and lexical keys for human-readable identifiers.

```bash
curl -s -X POST \
  -H "Content-Type: application/json" \
  -H "$AUTH" \
  -d '[
    {
      "@type": "Class",
      "@id": "Category",
      "@key": { "@type": "Lexical", "@fields": ["code"] },
      "@documentation": {
        "@comment": "A product category in the reference taxonomy"
      },
      "code": "xsd:string",
      "label": "xsd:string",
      "description": "xsd:string"
    },
    {
      "@type": "Class",
      "@id": "UnitOfMeasure",
      "@key": { "@type": "Lexical", "@fields": ["symbol"] },
      "@subdocument": [],
      "symbol": "xsd:string",
      "name": "xsd:string"
    },
    {
      "@type": "Class",
      "@id": "Product",
      "@key": { "@type": "Lexical", "@fields": ["sku"] },
      "@documentation": {
        "@comment": "A product in the enterprise catalog"
      },
      "sku": "xsd:string",
      "name": "xsd:string",
      "category": "Category",
      "price": "xsd:decimal",
      "weight_kg": "xsd:decimal",
      "unit": "UnitOfMeasure",
      "active": "xsd:boolean"
    }
  ]' \
  "$SERVER/api/document/$DB?author=admin&message=Initial+schema&graph_type=schema"
```

**Expected response:** A list of the created schema document IDs.

**Design notes:**

- **Lexical keys** (`@key`) produce deterministic, human-readable IDs like `Product/SKU-1001` — essential when external systems need stable references (they are full IRIs, with a common base prefix for the data product).
- **Subdocuments** (`@subdocument`) like `UnitOfMeasure` are embedded inside their parent and do not have independent lifecycle — perfect for value objects that only make sense in context
- **Typed fields** (`xsd:decimal`, `xsd:boolean`) enforce data quality at write time, preventing the "garbage in" problem that plagues loosely-typed reference stores.

---

## Step 3 — Insert reference data

Now populate the catalog with a category and two products. In a real system, this is where an ETL pipeline or data steward would load master data:

```bash
curl -s -X POST \
  -H "Content-Type: application/json" \
  -H "$AUTH" \
  -d '[
    {
      "@type": "Category",
      "code": "ELEC",
      "label": "Electronics",
      "description": "Consumer and industrial electronic products"
    },
    {
      "@type": "Product",
      "sku": "SKU-1001",
      "name": "Industrial Sensor Module",
      "category": "Category/ELEC",
      "price": 249.99,
      "weight_kg": 0.35,
      "unit": {
        "@type": "UnitOfMeasure",
        "symbol": "pcs",
        "name": "pieces"
      },
      "active": true
    },
    {
      "@type": "Product",
      "sku": "SKU-1002",
      "name": "Precision Thermocouple",
      "category": "Category/ELEC",
      "price": 87.50,
      "weight_kg": 0.12,
      "unit": {
        "@type": "UnitOfMeasure",
        "symbol": "pcs",
        "name": "pieces"
      },
      "active": true
    }
  ]' \
  "$SERVER/api/document/$DB?author=admin&compress_id=true&message=Initial+product+data"
```

**Expected response:** The full IRIs of the three inserted documents:

```json
[
  "terminusdb:///data/Category/ELEC",
  "terminusdb:///data/Product/SKU-1001",
  "terminusdb:///data/Product/SKU-1002"
]
```

The insert endpoint returns full IRIs rather than compressed IDs. The IDs themselves are deterministic and readable because of the lexical key strategy — `Product/SKU-1001` directly encodes the business key. When you retrieve documents with GET, the response uses compressed IDs by default (controlled by the `prefixed` parameter).

---

## Step 4 — Retrieve as JSON (default)

The default format is the standard TerminusDB JSON representation — compact, with compressed IRIs relative to the database context. This is the format your internal APIs and applications will typically use.

### Retrieve all products

```bash
curl -s -X GET \
  -H "$AUTH" \
  "$SERVER/api/document/$DB?type=Product&as_list=true" | jq
```

**Expected response:**

```json
[
  {
    "@id": "Product/SKU-1001",
    "@type": "Product",
    "active": true,
    "category": "Category/ELEC",
    "name": "Industrial Sensor Module",
    "price": 249.99,
    "sku": "SKU-1001",
    "unit": {
      "@id": "Product/SKU-1001/unit/UnitOfMeasure/pcs",
      "@type": "UnitOfMeasure",
      "name": "pieces",
      "symbol": "pcs"
    },
    "weight_kg": 0.35
  },
  {
    "@id": "Product/SKU-1002",
    "@type": "Product",
    "active": true,
    "category": "Category/ELEC",
    "name": "Precision Thermocouple",
    "price": 87.50,
    "sku": "SKU-1002",
    "unit": {
      "@id": "Product/SKU-1002/unit/UnitOfMeasure/pcs",
      "@type": "UnitOfMeasure",
      "name": "pieces",
      "symbol": "pcs"
    },
    "weight_kg": 0.12
  }
]
```

### Retrieve a single document by ID

```bash
curl -s -X GET \
  -H "$AUTH" \
  "$SERVER/api/document/$DB?id=Product/SKU-1001" | jq
```

**When to use JSON:** Internal microservices, REST API consumers, front-end applications, and any system that speaks idiomatic JSON. This is the leanest format with the smallest payload size.

---

## Step 5 — Retrieve as JSON-LD

JSON-LD adds semantic context to the same data, making it self-describing. Each document carries an `@context` that maps short property names to full IRIs — turning your product data into linked data without changing its shape.

### Stream mode (one context per document)

```bash
curl -s -X GET \
  -H "$AUTH" \
  "$SERVER/api/document/$DB?type=Product&format=jsonld" | jq
```

Each document in the response is a self-contained JSON-LD object:

```json
{
  "@context": {
    "@base": "terminusdb:///data/",
    "@vocab": "terminusdb:///schema#"
  },
  "@id": "Product/SKU-1001",
  "@type": "Product",
  "active": true,
  "category": "Category/ELEC",
  "name": "Industrial Sensor Module",
  "price": 249.99,
  "sku": "SKU-1001",
  "unit": {
    "@id": "Product/SKU-1001/unit/UnitOfMeasure/pcs",
    "@type": "UnitOfMeasure",
    "name": "pieces",
    "symbol": "pcs"
  },
  "weight_kg": 0.35
}
```

### List mode (shared context with @graph)

For batch processing, the list mode wraps all documents in a single JSON-LD envelope with a shared `@context`:

```bash
curl -s -X GET \
  -H "$AUTH" \
  "$SERVER/api/document/$DB?type=Product&format=jsonld&as_list=true" | jq
```

**Expected response:**

```json
{
  "@context": {
    "@base": "terminusdb:///data/",
    "@vocab": "terminusdb:///schema#"
  },
  "@graph": [
    {
      "@id": "Product/SKU-1001",
      "@type": "Product",
      "active": true,
      "category": "Category/ELEC",
      "name": "Industrial Sensor Module",
      "price": 249.99,
      "sku": "SKU-1001",
      "unit": {
        "@id": "Product/SKU-1001/unit/UnitOfMeasure/pcs",
        "@type": "UnitOfMeasure",
        "name": "pieces",
        "symbol": "pcs"
      },
      "weight_kg": 0.35
    },
    {
      "@id": "Product/SKU-1002",
      "@type": "Product",
      "active": true,
      "category": "Category/ELEC",
      "name": "Precision Thermocouple",
      "price": 87.50,
      "sku": "SKU-1002",
      "unit": {
        "@id": "Product/SKU-1002/unit/UnitOfMeasure/pcs",
        "@type": "UnitOfMeasure",
        "name": "pieces",
        "symbol": "pcs"
      },
      "weight_kg": 0.12
    }
  ]
}
```

### Using the Accept header instead

You can also request JSON-LD through content negotiation rather than the query parameter:

```bash
curl -s -X GET \
  -H "$AUTH" \
  -H "Accept: application/ld+json" \
  "$SERVER/api/document/$DB?type=Product&as_list=true" | jq
```

The result is identical. The `format=` query parameter takes precedence if both are present.

**When to use JSON-LD:** Data catalog registration, linked-data publishing, semantic web integrations, and any scenario where the consumer needs to resolve property names to globally unique IRIs. JSON-LD is also the format of choice when submitting reference data to industry consortia or standards bodies that require RDF-compatible metadata.

---

## Step 6 — Retrieve as RDF/XML

RDF/XML is the classic serialization for the Resource Description Framework, still required by many regulatory and standards-based systems. TerminusDB Enterprise serializes your documents directly into well-formed RDF/XML with proper namespace declarations.

```bash
curl -s -X GET \
  -H "$AUTH" \
  "$SERVER/api/document/$DB?type=Product&format=rdfxml"
```

**Expected response:**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF
    xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
    xmlns:xsd="http://www.w3.org/2001/XMLSchema#"
    xmlns:schema="terminusdb:///schema#"
    xml:base="terminusdb:///data/"
>
<schema:Product rdf:about="terminusdb:///data/Product/SKU-1001">
  <schema:active rdf:datatype="http://www.w3.org/2001/XMLSchema#boolean">true</schema:active>
  <schema:category rdf:resource="terminusdb:///data/Category/ELEC"/>
  <schema:name>Industrial Sensor Module</schema:name>
  <schema:price rdf:datatype="http://www.w3.org/2001/XMLSchema#decimal">249.99</schema:price>
  <schema:sku>SKU-1001</schema:sku>
  <schema:unit>
    <schema:UnitOfMeasure rdf:about="terminusdb:///data/Product/SKU-1001/unit/UnitOfMeasure/pcs">
      <schema:name>pieces</schema:name>
      <schema:symbol>pcs</schema:symbol>
    </schema:UnitOfMeasure>
  </schema:unit>
  <schema:weight_kg rdf:datatype="http://www.w3.org/2001/XMLSchema#decimal">0.35</schema:weight_kg>
</schema:Product>
<schema:Product rdf:about="terminusdb:///data/Product/SKU-1002">
  <schema:active rdf:datatype="http://www.w3.org/2001/XMLSchema#boolean">true</schema:active>
  <schema:category rdf:resource="terminusdb:///data/Category/ELEC"/>
  <schema:name>Precision Thermocouple</schema:name>
  <schema:price rdf:datatype="http://www.w3.org/2001/XMLSchema#decimal">87.50</schema:price>
  <schema:sku>SKU-1002</schema:sku>
  <schema:unit>
    <schema:UnitOfMeasure rdf:about="terminusdb:///data/Product/SKU-1002/unit/UnitOfMeasure/pcs">
      <schema:name>pieces</schema:name>
      <schema:symbol>pcs</schema:symbol>
    </schema:UnitOfMeasure>
  </schema:unit>
  <schema:weight_kg rdf:datatype="http://www.w3.org/2001/XMLSchema#decimal">0.12</schema:weight_kg>
</schema:Product>
</rdf:RDF>
```

You can also use the Accept header:

```bash
curl -s -X GET \
  -H "$AUTH" \
  -H "Accept: application/rdf+xml" \
  "$SERVER/api/document/$DB?type=Product"
```

**Key observations about the RDF/XML output:**

- **Namespace declarations** (`xmlns:schema`, `xmlns:rdf`, `xmlns:xsd`) are generated from your database context
- **Typed literals** carry `rdf:datatype` attributes — `xsd:decimal` for prices, `xsd:boolean` for flags
- **String values** omit the datatype (the RDF default)
- **References** to other documents use `rdf:resource` (e.g., the `category` link)
- **Subdocuments** are nested inline as child elements

**When to use RDF/XML:** Regulatory submissions (pharmaceutical, financial, government data standards), SKOS taxonomy exchange, OWL ontology alignment, and integration with legacy triple stores or SPARQL endpoints that require RDF/XML as their ingest format.

---

## Step 7 — Retrieve as Turtle

Turtle (Terse RDF Triple Language) is a compact, human-readable RDF serialization. Where RDF/XML is verbose and XML-centric, Turtle uses prefixed names, semicolons for predicate grouping, and square brackets for blank nodes — making it easy to read, diff, and hand-edit.

```bash
curl -s -X GET \
  -H "$AUTH" \
  "$SERVER/api/document/$DB?type=Product&format=turtle"
```

**Expected response:**

```turtle
@base <terminusdb:///data/> .
@prefix schema: <terminusdb:///schema#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .

<terminusdb:///data/Product/SKU-1001>
  a schema:Product ;
  schema:active "true"^^xsd:boolean ;
  schema:category <terminusdb:///data/Category/ELEC> ;
  schema:name "Industrial Sensor Module" ;
  schema:price "249.99"^^xsd:decimal ;
  schema:sku "SKU-1001" ;
  schema:unit [
    a schema:UnitOfMeasure ;
    schema:name "pieces" ;
    schema:symbol "pcs"
  ] ;
  schema:weight_kg "0.35"^^xsd:decimal .

<terminusdb:///data/Product/SKU-1002>
  a schema:Product ;
  schema:active "true"^^xsd:boolean ;
  schema:category <terminusdb:///data/Category/ELEC> ;
  schema:name "Precision Thermocouple" ;
  schema:price "87.50"^^xsd:decimal ;
  schema:sku "SKU-1002" ;
  schema:unit [
    a schema:UnitOfMeasure ;
    schema:name "pieces" ;
    schema:symbol "pcs"
  ] ;
  schema:weight_kg "0.12"^^xsd:decimal .
```

You can also use the Accept header:

```bash
curl -s -X GET \
  -H "$AUTH" \
  -H "Accept: text/turtle" \
  "$SERVER/api/document/$DB?type=Product"
```

**Key observations about the Turtle output:**

- **`@prefix` declarations** map short prefixes to full IRIs — the same role as JSON-LD `@context` and RDF/XML `xmlns:`
- **`@base`** sets the base IRI for resolving relative references
- **`a`** is Turtle shorthand for `rdf:type`
- **Typed literals** use `^^` notation — `"249.99"^^xsd:decimal`, `"true"^^xsd:boolean`
- **String values** are plain quoted literals without a datatype
- **References** to other documents use full IRIs in angle brackets
- **Subdocuments** are nested as blank node blocks using `[ ... ]`

**When to use Turtle:** Human review of RDF data, version control diffs where line-oriented output makes changes easy to spot, SPARQL tooling, academic data exchange, and debugging scenarios where compact readable output helps trace data issues.

---

## Step 8 — Import from JSON-LD

The round-trip works in both directions. You can POST documents as JSON-LD and TerminusDB will parse the `@context`, resolve IRIs, and insert them into the instance graph. Here we are also remapping `@id` and `@type` into `id` and `type` using the context, to enable easier document exchange with other systems.

First, let's create a new product by submitting JSON-LD:

```bash
curl -s -X POST \
  -H "Content-Type: application/ld+json" \
  -H "$AUTH" \
  -d '{
    "@context": {
      "@base": "terminusdb:///data/",
      "@vocab": "terminusdb:///schema#",
      "id": "@id",
      "type": "@type"
    },
    "type": "Product",
    "id": "Product/SKU-2001",
    "sku": "SKU-2001",
    "name": "Fiber Optic Transceiver",
    "category": "Category/ELEC",
    "price": 189.00,
    "weight_kg": 0.08,
    "unit": {
      "type": "UnitOfMeasure",
      "symbol": "pcs",
      "name": "pieces"
    },
    "active": true
  }' \
  "$SERVER/api/document/$DB?author=admin&message=Import+from+JSON-LD"
```

**Expected response:**

```json
["terminusdb:///data/Product/SKU-2001"]
```

The `Content-Type: application/ld+json` header tells TerminusDB to parse the body as JSON-LD. The `@context` in the payload is used to resolve any prefixed or relative IRIs back to full identifiers.

**Why this matters:** When a partner sends you their product master data as JSON-LD (perhaps exported from their own linked-data platform), you can ingest it directly without pre-processing. The semantic context travels with the data.

---

## Step 9 — Import from RDF/XML

You can also submit documents as RDF/XML. This is particularly valuable when receiving data from systems that only export in RDF formats — common in government, healthcare, and standards bodies. TwinfoxDB automatically reflows the document to match the internal structure of the document and its subdocuments.

```bash
curl -s -X POST \
  -H "Content-Type: application/rdf+xml" \
  -H "$AUTH" \
  -d '<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF
    xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
    xmlns:xsd="http://www.w3.org/2001/XMLSchema#"
    xmlns:schema="terminusdb:///schema#"
    xml:base="terminusdb:///data/"
>
<schema:Product rdf:about="terminusdb:///data/Product/SKU-3001">
  <schema:sku>SKU-3001</schema:sku>
  <schema:name>Calibration Reference Standard</schema:name>
  <schema:category rdf:resource="terminusdb:///data/Category/ELEC"/>
  <schema:price rdf:datatype="http://www.w3.org/2001/XMLSchema#decimal">425.00</schema:price>
  <schema:weight_kg rdf:datatype="http://www.w3.org/2001/XMLSchema#decimal">1.20</schema:weight_kg>
  <schema:unit>
    <schema:UnitOfMeasure rdf:about="terminusdb:///data/Product/SKU-3001/unit/UnitOfMeasure/pcs">
      <schema:symbol>pcs</schema:symbol>
      <schema:name>pieces</schema:name>
    </schema:UnitOfMeasure>
  </schema:unit>
  <schema:active rdf:datatype="http://www.w3.org/2001/XMLSchema#boolean">true</schema:active>
</schema:Product>
</rdf:RDF>' \
  "$SERVER/api/document/$DB?author=admin&message=Import+from+RDF/XML"
```

**Expected response:**

```json
["terminusdb:///data/Product/SKU-3001"]
```

The `Content-Type: application/rdf+xml` header triggers the RDF/XML parser. TerminusDB reads the namespace declarations, resolves `rdf:about` URIs to document IDs, maps `rdf:datatype` to schema types, and inserts the documents into the instance graph — all validated against your schema.

**Why this matters:** Consider a regulatory agency that publishes approved product classifications as RDF/XML. Instead of writing a custom parser, you POST the file directly into your reference data system. Schema validation catches any structural issues immediately, and the data is instantly queryable alongside your existing catalog.

---

## Step 10 — Import from Turtle

Turtle input works the same way. Set `Content-Type: text/turtle` and submit a Turtle document with `@prefix` declarations and typed literals:

```bash
echo '@prefix schema: <terminusdb:///schema#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

<terminusdb:///data/Product/SKU-4001>
  a schema:Product ;
  schema:sku "SKU-4001" ;
  schema:name "RF Signal Analyzer" ;
  schema:category <terminusdb:///data/Category/ELEC> ;
  schema:price "1850.00"^^xsd:decimal ;
  schema:weight_kg "3.40"^^xsd:decimal ;
  schema:unit [
    a schema:UnitOfMeasure ;
    schema:symbol "pcs" ;
    schema:name "pieces"
  ] ;
  schema:active "true"^^xsd:boolean .' | \
curl -s -X POST \
  -H "Content-Type: text/turtle" \
  -H "$AUTH" \
  --data-binary @- \
  "$SERVER/api/document/$DB?author=admin&message=Import+from+Turtle" 
```

The reason for the odd syntax is that the document starts with `@` which is treated as a file reference by `curl`, so we use stdin here instead.

**Expected response:**

```json
["terminusdb:///data/Product/SKU-4001"]
```

The `Content-Type: text/turtle` header triggers the Turtle parser. TerminusDB reads the `@prefix` declarations, resolves prefixed names to full IRIs, maps `^^xsd:decimal` to schema types, and inserts the documents into the instance graph — all validated against your schema.

**Why this matters:** Turtle is the format most commonly produced by SPARQL `CONSTRUCT` queries, academic data repositories, and linked-data tooling. Being able to ingest Turtle directly means you can integrate data from these sources without an intermediate conversion step.

---

## Step 11 — Verify the round-trip

Now confirm that all six products — the original two, the JSON-LD import, the RDF/XML import, and the Turtle import — are in the database and structurally consistent:

```bash
curl -s -X GET \
  -H "$AUTH" \
  "$SERVER/api/document/$DB?type=Product&as_list=true" | jq
```

You should see six Product documents. Let's verify the imported ones individually:

### Check the JSON-LD import

```bash
curl -s -X GET \
  -H "$AUTH" \
  "$SERVER/api/document/$DB?id=Product/SKU-2001" | jq
```

### Check the RDF/XML import

```bash
curl -s -X GET \
  -H "$AUTH" \
  "$SERVER/api/document/$DB?id=Product/SKU-3001" | jq
```

### Check the Turtle import

```bash
curl -s -X GET \
  -H "$AUTH" \
  "$SERVER/api/document/$DB?id=Product/SKU-4001" | jq
```

All three imports should have the same structure as the originals — the same fields, the same types, the same subdocument nesting. The format used for insertion is transparent; once stored, every document is a first-class citizen regardless of how it arrived.

### Full-format cross-check

For a thorough verification, retrieve the same document in all four formats and confirm the data is identical:

```bash
echo "=== JSON ==="
curl -s -X GET -H "$AUTH" "$SERVER/api/document/$DB?id=Product/SKU-4001"

echo ""
echo "=== JSON-LD ==="
curl -s -X GET -H "$AUTH" "$SERVER/api/document/$DB?id=Product/SKU-4001&format=jsonld"

echo ""
echo "=== RDF/XML ==="
curl -s -X GET -H "$AUTH" "$SERVER/api/document/$DB?id=Product/SKU-4001&format=rdfxml"

echo ""
echo "=== Turtle ==="
curl -s -X GET -H "$AUTH" "$SERVER/api/document/$DB?id=Product/SKU-4001&format=turtle"
```

All four representations describe the same underlying graph — a single product with its properties, types, and relationships. The format is just a view over the immutable, versioned data.

---

## Step 12 — Replace a document using JSON-LD

Multi-format support extends to updates as well. Use PUT with `Content-Type: application/ld+json` to replace a document:

```bash
curl -s -X PUT \
  -H "Content-Type: application/ld+json" \
  -H "$AUTH" \
  -d '{
    "@context": {
      "@base": "terminusdb:///data/",
      "@vocab": "terminusdb:///schema#"
    },
    "@id": "Product/SKU-2001",
    "@type": "Product",
    "sku": "SKU-2001",
    "name": "Fiber Optic Transceiver (Gen 2)",
    "category": "Category/ELEC",
    "price": 199.00,
    "weight_kg": 0.07,
    "unit": {
      "@type": "UnitOfMeasure",
      "symbol": "pcs",
      "name": "pieces"
    },
    "active": true
  }' \
  "$SERVER/api/document/$DB?author=admin&message=Update+via+JSON-LD"
```

The document is replaced in place. Because TerminusDB is immutable under the hood, the previous version remains in the commit history — retrievable at any time through the version control endpoints.

---

## Step 13 — Clean up

When you are done experimenting, remove the test database:

```bash
curl -s -X DELETE \
  -H "$AUTH" \
  "$SERVER/api/db/$DB"
```

---

## Format selection guide

Choosing the right format depends on who is consuming the data and what they need:

{% table %}

- Scenario
- Format
- Reason

---

- Internal REST API
- JSON
- Smallest payload, fastest parsing, native to most frameworks

---

- Data catalog or metadata registry
- JSON-LD
- Self-describing with semantic context, compatible with linked-data tooling

---

- Regulatory or standards submission
- RDF/XML
- Required by many government and industry bodies, compatible with SPARQL endpoints

---

- Partner data exchange (inbound)
- JSON-LD, RDF/XML, or Turtle
- Accept whatever your partner produces, no custom parser needed

---

- Backup and migration
- JSON
- Compact, easy to diff, works with TerminusDB version control

---

- Knowledge graph federation
- JSON-LD
- Context enables merging data from multiple sources without IRI conflicts

---

- Human review and debugging
- Turtle
- Compact, line-oriented, easy to read and hand-edit

---

- Version control diffs
- Turtle
- Line-oriented format produces clean, readable diffs in pull requests

---

- SPARQL tooling
- Turtle
- Native format for SPARQL CONSTRUCT output and academic data exchange

{% /table %}

## Key takeaways

Multi-format support in TerminusDB Enterprise means your reference data system speaks the language of every downstream consumer. A few points worth remembering:

- **The data is format-independent.** Documents are stored as typed, schema-validated graphs. JSON, JSON-LD, RDF/XML, and Turtle are just views over that graph.
- **Round-trips are lossless.** Export as Turtle, send it to a partner, receive their edits back as JSON-LD, and import — nothing is lost or distorted.
- **Schema validation applies to every format.** Whether you POST JSON, JSON-LD, RDF/XML, or Turtle, the data is checked against the schema before it is committed. Bad data is rejected with a clear error regardless of the input format.
- **Content negotiation is standard HTTP.** Use the `Accept` header or the `format` query parameter — both work, and the query parameter takes precedence when both are present.
- **Version control covers everything.** Every insert, update, and replace — in any format — creates an immutable commit in the history. You can always go back to see exactly what was submitted and when.

## Further reading

- [Document API Reference Guide](/docs/document-insertion/) — full parameter reference for the document endpoint
- [How to use the HTTP Documents API](/docs/http-documents-api/) — authentication and connection examples
- [Document Graph API Howto](/docs/document-graph-api/) — overview of the document and schema endpoints
- [Documents in a knowledge graph](/docs/documents-explanation/) — how documents and graphs work together in TerminusDB
- [Immutability and version control](/docs/immutability-explanation/) — how commits and branches provide an audit trail
