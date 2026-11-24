# TerminusDB 12 Compatibility Plan

## 12.1.0-rc3 upgrade (2026-11-17)

Our fork pin `v12.1-rc-paraplu.1` was based on upstream `12.0.6+140`
(`d0fcdc54`, 2026-07-13). Upstream is now **925 commits ahead** at
`v12.1.0-rc3`, spanning releases 12.0.7, 12.0.8, 12.0.9 and 12.1.0-rc1/2/3.
The surface below was derived by diffing the server tree itself
(`src/terminus-schema/woql.json`, `src/server/routes.pl`, `src/core/api/`),
cross-checked against `docs/terminusdb/releases/`. Procedure captured in the
`terminusdb-version-upgrade` skill.

### Server pin

- [x] `v12.1.0-rc3-paraplu.2` — `v12.1.0-rc3-paraplu.1` plus the fix for a
      background database load wedging the database it loaded
      (`generate_operation_id/1` threw on the erased output stream of a
      `detached(true)` loader thread, so `db_unservable_loading/3` was never
      retracted and a healthy database answered 503 for the life of the
      process). On the fork's `main` and on its `v12.1.0-rc3-paraplu` release
      line; `TERMINUSDB_VERSION` in `crates/bin/build.rs` names the tag. The
      crate pins are unchanged — the fix is Prolog only.
- [x] Rebase the 4 fork patches onto `v12.1.0-rc3`, tag `v12.1.0-rc3-paraplu.1`,
      bump `TERMINUSDB_VERSION` in `crates/bin/build.rs`, `crates/gql`'s
      `terminusdb-community` rev, and the `[patch.crates-io]` pins in the root
      `Cargo.toml` (rc3 pins `tdb-succinct` at tag `v0.2.1` = `3e57cc19`).
- [x] Rebuild `terminusdb-bin`; `cargo test -p terminusdb-bin` green (15/15,
      including all five server-start tests). Build from the persistent checkout
      at `/home/paseo/projects/terminusdb-fork` via `TERMINUSDB_SOURCE` — the
      clone-to-`/tmp` path deletes the tree the saved state loads `librust.so`
      from, and a set `CARGO_TARGET_DIR` leaks into the inner cargo. Both traps
      are written up in the `terminusdb-version-upgrade` skill.
- [x] Fallout of the bump, fixed here: `crates/rug-shim` needed the rug API
      tdb-succinct v0.2.1 and store-prolog now use (reference-operand
      arithmetic, `to_i128`, `ops::Pow`, `remove_factor`) or `crates/gql` would
      not compile; `u64` above `i64::MAX` is rejected by rc3's new
      `cast_specificity` ladder and is now sent as a decimal string; and insert
      responses moved to compressed IDs, which two client tests assumed were
      full IRIs.

### WOQL AST (`crates/woql2`)

Our AST already covers every rc3 `woql.json` class except these. Each needs the
struct, a `Query` enum variant, DSL/macro constructors, and a **live** test.

- [x] `TripleRangeCount { subject, predicate, low, high, count, graph? }` —
      index-answered count of triples whose object falls in `[low, high)`.
- [x] `TripleCount` pattern form — add optional `subject`/`predicate`/`object`/
      `graph` alongside the existing `resource` form (both forms are valid).
- [x] `TripleRelation { subject, predicate, object, relation, value }` — Allen
      relation match against an interval/instant reference value. Relations:
      before, after, meets, met_by, overlaps, overlapped_by, starts, started_by,
      during, contains, finishes, finished_by, equals.
- [x] `UuidV7 { base, uri }` — idgen predicate, sibling of `RandomKey`.
- [x] `Typecast` gained a `prefixes` property.

Live-verified in `crates/client/tests/test_v12_1_woql_ops.rs` (6/6 green against
v12.1.0-rc3). Two behaviours the server source settled, both now encoded in the
tests:

* `TripleCount`'s pattern form only triggers when `subject`, `predicate` **and**
  `object` are all *present* in the JSON-LD (`json_woql.pl` matches the dict
  with `:<`), even though woql.json marks them optional. An unrestricted
  position is therefore an unbound variable, not an omitted field — hence
  `TripleCount::pattern` takes all three, with `wildcard_node`/`wildcard_value`
  (and `_` in `triple_count_pattern!`) for the unrestricted case.
