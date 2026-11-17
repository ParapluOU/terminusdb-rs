---
tags:
  - python
  - schema
  - tutorial
  - beginner
  - graphql
title: Your First TerminusDB App with Pydantic
nextjs:
  metadata:
    title: Your First TerminusDB App with Pydantic — Tutorial
    description: Step-by-step tutorial for new Python users — declare a TerminusDB schema as pydantic models, write and read documents, query with GraphQL and WOQL, and evolve the schema safely.
    keywords: terminusdb, pydantic, python client, tutorial, TerminusModel, graphql, woql, getting started
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/pydantic-quickstart/
media: []
---

{% callout type="note" %}
Pydantic support ships with the upcoming TerminusDB 12.1 server release together with an updated Python client. Until then, the interface described here is available on the client's pydantic branch.
{% /callout %}

Build a small product-information-management (PIM) catalogue end to end: a category taxonomy, products with typed attributes, seeded test data, and queries in GraphQL. By the end you will know the model layer well enough to design your own schema — and where to go deeper when you need it.

**You will learn to**

- declare a TerminusDB schema as ordinary pydantic models
- write and read documents, and understand how links, subdocuments and tagged unions behave on the wire
- query your data with GraphQL (the everyday path) and WOQL (the escape hatch for graph traversal)
- evolve the schema safely with `diff`, `from_db` and `tdbpy`

**Prerequisites.** Python 3.10+, `pip`, and basic pydantic fluency (you can declare a `BaseModel` with typed fields). No TerminusDB knowledge assumed. For client installation details see [Use the Python Client](/docs/use-the-python-client); for server setup see [Create a Database](/docs/create-a-database).

## How TerminusDB thinks — the 60-second model

TerminusDB is a versioned document/graph database. Three ideas carry the whole system:

1. **Classes describe documents.** Every stored document has a `@type` — a class from the schema — and a `@id` minted from the class's *key*.
2. **Fields are values or links.** A field holding a primitive is a value. A field holding another model is a *link* — a graph edge to that document's id. Links are how the taxonomy, brand and media graphs form.
3. **Your pydantic model IS the schema.** You declare models once; the schema documents are generated from them, and documents coming back validate into the same models.

The mapping you will keep reaching for:

| pydantic | TerminusDB |
|---|---|
| `TerminusModel` subclass | class (`@type: Class`) |
| `_key = LexicalKey("sku")` | `@key` — id minted as `Class/value` |
| `str`, `int`, `float`, `bool` | `xsd:string/integer/decimal/boolean` |
| `decimal.Decimal`, `datetime.date` | `xsd:decimal`, `xsd:date` |
| `dict` | `sys:JSON` (schemaless blob) |
| `Optional[X]` / `List[X]` / `Set[X]` | `Optional`/`List`/`Set` property |
| `Annotated[X, Unfold]` | linked property marked `@unfold` |
| `Annotated[list[X], Array]` | `Array` (indexed, multi-dimensional) |
| another `TerminusModel` field | link to a document (or subdocument) |
| `TerminusEnum` | `@type: Enum` |
| `TerminusUnion` | `@type: TaggedUnion` |
| `_subdocument`, `_abstract`, `_shared`, `_unfoldable`, `_foreign`, `_label` | the matching `@`-markers and `@label` |
| `Field(min_length=, max_length=)` on a `Set` | `@min_cardinality`/`@max_cardinality` |
| docstring + `Field(title=, description=)` | `@documentation` label/comment/properties |

Everything in the right column is emitted automatically — the rest of this tutorial is learning to read and drive it.

## 1. Setup

Run a local TerminusDB (any install method works; Docker is the quickest):

```bash
docker run -d -p 6363:6363 --name terminusdb \
  terminusdb/terminusdb-server:latest
curl -sf http://127.0.0.1:6363/api/ok && echo "server up"
```

Create a virtualenv and install the client with the pydantic extra:

```bash
python3 -m venv .venv && .venv/bin/pip install "terminusdb[pydantic]" faker
```

The seed step later uses `faker`; any fake-data generator works.

## 2. Connect and commit your first model

A `PydanticSchema` is a namespace that collects your model classes; each model registers itself with `_schema = SCHEMA` at class-creation time.

