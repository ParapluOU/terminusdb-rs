#!/usr/bin/env bash
set -e

# Load environment variables
source /backup/scripts/backup-env.sh

# Configuration
BACKUP_DIR="$GIT_REPO_DIR"

echo "====================================================================="
echo "TerminusDB Backup - $(date)"
echo "====================================================================="

# Function to dump a database
dump_database() {
    local org="$1"
    local db="$2"
    local output_dir="$3"

    echo "Backing up database: $org/$db"

    # Create directory for this database
    mkdir -p "$output_dir/$org/$db"

    # Dump instance graph - overwrite same file each time
    /app/terminusdb/terminusdb triples dump "$org/$db/local/branch/main/instance" \
        --format=turtle > "$output_dir/$org/$db/instance.ttl" 2>&1 || {
        echo "Warning: Failed to dump instance graph for $org/$db"
    }

    # Dump schema graph - overwrite same file each time
    /app/terminusdb/terminusdb triples dump "$org/$db/local/branch/main/schema" \
        --format=turtle > "$output_dir/$org/$db/schema.ttl" 2>&1 || {
        echo "Warning: Failed to dump schema graph for $org/$db"
    }

    echo "  ✓ Dumped $org/$db"
}

# Get list of databases to backup
# BACKUP_DATABASES should be formatted as: "admin/db1,admin/db2,org2/db3"
if [ -n "$BACKUP_DATABASES" ]; then
    IFS=',' read -ra DBS <<< "$BACKUP_DATABASES"
    for db_path in "${DBS[@]}"; do
        IFS='/' read -ra PARTS <<< "$db_path"
        ORG="${PARTS[0]}"
        DB="${PARTS[1]}"
        dump_database "$ORG" "$DB" "$BACKUP_DIR"
    done
else
    echo "Warning: BACKUP_DATABASES not set. No databases will be backed up."
    echo "Set BACKUP_DATABASES environment variable with format: admin/db1,admin/db2"
    exit 0
fi

# Update metadata file (overwrite each time)
cat > "$BACKUP_DIR/backup_metadata.txt" <<EOF
Last Backup: $(date -Iseconds)
Hostname: $(hostname)
TerminusDB Version: $(/app/terminusdb/terminusdb --version 2>&1 || echo "unknown")
Databases: ${BACKUP_DATABASES}
EOF

echo "====================================================================="
echo "Backup completed - $(date)"
echo "git-sync-rs will automatically commit and push changes"
echo "====================================================================="