* `TripleRelation` compiles to `woql_interval_relation_typed/3`, so the object
  and the reference value must be date-ish; integers never match. Against a
  *point* reference, `after` is deliberately **on-or-after** (as-of semantics,
  `check_interval_relation/5`), so an instant equal to the reference matches
  both `after` and `equals`.

Not expressible yet: a literal of an arbitrary type such as
`xdd:dateTimeInterval` — `XSDAnySimpleType` has no escape hatch, so the tests
use `xsd:dateTime` instants (which the server coerces to point intervals).

### Schema & derive (`crates/schema`, `crates/schema/derive`)

- [x] `Key::UuidV7` (`@key: {"@type": "UuidV7"}`). Accepts an externally
      supplied `@id` that need not sit under the class `@base`; otherwise mints
      `<base>/<uuid-v7>`. Valid on classes, `@subdocument` classes and
      `TaggedUnion` members.
- [x] `@shared` class annotation (`sys:shared`, serialised as `"@shared": []`):
      own IRI, referenceable by many, cascade-deleted in the same transaction
      when the last referrer goes. Was deferred in M7 — land it now.
- [x] Field-level `@unfold` on `Optional`/`Set`/`List`/`Array` properties,
      independent of the target class being `@unfoldable`.

Live-verified in `crates/client/tests/test_v12_1_schema_annotations.rs` (4/4
green). Derive surface: `#[tdb(key = "uuid_v7")]`, `#[tdb(shared = true)]` and
field-level `#[tdb(unfold)]`. `Property` gained an `unfold: bool` and
`Schema::Class`/`Schema::TaggedUnion` a `shared: bool` (both default-off and
emitted only when set), which is why ~175 struct-literal sites across
`schema`, `gql`, `xsd` and `woql2` changed mechanically. The cascade-delete
half of `@shared` is asserted end-to-end: two referrers, delete one (shared doc
survives), delete the last (shared doc is gone).

### Client endpoints & parameters (`crates/client`)

- [x] `GET /api/document/<path>?counted=true` → `{"@type":
      "api:DocumentCountResponse", "api:count": N}`. Mutually exclusive with
      `id`/`ids`/`query` (server answers `api:CountedNotSupportedForSelector`).
- [x] Remaining new document-list params: `identifiers`, `id_prefix`, `raw_id`,
      `xsd` (canonicalisation version; `1.0` is enterprise-only).
- [x] Document output formats `format=embedding` (Handlebars template from
      `@metadata.embedding`) and `format=markdown`. `jsonld`/`rdfxml`/`turtle`
      are enterprise-gated — out of scope.
- [x] `GET /api/changes/<path>?count=N` → `{added, changed, deleted}` document
      ID lists for a branch head or a specific commit. New endpoint.
- [x] `GET /api/schema/<path>` — the class-frame endpoint, which we never
      wrapped at all, plus its new `format=jsonschema` mode (`type`,
      `compress_ids`, `expand_abstract`).

Live-verified in `crates/client/tests/test_v12_1_endpoints.rs` (6/6 green):
`document_count`, `get_changes`, `get_class_frame`/`get_json_schema`,
`get_documents_text` (markdown/embedding) and the new `GetOpts` params
(`identifiers`, `id_prefix`, `raw_id`, `xsd`, `format`). Three things the
server source settled:

* The document reads that POST their selector in a JSON body (needed so
  url-encoded lexical/hash ids survive) must pass the new params in that body
  too — hence `GetOpts::apply_to_body`. **Except `format`**: the server's
  `detect_output_format/3` reads it only from the query string or the `Accept`
  header, never from the body.
* `format=markdown`/`format=embedding` answer `text/markdown` / `text/plain`
  bodies (one rendered block per document), not JSON, so they get their own
  text-returning client methods rather than riding the JSON getters.
* `GET /api/changes` answers *expanded* IRIs
  (`terminusdb:///data/Member/second`), not the compressed `Type/id` form the
  document API accepts.

Not covered: the `api:CountedNotSupportedForSelector` error path — our
`document_count` cannot express `counted` together with `id`/`ids`/`query`, so
there is nothing to test.
- [x] History params we lack: `diff`, `streaming` (NDJSON), `fast`, `before`,
      `after`, `complete`, `graph_type`.
- [x] `unfold` on `POST /api/diff` (default `true`; `false` compares documents
      as stored).
