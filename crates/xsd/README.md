# terminusdb-xsd

XSD to TerminusDB schema converter using **xmlschema-rs** (pure Rust).

## Overview

This crate enables parsing XSD schemas and converting them to TerminusDB schema definitions, allowing XML documents to be stored in TerminusDB's native format.

## Architecture

```
┌─────────────┐
│  XSD File   │
└──────┬──────┘
       │ xmlschema-rs (Rust)
┌──────▼──────┐
│  XsdSchema  │  (schema_model.rs)
└──────┬──────┘
       │ XsdToSchemaGenerator
┌──────▼──────┐
│ Vec<Schema> │  (TerminusDB schemas)
└──────┬──────┘
       │
┌──────▼──────┐
│  Database   │
└─────────────┘

XML Instance Parsing:
┌─────────────┐
│  XML File   │
└──────┬──────┘
       │ XmlParser
┌──────▼──────┐
│ Vec<Instance>│ (TerminusDB instances)
└─────────────┘
```

### Why xmlschema-rs?

- ✅ **Pure Rust** - No Python/FFI dependencies
- ✅ **Fast** - Native performance, no runtime overhead
- ✅ **Comprehensive** - Handles complex XSD features (includes, imports, groups)
- ✅ **Battle-tested** - Based on Python xmlschema's well-tested logic

## Requirements

- **Rust** 1.85+

### Installation

```bash
# Add to Cargo.toml
terminusdb-xsd = { path = "path/to/crates/xsd" }

# Build the Rust crate
cargo build --release
```

## Usage

### High-Level API (XsdModel)

```rust
use terminusdb_xsd::XsdModel;

fn main() -> anyhow::Result<()> {
    // Load and convert an XSD schema
    let model = XsdModel::from_file("path/to/schema.xsd", None::<&str>)?;

    // Get generated TerminusDB schemas
    let schemas = model.schemas();
    println!("Generated {} schemas", schemas.len());

    // Get class names
    for name in model.class_names() {
        println!("  Class: {}", name);
    }

    // Find a specific schema
    if let Some(schema) = model.find_schema("BookType") {
        println!("Found BookType schema");
    }

    // Get statistics
    let stats = model.stats();
    println!("Complex types: {}", stats.total_complex_types);

    Ok(())
}
```

### Low-Level API

```rust
use terminusdb_xsd::schema_model::XsdSchema;
use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;

fn main() -> anyhow::Result<()> {
    // Parse XSD schema
    let xsd_schema = XsdSchema::from_xsd_file("path/to/schema.xsd", None::<&str>)?;

    // Generate TerminusDB schemas
    let generator = XsdToSchemaGenerator::with_namespace("http://example.com/ns#");
    let schemas = generator.generate(&xsd_schema)?;

    // Convert to JSON for TerminusDB
    use terminusdb_schema::json::ToJson;
    for schema in &schemas {
        let json = schema.to_json();
        println!("{}", serde_json::to_string_pretty(&json)?);
    }

    Ok(())
}
```

### Directory-Based Generation

```rust
use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;

// Process entire schema bundles with automatic entry point detection
let generator = XsdToSchemaGenerator::with_namespace("http://example.com/ns#");
let schemas = generator.generate_from_directory(&schema_dir, None::<PathBuf>)?;
```

### XML Instance Parsing

```rust
use terminusdb_xsd::xml_parser::XmlParser;

let parser = XmlParser::new();
let instances = parser.parse_xml_file("path/to/document.xml", &xsd_schema)?;

for instance in instances {
    println!("Instance: {:?}", instance);
}
```

## Examples

```bash
# Entry point analysis demo
cargo run --example analyze_entry_points

# Generate from directory
cargo run --example generate_from_directory

# NISO-STS schemas
cargo run --example generate_niso_schemas

# DITA Learning schemas
cargo run --example generate_dita_learning

# S1000D Issue 6 schemas
cargo run --example generate_s1000d
```

## Testing

```bash
# Run all tests
cargo test

# Run schema generation tests
cargo test --test schema_generation_tests

# Run real-world schema tests
cargo test --test real_world_schemas
```

## XSD to TerminusDB Mapping

