---
tags:
  - reference
  - schema
  - curl
  - intermediate
title: Prefix Management API
nextjs:
  metadata:
    title: Prefix Management API
    description: Learn how to manage namespace prefixes in TerminusDB to simplify your document IRIs and organize your data vocabularies.
    keywords: prefix, namespace, IRI, REST API, CRUD, vocabulary, semantic web
    alternates:
      canonical: https://terminusdb.org/docs/prefix-management
---

## What You'll Learn

This guide teaches you how to manage namespace prefixes in TerminusDB. You'll learn:
- **What** prefixes are and why they matter
- **When** to use each operation (GET, POST, PUT, DELETE)
- **How** to add, update, and remove prefixes safely
- **Best practices** for organizing your data vocabularies

## What Are Prefixes?

Prefixes are shorthand aliases for long namespace URIs. They make your documents cleaner and easier to read. There are default prefixes in TerminusDB that are normally relatively invisible, the `@base`, and `@schema` prefixes that are tied to TerminusDB.

Instead of using the built in prefixes, it's also possible to use global standards and setup the TerminusDB data product to align with standard ontologies.

**Without prefixes:**
```json
{
  "@id": "http://example.org/data/Person/alice",
  "@type": "http://schema.org/Person",
  "http://schema.org/name": "Alice"
}
```

**With prefixes:**
```json
{
  "@id": "ex:Person/alice",
  "@type": "schema:Person",
  "schema:name": "Alice"
}
```

### Why Use Prefixes?

1. **Readability**: `schema:Person` is clearer than `http://schema.org/Person`
2. **Maintainability**: Make software easier to change when prefixes change
3. **Interoperability**: Use standard vocabularies (schema.org, FOAF, Dublin Core)
4. **Organization**: Group related concepts under prefixes (`company:Employee`, `company:Department`)

## Common Use Cases

### Scenario 1: Setting Up Standard Vocabularies

When starting a new database, add common prefixes for vocabularies you'll use:

```bash
# Add schema.org for general types
curl -X POST http://localhost:6363/api/prefix/admin/MyDatabase/schema \
  -u admin:root -H "Content-Type: application/json" \
  -d '{"uri": "http://schema.org/"}'

# Add Dublin Core for metadata
curl -X POST http://localhost:6363/api/prefix/admin/MyDatabase/dc \
  -u admin:root -H "Content-Type: application/json" \
  -d '{"uri": "http://purl.org/dc/elements/1.1/"}'
```

### Scenario 2: Organizing Company Data

Use custom prefixes to organize your domain-specific data:

```bash
# Company schema
curl -X POST http://localhost:6363/api/prefix/admin/MyDatabase/company \
  -u admin:root -H "Content-Type: application/json" \
  -d '{"uri": "http://mycompany.com/schema/"}'

# Then use in documents: company:Employee, company:Department
```

### Scenario 3: API Version Migration

Update a prefix when your API schema changes:

```bash
# Update to v2 of your API schema
curl -X PUT http://localhost:6363/api/prefix/admin/MyDatabase/api \
  -u admin:root -H "Content-Type: application/json" \
  -d '{"uri": "http://api.mycompany.com/v2/schema/"}'
```

## API Reference

### Base Endpoint

```
/api/prefix/{organization}/{database}/{prefix_name}
```

### Quick Reference

| Method | Use When... | What It Does |
|--------|-------------|--------------|
| **GET** | You need to check what a prefix expands to | Retrieves the IRI for a prefix |
| **POST** | Adding a new prefix for the first time | Creates a prefix (fails if exists) |
| **PUT** | Changing where an existing prefix points | Updates a prefix (fails if not exists) |
| **PUT ?create=true** | You want the prefix to exist regardless | Creates OR updates (always succeeds) |
| **DELETE** | Removing an unused prefix | Deletes a prefix |

## Operations

### GET - Check a Prefix

**Use when:** You need to verify what IRI a prefix expands to, or check if a prefix exists before using it in documents.

```bash
curl -X GET \
  http://localhost:6363/api/prefix/admin/MyDatabase/myprefix \
  -u admin:root
```

**Response (200 OK):**
```json
{
  "@type": "api:PrefixResponse",
  "api:prefix_name": "myprefix",
  "api:prefix_uri": "http://example.org/myprefix/",
  "api:status": "api:success"
}
```

**Error (404 Not Found):**
```json
{
  "@type": "api:PrefixErrorResponse",
  "api:status": "api:not_found",
  "api:error": {
    "@type": "api:PrefixNotFound",
    "api:prefix_name": "myprefix"
  },
  "api:message": "Prefix 'myprefix' not found"
}
```

---

### POST - Add a New Prefix

**Use when:** You're adding a prefix for the first time. This ensures you don't accidentally overwrite an existing prefix.

