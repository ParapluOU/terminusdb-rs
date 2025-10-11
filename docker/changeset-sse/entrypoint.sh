#!/usr/bin/env bash
set -e

TERMINUSDB_SERVER_PORT=${TERMINUSDB_SERVER_PORT:-6363}

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

file_env 'TERMINUSDB_ADMIN_PASS'
TERMINUSDB_ADMIN_PASS=${TERMINUSDB_ADMIN_PASS:-root}

# Initialize store if needed (WITHOUT plugins loaded)
if [ ! -d /app/terminusdb/storage/db ]; then
    /app/terminusdb/terminusdb store init --key "$TERMINUSDB_ADMIN_PASS"
fi

echo "SERVER_PORT $TERMINUSDB_SERVER_PORT"

# NOW set plugin path before serving (so plugins load after init)
export TERMINUSDB_PLUGINS_PATH=/opt/terminusdb-plugins

exec /app/terminusdb/terminusdb serve
