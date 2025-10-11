# TerminusDB with Changeset SSE Plugin

This Docker setup provides a TerminusDB instance pre-configured with the `changeset-sse` plugin.

## Overview

The changeset-sse plugin enables Server-Sent Events (SSE) streaming of database changesets, allowing clients to receive real-time notifications when documents are added, updated, or deleted.

## Architecture

The setup uses a minimal entrypoint script that defers plugin loading until after store initialization:

1. The plugin is bundled in the Docker image at `/opt/terminusdb-plugins/changeset-sse.pl`
2. The entrypoint script runs standard TerminusDB store initialization WITHOUT plugins loaded
3. After initialization, `TERMINUSDB_PLUGINS_PATH` is set to `/opt/terminusdb-plugins`
4. The server starts and loads plugins at that point

This approach ensures:
- Plugins are NOT loaded during store initialization (which can fail if the store doesn't exist yet)
- Uses TerminusDB's built-in `TERMINUSDB_PLUGINS_PATH` mechanism for plugin loading
- Minimal custom code - just defers when the env var is set
- Storage volumes can be mounted independently

## Quick Start

### Using Docker Compose

```bash
# Build and start the service
cd docker/changeset-sse
docker compose up -d

# View logs
docker compose logs -f

# Stop the service
docker compose down

# Stop and remove volumes (full reset)
docker compose down -v
```

### Using Docker Build

```bash
# Build the image (from repository root)
docker build -f docker/changeset-sse/Dockerfile -t terminusdb-changeset-sse .

# Run the container
docker run -d \
  --name terminusdb-changeset-sse \
  -p 6363:6363 \
  -e TERMINUSDB_ADMIN_PASS=root \
  -v terminusdb_storage:/app/terminusdb/storage \
  terminusdb-changeset-sse
```

## Configuration

### Environment Variables

- `TERMINUSDB_SERVER_PORT`: Server port (default: 6363)
- `TERMINUSDB_ADMIN_PASS`: Admin password (default: root)
- `TERMINUSDB_ADMIN_PASS_FILE`: Path to file containing admin password (alternative to `TERMINUSDB_ADMIN_PASS`)
- `TERMINUSDB_PLUGINS_PATH`: Plugin directory (set to `/opt/terminusdb-plugins` by entrypoint before serving)

### Volumes

- `/app/terminusdb/storage`: TerminusDB data and plugins directory

## Plugin Functionality

The changeset-sse plugin provides:

### SSE Endpoint

- **URL**: `http://localhost:6363/api/changesets/stream`
- **Method**: GET
- **Authentication**: Requires valid TerminusDB authentication
- **Content-Type**: `text/event-stream`

### Event Format

```javascript
event: changeset
data: {
  "resource": "org/db/repo/branch/name",
  "branch": "main",
  "commit": {
    "id": "abc123...",
    "author": "user@example.com",
    "message": "Update documents",
    "timestamp": 1234567890
  },
  "metadata": {
    "inserts_count": 10,
    "deletes_count": 5,
    "documents_added": 3,
    "documents_deleted": 1,
    "documents_updated": 2
  },
  "changes": [
    {"id": "Document/123", "action": "added"},
    {"id": "Document/456", "action": "updated"}
  ]
}
```

### Security

- Only users with read access to a database receive changeset events for that database
- Events are filtered based on user permissions
- Unauthorized access attempts are logged

## Testing

Connect to the SSE stream using curl:

```bash
curl -N -H "Authorization: Basic cm9vdDpyb290" \
  http://localhost:6363/api/changesets/stream
```

Or using JavaScript:

```javascript
const eventSource = new EventSource('http://localhost:6363/api/changesets/stream', {
  withCredentials: true
});

eventSource.addEventListener('changeset', (event) => {
  const data = JSON.parse(event.data);
  console.log('Changeset received:', data);
});
```

## Troubleshooting

### Plugin Not Loading

Check if the plugin is present in the image:

```bash
docker exec terminusdb-changeset-sse ls -la /opt/terminusdb-plugins/
```

### View Server Logs

```bash
docker compose logs -f terminusdb
```

### Check Plugin Initialization

Look for SSE plugin log messages in the container output:
- `SSE Plugin: Client connecting`
- `SSE Plugin: Broadcast changeset ...`

## Files

- `Dockerfile`: Bundles the plugin and custom entrypoint
- `entrypoint.sh`: Minimal script that sets `TERMINUSDB_PLUGINS_PATH` only before serving
- `docker-compose.yml`: Orchestration for easy deployment
- `../../plugins/changeset-sse.pl`: The actual plugin source code
- `.dockerignore`: Optimizes build by only including necessary files

## Notes

- The plugin is bundled in the Docker image at `/opt/terminusdb-plugins/changeset-sse.pl`
- The entrypoint script sets `TERMINUSDB_PLUGINS_PATH=/opt/terminusdb-plugins` only right before starting the server
- This ensures plugins are loaded after store initialization completes
- Storage volumes can be mounted at `/app/terminusdb/storage` independently of the plugin
