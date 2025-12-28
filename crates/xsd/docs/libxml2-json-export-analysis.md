# Analysis: Adding JSON Schema Export to libxml2

## Executive Summary

**Feasibility: MODERATE** - Doable but requires C programming and understanding of libxml2 internals.

**Effort Estimate: 2-3 days** for basic implementation, 1 week for production-quality with tests.

**Better Alternative: Continue with Python xmlschema** - already working, handles catalogs, no C maintenance burden.

---

## libxml2 Schema Architecture

### Core Data Structures

#### `xmlSchema` (top-level container)
```c
struct _xmlSchema {
    const xmlChar *name;
    const xmlChar *targetNamespace;
    const xmlChar *version;

    xmlHashTable *typeDecl;      // Type declarations
    xmlHashTable *attrDecl;      // Attribute declarations
    xmlHashTable *attrgrpDecl;   // Attribute group declarations
    xmlHashTable *elemDecl;      // Element declarations
    xmlHashTable *notaDecl;      // Notation declarations
    xmlHashTable *groupDecl;     // Group declarations

    xmlHashTable *schemasImports;
    xmlDict *dict;               // Shared string dictionary
};
```

#### `xmlSchemaType` (type definition)
```c
struct _xmlSchemaType {
    xmlSchemaTypeType type;          // Kind: BASIC, SIMPLE, COMPLEX, etc.
    const xmlChar *name;
    const xmlChar *targetNamespace;

    xmlSchemaContentType contentType; // EMPTY, ELEMENTS, MIXED, SIMPLE
    const xmlChar *base;              // Base type name
    const xmlChar *baseNs;
    xmlSchemaType *baseType;          // Resolved base type

    xmlSchemaFacet *facets;           // Restrictions (minLength, pattern, etc.)
    xmlSchemaAttributeLink **attributeUses;
    xmlSchemaWildcard *attributeWildcard;

    xmlSchemaTypeLink *memberTypes;   // For union types
    xmlSchemaType *contentTypeDef;    // For complex types
    xmlRegexp *contModel;             // Content model automaton
};
```

### Existing Debug Infrastructure

libxml2 **already has** schema introspection code (when `LIBXML_DEBUG_ENABLED`):

```c
// From xmlschemas.c (lines ~30890-30920)
void xmlSchemaDump(FILE *output, xmlSchema *schema) {
    fprintf(output, "Schemas: %s\n", schema->name);
    fprintf(output, "Target Namespace: %s\n", schema->targetNamespace);

    // Iterate all types
    xmlHashScan(schema->typeDecl, xmlSchemaTypeDumpEntry, output);

    // Iterate all elements
    xmlHashScanFull(schema->elemDecl, xmlSchemaElementDump, output);
}

static void xmlSchemaTypeDump(xmlSchemaTypePtr type, FILE *output) {
    fprintf(output, "Type: '%s' ns '%s'\n", type->name, type->targetNamespace);
    // ... dumps base type, content model, attributes, etc.
}
```

---

## Implementation Plan

### Option A: Extend Existing Debug Output

**Modify** `xmlSchemaDump` to output JSON instead of plain text.

**Steps:**
1. Add `xmlSchemaDumpJSON(FILE *output, xmlSchema *schema)` function
2. Implement JSON serialization for each type:
   - `jsonDumpType(xmlSchemaTypePtr type, FILE *output)`
   - `jsonDumpElement(xmlSchemaElementPtr elem, FILE *output)`
   - `jsonDumpAttribute(xmlSchemaAttributePtr attr, FILE *output)`
3. Handle hash table iteration with JSON array formatting
4. Escape strings properly for JSON

**Code Structure:**
```c
void xmlSchemaDumpJSON(FILE *output, xmlSchema *schema) {
    fprintf(output, "{\n");
    fprintf(output, "  \"name\": \"%s\",\n", schema->name);
    fprintf(output, "  \"targetNamespace\": \"%s\",\n", schema->targetNamespace);

    fprintf(output, "  \"types\": [\n");
    xmlHashScan(schema->typeDecl, jsonDumpTypeEntry, output);
    fprintf(output, "  ],\n");

    fprintf(output, "  \"elements\": [\n");
    xmlHashScanFull(schema->elemDecl, jsonDumpElementEntry, output);
    fprintf(output, "  ]\n");

    fprintf(output, "}\n");
}
```

### Option B: Add CLI Flag to xmllint

**Extend** `xmllint.c` to add `--schema-dump-json` flag.

**Steps:**
1. Add new command-line option parsing
2. After `xmlSchemaParse()`, call JSON dump function
3. Output to stdout or specified file

**Usage:**
```bash
xmllint --schema-dump-json schema.xsd > schema.json

# With catalog support (already exists!)
xmllint --catalogs --schema-dump-json schema.xsd > schema.json
```

---

## Challenges

### 1. **Hash Table Iteration with JSON Formatting** ⚠️
libxml2 uses `xmlHashTable` for storing types/elements. Need to track:
- First/last item (no trailing comma on last item)
- Nested structures
- Proper JSON array formatting

**Solution:** Use callback context to track state:
```c
typedef struct {
    FILE *output;
    int first;
    int count;
} JsonDumpContext;
```