```python
# catalog.py
from terminusdb_client.client.Client import Client
from terminusdb_client.schema import LexicalKey
from terminusdb_client.schema.pydantic import PydanticSchema, TerminusModel
from pydantic import Field

SCHEMA = PydanticSchema(title="Catalogue", authors=["you@example.com"])

class Brand(TerminusModel):
    """A product brand."""

    _schema = SCHEMA
    _key = LexicalKey("name")
    _label = "Brand"

    name: str = Field(title="Name", description="Brand name")
    country: str | None = None

client = Client("http://127.0.0.1:6363")
client.connect(user="admin", key="root", team="admin")
client.create_database("catalogue", label="Catalogue")
client.connect(user="admin", key="root", team="admin", db="catalogue")

SCHEMA.commit(client, "add Brand")
```

Three things to notice:

- `_key = LexicalKey("name")` makes the id `Brand/<name>` — meaningful and idempotent. Other schemes: `RandomKey`, `ValueHashKey` (id derived from content — deduplicates uploads), `UuidV7Key`, `HashKey`.
- The docstring, `_label` and `Field(title=, description=)` become the class's `@documentation` — write them for real; they are the schema's documentation surface.
- `connect` takes your credentials; the same credentials serve every API on the server — there is no extra auth layer for pydantic or GraphQL.

**Check your work** — the schema round-trips cleanly:

```python
assert SCHEMA.diff(client) == []        # emitted docs == stored docs
remote = PydanticSchema().from_db(client)
print(remote.object["Brand"].model_fields)   # models rebuilt from the DB
```

`diff` compares your local models against the schema graph — an empty diff means zero drift. `from_db` builds working models straight from the database; it is how tools and notebooks consume a schema without importing your code.

## 3. Write and read a document

```python
brand = Brand(name="Acme", country="SE")
client.insert_document(brand)

print(brand._backend_id)     # 'Brand/Acme' — stamped by insert
doc = client.get_document("Brand/Acme")
parsed = SCHEMA.parse_document(doc)
assert parsed == brand       # round-trips through pydantic validation
```

- `insert_document` serialises the model, writes it, and stamps `obj._backend_id`. Re-serialising that object anywhere else emits an id link (`{"@id": ...}`) instead of a duplicate document.
- `SCHEMA.parse_document` validates a raw document back into your model — drift fails loudly as a `ValidationError`, never silently.

## 4. Build the real model

Now the PIM shape — one construct at a time.

### Enums — controlled vocabularies

```python
from terminusdb_client.schema.pydantic import TerminusEnum

class AttributeKind(TerminusEnum):
    _schema = SCHEMA
    text = "text"; number = "number"; boolean = "boolean"; choice = "choice"
```

Stored as the string value; validated as the enum in both directions.

### Taxonomy — self-referencing links

```python
from typing import Optional, Set

class Category(TerminusModel):
    """A node in the product taxonomy."""
    _schema = SCHEMA
    _key = LexicalKey("code")
    _label = "Category"

    code: str = Field(title="Code", description="URL-safe category code")
    name: str
    parent: Optional["Category"] = None              # forward ref is fine
    attribute_definitions: Set["AttributeDefinition"] = set()
```

Links are plain model fields. On write you pass objects; on read the wire carries ids and the parsed model holds a stub carrying `_backend_id` — fetch it when you need it. That is the *fold* contract: reads are cheap by default and you expand what you ask for.

### Subdocuments — embedded value objects

```python
class AttributeAssignment(TerminusModel):
    """An attribute value bound to a product."""
    _schema = SCHEMA
    _subdocument = []                 # embedded: no id of its own
    _label = "Attribute assignment"

    attribute: AttributeDefinition    # a subdocument may link out
    value: "AttributeValue"
```

### Tagged unions — heterogeneous values

```python
class AttributeValue(TerminusUnion):
    """Exactly one member is set."""
    _schema = SCHEMA
    _label = "Attribute value"

    text: Optional[str] = None
    number: Optional[float] = None
    flag: Optional[bool] = None
    choice: Optional[str] = None
```

Union members become terminal value-nodes in the graph — they are stored as linked documents with minted ids, and queries select whichever member fields exist.

### Inheritance and abstract classes

```python
class Sellable(TerminusModel):
    """Anything with a price and a lifecycle status."""
    _schema = SCHEMA
    _abstract = []                    # no instances of Sellable
    _label = "Sellable item"

    name: str
    status: "ProductStatus"
    price: Optional[Decimal] = None   # xsd:decimal — exact, never float

class Product(Sellable):
    _schema = SCHEMA
    _key = LexicalKey("sku")
    _label = "Product"
    sku: str = Field(title="SKU", description="Stock-keeping unit")
    # ... fields below

class Bundle(Product):
    _schema = SCHEMA
    _label = "Product bundle"
    items: Set[Product] = set()
    discount_pct: Optional[float] = None
```