| XSD Construct | TerminusDB Schema |
|---------------|-------------------|
| `xs:complexType` | `Schema::Class` |
| `xs:element` (child) | `Property` |
| `xs:attribute` | `Property` |
| `xs:simpleType` (enum) | `Schema::Enum` |
| `xs:string` | `xsd:string` |
| `xs:integer` | `xsd:integer` |
| `xs:decimal` | `xsd:decimal` |
| `xs:boolean` | `xsd:boolean` |
| `xs:dateTime` | `xsd:dateTime` |
| `maxOccurs="unbounded"` | `TypeFamily::Set` |
| `minOccurs="0"` | `TypeFamily::Optional` |
| Named types | `Key::ValueHash` |
| Anonymous types | `Key::Random` (subdocument) |

## Known Limitations

### 1. `xs:union` Types Not Converted

**Issue:** XSD union types are parsed by xmlschema-rs but not extracted/converted to TerminusDB schemas.

**Current Behavior:** Union types are silently ignored.

**Correct Mapping:** Should map to `Schema::TaggedUnion` or `Schema::OneOfClass`.

**Root Cause:** `schema_model.rs` XsdSimpleType struct doesn't extract `variety()` or `member_types` from xmlschema-rs.

**Example:**
```xml
<xs:simpleType name="stringOrNumber">
  <xs:union memberTypes="xs:string xs:integer"/>
</xs:simpleType>
```

Should become:
```json
{
  "@type": "TaggedUnion",
  "@id": "StringOrNumber",
  "stringValue": "xsd:string",
  "integerValue": "xsd:integer"
}
```

### 2. `xs:list` Types Incorrectly Mapped to Set

**Issue:** XSD list types (space-separated values) are mapped to `TypeFamily::Set` instead of `TypeFamily::List`.

**Current Behavior:** Uses Set (unordered, no duplicates) based on cardinality heuristics.

**Correct Mapping:** Should map to `TypeFamily::List` (ordered, allows duplicates).

**Root Cause:** `schema_generator.rs` determines collection type from `maxOccurs` cardinality, not from `SimpleTypeVariety::List`.

**Example:**
```xml
<xs:simpleType name="integerList">
  <xs:list itemType="xs:integer"/>
</xs:simpleType>
```

Should use `TypeFamily::List`, not `TypeFamily::Set`.

### 3. `xs:redefine` Not Supported

**Issue:** XSD redefine directive is not supported by xmlschema-rs.

**Impact:** DITA schemas that use `xs:redefine` for domain specialization may have missing elements (e.g., `title` element in TopicClass).

**Workaround:** Use URL-based schemas (`xsd1.2-url`) instead of catalog-based schemas where possible.

See [PLAN.md](../../.claude/plans/) for implementation plan.

### 4. Catalog Resolution

**Issue:** URN-based schemas may fail resolution.

**Current Behavior:** URL-based schemas work reliably; URN-based schemas depend on catalog configuration.

**Workaround:** Use `xsd1.2-url` instead of `xsd1.2` for DITA schemas.

### 5. xs:any and xs:anyAttribute

**Issue:** Wildcard elements and attributes are not fully represented in generated schemas.

**Current Behavior:** Wildcard content may be omitted or simplified.

## Tested Schema Standards

| Standard | Schemas Generated | Status |
|----------|-------------------|--------|
| DITA Base 1.2 | 286 | ✅ |
| NISO-STS (JATS) | 347 | ✅ |
| DITA Learning | 744 | ✅ |
| S1000D Issue 6 | 1,166 | ✅ |
| **Total** | **2,543** | ✅ |

See [TESTING.md](./TESTING.md) for detailed test results.

## Related Documentation

- [TESTING.md](./TESTING.md) - Detailed testing summary
- [FINDINGS.md](./FINDINGS.md) - Historical PyO3 experiment findings

## Contributing

Contributions welcome! Priority areas:

1. **xs:union support** - Extract `variety()` and `member_types` from xmlschema-rs
2. **xs:list support** - Map to `TypeFamily::List` instead of Set
3. **xs:redefine support** - Implement in xmlschema-rs upstream

## License

MIT OR Apache-2.0 (same as workspace)

## References

- [xmlschema-rs](https://github.com/your-fork/xmlschema-rs) - XSD parsing library
- [TerminusDB](https://terminusdb.com/)
- [W3C XML Schema Spec](https://www.w3.org/TR/xmlschema11-1/)
