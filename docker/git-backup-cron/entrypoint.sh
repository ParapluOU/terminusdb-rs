#!/usr/bin/env bash
set -e

# ===================================================================
# TerminusDB Git Backup Entrypoint
# Based on: https://github.com/terminusdb/terminusdb/blob/main/distribution/init_docker.sh
# ===================================================================

TERMINUSDB_SERVER_PORT=${TERMINUSDB_SERVER_PORT:-6363}
BACKUP_CRON_SCHEDULE=${BACKUP_CRON_SCHEDULE:-"0 2 * * *"}  # Default: 2 AM daily
GIT_REPO_DIR=${GIT_REPO_DIR:-"/backup/repo"}

# Handle password from file or environment (from official init script)
file_env() {
	local var="$1"
	local fileVar="${var}_FILE"
	local def="${2:-}"
	if [ "${!var:-}" ] && [ "${!fileVar:-}" ]; then
		echo >&2 "error: both $var and $fileVar are set (but are exclusive)"
		exit 1
	fi
	local val="$def"
	if [ "${!var:-}" ]; then
		val="${!var}"
	elif [ "${!fileVar:-}" ]; then
		val="$(< "${!fileVar}")"
	fi
	export "$var"="$val"
	unset "$fileVar"
}

# Load passwords from files if provided
file_env 'TERMINUSDB_ADMIN_PASS'
file_env 'GIT_REPO_PASSWORD'

TERMINUSDB_ADMIN_PASS=${TERMINUSDB_ADMIN_PASS:-root}

# Validate required git environment variables
if [ -z "$GIT_REPO_URL" ]; then
    echo >&2 "error: GIT_REPO_URL is required"
    exit 1
fi

if [ -z "$GIT_REPO_USER" ]; then
    echo >&2 "error: GIT_REPO_USER is required"
    exit 1
fi

if [ -z "$GIT_REPO_PASSWORD" ]; then
    echo >&2 "error: GIT_REPO_PASSWORD is required (or GIT_REPO_PASSWORD_FILE)"
    exit 1
fi

# Initialize TerminusDB store if needed (from official script)
if [ ! -d /app/terminusdb/storage/db ]; then
    echo "Initializing TerminusDB store..."
    /app/terminusdb/terminusdb store init --key "$TERMINUSDB_ADMIN_PASS"
fi

# Configure git
echo "Configuring git credentials..."
git config --global user.name "${GIT_REPO_USER}"
git config --global user.email "${GIT_REPO_EMAIL:-backup@terminusdb.local}"