**⚠️ Important:** This operation fails if the prefix already exists. Use PUT with `?create=true` if you want upsert behavior.

```bash
curl -X POST \
  http://localhost:6363/api/prefix/admin/MyDatabase/myprefix \
  -u admin:root \
  -H "Content-Type: application/json" \
  -d '{"uri": "http://example.org/myprefix/"}'
```

**Response (201 Created):**
```json
{
  "@type": "api:PrefixAddResponse",
  "api:prefix_name": "myprefix",
  "api:prefix_uri": "http://example.org/myprefix/",
  "api:status": "api:success"
}
```

**Common Errors:**

| Status | Error Type | Cause | Solution |
|--------|-----------|-------|----------|
| 400 | `PrefixAlreadyExists` | Prefix name already in use | Use PUT to update, or DELETE then POST |
| 400 | `ReservedPrefix` | Name starts with `@` | Choose different name (@ prefixes are system-reserved) |
| 400 | `InvalidIRI` | URI missing scheme | Add `http://` or `https://` to the URI |

**Example - Prefix Already Exists:**
```json
{
  "@type": "api:PrefixErrorResponse",
  "api:status": "api:failure",
  "api:error": {
    "@type": "api:PrefixAlreadyExists",
    "api:prefix_name": "myprefix"
  },
  "api:message": "Prefix 'myprefix' already exists"
}
```

---

### PUT - Update an Existing Prefix

**Use when:** You need to change where an existing prefix points. For example, migrating from `http://v1.api/` to `http://v2.api/`.

**⚠️ Important:** This operation fails if the prefix doesn't exist. Use PUT with `?create=true` for upsert behavior (create if missing).

```bash
curl -X PUT \
  http://localhost:6363/api/prefix/admin/MyDatabase/myprefix \
  -u admin:root \
  -H "Content-Type: application/json" \
  -d '{"uri": "http://example.org/updated/"}'
```

**Response (200 OK):**
```json
{
  "@type": "api:PrefixUpdateResponse",
  "api:prefix_name": "myprefix",
  "api:prefix_uri": "http://example.org/updated/",
  "api:status": "api:success"
}
```

---

### PUT ?create=true - Upsert (Create or Update)

**Use when:** You want to ensure a prefix has a specific value, regardless of whether it already exists. Perfect for initialization scripts or idempotent operations.

**💡 Tip:** Upsert is safer for automation - it always succeeds whether the prefix exists or not.

```bash
curl -X PUT \
  "http://localhost:6363/api/prefix/admin/MyDatabase/myprefix?create=true" \
  -u admin:root \
  -H "Content-Type: application/json" \
  -d '{"uri": "http://example.org/myprefix/"}'
```

**When to use upsert vs POST/PUT:**
- **Upsert (`?create=true`)**: Initialization scripts, ensuring consistency
- **POST**: Interactive use, want to know if prefix already exists
- **PUT**: Explicit updates, want to know if prefix is missing

---

### DELETE - Remove a Prefix

**Use when:** Cleaning up unused prefixes or removing deprecated namespace mappings.

**⚠️ Warning:** Deleting a prefix does NOT delete documents using that prefix. Existing documents will still reference the full IRI.

```bash
curl -X DELETE \
  http://localhost:6363/api/prefix/admin/MyDatabase/myprefix \
  -u admin:root
```

**Response (200 OK):**
```json
{
  "@type": "api:PrefixDeleteResponse",
  "api:prefix_name": "myprefix",
  "api:status": "api:success"
}
```

---

## Best Practices & Guidelines

### Naming Conventions

**Character Rules (XML NCName - same as used throughout TerminusDB):**

Prefix names must follow these rules:
1. **Start with**: Letter (`A-Z`, `a-z`) or underscore (`_`)
2. **Continue with**: Letters, digits (`0-9`), hyphen (`-`), dot (`.`), underscore (`_`)
3. **Cannot contain**: Colon (`:`)
4. **Cannot start with**: `@` (reserved) or digit

**Valid examples:**
- `schema` - Simple name
- `v1.0` - Version with dot
- `my-api_v2` - Hyphens and underscores
- `_internal` - Starts with underscore (allowed)
- `v1.` - Ends with dot (allowed)
- `dc` - Short name

**Invalid examples:**
- `@custom` - Starts with `@` (reserved for system)
- `1version` - Starts with digit
- `-prefix` - Starts with hyphen
- `bad:name` - Contains colon (reserved in XML)
- `bad/name` - Contains slash
- `bad name` - Contains space

**Recommendations:**
- Use descriptive names: `schema`, `company`, `api_v2`
- Follow conventions: `dc` (Dublin Core), `foaf` (Friend of a Friend)
- Version naming: `v1`, `v2`, `v1.0`, `api_v2`
- Underscore prefixes for internal use: `_temp`, `_draft`
- Avoid single letters: `x`, `y`, `z` (not self-documenting)

