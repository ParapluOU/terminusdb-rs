# TerminusDB MCP Server

An MCP (Model Context Protocol) server that enables AI assistants to query TerminusDB using WOQL DSL.

## Features

- Execute WOQL queries using DSL syntax or JSON-LD format
- Persistent connection management with the `connect` command
- Configure TerminusDB connection parameters via environment variables or .env files
- List databases and schemas
- Check server status
- Reset databases
- Support for time-travel queries with commit references

## Usage

Start the server:
```bash
terminusdb-mcp
```

The server exposes the following tools via MCP:

### connect
Establish and save a connection to TerminusDB. Once connected, other commands will use these saved credentials automatically.

Parameters:
- `host`: TerminusDB server URL (default: "http://localhost:6363" or $TERMINUSDB_HOST)
- `user`: Username (default: "admin" or $TERMINUSDB_USER)
- `password`: Password (default: "root" or $TERMINUSDB_PASSWORD)
- `database`: Optional database name
- `branch`: Branch name (default: "main")
- `commit_ref`: Optional commit ID for time-travel queries
- `env_file`: Optional path to .env file to load additional environment variables

Example:
```json
{
  "tool": "connect",
  "arguments": {
    "host": "https://cloud.terminusdb.com",
    "user": "myuser",
    "password": "mypassword",
    "database": "mydb",
    "env_file": "/path/to/.env"
  }
}
```

### execute_woql
Execute a WOQL query using DSL syntax or JSON-LD format.

Parameters:
- `query`: WOQL query string (DSL or JSON-LD format)
- `connection`: Optional connection configuration (overrides saved connection)
  - `host`: TerminusDB server URL
  - `user`: Username
  - `password`: Password
  - `database`: Database name
  - `branch`: Branch name
  - `commit_ref`: Optional commit ID for time-travel queries

Example with DSL:
```json
{
  "tool": "execute_woql",
  "arguments": {
    "query": "select([$Name], triple($Person, \"@schema:name\", $Name))"
  }
}
```

Example with JSON-LD:
```json
{
  "tool": "execute_woql",
  "arguments": {
    "query": "{\"@type\": \"Select\", \"variables\": [\"Name\"], \"query\": {\"@type\": \"Triple\", \"subject\": {\"@type\": \"NodeValue\", \"variable\": \"Person\"}, \"predicate\": {\"@type\": \"NodeValue\", \"node\": \"@schema:name\"}, \"object\": {\"@type\": \"Value\", \"variable\": \"Name\"}}}"
  }
}
```

Note: If no connection is provided and no connection has been saved with the `connect` command, the tool will use default values or environment variables.

### list_databases
List all available databases.

Parameters:
- `connection`: Optional connection configuration (overrides saved connection)

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

Parameters:
- `database`: Database name (required)
- `connection`: Optional connection configuration (overrides saved connection)

Returns WOQL query results containing schema information.

Example:
```json
{
  "tool": "get_schema",
  "arguments": {
    "database": "mydb"
  }
}
```

### check_server_status
Check if the TerminusDB server is running and accessible.

Parameters:
- `connection`: Optional connection configuration (overrides saved connection)

Returns:
- `status`: "running", "offline", or "error"
- `connected`: Boolean indicating connection status
- `server_info`: Server information (when available)
- `error`: Error message (when applicable)

Example:
```json
{
  "tool": "check_server_status",
  "arguments": {}
}
```

### reset_database
Reset a database by deleting and recreating it. WARNING: This permanently deletes all data in the database!

Parameters:
- `database`: Database name to reset (required)
- `connection`: Optional connection configuration (overrides saved connection)

Returns:
- `status`: "success" or error
- `message`: Confirmation message
- `database`: Database name that was reset

Example:
```json
{
  "tool": "reset_database",
  "arguments": {
    "database": "test-db"
  }
}
```

## Configuration

The server can be configured in multiple ways:

### 1. Environment Variables
Set these environment variables before starting the server:
- `TERMINUSDB_HOST`: Default server URL (default: "http://localhost:6363")
- `TERMINUSDB_USER`: Default username (default: "admin")
- `TERMINUSDB_PASSWORD`: Default password (default: "root")
- `TERMINUSDB_DATABASE`: Default database name (optional)
- `TERMINUSDB_BRANCH`: Default branch name (default: "main")
- `TERMINUSDB_COMMIT_REF`: Default commit reference for time-travel queries (optional)

### 2. Using .env Files
You can load environment variables from a .env file using the `connect` command:
```json
{
  "tool": "connect",
  "arguments": {
    "env_file": "/path/to/.env"
  }
}
```

### 3. Connection Precedence
The server uses the following precedence for connection parameters:
1. Parameters provided directly to a tool
2. Saved connection from the `connect` command
3. Environment variables
4. Default values

## Time-Travel Queries

You can query historical states of your database by providing a `commit_ref` in the connection configuration:

```json
{
  "tool": "execute_woql",
  "arguments": {
    "query": "select([$Name], triple($Person, \"@schema:name\", $Name))",
    "connection": {
      "database": "mydb",
      "commit_ref": "abc123def456"
    }
  }
}
```