# Set up git credential helper for HTTPS authentication
# This creates a credential helper script that provides username/password
if [[ "$GIT_REPO_URL" =~ ^https?:// ]]; then
    echo "Setting up git credential helper for HTTPS..."

    # Create credential helper script
    cat > /usr/local/bin/git-credential-helper <<CREDHELPER
#!/bin/sh
echo "username=${GIT_REPO_USER}"
echo "password=${GIT_REPO_PASSWORD}"
CREDHELPER
    chmod +x /usr/local/bin/git-credential-helper

    # Configure git to use the credential helper
    git config --global credential.helper "/usr/local/bin/git-credential-helper"
fi

# Create the repo directory if it doesn't exist
mkdir -p "$GIT_REPO_DIR"

# Auto-clone or pull git repository
if [ ! -d "$GIT_REPO_DIR/.git" ]; then
    echo "Git repository not found. Cloning from $GIT_REPO_URL..."
    git clone "$GIT_REPO_URL" "$GIT_REPO_DIR" || {
        echo "Warning: Failed to clone repository. git-sync-rs will try to clone on first sync."
    }

    # If repository is empty (new repo), create initial commit
    if [ -d "$GIT_REPO_DIR/.git" ]; then
        cd "$GIT_REPO_DIR"
        if [ -z "$(git log --oneline 2>/dev/null)" ]; then
            echo "Empty repository detected. Creating initial commit..."
            echo "# TerminusDB Backup Repository" > README.md
            echo "" >> README.md
            echo "This repository contains automated backups from TerminusDB." >> README.md
            echo "Backups are organized by organization and database name." >> README.md
            git add README.md
            git commit -m "Initial commit: Setup TerminusDB backup repository"
            git push origin HEAD:master 2>/dev/null || git push origin HEAD:main 2>/dev/null || {
                echo "Warning: Failed to push initial commit. Will continue anyway."
            }
        fi
        cd -
    fi
else
    echo "Git repository exists. Pulling latest changes..."
    (cd "$GIT_REPO_DIR" && git pull) || {
        echo "Warning: Failed to pull latest changes. Will continue with existing repository."
    }
fi

# Export git-sync-rs environment variables
export GIT_SYNC_REPOSITORY="$GIT_REPO_URL"
export GIT_SYNC_DIRECTORY="$GIT_REPO_DIR"
export GIT_SYNC_INTERVAL="${GIT_SYNC_INTERVAL:-60}"
export GIT_SYNC_NEW_FILES="${GIT_SYNC_NEW_FILES:-true}"
export GIT_SYNC_REMOTE="${GIT_SYNC_REMOTE:-origin}"
export GIT_SYNC_COMMIT_MESSAGE="${GIT_SYNC_COMMIT_MESSAGE:-TerminusDB backup sync from {hostname} at {timestamp}}"

# Export environment variables for cron jobs and git-sync-rs
cat > /backup/scripts/backup-env.sh <<EOF
export TERMINUSDB_ADMIN_PASS="$TERMINUSDB_ADMIN_PASS"
export TERMINUSDB_SERVER_DB_PATH="/app/terminusdb/storage/db"
export BACKUP_DATABASES="$BACKUP_DATABASES"
export GIT_REPO_DIR="$GIT_REPO_DIR"
export GIT_REPO_USER="$GIT_REPO_USER"
export GIT_REPO_PASSWORD="$GIT_REPO_PASSWORD"
export GIT_REPO_EMAIL="${GIT_REPO_EMAIL:-backup@terminusdb.local}"
export GIT_SYNC_REPOSITORY="$GIT_REPO_URL"
export GIT_SYNC_DIRECTORY="$GIT_REPO_DIR"
export GIT_SYNC_INTERVAL="${GIT_SYNC_INTERVAL:-60}"
export GIT_SYNC_NEW_FILES="${GIT_SYNC_NEW_FILES:-true}"
export GIT_SYNC_REMOTE="${GIT_SYNC_REMOTE:-origin}"
export GIT_SYNC_COMMIT_MESSAGE="${GIT_SYNC_COMMIT_MESSAGE:-TerminusDB backup sync from {hostname} at {timestamp}}"
export GIT_SYNC_DEBOUNCE="${GIT_SYNC_DEBOUNCE:-2}"
export GIT_SYNC_MIN_INTERVAL="${GIT_SYNC_MIN_INTERVAL:-5}"
export PATH="/usr/local/cargo/bin:\$PATH"
EOF
chmod 600 /backup/scripts/backup-env.sh

# Set up cron job
echo "Setting up cron job with schedule: $BACKUP_CRON_SCHEDULE"
mkdir -p /var/spool/cron/crontabs
cat > /var/spool/cron/crontabs/root <<EOF
# TerminusDB backup cron job
$BACKUP_CRON_SCHEDULE /backup/scripts/backup-triples.sh >> /var/log/backup/cron.log 2>&1
EOF

# Set correct permissions for crontab (cron requires 600 and root:crontab ownership)
chmod 600 /var/spool/cron/crontabs/root
chown root:crontab /var/spool/cron/crontabs/root

# Start git-sync-rs in the background
echo "Starting git-sync-rs..."
/backup/scripts/git-sync-wrapper.sh &

# Start cron
echo "Starting cron..."
service cron start

echo "Backup configuration:"
echo "  Schedule: $BACKUP_CRON_SCHEDULE"
echo "  Repository: $GIT_REPO_DIR"
echo "  Cron log: /var/log/backup/cron.log"
echo ""
echo "SERVER_PORT $TERMINUSDB_SERVER_PORT"

# Start TerminusDB server (from official script)
exec /app/terminusdb/terminusdb serve
