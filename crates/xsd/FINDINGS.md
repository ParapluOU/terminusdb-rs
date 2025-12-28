# Python + Rust XSD Parser Experiment: Findings

## Objective

Test whether we can use Python's `xmlschema` module from Rust to parse XSD schemas, avoiding the need to write a full XSD parser in Rust.

## Approaches Tested

### Approach 1: RustPython (Pure Rust Python Interpreter)

**Status:** ❌ **FAILED**

**What is RustPython?**
- A Python interpreter written entirely in Rust
- No Python runtime dependency
- Can compile to WebAssembly
- Still experimental (version 0.4)

**Results:**
- ✅ Basic Python code executes
- ❌ JSON module not available ("ModuleNotFoundError: No module named 'json'")
- ❌ xmlschema cannot be imported (expected)
- ❌ Incomplete stdlib support
- ❌ No third-party package support

**Conclusion:** RustPython's stdlib coverage is too incomplete for production use with complex libraries like xmlschema.

---

### Approach 2: PyO3 (FFI Bindings to CPython)

**Status:** ✅ **SUCCESS!**

**What is PyO3?**
- Rust bindings for the CPython interpreter
- Uses FFI to call real Python code
- Requires Python runtime at deployment
- Full stdlib and package support

**Results:**
- ✅ Python execution works perfectly
- ✅ JSON module works
- ✅ **xmlschema imported successfully (version 4.2.0)**
- ✅ **XSD schemas can be parsed**
- ✅ **Schema information extracted as JSON**
- ✅ All Python packages available

**Test Results:**
```
=== PyO3 + xmlschema Test ===

Test 1: Basic Python
✓ Python works! 2 + 2 = 4

Test 2: JSON module
✓ JSON module works! Extracted: value

Test 3: xmlschema import
✓ SUCCESS! xmlschema version 4.2.0 imported!
  PyO3 CAN use the xmlschema module!

Test 4: Create sample XSD and parse it
✓ XSD parsing works!
  Parsed schema: {
  "elements": [
    "person"
  ],
  "root_elements": 1,
  "target_namespace": ""
}

=== ALL TESTS PASSED ===
```

## Recommendation

**✅ Use PyO3 for XSD parsing**

### Pros

1. **Proven Solution**: Leverages Python's excellent `xmlschema` library (4.2.0)
2. **Full XSD Support**: Handles complex XSD features without custom implementation
3. **Maintained**: `xmlschema` is actively maintained and well-tested
4. **Fast Development**: No need to write/maintain XSD parser
5. **Comprehensive**: Full W3C XML Schema 1.0/1.1 support
6. **Type-Safe**: PyO3 provides type-safe Rust ↔ Python interop

### Cons

1. **Python Dependency**: Requires Python 3.7+ runtime at deployment
2. **Package Requirement**: Needs `pip install xmlschema`
3. **FFI Overhead**: Small performance cost for Rust↔Python calls
4. **Deployment Complexity**: Must ensure Python + packages available

### Deployment Options

#### Option A: System Python (Simplest)
```bash
# Users install Python and xmlschema
pip install xmlschema
cargo run --release
```

#### Option B: Bundled Python (Better UX)
- Use PyOxidizer to bundle Python + xmlschema into the binary
- Single binary distribution
- No user setup required

#### Option C: Docker (Most Reliable)
```dockerfile
FROM rust:latest
RUN apt-get update && apt-get install -y python3-pip
RUN pip3 install xmlschema
COPY . .
RUN cargo build --release
```

## Implementation Plan

### Phase 1: Core Functionality ✅

- [x] PyO3 integration
- [x] xmlschema import verification
- [x] Basic XSD parsing
- [x] JSON extraction

### Phase 2: XSD → TerminusDB Mapping

1. **Extract Full Schema Structure**
   - Elements with types
   - Complex types
   - Attributes
   - Inheritance/extension
   - Groups
   - Restrictions

2. **Design TerminusDB Mapping**
   ```
   XSD complexType → TerminusDB Class
   XSD element      → TerminusDB Property
   XSD simpleType   → TerminusDB DataType
   XSD extension    → TerminusDB Inheritance
   ```

3. **Generate TerminusDB Schema**
   - Output as JSON schema
   - Or generate Rust code with terminusdb-schema derive macros

### Phase 3: DITA Schema Testing

1. Get DITA XSD schemas
2. Parse with xmlschema
3. Generate TerminusDB models
4. Validate against real DITA documents

### Phase 4: Production Features

- Error handling
- Schema validation
- Namespace handling
- Include/import resolution
- Documentation generation

## Alternative Approaches (For Reference)

These are still viable if PyO3 proves problematic:

### 1. Pure Rust XSD Parser

**Pros:** No dependencies, full control
**Cons:** Significant development effort, XSD is complex

Libraries to consider:
- `quick-xml`
- `roxmltree`
- `xml-rs`

### 2. Java-based Parser (via JNI)

**Pros:** Mature XSD parsers (Apache Xerces)
**Cons:** JVM dependency, JNI complexity

### 3. XSLT Transformation

**Pros:** Declarative mapping
**Cons:** XSLT complexity, limited transformation capability

## Next Steps

1. ✅ Verify PyO3 + xmlschema works (DONE!)
2. Extract comprehensive schema information
3. Design XSD → TerminusDB type mapping
4. Implement mapping logic
5. Test with DITA schemas
6. Add error handling and edge cases
7. Create deployment strategy

## Dependencies

**Python Package:**
```bash
pip install xmlschema==4.2.0
```

**Rust Dependencies:**
```toml
[dependencies]
pyo3 = { version = "0.23", features = ["auto-initialize"] }
serde_json = "1"
```

## Conclusion

**PyO3 + xmlschema is the pragmatic choice** for the `terminusdb-xsd` crate:

- ✅ Works out of the box
- ✅ Full XSD support
- ✅ Battle-tested Python library
- ✅ Faster time to market
- ✅ Lower maintenance burden

The Python dependency is acceptable given the deployment context (likely server/Docker) and the significant development time saved.

---

## References

- [PyO3 Documentation](https://pyo3.rs/)
- [xmlschema Package](https://github.com/sissaschool/xmlschema)
- [W3C XML Schema Spec](https://www.w3.org/TR/xmlschema11-1/)
- [TerminusDB Schema](https://github.com/terminusdb/terminusdb-rs)

**Sources:**
- [GitHub - PyO3/pyo3: Rust bindings for the Python interpreter](https://github.com/PyO3/pyo3)
- [GitHub - RustPython/RustPython: A Python Interpreter written in Rust](https://github.com/RustPython/RustPython)
- [Introduction - PyO3 user guide](https://pyo3.rs/)