Markers are class-local: `Product` does not become abstract just because its parent is — the client emits `@abstract` only where you declare it. Subclass instances show up in queries for the parent type (`Product` results include bundles).

### Special-purpose classes

```python
class MediaAsset(TerminusModel):
    """Reference-counted, always expanded inline, content-addressed id."""
    _schema = SCHEMA
    _key = ValueHashKey()
    _shared = True
    _unfoldable = True
    _label = "Media asset"
    url: str
    kind: "MediaKind" = MediaKind.image
    alt: Optional[str] = None

class SupplierRef(TerminusModel):
    """Opaque IRI to a supplier owned by another data product."""
    _schema = SCHEMA
    _foreign = True
    _label = "Supplier reference"
```

- `@shared` — reference-counted: deleted when the last referrer goes away.
- `@unfoldable` — always arrives expanded inline on reads.
- `@foreign` — stores a bare IRI string, no referential integrity. Assign `SupplierRef(_id="sup-1")`; on the wire it is `"SupplierRef/sup-1"`.

### Product, assembled

```python
from typing import Annotated, List

class Product(Sellable):
    """A sellable product in the catalogue."""
    _schema = SCHEMA
    _key = LexicalKey("sku")
    _label = "Product"

    sku: str = Field(title="SKU", description="Stock-keeping unit")
    description: Optional[str] = None
    brand: Annotated[Optional[Brand], Unfold] = None   # arrives expanded
    categories: Set[Category] = set()                  # folded links
    attributes: Set[AttributeAssignment] = set()       # embedded subdocs
    media: Set[MediaAsset] = set()                     # shared, unfoldable
    keywords: List[str] = []                           # ordered
    highlights: Set[str] = Field(default_factory=set, max_length=5)
    launch_date: Optional[datetime.date] = None
    supplier: Optional[SupplierRef] = None
    extra: Optional[dict] = None                       # sys:JSON
    variant_matrix: Annotated[list[list[str]], Array] = []  # 2-D array
```

Two read-side behaviours worth internalising:

- **`Unfold` on a field** (like `brand`) means the link arrives *expanded* — the parsed model gets a real `Brand`. Ordinary links (`categories`) arrive folded as stubs. `_unfoldable` on the *class* means expanded on every read, no marker needed.
- **`Field(max_length=5)` on a `Set`** is both pydantic validation and the schema's `@max_cardinality` — one declaration, enforced twice.

## 5. Seed it with Faker

Build objects sharing Python references; insert entities first so later inserts see stamped ids and emit links:

```python
fake = Faker(); fake.seed_instance(42)

for stage in (attr_defs, root_categories, leaf_categories, brands,
              products, bundles):
    client.insert_document(stage, commit_msg="seed", overwrite=True)
```

`overwrite=True` makes the seed idempotent — rerun it safely. The `pydantic-getting-started` starter project implements `build_catalog` deterministically (same seed → same catalogue) plus a `python -m pim.seed --db pim_demo` CLI; lift the pattern wholesale.

## 6. Query with GraphQL

GraphQL is the everyday query interface: a schema is generated from your classes automatically, and the endpoint is plain HTTP with the *same* credentials you gave `client.connect` — no extra auth layer:

```python
import requests
GQL = "http://127.0.0.1:6363/api/graphql/admin/catalogue"

def gql(query, variables=None):
    r = requests.post(GQL, json={"query": query, "variables": variables},
                    auth=("admin", "root"))
    r.raise_for_status()
    return r.json()
```

Every class becomes an `OBJECT` type with the same field names. Query by type:

```graphql
{ Product(limit: 3) { _id sku status price
                      brand { name }
                      categories { name }
                      dimensions { width_mm } } }
```

Links expand as nested objects, subdocuments inline — GraphQL ignores the fold/unfold distinction entirely (it is a document-API concern).

**Filtering, ordering, paging:**

```graphql
{ Product(filter: {status: {eq: active}, name: {allOfTerms: "laptop"}},
          orderBy: {price: DESC}, limit: 20, offset: 40)
  { sku price } }
```

