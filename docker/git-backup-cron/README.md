# TerminusDB Git Backup with Cron

This Docker image provides automated backups of TerminusDB databases to a Git repository. It combines:

- **TerminusDB Server** (v11.2.0-rc5)
- **Cron** for scheduled backups
- **git-sync-rs** for automatic git synchronization

## Features

- Scheduled RDF triple dumps using cron
- Automatic git commit and push via [git-sync-rs](https://github.com/colonelpanic8/git-sync-rs)
- File watching with debouncing for efficient syncing
- Git deduplication for efficient storage (same files overwritten)
- Support for multiple databases
- Git credential helper for secure authentication
- Docker secrets support for sensitive credentials

## Quick Start

### Using Docker Compose

```yaml
services:
  terminusdb-backup:
    build:
      context: ../..
      dockerfile: docker/git-backup-cron/Dockerfile
    environment:
      # TerminusDB Configuration
      TERMINUSDB_ADMIN_PASS: root
      TERMINUSDB_SERVER_PORT: 6363

      # Git Repository Configuration
      GIT_REPO_URL: https://github.com/your-org/terminusdb-backups.git
      GIT_REPO_USER: your-username
      GIT_REPO_PASSWORD: your-token
      GIT_REPO_EMAIL: backup@example.com

      # Backup Configuration
      BACKUP_DATABASES: admin/database1,admin/database2
      BACKUP_CRON_SCHEDULE: "0 2 * * *"  # 2 AM daily

      # Git Sync Configuration (optional)
      GIT_SYNC_INTERVAL: 60           # Periodic sync interval (seconds)
      GIT_SYNC_DEBOUNCE: 2            # Debounce file changes (seconds)
      GIT_SYNC_MIN_INTERVAL: 5        # Minimum time between syncs (seconds)
      GIT_SYNC_NEW_FILES: "true"      # Include untracked files
      GIT_SYNC_REMOTE: origin         # Git remote name
      GIT_SYNC_COMMIT_MESSAGE: "TerminusDB backup sync from {hostname} at {timestamp}"

    volumes:
      - terminusdb-storage:/app/terminusdb/storage
      - git-backup-repo:/backup/repo
    ports:
      - "6363:6363"

volumes:
  terminusdb-storage:
  git-backup-repo:
```

### Using Docker Secrets (Recommended for Production)

```yaml
services:
  terminusdb-backup:
    build:
      context: ../..
      dockerfile: docker/git-backup-cron/Dockerfile
    environment:
      GIT_REPO_URL: https://github.com/your-org/terminusdb-backups.git
      GIT_REPO_USER: your-username
      GIT_REPO_EMAIL: backup@example.com
      BACKUP_DATABASES: admin/database1,admin/database2
      BACKUP_CRON_SCHEDULE: "0 2 * * *"
    secrets:
      - terminusdb_admin_pass
      - git_repo_password
    volumes:
      - terminusdb-storage:/app/terminusdb/storage
      - git-backup-repo:/backup/repo
    ports:
      - "6363:6363"

secrets:
  terminusdb_admin_pass:
    file: ./secrets/terminusdb_admin_pass.txt
  git_repo_password:
    file: ./secrets/git_repo_password.txt

volumes:
  terminusdb-storage:
  git-backup-repo:
```

## Environment Variables

### Required

| Variable | Description | Example |
|----------|-------------|---------|
| `GIT_REPO_URL` | Git repository URL for backups | `https://github.com/user/backups.git` |
| `GIT_REPO_USER` | Git username | `github-user` |
| `GIT_REPO_PASSWORD` | Git password or token | `ghp_xxxxx` |
| `BACKUP_DATABASES` | Comma-separated list of databases | `admin/db1,admin/db2` |

### Optional

| Variable | Default | Description |
|----------|---------|-------------|
| `TERMINUSDB_ADMIN_PASS` | `root` | TerminusDB admin password |
| `TERMINUSDB_SERVER_PORT` | `6363` | TerminusDB server port |
| `GIT_REPO_EMAIL` | `backup@terminusdb.local` | Git commit email |
| `GIT_REPO_DIR` | `/backup/repo` | Directory for git repository |
| `BACKUP_CRON_SCHEDULE` | `0 2 * * *` | Cron schedule (2 AM daily) |
| `GIT_SYNC_INTERVAL` | `60` | Periodic sync interval (seconds) |
| `GIT_SYNC_DEBOUNCE` | `2` | Debounce file changes (seconds) |
| `GIT_SYNC_MIN_INTERVAL` | `5` | Minimum time between syncs (seconds) |
| `GIT_SYNC_NEW_FILES` | `true` | Include untracked files in sync |
| `GIT_SYNC_REMOTE` | `origin` | Git remote name |
| `GIT_SYNC_COMMIT_MESSAGE` | Auto-generated | Custom commit message template |

### Docker Secrets

Instead of environment variables, you can use Docker secrets:

- `TERMINUSDB_ADMIN_PASS_FILE` - Path to file containing admin password
- `GIT_REPO_PASSWORD_FILE` - Path to file containing git password/token

## Cron Schedule Format

The `BACKUP_CRON_SCHEDULE` uses standard cron syntax:

```
┌───────────── minute (0 - 59)
│ ┌───────────── hour (0 - 23)
│ │ ┌───────────── day of month (1 - 31)
│ │ │ ┌───────────── month (1 - 12)
│ │ │ │ ┌───────────── day of week (0 - 6) (Sunday to Saturday)
│ │ │ │ │
* * * * *
```

Examples:
- `0 2 * * *` - Every day at 2:00 AM
- `0 */6 * * *` - Every 6 hours
- `0 0 * * 0` - Every Sunday at midnight
- `*/30 * * * *` - Every 30 minutes

## How It Works

1. **Container Startup**:
   - Initializes TerminusDB store (if needed)
   - Configures git credentials using credential helper
   - Auto-clones git repository (or pulls if already exists)
   - Handles empty repositories by creating initial commit
   - Sets up cron job for scheduled backups
   - Starts git-sync-rs in watch mode
   - Starts cron for scheduled tasks
   - Starts TerminusDB server

2. **Backup Process** (triggered by cron):
   - Dumps RDF triples for each configured database
   - Saves to `<org>/<db>/instance.ttl` and `<org>/<db>/schema.ttl`
   - Overwrites same files each time (git tracks changes for deduplication)

3. **Git Sync** (continuous, handled by git-sync-rs):
   - Watches the repository directory for file changes
   - Debounces changes to avoid excessive commits
   - Automatically commits changes with timestamped messages
   - Pushes to remote repository
   - Also performs periodic syncs at the configured interval

## Repository Structure

After backups run, your git repository will contain:

```
repo/
├── backup_metadata.txt
├── admin/
│   ├── database1/
│   │   ├── instance.ttl
│   │   └── schema.ttl
│   └── database2/
│       ├── instance.ttl
│       └── schema.ttl
└── org2/
    └── database3/
        ├── instance.ttl
        └── schema.ttl
```

## Volumes

**Important**: Use named volumes to ensure data persists and isn't embedded in the container filesystem:

- `terminusdb-storage` - TerminusDB database files
- `git-backup-repo` - Cloned git repository (can be large)

## Monitoring

View cron logs:
```bash
docker exec <container> tail -f /var/log/backup/cron.log
```

View git-sync-rs logs:
```bash
docker logs <container>
```

## GitHub Personal Access Token

For GitHub, create a Personal Access Token with `repo` scope:
1. Go to Settings → Developer settings → Personal access tokens
2. Generate new token with `repo` scope
3. Use token as `GIT_REPO_PASSWORD`

## Restoring from Backup

To restore triples from a backup:

```bash
# Load schema first
terminusdb triples load <org>/<db>/local/branch/main/schema < schema.ttl

# Then load instance data
terminusdb triples load <org>/<db>/local/branch/main/instance < instance.ttl
```

## Troubleshooting

### Backup not running

Check cron logs:
```bash
docker exec <container> cat /var/log/backup/cron.log
```

Verify cron is running:
```bash
docker exec <container> ps aux | grep cron
```

### Git push failing

- Verify `GIT_REPO_PASSWORD` has write permissions
- Check git-sync-rs logs: `docker logs <container>`
- Ensure repository exists and is accessible

### Large repository size

The git repository uses a volume, so it won't bloat the container. However, git may grow over time. Consider:
- Using git-sync interval wisely
- Periodic repository optimization
- Using `.gitignore` if needed

## Notes

- The git repository is stored in a **volume** to prevent container filesystem bloating
- Files are overwritten each backup - git provides versioning and deduplication
- Git repository is **automatically cloned** on first startup (no manual setup needed)
- Empty repositories are initialized with a README on first run
- git-sync-rs handles all ongoing git operations (commit, push, pull)
- The backup script only dumps triples to files - no git commands needed
- Git credentials are managed via credential helper (not embedded in URLs)
- git-sync-rs uses file watching with debouncing for efficient syncing

## git-sync-rs Configuration

git-sync-rs provides several tunable parameters for optimizing sync behavior:

- **GIT_SYNC_INTERVAL**: Periodic sync interval regardless of file changes
- **GIT_SYNC_DEBOUNCE**: Wait time after file changes before syncing (prevents rapid commits)
- **GIT_SYNC_MIN_INTERVAL**: Minimum time between syncs (rate limiting)
- **GIT_SYNC_NEW_FILES**: Whether to include untracked files in commits

These parameters allow you to balance between real-time syncing and resource efficiency.