- [x] Typed `Insert`/`Delete` patch operations for `POST /api/patch` alongside
      the existing field-level `SwapValue`, including `@capture`/`@ref`.

Live-verified in `crates/client/tests/test_v12_1_history_diff_patch.rs` (4/4
green). Notes from the server:

* `fast=true` is **not** a community feature: the server answers
  `api:FastHistoryNotAvailable` ("not available in the community edition")
  rather than falling back to the slow path. The parameter is exposed and the
  test asserts the refusal — matching this document's enterprise deferral.
* `before`/`after` take ISO-8601 and must be url-encoded; an unencoded RFC-3339
  offset (`+00:00`) reaches the server as a space and fails to parse. The URL
  builder now encodes them.
* `diff=true` attaches a `diff` object per entry (`SwapValue` per changed
  field, `@insert` for the creating commit), on both the collected and the
  `streaming=true` NDJSON path.
* Patch `Insert` documents are raw JSON-LD, so a model whose id field is an
  `EntityIDFor` must also carry that field explicitly — it is a required schema
  property like any other.

### GraphQL

- [x] Native **subscriptions over SSE** (graphql-sse, distinct-connections
      mode) on `POST /api/graphql/<org>/<db>` with `Accept: text/event-stream`:
      `connected` / `next` / `complete` events, `<Type>_added|_updated|_deleted`
      fields, `_ChangeSet` (one event per commit, `include_children`), and
      `_CommitMetadata`. This supersedes our own deprecated SSE plugin
      (`http/sse_manager.rs`, `http/changeset.rs`, `http/change_listener.rs`) —
      do not revive that; build on the server's protocol.

Landed as `crates/client/src/http/subscription.rs`
(`subscribe_graphql` / `subscribe_graphql_system`, yielding a `Stream` of
`SubscriptionEvent::{Connected, Next, Errors, Complete}`), with the SSE block
parser unit-tested and five live tests in
`crates/client/tests/test_v12_1_graphql_subscriptions.rs` (per-document insert,
`_ChangeSet` batching + `_CommitMetadata`, deleted-document resolution from the
pre-commit layer, and in-stream validation errors). The deprecated SSE plugin
was left untouched.

Two places where rc3's behaviour differs from `docs/terminusdb/graphql-subscriptions.md`:

* **`next` payloads are wrapped in the GraphQL `data` envelope**
  (`{"data":{"Person_added":{...}}}`), where the doc shows the selection
  unwrapped. `SubscriptionEvent::payload()` unwraps it; `raw_payload()` keeps it.
* **The `filter` argument is parsed but not applied** by the community server.
  `parse_subscription_query` extracts `filter_canonical_json` and the SSE
  handler forwards it, but `register_subscription_parsed/8` ignores it
  (`_FilterJson`) and keys the broadcast cohort on
  (descriptor, class, operation, selection hash, mode). A filtered subscription
  therefore still receives non-matching documents and must filter client-side.
  `subscription_filter_is_accepted_but_not_yet_applied` pins this so a future
  release that starts honouring filters fails loudly.
- [x] Fast `_count` (unfiltered `_count` is index-answered in 12.1).

### Explicitly deferred (with reason)

- **Versioned search** (`api_search.pl`: search, similar, duplicates,
  statistics, resolve, compare, delete-domain) — fronts an external
  `tdb-search` engine and is gated on `indexer_backend(http_vectorlink)`. Not
  runnable against the embedded community server, so untestable here.
- **Enterprise-only surfaces** — `jsonld`/`rdfxml`/`turtle` document output,
  `xsd=1.0` canonicalisation, the `fast` history enrichment path.

---

## Live verification against 12.1 (2026-07-16)

Target retargeted v12.0.6 → **v12.1** (upstream `12.1-rc` branch; no v12.1 tag
yet). Server fork rebased onto it (`v12.1-rc-paraplu.1`), embedded build + 28
bin tests + 244 client integration tests all green. New behaviors probed live
(`crates/client/tests/test_v12_woql_ops.rs`, `test_v12_numeric_intervals.rs`):

- **Work correctly:** all new WOQL ops — SetUnion/Intersection/Difference,
  ListToSet, Slice (half-open), Gte/Lte, InRange; and IntervalRelation returns
  the right Allen relation (`overlaps`). Confirms the 33 new AST classes'
  JSON-LD serialization matches the server, not just the offline schema.
