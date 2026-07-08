---
tags:
  - explanation
  - dashboard
  - dfrnt-cloud
title: TerminusDB Dashboard Status
nextjs:
  metadata:
    title: TerminusDB Dashboard Status
    description: Information about the TerminusDB Dashboard component, its discontinuation, and how to continue using it if needed.
    keywords: terminusdb, dashboard, dfrnt hub, terminuscms, terminusdb dashboard status
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/dashboard/
media: []
---

## Dashboard Discontinuation Notice

As of **TerminusDB v11.2**, the built-in dashboard component has been **discontinued** and is no longer included with the core TerminusDB server distribution.

The dashboard mounting feature still works perfectly and can be used for mounting either the discontinued dashboard component, or other custom react-style dashboards (see file structure further down).

### Why Was the Dashboard Discontinued?

The decision to discontinue the dashboard was made to:

- **Inconsistent User Interface**: Features working correctly in the core database did not work properly in the user interface, leaving a bad impression. With significantly better alternatives now on the market (such as DFRNT), we recommend to use them instead of the built in dashboard.
- **Focus on Core Database Features**: Concentrate development efforts on the database engine, query languages (WOQL, Document API, GraphQL), and performance improvements
- **Embrace Modern Development Practices**: Encourage users to build custom interfaces using TerminusDB's powerful APIs rather than maintaining a one-size-fits-all UI
- **Reduce Maintenance Burden**: The dashboard required significant maintenance effort that could be better spent on core database functionality
- **Better Client Library Support**: Improved JavaScript and Python clients now make it easier to build custom interfaces

### Recommended Alternatives

Instead of the built-in dashboard, we recommend:

1. **Use TerminusDB Client Libraries**
   - [JavaScript Client](/docs/use-the-javascript-client/) - Build custom web interfaces with React, Vue, or vanilla JS
   - [Python Client](/docs/use-the-python-client/) - Create data management scripts and notebooks
   
