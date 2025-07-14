# TerminusDB MCP Server

An MCP (Model Context Protocol) server that enables AI assistants to query TerminusDB using WOQL DSL.

## Features

- Execute WOQL queries using DSL syntax
- Configure TerminusDB connection parameters
- List databases and schemas
- Perform document CRUD operations

## Usage

Start the server:
```bash
terminusdb-mcp
```

The server exposes the following tools via MCP:

### execute_woql
Execute a WOQL query using DSL syntax.

Parameters:
- `query`: WOQL DSL query string
- `connection`: Optional connection configuration
  - `host`: TerminusDB server URL (default: "http://localhost:6363")
  - `user`: Username (default: "admin")
  - `password`: Password (default: "root")
  - `database`: Database name
  - `branch`: Branch name (default: "main")

Example:
```json
{
  "tool": "execute_woql",
  "arguments": {
    "query": "select([$Name], triple($Person, \"@schema:name\", $Name))",
    "connection": {
      "database": "mydb"
    }
  }
}
```

### list_databases
List all available databases.

Parameters:
- `connection`: Optional connection configuration (same as execute_woql)

Returns a JSON object containing:
- `databases`: Array of database objects with:
  - `path`: Full database path (e.g., "admin/mydb")
  - `name`: Database name
  - `organization`: Organization name
  - `id`: Database ID (when available)
  - `type`: Database type (when available)
  - `state`: Database state (when available)
- `count`: Total number of databases

Example:
```json
{
  "tool": "list_databases",
  "arguments": {}
}
```

Response example:
```json
{
  "databases": [
    {
      "path": "admin/test",
      "name": "test",
      "organization": "admin",
      "id": "UserDatabase/abc123",
      "type": "UserDatabase",
      "state": "finalized"
    }
  ],
  "count": 1
}
```

### get_schema
Get the schema for a specific database.

## Configuration

The server can be configured via environment variables:
- `TERMINUSDB_HOST`: Default server URL
- `TERMINUSDB_USER`: Default username
- `TERMINUSDB_PASSWORD`: Default password