---
tags:
  - python
  - schema
  - reference
title: Pydantic Schema Reference
nextjs:
  metadata:
    title: "Pydantic Schema Reference: TerminusModel, Annotations, CLI"
    description: "Reference for the TerminusDB pydantic schema layer: TerminusModel class attributes, annotation-to-schema mappings, unfold and array markers, PydanticSchema API, tdbpy flags and errors."
    keywords: terminusdb, pydantic, TerminusModel, TerminusUnion, schema reference, python client, tdbpy
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.com/docs/pydantic-schema-reference/
media: []
---

{% callout type="note" %}
Pydantic support ships with the upcoming TerminusDB 12.1 server release together with an updated Python client. Until then, the interface described here is available on the client's pydantic branch.
{% /callout %}

Complete reference for the pydantic schema layer (`terminusdb_client.schema.pydantic`). For the guided walkthrough see [Pydantic Schema Management](/docs/pydantic-schema-management); for the step-by-step tutorial see the [Pydantic Quickstart](/docs/pydantic-quickstart).

Requires `pip install "terminusdb[pydantic]"` (pydantic v2 only, `>=2.7,<3`).

## Classes and functions

| Name | Kind | Purpose |
|---|---|---|
| `TerminusModel` | `BaseModel` subclass | Declares a `Class` schema document; instances serialise via `_obj_to_dict` |
| `TerminusUnion` | `TerminusModel` subclass | Declares `{"@type": "TaggedUnion"}`; fields are the members |
| `TerminusEnum` | `str, Enum` | Declares `{"@type": "Enum"}`; optional `_schema` registration; `str(member)` returns the wire value |
| `PydanticSchema` | `Schema` subclass | `from_db` rebuilds pydantic models; `parse_document`/`import_objects` materialise instances |
| `Unfold` | `Annotated` marker | Emits `"@unfold": true` on the property |
| `Array` | `Annotated` marker | Emits `{"@type": "Array"}`; `@dimensions` from list nesting |

## Class attributes

| Attribute | Emits | Notes |
|---|---|---|
| `_key` | `@key` | `HashKey`, `LexicalKey`, `ValueHashKey`, `RandomKey` (default), `UuidV7Key` |
| `_subdocument = []` | `@subdocument` | Embedded; key restricted to Random/ValueHash/UuidV7; mutually exclusive with `_shared` |
| `_abstract = []` | `@abstract` | |
| `_base = "str"` | `@base` | Id prefix override |
| `_unfoldable = True` | `@unfoldable` | References to this class always unfold; class-local, never inherited |
| `_shared = True` | `@shared` | Regular document, cascade-deleted on last unreference; class-local; not auto-unfolded |
| `_foreign = True` | `{"@type": "Foreign"}` | Opaque cross-data-product IRI; class declares no fields |
| `_metadata = {...}` | `@metadata` | `render_as`, `order_by`, … |
| `_schema = schema` | — | Registers the class into the `Schema` at definition time |

## Annotation mapping

| Annotation | Schema property spec |
|---|---|
| `str` | `"xsd:string"` |
| `int` | `"xsd:integer"` |
| `float` | `"xsd:decimal"` |
| `bool` | `"xsd:boolean"` |
| `dt.datetime` | `"xsd:dateTime"` |
| `dt.date` | `"xsd:date"` |
| `dt.time` | `"xsd:time"` |
| `dt.timedelta` | `"xsd:duration"` |
| `decimal.Decimal` | `"xsd:decimal"` |
| `bytes` | `"xsd:base64Binary"` |
| `dict` | `"sys:JSON"` |
| `None` / `NoneType` | `"sys:Unit"` |
| `Optional[X]`, `X \| None`, `Union[X, None]` | `{"@type": "Optional", "@class": X}` |
| `List[X]`, `list[X]` | `{"@type": "List", "@class": X}` |
| `Set[X]`, `set[X]` | `{"@type": "Set", "@class": X}` |
| `MyModel`, `"MyModel"`, forward refs | `"MyModel"` (class link) |
| `MyEnum` | `"MyEnum"` (enum link) |
| `Annotated[X, Unfold]` | spec + `"@unfold": true` |
| `Annotated[list[X], Array]` | `{"@type": "Array", "@class": X[, "@dimensions": n]}` |
| `Set[X]` with `Field(min_length=a, max_length=b)` | `{"@type": "Set", "@class": X, "@min_cardinality": a, "@max_cardinality": b}` (equal bounds emit `@cardinality`) |
| woql_type NewTypes (`gYear`, `IDRef`, `decimal`, `PositiveInteger`, …) | mapped XSD name |

`@unfold` is placed inside complex wrappers when combined: `Annotated[Optional[X], Unfold]` → `{"@type": "Optional", "@class": X, "@unfold": true}`.

### Rejected annotations

| Annotation | Result |
|---|---|
| `Union[A, B]` (non-Optional) | `TypeError` — use `TerminusUnion` |
| `Literal[...]` | `TypeError` — use an `Enum` |
| `frozenset` | `TypeError` — use `Set`/`set` |
| `Optional[Set[X]]`, `Optional[List[X]]` | `TypeError` — collections already permit zero elements |
| `tuple`, unmapped types | `TypeError` |
| class-level general `@oneOf` | not supported — `TerminusUnion` is the tagged-union surface |
| `Table`, `sys:JSONDocument`, `@values` docs, `@language`, context prefixes | not mapped |

