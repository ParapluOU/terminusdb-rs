# terminusdb-xsd

XSD to TerminusDB model converter using **PyO3** and Python's `xmlschema`.

## Overview

This crate enables parsing XSD schemas and converting them to TerminusDB model definitions, allowing XML documents to be stored in TerminusDB's native format.

## Status

✅ **Experimental - PyO3 approach WORKS!**

Successfully tested:
- Python integration via PyO3
- xmlschema module import (v4.2.0)
- XSD schema parsing
- JSON data extraction

## Approach

Uses PyO3 (Rust bindings for Python) to call the battle-tested `xmlschema` Python library:

```
XSD File → xmlschema (Python) → JSON → TerminusDB Model (Rust)
```

### Why PyO3?

- ✅ **Works immediately** - Full Python stdlib and package support
- ✅ **Proven solution** - Leverages mature `xmlschema` library
- ✅ **Complete XSD support** - W3C XML Schema 1.0/1.1 compliant
- ✅ **Maintained** - Active Python package development
- ✅ **Fast development** - No custom XSD parser needed

### Why Not RustPython?

RustPython (pure Rust Python interpreter) was tested but failed:
- ❌ Incomplete stdlib (no JSON module!)
- ❌ No third-party package support
- ❌ Not production-ready for complex libraries

See [FINDINGS.md](./FINDINGS.md) for detailed comparison.

## Requirements

- **Rust** 1.85+
- **Python** 3.7+
- **xmlschema** Python package (required)
- **lxml** Python package (recommended for DITA/OASIS catalog support)

### Installation

```bash
# Install required Python packages
pip install xmlschema

# Optional: Install lxml for proper OASIS XML Catalog support (DITA schemas)
pip install lxml

# Build the Rust crate
cargo build --release
```

## Usage

```rust
use terminusdb_xsd::{XsdParser, Result};

fn main() -> Result<()> {
    // Create parser (initializes Python via PyO3)
    let parser = XsdParser::new()?;

    // Verify xmlschema is available
    let version = parser.test_xmlschema_import()?;
    println!("Using xmlschema version: {}", version);

    // Parse an XSD schema
    let schema_json = parser.parse_xsd_to_json("path/to/schema.xsd")?;
    println!("{}", serde_json::to_string_pretty(&schema_json)?);

    // Get detailed element information
    let elements = parser.get_schema_elements("path/to/schema.xsd")?;
    println!("{}", serde_json::to_string_pretty(&elements)?);

    // Get comprehensive schema info
    let info = parser.get_schema_comprehensive("path/to/schema.xsd")?;
    println!("{}", serde_json::to_string_pretty(&info)?);

    Ok(())
}
```

## Examples

```bash
# Test PyO3 integration
cargo run --example test_pyo3

# Parse an XSD file
cargo run --example test_pyo3 -- path/to/schema.xsd
```

## Testing

```bash
# Run basic tests (no xmlschema needed)
cargo test

# Run integration test (requires xmlschema)
cargo test --ignored
```

## Deployment Options

### Option 1: System Python (Simplest)

Users install Python and xmlschema:
```bash
pip install xmlschema
cargo run --release
```

### Option 2: Docker (Recommended)

```dockerfile
FROM rust:latest
RUN apt-get update && apt-get install -y python3-pip
RUN pip3 install xmlschema==4.2.0
COPY . .
RUN cargo build --release
```

### Option 3: Bundled Python

Use [PyOxidizer](https://github.com/indygreg/PyOxidizer) to bundle Python + xmlschema into a single binary (advanced).

## Development Roadmap

### Phase 1: Core Functionality ✅

- [x] PyO3 integration
- [x] xmlschema import verification
- [x] Basic XSD parsing
- [x] JSON extraction

### Phase 2: XSD → TerminusDB Mapping

- [ ] Extract full schema structure (elements, types, attributes)
- [ ] Design XSD to TerminusDB type mapping
- [ ] Generate TerminusDB JSON schema
- [ ] Generate Rust code with terminusdb-schema macros

### Phase 3: DITA Support

- [ ] Parse DITA XSD schemas
- [ ] Generate TerminusDB models for DITA
- [ ] Validate against real DITA documents

### Phase 4: Production

- [ ] Comprehensive error handling
- [ ] Schema validation
- [ ] Namespace resolution
- [ ] Include/import handling
- [ ] CLI tool

## Architecture

```
┌─────────────┐
│  Rust Code  │
└──────┬──────┘
       │ PyO3 FFI
┌──────▼──────┐
│   Python    │
│ xmlschema   │
└──────┬──────┘
       │
┌──────▼──────┐
│  XSD File   │
└─────────────┘
       │
       ▼
┌─────────────┐
│    JSON     │
└──────┬──────┘
       │
┌──────▼──────┐
│ TerminusDB  │
│   Schema    │
└─────────────┘
```

## Limitations

1. **Python Dependency**: Requires Python runtime (acceptable for server deployments)
2. **FFI Overhead**: Small performance cost for Rust ↔ Python calls
3. **Package Management**: xmlschema must be installed separately

## Alternatives Considered

- **Pure Rust XSD Parser**: Too complex, significant development effort
- **RustPython**: Incomplete stdlib, not production-ready
- **Java via JNI**: JVM dependency, added complexity
- **XSLT**: Limited transformation capability

## Contributing

This is an experimental crate. Feedback and contributions welcome!

## License

MIT OR Apache-2.0 (same as workspace)

## References

- [PyO3 Documentation](https://pyo3.rs/)
- [xmlschema Package](https://github.com/sissaschool/xmlschema)
- [W3C XML Schema Spec](https://www.w3.org/TR/xmlschema11-1/)
- [TerminusDB](https://terminusdb.com/)