- **Fixed a pre-existing bug:** `ArithmeticExpression`/`ArithmeticValue` enums
  lacked `#[tdb(abstract_class)]` / `#[tdb(rename_all="lowercase")]`, so `Eval`
  arithmetic serialized an illegal `{"@type":"ArithmeticExpression",...}` wrapper
  the server rejected ("Not well formed WOQL JSON-LD"). Now fixed; Eval works.
- **KEY M1 FINDING — decimal precision was lost client-side → FIXED.** `Eval(1/3)`
  on 12.1 returns `xsd:decimal` at full rational precision, but the client showed
  `0.33333333333333337` (f64 artifact) because response parsing used serde_json
  without `arbitrary_precision`, truncating decimals past ~16 digits through f64.
  Fixed by enabling `arbitrary_precision` workspace-wide; `Eval(1/3)` now
  round-trips as `0.33333333333333333333`. Fallout: the feature's number token
  can't pass through serde's `Content` buffer, so three untagged/flatten enums
  (`ApiResponse`, `GraphQLPathSegment`, `xsd::Cardinality`) got manual `Deserialize`
  impls that dispatch on a `serde_json::Value`. Full client suite green (248/248).
  **Remaining M1 pieces:** `f64`→`xsd:double` (schema-shape change — currently
  `f64`→`xsd:float`), a first-class high-precision decimal field type
  (`bigdecimal`), and the strict-v12 read-path cleanup.
- **Minor follow-up:** `Interval(start,end→var)` construction returned an unbound
  result (None); needs the right binding mode / arg shape — revisit in M1/M3.

Status: draft (2026-07-08). Based on the refreshed docs mirror in `docs/terminusdb/`
(upstream commit `0f36c9a`, covering TerminusDB 12.0.x through the May-2026 doc
revision), the refreshed `docs/openapi.yaml` (API v12.0.5), and a diff of the old
vs new machine-readable WOQL schema (`docs/schemas/woql.json`).

TerminusDB 12 (first release 2025-12-08, maintained by DFRNT; current 12.0.6)
keeps the v11 on-disk format but changes interfaces: native JSON numbers
everywhere, rational/high-precision decimals, ~33 new WOQL AST classes, new
schema annotations, new document-API parameters, WOQL streaming, and a typed
error envelope without stack traces.

Key doc references (all in `docs/terminusdb/`): `numeric-precision-reference.md`,
`data-types.md`, `woql-class-reference-guide.md` (authoritative 131-class AST),
`schema-reference-guide.md`, `document-insertion.md`,
`document-unfolding-reference.md`, `schema-migration-reference-guide.md`,
`graphql-query-reference.md`, `woql-query-streaming.md`, `json-diff-and-patch.md`,
`capabilities-api-modes.md`, `prefix-management.md`.

---

## P0 — Prerequisites & breaking-change correctness

### 0.1 Pin the embedded server to a v12 release
`crates/bin/build.rs` builds the server from the `ParapluOU/terminusdb` fork's
**unpinned `main`** (`build.rs:154`, `build.rs:1090-1097`). Everything else in
this plan needs a reproducible v12 server to test against.

- [ ] Rebase/update the `ParapluOU/terminusdb` fork onto upstream `v12.0.6` (or
      point the clone at `terminusdb/terminusdb` directly if the fork carries no
      needed patches).
- [ ] Change the default `TERMINUSDB_VERSION` from `"main"` to a pinned v12 tag.
- [ ] Verify the SWI-Prolog pin (`build.rs:572`, currently `10.0.0`) matches what
      v12.0.4+ requires (release notes: "SWI-Prolog 10 support").
- [x] `docker/changeset-sse`: bumped base v11.1.14 → v12.0.6, but the plugin is
      DEPRECATED/unstable (our own, unmaintained) so it is now DISABLED, not
      ported — the Dockerfile no longer COPYs it and the entrypoint no longer
      sets `TERMINUSDB_PLUGINS_PATH`. Prefer v12 native history diff+streaming.
- [ ] Refresh prebuilt binaries under `crates/bin/prebuilt/`.

### 0.2 Numeric handling (the headline v12 breaking change)
v12 returns numbers as **native JSON numbers** in all APIs; `xsd:decimal` is an
exact rational printed with up to 20 significant digits; arithmetic follows
Prolog contagion rules (`numeric-precision-reference.md`).