## Instance API

| Member | Type | Behaviour |
|---|---|---|
| `obj._obj_to_dict()` | method | → `(document_dict, references)` |
| `obj.to_document()` | method | public alias for `_obj_to_dict` |
| `obj._embedded_rep()` | method | nested-doc, `{"@id"}` link, or `{"@ref"}` capture rep |
| `obj._backend_id` | `PrivateAttr` | server-assigned IRI after load/insert-stamp |
| `obj._custom_id` | `PrivateAttr` | user-supplied id (verbatim under `UuidV7Key`) |
| `obj._id` | property | backend id → custom id → `None` |
| `obj._isinstance`, `obj._capture` | `PrivateAttr` | client serialisation contract |
| `TerminusModel._stub(id)` | classmethod | id-carrying instance with no fields (folded link) |
| `TerminusModel.from_document(doc, registry)` | classmethod | validated instance from a document dict |
| `TerminusModel._to_dict()` | classmethod | the class's schema document |
| `TerminusEnum._to_dict()` | classmethod | `{"@type": "Enum", "@id": name, "@value": [...]}` |

Model instances intentionally have **no `to_dict`** — that method means "schema object" to the client and would be rejected from instance graphs.

## `PydanticSchema` API

| Member | Behaviour |
|---|---|
| `PydanticSchema(title=, description=, authors=, schema_ref=, base_ref=, metadata=)` | context emitted into the `@context` document; `metadata` becomes context `@metadata` |
| `.add_obj(name, cls)` | register a class/enum |
| `.to_dict()` | `[context_doc, *class_docs]` sorted by `@id` |
| `.from_db(client, select=None)` | rebuild all (or `select`ed) classes; topo-ordered `@inherits`; `model_rebuild` resolves forward refs; returns `self` |
| `.parse_document(doc)` | document dict → validated instance (or `ValueError` for unknown `@type`) |
| `.import_objects(docs)` | list/dict → parsed instances |
| `.commit(client, commit_msg=, full_replace=)` | inherited from `Schema` |
| `.diff(client, ignore_docs=False)` | → `list[str]`; `[]` when in sync. Normalises `@key:Random` vs absent and `@inherits` string vs list |

`from_db` loads enums as `TerminusEnum`, classes via `create_model` (inheritance honoured), and restores `@properties` docs faithfully — object-form docs come back as `Field(title=, description=)`, string labels round-trip unchanged so `diff` stays clean. `Set`/`List` fields get `default_factory` defaults.

## `tdbpy` CLI

| Command / flag | Behaviour |
|---|---|
| `tdbpy sync --format pydantic` | writes `schema.py` of `TerminusModel`/`TerminusUnion`/`TerminusEnum` classes; records `model_backend` in `config.json` |
| `tdbpy sync` (no flag) | uses `model_backend` from `config.json`, default `template` |
| `tdbpy commit` | collects `DocumentTemplate`, `TerminusModel`, `TerminusUnion` and enum classes from `schema.py`; `full_replace` commit |
| `tdbpy check` | diffs `schema.py` vs database schema — exit `0` in sync, `1` on drift, `2` when no schema module/classes found |
| `tdbpy check --ignore-docs` | `@documentation` differences are tolerated |

## Instance document shapes

| Wire value | Parsed into |
|---|---|
| `"Class/id"` string | stub (`_backend_id` set, fields absent) |
| `{"@type": "@id", "@id": "Class/id"}` | stub |
| `{"@type": T, ...fields}` | nested `TerminusModel` (subdocument or unfolded doc) |
| `{"@ref": cap}` | linked object via capture graph |
| `xsd` scalar | python scalar (datetime/date/time/timedelta round-trip) |
| enum member string | `TerminusEnum` member (serialised via `member.value`) |
| `sys:Unit` (`[]`) | `None` |

## Error catalogue

| Raised | When |
|---|---|
| `TypeError` | unsupported annotation (`Union`, `Literal`, `frozenset`, `Optional` over collections, unmapped types) |
| `ValueError` | `_shared` + `_subdocument` together; `_foreign` class with fields; subdocument `_key` not Random/ValueHash/UuidV7; custom `_id` on keyed/subdocument class; `parse_document` on a type not in the schema |
| `RecursionError` | subdocument embedding (a superclass of) itself — mirrors `DocumentTemplate` |
| `RuntimeError` | `from_db` on a malformed spec, missing parent class, or inheritance cycle |
| `pydantic.ValidationError` | instance data violating field types |

## Known limitations

- `client.get_document(id)` cannot parse the multi-document JSONL returned when a class is `@unfoldable` — use `get_all_documents`/`query_document` for those classes (pre-existing client limitation).
- `@documentation` round-trips through `Field(title=)`, `Field(description=)`, `_label` and `__doc__`; exotic doc shapes may drift — use `tdbpy check --ignore-docs`.
- `@key` is emitted on every class (effective key), matching `DocumentTemplate` byte-for-byte; the engine repeats inherited keys on subclasses.