### 2. **String Escaping** ⚠️
Need to escape special JSON characters: `"`, `\`, `/`, control chars.

**Solution:** Implement `jsonEscapeString()` helper:
```c
static void jsonEscapeString(FILE *output, const xmlChar *str) {
    while (*str) {
        if (*str == '"') fprintf(output, "\\\"");
        else if (*str == '\\') fprintf(output, "\\\\");
        else if (*str == '\n') fprintf(output, "\\n");
        // ... etc
        else fputc(*str, output);
        str++;
    }
}
```

### 3. **Circular References** ⚠️
Types can reference each other (e.g., base types, member types).

**Solution:** Only output type *names* for references, not full nesting:
```json
{
  "name": "DerivedType",
  "baseType": "BaseTypeName",  // Reference by name, not nested
  "baseTypeNs": "http://..."
}
```

### 4. **Anonymous Types** ⚠️
Complex types can be inlined without names.

**Solution:** Generate synthetic names like `anonymous_element_foo_type`.

---

## Pros & Cons

### Pros ✅
- **Catalog support already exists** in libxml2/xmllint
- **All data structures accessible** via existing debug code
- **No Python dependency** for final tool
- **Maintained by GNOME project** - widely used, stable
- **Fast C implementation** - better performance than Python

### Cons ❌
- **Requires C programming** - steeper learning curve than Python
- **Need to compile libxml2** - can't just `pip install`
- **Maintenance burden** - need to keep patch up-to-date with libxml2 releases
- **JSON library not included** - libxml2 doesn't have built-in JSON writer
- **Already working solution** - Python xmlschema does everything we need
- **Would need to write JSON manually** - no high-level serialization like serde

---

## Recommendation: ❌ Don't Implement

### Why NOT to Add JSON Export to libxml2:

1. **Python xmlschema is already working**
   - Handles DITA (347 types) and NISO (326 types) perfectly
   - Custom catalog/URN resolution via `uri_mapper`
   - No bugs or limitations found

2. **Maintenance burden**
   - Would need to maintain C code patch
   - Recompile libxml2 for each deployment
   - Track libxml2 updates and rebase patches

3. **No clear advantage**
   - PyO3 overhead is acceptable (one-time schema parsing)
   - Native Rust (xsd-parser) failed on both schemas
   - libxml2 approach would still require custom JSON serialization code

4. **Better use of time**
   - Focus on **Phase 2: XSD → TerminusDB mapping**
   - Design type conversion strategy
   - Generate TerminusDB schemas from extracted data

### If You REALLY Want Pure Rust:

Better to **contribute catalog support to xsd-parser** than patch libxml2:
- Rust ecosystem benefits
- No C maintenance
- Already has MetaTypes model
- Just needs URN resolution

---

## Code Snippet: Minimal JSON Export

If you still want to try it, here's a starting point:

```c
// Add to xmlschemas.c

#ifdef LIBXML_DEBUG_ENABLED

static void
jsonDumpTypeEntry(void *type, void *context, const xmlChar *name) {
    JsonDumpContext *ctx = (JsonDumpContext *)context;
    xmlSchemaTypePtr t = (xmlSchemaTypePtr)type;

    if (!ctx->first) fprintf(ctx->output, ",\n");
    ctx->first = 0;

    fprintf(ctx->output, "    {\n");
    fprintf(ctx->output, "      \"name\": \"");
    jsonEscapeString(ctx->output, t->name);
    fprintf(ctx->output, "\",\n");

    fprintf(ctx->output, "      \"type\": \"%s\",\n",
            xmlSchemaGetTypeKind(t->type));

    if (t->targetNamespace) {
        fprintf(ctx->output, "      \"namespace\": \"");
        jsonEscapeString(ctx->output, t->targetNamespace);
        fprintf(ctx->output, "\",\n");
    }

    if (t->base) {
        fprintf(ctx->output, "      \"baseType\": \"");
        jsonEscapeString(ctx->output, t->base);
        fprintf(ctx->output, "\"\n");
    }

    fprintf(ctx->output, "    }");
}

void
xmlSchemaDumpJSON(FILE *output, xmlSchema *schema) {
    JsonDumpContext ctx = { output, 1, 0 };

    fprintf(output, "{\n");
    fprintf(output, "  \"name\": \"");
    if (schema->name) jsonEscapeString(output, schema->name);
    fprintf(output, "\",\n");

    fprintf(output, "  \"targetNamespace\": \"");
    if (schema->targetNamespace)
        jsonEscapeString(output, schema->targetNamespace);
    fprintf(output, "\",\n");

    fprintf(output, "  \"types\": [\n");
    xmlHashScan(schema->typeDecl, jsonDumpTypeEntry, &ctx);
    fprintf(output, "\n  ]\n");
    fprintf(output, "}\n");
}

#endif /* LIBXML_DEBUG_ENABLED */
```

---

## Conclusion

While **technically feasible** (~2-3 days work), adding JSON export to libxml2 is **not recommended** because:

1. ✅ **Current solution works perfectly** - Python xmlschema handles everything
2. ❌ **Maintenance overhead** - C code, compilation, version tracking
3. ❌ **No performance benefit** - schema parsing is one-time operation
4. ⏱️ **Better use of time** - focus on XSD→TerminusDB conversion logic

**Verdict:** Stick with Python xmlschema, move to Phase 2 (type mapping) 🎯