Current state and fixes:

- [ ] **`f64` maps to `xsd:float`** (`crates/schema/src/primitive.rs:186-188`);
      the `DOUBLE` constant exists but nothing uses it. Map `f64` → `xsd:double`
      (keep `f32` → `xsd:float`). This is a schema-shape change for existing
      databases — document it and note the "Schema failure → drop database"
      troubleshooting path, or ship a migration helper (see 2.6).
- [ ] **Decimal deserialization must not lose precision.** Responses now carry
      decimals as bare JSON numbers with up to 20 significant digits; parsing
      through `f64` silently truncates. Enable serde_json's
      `arbitrary_precision` feature (workspace-wide — check compatibility with
      `serde_json::Number` usage in `PrimitiveValue`,
      `crates/schema/src/instance/value_primitive.rs:4-14`) so `@value` numbers
      round-trip losslessly.
- [ ] **Provide a first-class decimal field type.** Today `decimal_rs::Decimal`
      exists only inside `XSDAnySimpleType`/woql-builder literals; models cannot
      declare a decimal property. Add `ToSchemaClass` + `ToInstanceProperty` +
      `FromInstanceProperty` for a decimal type mapping to `xsd:decimal`
      (`rust_decimal` is already declared in workspace deps but unused — pick
      one of `rust_decimal`/`bigdecimal` and remove the other; note the docs
      recommend ≥20-significant-digit capability, `bigdecimal` is the safe
      choice for arbitrary precision).
- [ ] **Keep string-wrapped decimals on the request side.** The docs explicitly
      bless sending decimals as strings in `@value`
      (`numeric-precision-reference.md`), so the existing string-wrapping in
      `crates/schema/src/xsdtype.rs:292-297` is fine for writes — do not break it.
- [ ] **Add string fallback to `f32`/`f64` deserialization**
      (`crates/schema/src/json/impls.rs:139-158`) so the client stays compatible
      with v11 servers that string-wrap; integer paths already accept both.
- [ ] Add integration tests: insert/read `xsd:decimal` with >15 significant
      digits, `xsd:integer` beyond `u64`, division producing rationals
      (`1 rdiv 3` → 20-digit output), and float-contagion cases.

### 0.3 `group_by` single-element template unwrap
v12 returns `["value", ...]` instead of `[["value"], ...]` for single-element
templates (`group-query-results.md`). The audit found no code hard-coding the
old shape (`crates/woql2/src/order.rs:43-53` passes templates through verbatim),
but user-facing helpers and docs should state the new shape.

- [ ] Add round-trip integration tests for both single- and multi-element
      `group_by` templates against the embedded v12 server.
- [ ] Check ORM/result helpers (`crates/orm/src/result.rs`) and any examples that
      destructure `grouped` bindings.

### 0.4 Error envelope updates
v12 removes stack traces and standardizes
`{"@type":"api:ErrorResponse","api:error":{...},"api:status":...}`. Our parsing
(`crates/client/src/result.rs:181-273`, `crates/client/src/err/tdb.rs:275-360`)
never expected stack traces, so this is mostly additive:

- [ ] Extend typed error coverage for v12 error `@type`s we can now hit:
      `api:CaptureIdAlreadyBound`, `api:NotAllCapturesFound` (document capture),
      `api:PatchError` with `api:status: api:conflict` + HTTP 409 witnesses,
      `api:MigrationResponse` errors, prefix errors (`api:PrefixNotFound`,
      `api:PrefixAlreadyExists`, `api:ReservedPrefix`, `api:InvalidIRI`).
- [ ] Verify `api:status` handling covers `api:conflict` (409) alongside
      `api:failure`/`api:not_found`.

---

## P1 — New WOQL surface (woql2 AST → builder → DSL frontends)

Diffing old vs new `docs/schemas/woql.json` gives the exact additions — 33 new
classes, all absent from `crates/woql2` (verified by audit):