`eq`, `ne`, `lt`, `le`, `gt`, `ge`, `allOfTerms`, `anyOfTerms`, `startsWith`, `regex`, `and`/`or`/`not` combinators work on value fields — see [Filter with GraphQL](/docs/filter-with-graphql) and [Advanced Filtering](/docs/advanced-filtering-with-graphql). `_id` is always selectable; `_getDocument(id:)` fetches one; `_count` gives totals for paging — see the [GraphQL Query Reference](/docs/graphql-query-reference).

**Tagged unions are flat objects.** `AttributeValue` maps to an object with nullable members — select all of them, one is non-null:

```graphql
{ Product { sku attributes { attribute { code } value { text number flag choice } } } }
```

**Fragments — same-type only.** Named fragments work on the type they declare; aliases rename result keys freely:

```graphql
fragment Core on Product { sku status price }
{ A: Product(limit: 10) { ...Core }
  B: Bundle { sku items { sku } discount_pct } }
```

Class inheritance in TerminusDB does *not* generate GraphQL interfaces or unions — every class is an independent `OBJECT` type. So `... on Bundle` inside a `Product` selection fails validation even though `Product` results *include* Bundle instances. To get bundle-only fields, query `Bundle` directly (as `B:` above). The same rule means tagged unions need no inline fragments — they are objects, not GraphQL unions.

Mutations (`_insertDocuments`, `_replaceDocuments`, `_deleteDocuments`) exist on the endpoint — see [GraphQL Mutations](/docs/graphql-mutations) — but the document API covered in section 3 is the primary write path from Python, because it validates into your models.

## 7. When GraphQL isn't enough: WOQL

GraphQL answers "documents shaped like X matching Y". For graph-shaped questions — "all products anywhere under the `electronics` subtree" — use WOQL through `client.query`:

```python
from terminusdb_client.woqlquery.woql_query import WOQLQuery

q = (WOQLQuery()
     .path("v:cat", "parent*", "Category/electronics")   # ancestor traversal
     .triple("v:product", "categories", "v:cat")
     .triple("v:product", "rdf:type", "@schema:Product"))
for b in client.query(q)["bindings"]:
    print(b["product"])
```

`path(subject, "predicate*", object)` follows a link predicate zero or more hops — the workhorse for taxonomies, org charts and bills of materials. `triple` joins bind variables across document graphs. See [Path Queries in GraphQL](/docs/path-queries-in-graphql) for the GraphQL-flavoured version, and the WOQL guide for the full language.

## 8. Evolving the schema

Models change; the schema should never drift:

- **Add fields freely** — commit again; existing documents simply lack the new optional fields.
- **`SCHEMA.diff(client)`** — empty means your models and the schema graph agree exactly; use it in CI as a drift gate.
- **Schema-first teams:** edit the schema (dashboard or migration API), then `tdbpy sync` regenerates `schema.py` and `tdbpy check` guards CI — the full loop is documented in [Pydantic Schema Management](/docs/pydantic-schema-management).
- **`PydanticSchema().from_db(client)`** rebuilds runtime models with types, constraints and documentation intact — the right tool for consumers that should not import your model code.

Breaking changes (renames, type changes, moving fields between classes) are schema migrations — see the [Schema Migration Reference Guide](/docs/schema-migration-reference-guide) rather than dropping the database.

## Troubleshooting decoder

| Symptom | Means |
|---|---|
| `reifying_abstract_class` on insert | you instantiated an `@abstract` class — instantiate a concrete subclass |
| `foreign_type_not_id` | a `@foreign` property got a full document — assign the IRI object (`Ref(_id=...)`) so it serialises as a string |
| `document already exists` | reinserting a stamped object without `overwrite=True` |
| Query returns nothing on a link field | you compared an id-string against a link — join through a bound variable (section 7) |

## Where to next

- **This interface, exhaustively:** [Pydantic Schema Reference](/docs/pydantic-schema-reference) — every annotation, key type, API member and error.
- **The day-to-day workflow:** [Pydantic Schema Management](/docs/pydantic-schema-management) — commit, sync, check and the evolution loop.
- **The schema itself:** the [Schema Reference Guide](/docs/schema-reference-guide) — class markers, inheritance semantics, migrations.
- **Queries:** the [GraphQL Query Reference](/docs/graphql-query-reference), and [Path Queries](/docs/path-queries-in-graphql) for traversal.
