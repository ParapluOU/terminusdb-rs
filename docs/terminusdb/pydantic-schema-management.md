---
tags:
  - python
  - schema
  - how-to
  - intermediate
title: Pydantic Schema Management
nextjs:
  metadata:
    title: Pydantic Schema Management with the Python Client
    description: Declare TerminusDB schemas as pydantic models, commit them to the database, regenerate them with tdbpy sync, and keep schema and code in lockstep with tdbpy check.
    keywords: terminusdb, pydantic, python client, schema management, TerminusModel, schema evolution, tdbpy
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/pydantic-schema-management/
media: []
---

{% callout type="note" %}
Pydantic support ships with the upcoming TerminusDB 12.1 server release together with an updated Python client. Until then, the interface described here is available on the client's pydantic branch. New to the feature? Start with the [Pydantic Quickstart](/docs/pydantic-quickstart) tutorial.
{% /callout %}

TerminusDB schemas and Python domain models no longer need to drift apart. The `terminusdb[pydantic]` extra lets you declare your database schema as ordinary pydantic models, commit them to the database, pull them back as generated code, and read documents back into validated model instances.

The idea is simple: the pydantic model *is* the schema. `TerminusModel` subclasses emit exactly the same class documents the `DocumentTemplate` machinery has always emitted, so the two syntaxes mix freely in one `Schema` and one commit. Pydantic v2 is the only version supported.

{% callout type="note" %}
This page assumes a working TerminusDB server and the Python client installed. See [Use the Python Client](/docs/use-the-python-client) for setup.
{% /callout %}

## Install

```bash
pip install "terminusdb[pydantic]"
```

Everything else in the client works without pydantic; the extra only adds the schema layer.

## Declare a model

A document class is a pydantic `BaseModel` with TerminusDB class attributes:

```python
from typing import List, Optional
from terminusdb_client.schema import LexicalKey
from terminusdb_client.schema.pydantic import TerminusModel

class Person(TerminusModel):
    """A person tracked by the system.

    Attributes
    ----------
    name : str
        Full display name.
    """

    _key = LexicalKey("name")

    name: str
    age: Optional[int] = None
    tags: List[str] = []
```

This class emits the TerminusDB schema document:

```python
{"@type": "Class", "@id": "Person",
 "@documentation": {"@comment": "A person tracked by the system.",
                    "@properties": {"name": "Full display name."}},
 "@key": {"@type": "Lexical", "@fields": ["name"]},
 "name": "xsd:string",
 "age": {"@type": "Optional", "@class": "xsd:integer"},
 "tags": {"@type": "List", "@class": "xsd:string"}}
```

Write `Optional[X] = None` when the field should be optional at construction time — `Optional[X]` alone is still a *required* field in pydantic. `Optional[Set[X]]` and `Optional[List[X]]` are rejected with `TypeError`: a collection already permits zero elements.

### Class attributes

| Attribute | Effect |
|---|---|
| `_key` | `@key` — the document id scheme (`HashKey`, `LexicalKey`, `ValueHashKey`, `RandomKey` (default), `UuidV7Key`) |
| `_subdocument = []` | `@subdocument` — embedded, no own id |
| `_abstract = []` | `@abstract` — schema-only base |
| `_base` | `@base` — id prefix override |
| `_unfoldable = True` | `@unfoldable` — reads expand this class inline |
| `_shared = True` | `@shared` — cascade-delete on last unreference |
| `_foreign = True` | `{"@type": "Foreign"}` — opaque external IRI reference |
| `_metadata = {...}` | class `@metadata` (`render_as`, `order_by`, …) |
| `_schema = my_schema` | registers the class into a `Schema` at definition time |

Underscore-prefixed names are pydantic-private by convention, never fields. `UuidV7Key` accepts any supplied IRI as `@id` — useful when loading data that already carries external identifiers.

A few rules the engine enforces, checked at declaration: `_shared` and `_subdocument` are mutually exclusive; a subdocument's `_key` must be `RandomKey`, `UuidV7Key` or `ValueHashKey`; a `_foreign` class declares no fields.

### Field types

| Annotation | Emits |
|---|---|
| `str`, `int`, `float`, `bool` | `xsd:string/integer/decimal/boolean` |
| `dt.datetime`, `dt.date`, `dt.time`, `dt.timedelta` | `xsd:dateTime/date/time/duration` |
| `decimal.Decimal` | `xsd:decimal` |
| `dict` | `sys:JSON` |
| `Optional[X]` / `X \| None` | `{"@type": "Optional", "@class": X}` |
| `List[X]` / `list[X]` | `{"@type": "List", "@class": X}` |
| `Set[X]` / `set[X]` | `{"@type": "Set", "@class": X}` |
| `MyModel` / `"MyModel"` | class link |
| `MyEnum` | enum link |
| `Annotated[X, Unfold]` | property-level `@unfold` |
| `Annotated[list[X], Array]` | `{"@type": "Array", ...}` (`@dimensions` from nesting) |
| `Set[X]` + `Field(min_length=, max_length=)` | adds `@min_cardinality`/`@max_cardinality`/`@cardinality` |
| `None` | `sys:Unit` (a `TerminusUnion` member carrying no value) |
| woql_type NewTypes (`gYear`, `IDRef`, `decimal`, …) | the mapped XSD type |

Pydantic defaults and validators (`gt`, `pattern`, …) stay client-side — except `min_length`/`max_length` on `Set`, which deliberately change the schema to cardinality constraints.

