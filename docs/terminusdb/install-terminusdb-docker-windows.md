---
tags:
  - how-to
  - installation
  - curl
  - self-hosted
title: Install TerminusDB with Docker on Windows
nextjs:
  metadata:
    title: Install TerminusDB with Docker on Windows
    description: Complete guide to installing and running TerminusDB with Docker on Windows, including backup and restore instructions
    keywords: terminusdb, backup, docker, getting started, install, install terminusdb with docker on windows, setup
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/install-terminusdb-docker-windows/
media: []
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have TerminusDB running in Docker on Windows.
{% /callout %}

This guide provides step-by-step instructions for running TerminusDB on Windows using Docker, including how to set up persistent storage and create backups.

## Prerequisites

Before you begin, ensure you have:

- Docker Desktop for Windows installed and running
- Basic familiarity with Windows Command Prompt or PowerShell
- At least 4GB of RAM allocated to Docker (recommended)

> **Important:** On Windows, the default memory allocated for Docker is 2GB. Since TerminusDB is an in-memory database, it's recommended to increase this allocation in Docker Desktop settings to at least 4GB for optimal performance.

## Step 1: Set Up TerminusDB with Docker Volume

In this section, we'll create a directory structure, configure Docker Compose, and set up persistent storage for TerminusDB.

### Create the Project Directory

Create a directory to keep your TerminusDB-related files:

```bash
mkdir terminusdbLocalhost
cd terminusdbLocalhost
```

### Create docker-compose.yml

Create a file called `docker-compose.yml` in the `terminusdbLocalhost` directory:

```bash
echo > docker-compose.yml
```

Open the file with Notepad and add the following contents:

```yaml
services:
  terminusdb-server:
    image: terminusdb/terminusdb-server:latest
    restart: unless-stopped
    ports:
      - "127.0.0.1:6363:6363"
    environment:
      - "TERMINUSDB_SERVER_PORT=6363"
      - "TERMINUSDB_ADMIN_PASS=$TERMINUSDB_ADMIN_PASS"
    volumes:
      - terminusdb_volume:/app/terminusdb/storage
volumes:
  terminusdb_volume:
    external: true
```

> **Important:** Make sure the file has a `.yml` extension only, not `.yml.txt`, as it will not work otherwise. When running `docker compose` commands, you need to be located in the `terminusdbLocalhost` directory.

This configuration tells Docker to:
- Create a container from the latest TerminusDB image
- Attach it to network port 6363 on localhost
- Start automatically when Docker starts (unless manually stopped)
- Read the admin password from an environment variable
- Store data in a persistent Docker volume

### Create .env File for Password

Create a `.env` file in the same directory to store your admin password:

```bash
TERMINUSDB_ADMIN_PASS=your_secure_password_here
```

Replace `your_secure_password_here` with your desired password.

> **Security Note:** If you use Git to version your files, create a `.gitignore` file immediately and add `.env` to it to prevent accidentally committing your password:

```bash
# .gitignore
.env
```

### Create the Docker Volume

Create a persistent data storage volume for TerminusDB:

```bash
docker volume create terminusdb_volume
```

You should see the output:
```
terminusdb_volume
```

Verify the volume was created:

```bash
docker volume ls
```

You should see:
```
DRIVER    VOLUME NAME
local     terminusdb_volume
```

### Start TerminusDB

Start TerminusDB using Docker Compose:

```bash
docker compose up -d
```

Wait for TerminusDB to start, then open your browser and navigate to:

```
http://localhost:6363/
```

Login with:
- Username: `admin`
- Password: The password you set in the `.env` file

### Verify Persistent Storage

To verify that your data is stored in the persistent volume, stop and restart the container:

```bash
docker compose down
docker compose up -d
```

Your data should persist across container restarts.

## Step 2: Backup and Restore TerminusDB

When running TerminusDB on localhost, it's important to keep backup copies of your data to protect against failures.

> **Important:** The backup and restore instructions assume TerminusDB is offline when copying for consistency. Always verify that backup procedures work for your use case before relying on them.