| Group | New `@type` classes |
|---|---|
| List/set ops | `Slice`, `ListToSet`, `SetUnion`, `SetIntersection`, `SetDifference`, `SetMember` |
| Control/misc | `Comment`, `Collect`, `Sequence` |
| Comparison/range | `Gte`, `Lte`, `InRange`, `RangeMin`, `RangeMax` |
| Ordered-triple range queries (v12.0.4) | `TripleSlice`, `TripleSliceRev`, `TripleNext`, `TriplePrevious` |
| ISO-8601 / Allen interval algebra | `Interval`, `IntervalStartDuration`, `IntervalDurationEnd`, `IntervalRelation`, `IntervalRelationTyped`, `DateDuration`, `DayAfter`, `DayBefore`, `IsoWeek`, `Weekday`, `WeekdaySundayStart`, `MonthStartDate`, `MonthStartDates`, `MonthEndDate`, `MonthEndDates` |

Notes:
- `idgen_random()` in the JS/Python clients compiles to the **existing
  `RandomKey`** node — no new AST class needed, but `RandomKey`'s schema
  definition changed (now `base: DataValue`, `uri: NodeValue` with `+`/`?`
  modes) — verify `crates/woql2/src/misc.rs` matches and add an
  `idgen_random()` builder alias.
- `Dot` and `Typecast` already exist (`crates/woql2/src/collection.rs:40`,
  `compare.rs:73`).

Work items:

- [ ] **woql2**: add the 33 AST structs + `Query` enum variants
      (`crates/woql2/src/query.rs:61-128`), matching field names/optionality
      from `docs/schemas/woql.json` (e.g. `Slice{list, start, end?, result}`).
      Suggested module placement: `collection.rs` (Slice, set ops), `misc.rs`
      (Comment, Collect, Sequence, InRange, RangeMin/Max), `compare.rs`
      (Gte, Lte), new `interval.rs` (time/interval family), `triple.rs` or new
      `range.rs` (TripleSlice family).
- [ ] **woql2 DSL rendering** (`crates/woql2/src/dsl.rs`): render the new nodes.
- [ ] **woql-builder**: fluent methods — `slice()`, `set_union()`,
      `set_intersection()`, `set_difference()`, `set_member()`, `list_to_set()`,
      `comment()`, `collect()`, `sequence()`, `in_range()`, `gte()`, `lte()`,
      `range_min()`, `range_max()`, `triple_slice()`, `triple_next()`,
      `triple_previous()`, `idgen_random()`, and the interval/date helpers
      (mirror the JS names in the refreshed `docs/woql/woql.js`).
- [ ] **xdd types**: add `xdd:json`, `xdd:dateTimeInterval`, `xdd:dateRange`
      awareness to `XSDAnySimpleType`/typecast helpers (casts documented:
      `xsd:string`→`xdd:json`, `xdd:dateRange`↔`xdd:dateTimeInterval`).
- [ ] **WOQL streaming mode** (`woql-query-streaming.md`): request flag
      `{"query":..., "streaming": true}`; parse ndjson `PrefaceRecord` →
      `Binding`* → `PostscriptRecord` (with `inserts`/`deletes`/`version`), and
      the error case where the last line is an `api:ErrorResponse` after HTTP
      200. Add `query_stream()` to `crates/client/src/http/query.rs` returning
      a `Stream` of bindings.
- [ ] **Conformance test**: a test that loads `docs/schemas/woql.json` and
      asserts every schema class has a corresponding woql2 type (and field
      parity), so future upstream drift is caught mechanically.
- [ ] Integration tests for each new operator family against the embedded v12
      server (per CLAUDE.md: WOQL is only proven by running it).

---

## P2 — Schema & derive (terminusdb-schema / -derive)

**Status (2026-07-16):** Set cardinality DONE — `#[tdb(cardinality = N)]` /
`#[tdb(min_cardinality = N, max_cardinality = M)]` field attrs drive
`TypeFamily::Set(SetCardinality::{Exact,Min,Max,Range})` (no core-struct change
needed: `TypeFamily::Set` already carries `SetCardinality`; the derive mutates
`prop.r#type`). Migration API client DONE (see P3). **Deferred as one core-struct
refactor:** field-level `@unfold` needs a new `Property.unfold` field and
class-level `@shared` needs a new `Schema::Class.shared` field — each touches
~160 `Property {` / ~56 `Schema::Class {` literal construction sites plus the
JSON emission, so they're a dedicated follow-up rather than folded in here.

- [ ] **Field-level `@unfold`** (`document-unfolding-reference.md`): new field
      attr `#[tdb(unfold)]` → emit `"@unfold": true` on the property. Distinct
      from the existing class-level `@unfoldable`
      (`crates/schema/src/schema/schema.rs:658-659`). *(deferred — needs
      `Property.unfold` field + all construction sites updated.)*