### Enums and tagged unions

Enums register with `TerminusEnum` (a `str, Enum` subclass); member values are the stored wire values:

```python
from terminusdb_client.schema.pydantic import TerminusEnum

class Colour(TerminusEnum):
    red = "red"
    blue = "blue"
    information_technology = "Information Technology"
```

A tagged union says "the document carries the fields of exactly one member" — declare members as fields on `TerminusUnion`:

```python
from terminusdb_client.schema.pydantic import TerminusUnion

class Contact(TerminusUnion):
    email: str                      # "xsd:string" member
    phone: Optional[str] = None
    leaf: Optional[None] = None     # "sys:Unit" member — no value
```

### Subdocuments and links

A `_subdocument` class is embedded inside its parent and mints no id of its own:

```python
class Address(TerminusModel):
    _subdocument = []
    street: str
    city: str

class Person(TerminusModel):
    address: Optional[Address] = None
```

An ordinary link (`other: Person`) stores a reference. On read it materialises as a **stub** — an instance carrying `_backend_id` and no loaded fields:

```python
person = schema.parse_document(client.get_document("Person/bob"))
person.best_friend._backend_id   # "Person/katy"
person.best_friend.name          # AttributeError — fetch it if needed
katy = schema.parse_document(client.get_document(person.best_friend._backend_id))
```

`Annotated[X, Unfold]` on a property (or `_unfoldable` on the linked class) makes reads expand the link into a full nested model instead.

## Schema lifecycle

### Commit

```python
from terminusdb_client.schema.pydantic import PydanticSchema
from terminusdb_client.client import Client

schema = PydanticSchema(title="People", description="Directory schema",
                      authors=["you"])
schema.add_obj("Person", Person)
schema.add_obj("Address", Address)
schema.add_obj("Colour", Colour)

client = Client("http://localhost:6363")
client.connect(db="my_db")
schema.commit(client)
```

Or keep models in `schema.py` and use `tdbpy commit` — it picks up `TerminusModel`, `TerminusUnion`, `TerminusEnum` and `DocumentTemplate` classes alike, so mixed schemas commit in one operation.

### Pull: `tdbpy sync --format pydantic`

`tdbpy sync` regenerates `schema.py` from the live schema graph. `--format pydantic` writes `TerminusModel` classes (the default remains `DocumentTemplate`) and records `model_backend` in `config.json` so later syncs keep the choice. The generated file round-trips: committing it reproduces the same schema documents.

### Guard: `tdbpy check`

```bash
tdbpy check                 # exit 0 in sync, 1 on drift, 2 when no schema.py
tdbpy check --ignore-docs   # tolerate documentation-only differences
```

This is the drift gate: run it in CI and schema/code divergence becomes a build failure instead of a surprise. `Schema.diff(client)` is the underlying call if you want the diff lines yourself.

### Evolution loop

1. Edit the models (or sync from a database someone else changed).
2. `tdbpy commit` — the schema graph is replaced with what the models say.
3. `tdbpy check` — proves the push landed and stays true.
4. Commit `schema.py` — the diff *is* the schema migration, reviewable in git.

## Typed documents

Parsing goes through the schema object, which holds the class registry:

```python
schema = PydanticSchema().from_db(client)        # rebuild models from the DB
doc = client.get_document("Person/bob")
bob = schema.parse_document(doc)
bob.age = 37
client.replace_document(bob)
```

`from_db` reconstructs every class — inheritance, enums, unions — and resolves forward references, so `bob` is a real validated model with `model_validate` semantics. `schema.import_objects(list_of_docs)` does the same for query results. Loaded classes live in `schema.object`, never in module globals.

Insert works directly on instances: `client.insert_document(bob)`. Objects without ids participate in `@capture`/`@ref` graphs exactly like `DocumentTemplate` instances; the client stamps `_backend_id` back onto every inserted object — top-level and referenced — so re-serialising them emits `{"@id"}` links.

## Migrating from DocumentTemplate

| `DocumentTemplate` | `TerminusModel` |
|---|---|
| `class P(DocumentTemplate):` | `class P(TerminusModel):` |
| `name: str` | unchanged |
| `_key`, `_subdocument`, `_abstract`, `_base` | unchanged |
| `EnumTemplate` + `transform_enum_dict` | `TerminusEnum` with explicit values |
| `TaggedUnion` | `TerminusUnion` (fields are the members) |
| typeguard on `__setattr__` | pydantic validators |
| `obj._id` | `obj._backend_id` for loaded ids; `_id` computes custom ids |

There is no rush: both systems are supported, and a `Schema` happily holds both kinds. Convert the classes you touch, keep the rest.

## Limitations

- General `@oneOf` on a `Class` is not mapped (`TerminusUnion` covers the tagged-union case) — `TypeError`.
- `Table` fields, `sys:JSONDocument` schema type, per-member enum docs (`@values`), multilingual `@language` documentation, and custom context prefixes are not mapped — `TypeError` or absent.
- `frozenset` cannot round-trip (a `Set` loads back as `set`); use `Set`/`set`.
- `Literal[...]` and non-Optional `Union[A, B]` are rejected — declare an `Enum` or a `TerminusUnion`.

See [Pydantic Schema Reference](/docs/pydantic-schema-reference) for the full annotation table, API surface, CLI flags and error catalogue.
