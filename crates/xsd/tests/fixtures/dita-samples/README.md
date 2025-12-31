# DITA Sample Content Fixtures

This directory contains DITA sample content from open source projects, used for
testing the XSD-to-TerminusDB conversion pipeline with real-world DITA documents.

## Sources and Licenses

### gnostyx-demo/

**Source:** [gnostyx/dita-demo-content-collection](https://github.com/gnostyx/dita-demo-content-collection)

**License:** Apache License 2.0

A demonstration content collection featuring a fictional software product called
StormCluster with its MobileView user interface. Contains realistic technical
documentation including user guides, integration guides, and proposal templates.

### dita-test-cases/

**Source:** [dita-community/dita-test-cases](https://github.com/dita-community/dita-test-cases)

**License:** Apache License 2.0

Informal repository of DITA test case documents covering various DITA features
including branch filtering, content references, key spaces, glossaries, and more.
Each test case is designed to be minimal and self-describing.

### dita-ot-docs/

**Source:** [dita-ot/docs](https://github.com/dita-ot/docs)

**License:** Apache License 2.0

The documentation source files for the DITA Open Toolkit project. Contains
comprehensive DITA documentation covering topics, reference materials, and
release notes.

## Usage

These fixtures are used by integration tests in `terminusdb-xsd` to verify that:

1. Real-world DITA XML documents can be parsed against DITA XSD schemas
2. The parsed content can be converted to TerminusDB instances
3. The instances can be successfully inserted into a TerminusDB database

## File Counts

| Directory | DITA Files |
|-----------|------------|
| gnostyx-demo | 258 |
| dita-test-cases | 737 |
| dita-ot-docs | 263 |
| **Total** | **1,258** |