- [ ] **Class-level `@shared`** (cascade-delete, v12.0.6,
      `schema-reference-guide.md`): new struct attr `#[tdb(shared)]` → emit
      `"@shared": []`. Enforce mutual exclusion with `subdocument` at derive
      time. Document the liveness/cascade-delete semantics and add integration
      tests (delete last referrer → shared doc gone; circular islands).
- [ ] **Derivable Set cardinality**: the model already supports
      `SetCardinality::{Exact,Min,Max,Range}` (`crates/schema/src/schema/set.rs`)
      but `HashSet`/`BTreeSet` always render `None`
      (`crates/schema/src/impl/set.rs:45,56`). Add field attrs
      `#[tdb(cardinality = N)]` / `#[tdb(min_cardinality = N, max_cardinality = M)]`.
      Deprecate any `Cardinality`-family usage in favor of `Set` + cardinality
      props (v12 deprecates the `Cardinality` family).
- [ ] **Foreign types**: support `{"@type":"Foreign","@id":...}` classes — e.g.
      a `TdbForeign` marker/attr for opaque external-IRI references.
- [ ] **`sys:JSONDocument`**: `serde_json::Value` already maps to `sys:JSON` for
      fields; add a top-level unstructured-document story (insert/get with
      `raw_json=true`, see P3) and a helper for the `@@`-escaping rule
      (`@id`→`@@id` etc.) required when inserting `sys:JSON` content via WOQL
      `InsertDocument`.
- [ ] **Schema migration API client** (`schema-migration-reference-guide.md`):
      new module + typed operations (`CreateClass`, `DeleteClass`, `MoveClass`,
      `ReplaceClassMetadata`, `ReplaceClassDocumentation`, `ReplaceContext`,
      `ExpandEnum`, `CreateClassProperty`, `DeleteClassProperty`,
      `MoveClassProperty`, `UpcastClassProperty`, `CastClassProperty`,
      `ChangeKey`) posting to `/api/migration/{path}` with
      `dry_run`/`verbose`. This gives users a real alternative to the current
      "drop the database on schema failure" workaround in CLAUDE.md.

---

## P3 — Client endpoints & parameters (terminusdb-client)

**Status (2026-07-16) — delivered, each live-verified against 12.1:**
- Document params `merge_repeats` + `raw_json` (POST/PUT), and `raw_json` on GET.
- Schema **migration** API client (`/api/migration`, 13 typed ops).
- **Prefix** management CRUD (`/api/prefix/...`: add/get/update/upsert/delete).
- **WOQL streaming** (`query_stream()` — ndjson Preface/Binding/Postscript, error
  after HTTP 200 surfaced as a stream `Err`).
- **Apply** endpoint (`apply_commit_diff` — cherry-pick / squash-merge commits).

**Still open (lower value; the generic error path already surfaces failures):**
`@capture`/`@ref` bulk-insert helpers; the four diff-endpoint body forms; typed
error variants for the new envelopes (prefix/migration/patch-409 currently fall
through `TypedErrorResponse::GenericError`, so they already raise as errors);
GET `compress_ids`/`ids`/`count`/`skip`; capabilities Name-vs-ID mode check.

Document API (`document-insertion.md`; current param handling in
`crates/client/src/http/document.rs` sends only `unfold`, `as_list`,
`minimized`, `graph_type`, `author`, `message`, `full_replace`, `nuke`, `id`):

- [ ] Add POST/PUT params: **`merge_repeats`** (currently only exposed via the
      embedded CLI wrapper, `crates/bin/src/api/options.rs:238`), **`overwrite`**
      (upsert on POST), `create` (upsert on PUT), **`raw_json`**,
      `require_migration`, `allow_destructive_migration`.
- [ ] Add GET params: `ids` (batch fetch — we already POST-override with a JSON
      body; align with the documented `ids` param), **`compress_ids`** (replaces
      deprecated `prefixed`), `count`/`skip` paging, `format` (reject or gate
      `jsonld`/`rdfxml`/`turtle` — enterprise-only, HTTP 400 on community).
- [ ] **`@capture` / `@ref`** ID-capture support in insert payload helpers, plus
      the two typed errors (see 0.4).