{% callout type="note" %}
Note that Turtle and SPARQL use a narrower specification. TerminusDB aligns with XML NCName for prefix names which is broader. Keep within the PN_PREFIX to avoid prefix surprises. TerminusDB stays XML-friendly where possible, and uses conventions from the XML world where it is reasonable.
{% /callout %}

### IRI Format Rules

All IRIs must include a valid scheme:

**Valid:**
- `http://example.org/`
- `https://schema.org/`
- `urn:isbn:`

**Invalid:**
- `example.org/` (missing scheme)
- `www.example.org/` (missing scheme)
- `//example.org/` (missing scheme)

**Error example:**
```json
{
  "@type": "api:PrefixErrorResponse",
  "api:status": "api:failure",
  "api:error": {
    "@type": "api:InvalidIRI",
    "api:iri": "example.org/"
  },
  "api:message": "Invalid IRI: 'example.org/'. IRIs must have a valid scheme (e.g., http://, https://)"
}
```

### Reserved Prefixes (JSON-LD Keywords)

**All prefixes starting with `@` are reserved for JSON-LD keywords and cannot be modified.**

JSON-LD uses the `@` namespace for special keywords. TerminusDB blocks all `@`-prefixed names to maintain JSON-LD compliance.

Common JSON-LD keywords include:

| Prefix | Purpose | Can Modify? |
|--------|---------|-------------|
| `@base` | Base IRI for instance data | ❌ No |
| `@schema` | Schema IRI | ❌ No |
| `@type` | Type indicator in JSON-LD | ❌ No |
| Any other `@*` | Reserved for future use | ❌ No |
| All other names | User-defined | ✅ Yes |

**Attempting to modify reserved prefixes:**
```json
{
  "@type": "api:PrefixErrorResponse",
  "api:status": "api:failure",
  "api:error": {
    "@type": "api:ReservedPrefix",
    "api:prefix_name": "@base"
  },
  "api:message": "Cannot modify reserved prefix '@base'. Prefixes starting with '@' are reserved."
}
```

### Unicode & Internationalization

Prefix IRIs support full Unicode, including emoji and international characters:

```bash
# Emoji in IRI (yes, this works!)
curl -X POST http://localhost:6363/api/prefix/admin/MyDatabase/rocket \
  -u admin:root -H "Content-Type: application/json" \
  -d '{"uri": "http://example.org/🚀/"}'

# International characters
curl -X POST http://localhost:6363/api/prefix/admin/MyDatabase/cn \
  -u admin:root -H "Content-Type: application/json" \
  -d '{"uri": "http://example.org/中文/"}'
```

### Atomicity & Concurrency

All operations are **ACID-compliant**:

- **Atomic**: Operations succeed completely or not at all
- **Consistent**: Database remains in valid state
- **Isolated**: Concurrent operations don't interfere
- **Durable**: Changes persist after successful response

**Safe for concurrent use:** Multiple clients can add/update/delete prefixes simultaneously without data corruption.

## Additional Resources

### View All Prefixes

Get the complete context document with all prefixes:

```bash
curl -X GET http://localhost:6363/api/prefixes/admin/MyDatabase -u admin:root
```

Returns `@base`, `@schema`, and all custom prefixes in a single JSON-LD context document.

### Client Libraries

For programmatic access, use the TerminusDB client libraries:

- **JavaScript**: `client.getPrefix()`, `client.addPrefix()`, etc. (not yet available — use HTTP API directly)
- **Python**: `client.get_prefix()`, `client.add_prefix()`, etc. (not yet available — use HTTP API directly)

### Related Documentation

- [JSON-LD Contexts](https://www.w3.org/TR/json-ld11/#the-context) - W3C standard
- [Schema.org](https://schema.org/) - Common vocabulary for structured data
- [Dublin Core](https://www.dublincore.org/) - Metadata vocabulary
- [FOAF](http://xmlns.com/foaf/spec/) - Friend of a Friend vocabulary

## Troubleshooting

### Common Issues

**Problem:** "Prefix already exists" when using POST
- **Solution:** Use PUT to update, or DELETE then POST, or use PUT with `?create=true` for upsert

**Problem:** "Invalid IRI" error
- **Solution:** Ensure URI includes scheme (`http://`, `https://`, etc.)

**Problem:** "Cannot modify reserved prefix"
- **Solution:** Use a different prefix name (don't start with `@`)

**Problem:** Changed prefix but documents still use old IRI
- **Solution:** This is expected. Changing a prefix only affects future prefix expansion, not existing documents. Documents store full IRIs internally.