2. **GraphQL Playground Tools**
   - Use [GraphQL Playground](https://github.com/graphql/graphql-playground) or [GraphiQL](https://github.com/graphql/graphiql)
   - Connect directly to TerminusDB's GraphQL endpoint
   
3. **Command Line Interface**
   - Use the [TerminusDB CLI](/docs/terminusdb-cli-commands/) for database management
   - Perfect for automation and scripting

4. **DFRNT Cloud**
   - For a fully-featured cloud dashboard experience, consider [DFRNT Cloud](https://studio.dfrnt.com)
   - Includes team management, full localhost modelling experience with self-sovereign data, visual schema modeling, and data curation

---

## Continue Using the Dashboard (Legacy Support)

If you still need to use the legacy dashboard component, you can manually download and mount the HTML release to your TerminusDB instance.

{% callout type="warning" title="Legacy Component" %}
The dashboard is no longer maintained or supported. It does not receive security updates, bug fixes, or feature additions. Use at your own risk.
{% /callout %}

### Step 1: Download the Dashboard

Download the last stable release (v6.0.10) of the dashboard:

```bash
# Download the pre-built dashboard release
curl -L https://github.com/terminusdb/terminusdb-dashboard/releases/download/v6.0.10/release.tar.gz -o dashboard.tar.gz

# Create dashboard directory and extract the archive into it
mkdir -p dashboard
tar -xzf dashboard.tar.gz -C dashboard

# The dashboard directory now contains all the dashboard files
```

Alternatively, you can download it directly from your browser:
[https://github.com/terminusdb/terminusdb-dashboard/releases/download/v6.0.10/release.tar.gz](https://github.com/terminusdb/terminusdb-dashboard/releases/download/v6.0.10/release.tar.gz)

{% callout type="note" title="Pre-built Release" %}
This release is already built and ready to use. No need to run npm install or build steps.
{% /callout %}

### Step 2: Mount to Docker Container

If you're running TerminusDB in Docker, mount the dashboard directory:

#### Using Docker Run

{% callout type="note" title="Outdated version tag" %}
This example references `v11_2_rc2`, a pre-release tag. TerminusDB 12 is the current release. Use `terminusdb/terminusdb-server:v12` or `terminusdb/terminusdb-server:latest` for the current stable version.
{% /callout %}

```bash
docker run -it \
  --name terminusdb \
  -p 6363:6363 \
  -v ~/dashboard:/app/terminusdb/dashboard \
  -e TERMINUSDB_DASHBOARD_ENABLED=true \
  terminusdb/terminusdb-server:v12
```

#### Using Docker Compose

Create or update your `docker-compose.yml`:

```yaml
version: '3'
services:
  terminusdb:
    image: terminusdb/terminusdb-server:v12
    container_name: terminusdb
    ports:
      - "6363:6363"
    volumes:
      - terminusdb_storage:/app/terminusdb/storage
      - ./dashboard:/app/terminusdb/dashboard
    environment:
      - TERMINUSDB_DASHBOARD_ENABLED=true
      - TERMINUSDB_ADMIN_PASS=mypassword
volumes:
  terminusdb_storage:
```

Then start the container:

```bash
docker-compose up -d
```

### Step 3: Enable the Dashboard

The dashboard must be explicitly enabled via an environment variable:

| Environment Variable | Value | Description |
|---------------------|-------|-------------|
| `TERMINUSDB_DASHBOARD_ENABLED` | `true` | Enables the dashboard component |

Without this variable set to `true`, TerminusDB will serve neither the discontinuation notice, nor the dashboard.

### Step 4: Access the Dashboard

Once configured, access the dashboard at:

```text
http://localhost:6363/dashboard
```

The default credentials are:
- **Username**: `admin`
- **Password**: The value set in `TERMINUSDB_ADMIN_PASS` (default: `root`)

---

## Directory Structure

When mounting the dashboard, use the structure from the release.tar.gz file, and the assets are by default in the right location with no additional directory hierarchy needed.

```text
/app/terminusdb/dashboard/
├── index.html          # Main dashboard entry point
└── assets/             # CSS, JS, and other assets
    ├── css/
    ├── js/
    └── images/
```

---

## Troubleshooting

### Dashboard Shows "Discontinued" Message

**Cause**: The environment variable is not set correctly.

**Solution**: Ensure `TERMINUSDB_DASHBOARD_ENABLED=true` is set in your Docker environment.

### 404 or File Not Found Errors

**Cause**: Dashboard files are not mounted correctly.

**Solution**: 
1. Verify the dashboard files exist: `ls dashboard/index.html`
2. Check the volume mount path in Docker
3. Ensure the container can read the mounted directory

### Assets Not Loading (CSS/JS)

**Cause**: Asset paths are incorrect or files are missing.

**Solution**:
1. Verify the `assets` directory exists: `ls dashboard/assets`
2. Check browser console for specific file errors
3. Ensure the dashboard was extracted correctly from the release

### Cannot Connect to TerminusDB

**Cause**: Server is not running or port is not exposed.

**Solution**:
1. Check TerminusDB is running: `docker ps`
2. Verify port 6363 is mapped: `docker port terminusdb`
3. Check firewall settings

---

## Migration Path

For users migrating away from the dashboard, here's a typical workflow transition:

### Before (Using Dashboard)

1. Log in to dashboard at `http://localhost:6363/dashboard`
2. Click through UI to create databases
3. Use visual schema builder
4. Use document editor for data entry

### After (Using Client Libraries)

1. **Database Management** - Use CLI or client libraries:
   ```bash
   # CLI approach
   terminusdb db create admin/tdb-example-mydb
   
   # Python approach
   from terminusdb_client import Client
   client = Client("http://localhost:6363")
   client.connect(user="admin", key="root")  # key= replaced password= in TerminusDB Python client v1.1+
   client.create_database("tdb-example-mydb")
   ```

2. **Schema Definition** - Define schema in code:
   ```python
   # Python schema definition
   from terminusdb_client.woqlschema import WOQLSchema, DocumentTemplate
   
   schema = WOQLSchema()
   
   class Person(DocumentTemplate):
       _schema = schema
       name: str
       age: int
   
   client.insert_document(schema.to_dict())
   ```

3. **Data Operations** - Use GraphQL or Document API:
   ```python
   # Insert documents
   client.insert_document({
       "@type": "Person",
       "name": "Alice",
       "age": 30
   })
   
   # Query with GraphQL
   result = client.query_document({
       "type": "Person",
       "query": {"age": {"@gt": 25}}
   })
   ```

---

## Additional Resources

- [JavaScript Client Documentation](/docs/use-the-javascript-client/)
- [Python Client Documentation](/docs/use-the-python-client/)
- [GraphQL Query Guide](/docs/graphql-basics/)
- [WOQL Query Guide](/docs/woql-basics/)
- [TerminusDB CLI Reference](/docs/terminusdb-cli-commands/)
- [Document API Reference](/docs/document-graph-api/)

For questions or support, join our community:
- [Discord Community](https://discord.gg/terminusdb)
- [GitHub Discussions](https://github.com/terminusdb/terminusdb/discussions)

---

{% callout type="note" title="TerminusDB Development" %}
TerminusDB development focuses on the core database engine — query performance, version control, and collaboration. See the [changelog](https://github.com/terminusdb/terminusdb/releases) for current capabilities.
{% /callout %}