- [ ] **`POST /api/apply`** endpoint (cherry-pick/squash a commit range:
      `before_commit`, `after_commit`, `commit_info`, `match_final_state`,
      `keep`) — not currently in `url_builder.rs`.
- [ ] **History with inline diffs**: `GET /api/history/...?id=X&diff=true`
      (per-commit `{author,message,identifier,timestamp,diff}`) — extend
      `crates/client/src/http/log.rs` / `versions.rs`. Evaluate replacing parts
      of the custom changeset-SSE plugin usage with native history streaming
      (v12.0.5 added "diff and streaming to history endpoint").
- [ ] **Prefix CRUD**: `/api/prefix/{org}/{db}/{prefix}` GET/POST/PUT/DELETE and
      `/api/prefixes/{org}/{db}` context fetch (`prefix-management.md`); the
      client currently only has a `prefixes` endpoint constant.
- [ ] **Capabilities API modes** (`capabilities-api-modes.md`): verify
      grant/revoke in `role.rs`/`organization.rs` support both Name Mode
      (`scope_type` + display names) and ID Mode (full doc IRIs), without mixing.
- [ ] **Diff endpoint variants** (`json-diff-and-patch.md`): support all four
      body forms (`before/after` objects, data-version + doc, two data-versions
      + doc, two data-versions all-docs) and options `keep`/`copy_value`;
      accept both `SwapValue` and legacy `ValueSwap` op spellings when parsing.
- [ ] **GraphQL**: use the server-side `_count` field for ORM `total_count`
      (currently client-side `Vec::len`, `crates/orm/src/graphql_query.rs`);
      check generated filters against v12 filter objects (BigInt values as
      strings; `someHave`/`allHave`; `_and`/`_or`/`_not`; `startsWith`,
      `allOfTerms`, `anyOfTerms`), back-links (`_<field>_of_<Class>`), and
      `_path_to_<Class>` path queries as ORM capabilities.
- [ ] Optional/nice-to-have: Prometheus `GET /api/metrics` accessor;
      `traceparent` header injection behind a `tracing`/OTel feature flag (note:
      request-correlation headers are in the v12 release notes but not yet in
      the docs mirror — confirm against the server first).

---

## P4 — Housekeeping & verification

- [x] Refresh `docs/terminusdb/` from `dfrnt-labs/terminusdb-docs-static`
      (236 pages + `INDEX.md` + provenance README). *(done 2026-07-08)*
- [x] Refresh `docs/openapi.yaml` (10.0.3 → 12.0.5), `docs/schemas/*.json`
      (WOQL/ref/repo/system schemas from upstream main), and `docs/woql/woql.js`
      (current JS builder, includes the new operators). *(done 2026-07-08)*
- [x] Deleted the stale doc artifacts superseded by the mirror:
      `docs/woql/class-reference.md`, `docs/woql/guide/`, and
      `docs/terminusdb-schema.md`. *(M7)*
- [x] Refreshed `docs/openapi.yaml` to the upstream **v12.0.6** tag. NB: this
      spec is a **read-only mirror of upstream `terminusdb/terminusdb`, not our
      API definition** — do NOT run a "generate our client from it" step.
- [x] CI: added `.github/workflows/tests.yml` — builds the embedded 12.1 server
      and runs `terminusdb-bin` boot tests + schema/woql2 unit tests + the client
      integration suite. **v12-only, no v11 job** (v11 support dropped). *(M7)*
- [ ] Rebuild `crates/bin/prebuilt/*-apple-darwin/` from the pinned v12.1 fork
      tag — needs a macOS host (current binaries are v11-era; flagged stale in
      the prebuilt README).
- [x] Updated `CLAUDE.md`: v12-only + numeric behavior; the schema-failure
      troubleshooting now points to the migration API for databases with data. *(M7)*
- [ ] Sequence note: land P0 first (pinned server + numeric correctness), then
      P1 (WOQL) and P2 (schema) in parallel, then P3. Each functional item needs
      an integration test against the real v12 server per CLAUDE.md.

## Non-goals / explicitly out of scope for now

- Enterprise-only formats (`jsonld`/`rdfxml`/`turtle` export, TwinfoxDB
  enterprise features) — gate errors cleanly, don't implement.
- VectorLink / embeddings-handlebars rendering API (v12.0.6) — revisit if needed.
- crates.io publication (the official docs note we're not yet published; nightly
  feature usage is the blocker) — separate effort.
