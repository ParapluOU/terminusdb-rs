#!/usr/bin/env bash
set -e

echo "TerminusDB with changeset-sse plugin starting..."

# Ensure plugins directory exists in storage
mkdir -p /app/terminusdb/storage/plugins

# Copy plugin from bundled location to storage/plugins
# This allows the storage volume to be mounted while still getting the plugin
echo "Installing changeset-sse plugin..."
cp /opt/terminusdb-plugins/changeset-sse.pl /app/terminusdb/storage/plugins/changeset-sse.pl
echo "Plugin installed to /app/terminusdb/storage/plugins/changeset-sse.pl"

# Set default port
TERMINUSDB_SERVER_PORT=${TERMINUSDB_SERVER_PORT:-6363}

# Handle password from file or environment
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

file_env 'TERMINUSDB_ADMIN_PASS'
TERMINUSDB_ADMIN_PASS=${TERMINUSDB_ADMIN_PASS:-root}

# Initialize store if needed
if [ ! -d /app/terminusdb/storage/db ]; then
    echo "Initializing TerminusDB storage..."
    /app/terminusdb/terminusdb store init --key "$TERMINUSDB_ADMIN_PASS"
fi

echo "SERVER_PORT $TERMINUSDB_SERVER_PORT"
echo "Starting TerminusDB server..."
exec /app/terminusdb/terminusdb serve
