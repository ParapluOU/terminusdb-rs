# `terminusdb-xpath` — XPath 3.1 → WOQL compliance roadmap

This crate compiles XPath expressions into TerminusDB WOQL queries. This document
tracks coverage against the **full official XPath 3.1 specification** — the
[language](https://www.w3.org/TR/xpath-31/) grammar and the
[Functions & Operators 3.1](https://www.w3.org/TR/xpath-functions-31/) library —
so gaps are explicit rather than implied. It is intentionally exhaustive: most of
the surface is not implemented, and much of it has no meaningful mapping onto a
graph query language.

**Pipeline:** `XPath string → xee-xpath-ast AST → ir::XPathQuery → terminusdb_woql2::Query`.

**Legend:**
- ✅ implemented (verified live)
- 🚧 partial / experimental
- ❌ not yet, but has a plausible WOQL mapping
- 🚫 out of scope — no sensible mapping onto TerminusDB's graph/document model
  (computational/functional/lexical features of a general-purpose expression
  language). May still be *parsed* by xee; we simply don't lower it.

> **Scope philosophy.** XPath 3.1 is a general-purpose expression language over
> XDM (nodes, atomic values, maps, arrays, functions). WOQL is a graph pattern /
> datalog language. The valuable, faithful subset to compile is **navigation +
> filtering + aggregation**. Sequence-algebra, higher-order functions, maps/arrays,
> JSON, and the type system are marked 🚫 unless a direct WOQL analogue exists.

---

## 1. Data-model mapping

XPath was designed for XML trees; TerminusDB is a document/graph store.

| XPath concept          | TerminusDB / WOQL meaning                                             |
| ---------------------- | -------------------------------------------------------------------- |
| context/root node      | a document (subject IRI)                                              |
| child step `foo`       | **object property** `@schema:foo` — a graph hop (link triple)        |
| attribute step `@foo`  | **value property** `@schema:foo` — a literal (data triple)           |
| `//foo`                | `foo` reachable via any chain of edges (`Path` with `star(any)`)     |
| predicate `[...]`      | a constraint (`And` of triples + comparisons) on the step node       |
| the final step         | the projected result (`Select([resultVar], …)`)                      |

Unqualified names get the `@schema:` prefix ([`CompileOptions::property_prefix`],
default `@schema:`); names already containing `:` (e.g. `rdf:type`) pass through.

---

## 2. TerminusDB context / resource functions

Non-standard pseudo-functions that establish where navigation begins.

| Feature            | Syntax                       | WOQL mapping                                  | Status |
| ------------------ | ---------------------------- | --------------------------------------------- | ------ |
| Select document    | `document("MyModel/1234")`   | starting subject node                         | ✅     |
| Select document    | `doc("MyModel/1234")`        | alias of `document(...)`                       | ✅     |
| Short id OR IRI    | `document("terminusdb:///data/…")` | subject node (both forms resolve)       | ✅     |
| Select database    | `db("name")`                 | `CompiledXPath::using_db` + optional `Using`  | 🚧     |
| Bare `db()` + path | `db("x")/foo`                | `Root` context (subject is a fresh variable)  | ✅     |
| Node(s) by id      | `id("a", "b")`               | disjunction over subject nodes                | ❌     |
| Read graph select  | `graph("schema"\|"instance")`| WOQL `From`                                   | ❌     |
| Branch / commit    | `branch(...)`, `commit(...)` | `BranchSpec` ref/commit selection             | ❌     |

---

## 3. Lexical / literals (XPath §2, §3.1.1)

| Feature                | Syntax                | WOQL mapping                    | Status |
| ---------------------- | --------------------- | ------------------------------- | ------ |
| Integer literal        | `42`                  | `xsd:integer` data value        | ✅ (in predicates) |
| Decimal literal        | `3.14`                | `xsd:decimal`                   | ✅ (in predicates) |
| Double literal         | `1.5e3`               | `xsd:double`/`float`            | ✅ (in predicates) |
| String literal         | `"x"`, `'x'`          | `xsd:string`                    | ✅ (in predicates / doc ids) |
| Variable reference     | `$x`                  | WOQL variable                   | ❌     |
| Parenthesized expr     | `(...)`               | grouping                        | 🚧 (unwrapped in predicates) |
| Context item           | `.`                   | the current step node           | ❌     |
| EQName / URIQualified  | `Q{uri}local`         | predicate IRI                   | ❌     |
| Comments               | `(: … :)`             | ignored                         | ✅ (via xee parser) |

---

## 4. Path expressions & steps (XPath §3.3)

### 4a. Axes

| Axis                 | Syntax             | WOQL mapping                            | Status |
| -------------------- | ------------------ | --------------------------------------- | ------ |
| child                | `foo`, `child::foo`| `Triple(subj, @schema:foo, obj)`        | ✅     |
| attribute            | `@foo`             | `Triple(subj, @schema:foo, litObj)`     | ✅     |
| descendant-or-self   | `//foo`            | `Path(subj, star(any)/foo, obj)`        | ✅     |
| descendant           | `descendant::foo`  | `Path(subj, star(any)/foo, obj)`        | ✅     |
| self                 | `.`, `self::foo`   | identity + name/type check              | ❌     |
| parent               | `..`, `parent::`   | inverse edge (`inv`)                    | ❌     |
| ancestor             | `ancestor::`       | inverse `Path`                          | ❌     |
| ancestor-or-self     | `ancestor-or-self::`| inverse `Path` incl. self              | ❌     |
| following-sibling    | `following-sibling::`| shared-parent ordering                | ❌     |
| preceding-sibling    | `preceding-sibling::`| shared-parent ordering                | ❌     |
| following            | `following::`      | document order                          | 🚫 (no document order in a graph) |
| preceding            | `preceding::`      | document order                          | 🚫     |
| namespace (deprecated)| `namespace::`     | n/a                                     | 🚫     |

### 4b. Node tests

| Test              | Syntax          | WOQL mapping                        | Status |
| ----------------- | --------------- | ----------------------------------- | ------ |
| name test         | `foo`           | predicate `@schema:foo`             | ✅     |
| wildcard          | `*`             | predicate is a fresh variable       | 🚧     |
| namespace wildcard| `ns:*`, `*:foo` | prefix/local-constrained predicate  | ❌     |
| `Q{uri}*`         | braced wildcard | ns-constrained predicate            | ❌     |
| `node()`          | `node()`        | any node                            | 🚧 (only inside `//`) |
| `text()`          | `text()`        | the node's literal value            | ❌     |
| `element(N, T)`   | element test    | `rdf:type` = `@schema:N`            | ❌     |
| `attribute(a, T)` | attribute test  | value property `a`                  | ❌     |
| `document-node()` | doc-node test   | is a top-level document             | ❌     |
| `comment()`       | comment test    | n/a                                 | 🚫     |
| `processing-instruction()` | PI test | n/a                                | 🚫     |
| `namespace-node()`| ns-node test    | n/a                                 | 🚫     |
| `schema-element()`/`schema-attribute()` | schema-aware | schema graph lookup      | ❌     |

### 4c. Predicates & path operators

| Feature               | Syntax                  | WOQL mapping                                | Status |
| --------------------- | ----------------------- | ------------------------------------------- | ------ |
| path `/`              | `a/b`                   | triple chain (join)                         | ✅     |
| path `//`             | `a//b`                  | `Path` + `star(any)`                        | ✅     |
| leading `/`           | `/a`                    | absolute path from a root                   | ❌     |
| predicate: existence  | `[submodel]`            | extra triple chain                          | ✅     |
| predicate: equality   | `[@name = "Jane"]`      | `Equals`                                    | ✅     |
| predicate: inequality | `[age != 5]`            | `Not(Equals)`                               | ✅     |
| predicate: ordered    | `[age > 21]`,`<`,`<=`,`>=` | `Greater`/`Less`/`Gte`/`Lte`             | ✅     |
| predicate: value cmp  | `[age gt 21]`, `eq`, …  | same as general comparisons                 | ✅     |
| predicate: nested path| `[a/b = "x"]`           | chained triples + comparison                | ✅     |
| predicate: `and`/`or` | `[a and b]`, `[a or b]` | `And` / `Or`                                | ❌     |
| predicate: `not(...)` | `[not(@x = 1)]`         | `Not`                                       | ❌     |
| predicate: positional | `[1]`, `[position()=n]` | `Start` + `Limit`                           | ❌     |
| predicate: `last()`   | `[last()]`              | `Count` + compare                           | ❌     |
| literal on LHS        | `["x" = @name]`         | operand swap                                | ❌     |

---

## 5. Sequence & set operators (XPath §3.4, §3.5)

| Feature            | Syntax            | WOQL mapping                     | Status |
| ------------------ | ----------------- | -------------------------------- | ------ |
| sequence concat    | `(a, b)`          | union of solutions              | ❌     |
| range              | `1 to 10`         | —                               | 🚫     |
| union              | `a \| b`, `a union b` | `Select([u], Or([a, b]))`   | 🚧 builder `\|` ✅, string form ❌ |
| intersect          | `a intersect b`   | `SetIntersection`               | ❌     |
| except             | `a except b`      | `SetDifference`                 | ❌     |

> **Union** works via the builder's `|` operator (`doc(a)/… | doc(b)/…`): each
> branch binds a shared result variable, combined with WOQL `Or` (branches share
> one variable counter so their internal vars don't clash). The **string** form
> `a | b` isn't lowered yet (xee parses it as a binary `union` expression).

---

## 6. Arithmetic (XPath §3.6)

| Feature            | Syntax            | WOQL mapping                     | Status |
| ------------------ | ----------------- | -------------------------------- | ------ |
| add / subtract     | `a + b`, `a - b`  | `Eval` + `Plus`/`Minus`         | ❌     |
| multiply / divide  | `a * b`, `a div b`| `Eval` + `Times`/`Divide`       | ❌     |
| integer divide     | `a idiv b`        | `Eval` + `Div`                  | ❌     |
| modulo             | `a mod b`         | `Eval`                          | ❌     |
| unary +/-          | `-a`              | `Eval`                          | ❌     |

---

## 7. Comparisons (XPath §3.7)

| Feature            | Syntax                    | WOQL mapping                | Status |
| ------------------ | ------------------------- | --------------------------- | ------ |
| general comparison | `=`,`!=`,`<`,`<=`,`>`,`>=`| `Equals`/`Less`/… (in predicates) | ✅ |
| value comparison   | `eq`,`ne`,`lt`,`le`,`gt`,`ge` | as above               | ✅     |
| node identity      | `a is b`                  | node equality               | ❌     |
| node order         | `a << b`, `a >> b`        | document order              | 🚫     |

---

## 8. Logical, conditional, iteration, binding (XPath §3.8–§3.10, §3.12)

| Feature            | Syntax                          | WOQL mapping             | Status |
| ------------------ | ------------------------------- | ------------------------ | ------ |
| logical and/or     | `a and b`, `a or b`             | `And` / `Or`             | ❌ (top-level; see predicates) |
| `if/then/else`     | `if (c) then a else b`          | `If`                     | ❌     |
| `for` expression   | `for $x in e return r`          | `GroupBy` / iteration    | ❌     |
| `let` expression   | `let $x := e return r`          | variable binding         | ❌     |
| quantified `some`  | `some $x in e satisfies p`      | existence                | ❌     |
| quantified `every` | `every $x in e satisfies p`     | `Not(exists ¬p)`         | ❌     |
| simple map `!`     | `a ! b`                         | per-item application     | ❌     |
| arrow `=>`         | `e => f()`                      | function application     | 🚫     |

---

## 9. Type system (XPath §3.13, §3.14, §2.5)

| Feature            | Syntax                    | WOQL mapping                | Status |
| ------------------ | ------------------------- | --------------------------- | ------ |
| `instance of`      | `e instance of T`         | `IsA` / `TypeOf`            | ❌     |
| `cast as`          | `e cast as xs:integer`    | `Typecast`                  | ❌     |
| `castable as`      | `e castable as T`         | `Typecast` guarded          | ❌     |
| `treat as`         | `e treat as T`            | type assertion              | ❌     |
| sequence types     | `item()`, `T?`,`T*`,`T+`  | cardinality                 | 🚫     |
| `element()`/`schema-element()` types | …       | schema graph                | ❌     |

---

## 10. Maps, arrays, functions (XPath 3.1 §3.11, §3.1.7)

| Feature              | Syntax                    | Notes                                  | Status |
| -------------------- | ------------------------- | -------------------------------------- | ------ |
| inline function      | `function($x){ … }`       | function values are not graph data     | 🚫     |
| named function ref   | `fn:name#2`               | —                                      | 🚫     |
| dynamic call         | `$f(x)`                   | —                                      | 🚫     |
| map constructor      | `map { "k": v }`          | —                                      | 🚫     |
| array constructor    | `[1,2]`, `array { … }`    | —                                      | 🚫     |
| lookup operator      | `$m?key`, `?*`            | —                                      | 🚫     |

---

## 11. Function & Operator library (F&O 3.1)

Grouped by category; representative functions per group (see F&O 3.1 for the full
list of ~200). ❌ = has a plausible WOQL mapping and is a candidate; 🚫 = no graph
analogue.

### 11a. Accessors (§2)
`node-name`, `nilled`, `string`, `data`, `base-uri`, `document-uri` — mostly ❌
(`string`/`data` map to reading a node's value; the rest 🚫).

### 11b. Numeric (§4)
`abs`, `ceiling`, `floor`, `round`, `round-half-to-even`, `number` → `Eval`/`Floor` ❌;
`format-integer`, `format-number` 🚫.

### 11c. Strings (§5)
- `concat`, `string-join` → `Concatenate`/`Join` ❌
- `substring`, `string-length`, `normalize-space`, `upper-case`, `lower-case`,
  `translate` → `Substring`/`Length`/`Trim`/`Upper`/`Lower`/`Pad` ❌
- `contains`, `starts-with`, `ends-with`, `substring-before`, `substring-after` →
  `Like`/`Regexp` ❌
- `compare`, `codepoint-equal`, `codepoints-to-string`, `string-to-codepoints`,
  `normalize-unicode`, `encode-for-uri`, `iri-to-uri` 🚫
- **Regex:** `matches`, `replace`, `tokenize` → `Regexp`/`Split` ❌; `analyze-string` 🚫

### 11d. Boolean (§7)
`true`, `false`, `not`, `boolean` → `True`/`Not` ❌.

### 11e. Durations, dates, times (§8, §9)
`current-dateTime`/`-date`/`-time`, component extraction (`year-from-dateTime`, …),
`adjust-*-to-timezone`, `dateTime`, `implicit-timezone`, `format-dateTime` → WOQL
`interval`/date ops partially cover these ❌; formatting/timezone 🚫.

### 11f. QNames (§10)
`resolve-QName`, `QName`, `local-name-from-QName`, `namespace-uri-from-QName`,
`namespace-uri-for-prefix`, `in-scope-prefixes` — 🚫.

### 11g. Nodes (§14)
`name`, `local-name`, `namespace-uri`, `lang`, `root`, `has-children`, `innermost`,
`outermost`, `path`, `generate-id` → `name`/`local-name`/`root`/`id` are ❌; the
tree-shape ones (`innermost`/`outermost`/`path`) 🚫.

### 11h. Sequences (§15)
- General: `empty`, `exists`, `head`, `tail`, `insert-before`, `remove`, `reverse`,
  `subsequence`, `unordered` → `exists`/`empty`/`Limit`/`Start` are ❌; order-based 🚫
- Cardinality: `zero-or-one`, `one-or-more`, `exactly-one` 🚫
- Dedup/compare: `distinct-values` → `Distinct` ❌; `index-of`, `deep-equal` 🚫
- **Aggregation:** `count`, `avg`, `max`, `min`, `sum` → `Count`/`Sum`/… ❌ *(high value)*
- Node-set by id: `id`, `element-with-id`, `idref` → `id` ❌
- Documents/collections: `doc`, `doc-available`, `collection`, `uri-collection`,
  `unparsed-text*`, `environment-variable` — `doc`/`collection` map to db access ❌; rest 🚫

### 11i. Context (§16 / accessors)
`position`, `last` → `Start`/`Limit`/`Count` ❌; `default-collation`,
`static-base-uri`, `function-lookup` 🚫.

### 11j. Higher-order functions (§16, 3.0+)
`for-each`, `filter`, `fold-left`, `fold-right`, `for-each-pair`, `apply`, `sort`,
`function-lookup`, `function-name`, `function-arity` — 🚫 (function values).

### 11k. Maps & arrays (§17, 3.1)
`map:get`/`put`/`contains`/`keys`/`merge`/`size`/`find`/`entry`/`remove`/`for-each`;
`array:get`/`size`/`append`/`subarray`/`flatten`/`join`/`sort`/`for-each`/… — 🚫.

### 11l. JSON & serialization (§17.5, 3.1)
`parse-json`, `json-doc`, `json-to-xml`, `xml-to-json`, `serialize`, `parse-xml` — 🚫.

---

## 12. Cross-cutting semantics & known gaps

- **Result semantics.** XPath yields an ordered node-set; WOQL yields a bag of
  bindings. We currently project one variable and treat results as a set
  (dedup at the edges). Document order, positional predicates, and `last()` are
  therefore ❌ (no inherent order in the graph).
- **Root-document predicates.** Predicates attach to *steps*; a relative query's
  starting subject has no step, so `doc[@year>2000]`-style filtering on the root
  document set isn't yet expressible. Needs a synthetic self-step. ❌
- **`db()` → resource descriptor.** Currently only records the name / can wrap in
  `Using`; a bare db name still needs qualifying into a full resource descriptor
  to resolve server-side. 🚧
- **Wildcard `*`** compiles to a variable predicate (works for a single hop);
  namespace-qualified wildcards and `*` semantics in all axes are 🚧/❌.
- **No schema introspection.** Anything needing the schema at compile time
  (kind tests, `element(T)`, schema-aware types) is ❌ pending a schema lookup pass.

---

## 13. Implemented today (quick reference)

Fully working and **verified live** (`tests/spec.rs`, `examples/showcase.rs`):
child steps, attribute steps, `//` descendant, name tests, `document()`/`doc()`
(short id or full IRI), relative paths, and predicates with `=`,`!=`,`<`,`<=`,`>`,`>=`
(general & value comparisons), nested relative paths, and existence — over
literal RHS of string/integer/decimal/double.

Next highest-value candidates: boolean predicates (`and`/`or`/`not`), the `|`
union operator, string functions (`contains`/`starts-with`), and aggregation
(`count`/`sum`).