### View Volume Contents

You can inspect the contents of your TerminusDB volume using an Ubuntu container:

```bash
docker run --rm -it -v terminusdb_volume:/terminusdb ubuntu ls -l ./terminusdb/db
```

The `--rm` flag ensures the temporary Ubuntu container is deleted after the command completes.

### Create a Compressed Backup

To create a backup, stop TerminusDB, create a compressed archive, and restart:

**For Command Prompt or PowerShell:**

```bash
docker compose stop terminusdb-server
docker run --rm -it -v "%CD%":/backup -v terminusdb_volume:/terminusdb ubuntu tar cfz /backup/backup.tar.gz /terminusdb
docker compose up -d terminusdb-server
```

**For Git Bash:**

```bash
docker compose stop terminusdb-server
MSYS_NO_PATHCONV=1 docker run --rm -it -v "$PWD":/backup -v terminusdb_volume:/terminusdb ubuntu tar cfz /backup/backup.tar.gz /terminusdb
docker compose up -d terminusdb-server
```

This creates a `backup.tar.gz` file in your current directory.

### Restore to a Separate Instance

To restore your backup to a new TerminusDB instance running on a different port:

1. Update your `docker-compose.yml` to add a second service:

```yaml
services:
  terminusdb-server:
    image: terminusdb/terminusdb-server:latest
    restart: unless-stopped
    ports:
      - "127.0.0.1:6363:6363"
    environment:
      - "TERMINUSDB_SERVER_PORT=6363"
      - "TERMINUSDB_ADMIN_PASS=$TERMINUSDB_ADMIN_PASS"
    volumes:
      - terminusdb_volume:/app/terminusdb/storage
  terminusdb-restored:
    image: terminusdb/terminusdb-server:latest
    restart: unless-stopped
    ports:
      - "127.0.0.1:6364:6363"
    environment:
      - "TERMINUSDB_SERVER_PORT=6363"
      - "TERMINUSDB_ADMIN_PASS=$TERMINUSDB_ADMIN_PASS"
    volumes:
      - terminusdb_restored:/app/terminusdb/storage
volumes:
  terminusdb_volume:
    external: true
  terminusdb_restored:
    external: true
```

2. Create the restore volume and restore the backup:

**For Command Prompt or PowerShell:**

```bash
docker volume create terminusdb_restored
docker run --rm -it -v "%CD%":/restore -v terminusdb_restored:/terminusdb ubuntu tar xvfz /restore/backup.tar.gz
docker run --rm -it -v terminusdb_restored:/terminusdb ubuntu ls -l ./terminusdb/db
docker compose up -d terminusdb-restored
```

**For Git Bash:**

```bash
docker volume create terminusdb_restored
MSYS_NO_PATHCONV=1 docker run --rm -it -v "$PWD":/restore -v terminusdb_restored:/terminusdb ubuntu tar xvfz /restore/backup.tar.gz
MSYS_NO_PATHCONV=1 docker run --rm -it -v terminusdb_restored:/terminusdb ubuntu ls -l ./terminusdb/db
docker compose up -d terminusdb-restored
```

The restored instance will be available at:
```
http://localhost:6364/
```

## Git Bash Considerations

When using Git Bash to run Docker on Windows, path conversion can cause issues. The `MSYS_NO_PATHCONV=1` prefix prevents automatic path conversion for Docker commands.

If you encounter path-related errors, ensure you're using the correct command format for your shell environment.

## Next Steps

Now that you have TerminusDB running on Windows:

- [Connect with the JavaScript Client](/docs/connect-with-the-javascript-client)
- [Connect with the Python Client](/docs/connect-with-python-client)
- [Learn about Documents & Schema](/docs/documents-explanation)
- [Explore GraphQL Queries](/docs/how-to-query-with-graphql)

## Additional Resources

- [General Docker Installation Guide](/docs/install-terminusdb-as-a-docker-container)
- [TerminusDB GitHub Repository](https://github.com/terminusdb/terminusdb)
- [DFRNT Cloud Service](https://dfrnt.com/sign-up)
