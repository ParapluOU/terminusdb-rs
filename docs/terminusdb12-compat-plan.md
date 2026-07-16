# TerminusDB 12 Compatibility Plan

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
- [ ] Delete or refresh the remaining stale doc artifacts superseded by the
      mirror: `docs/woql/class-reference.md` + `docs/woql/guide/` (old repo era)
      and `docs/terminusdb-schema.md` (old TerminusCMS schema guide — superseded
      by `docs/terminusdb/schema-reference-guide.md`).
- [ ] Regenerate the OpenAPI client (`cd client && make generate-client`)
      against the v12.0.5 spec and reconcile differences.
- [ ] CI: run the integration suite against the pinned v12 embedded server;
      consider a compatibility job against the last v11 tag if v11 support is
      to be retained (decide and document a support policy).
- [ ] Sequence note: land P0 first (pinned server + numeric correctness), then
      P1 (WOQL) and P2 (schema) in parallel, then P3. Each functional item needs
      an integration test against the real v12 server per CLAUDE.md.

## Non-goals / explicitly out of scope for now

- Enterprise-only formats (`jsonld`/`rdfxml`/`turtle` export, TwinfoxDB
  enterprise features) — gate errors cleanly, don't implement.
- VectorLink / embeddings-handlebars rendering API (v12.0.6) — revisit if needed.
- crates.io publication (the official docs note we're not yet published; nightly
  feature usage is the blocker) — separate effort.
