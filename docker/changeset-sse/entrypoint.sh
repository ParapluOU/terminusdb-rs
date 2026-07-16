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

# DEPRECATED / DISABLED: the changeset-sse Prolog plugin is our own, is not
# stable, and is no longer maintained. Do NOT load it. Leaving the plugin path
# unset means the server serves as a plain TerminusDB 12 instance. TerminusDB 12
# offers native diff+streaming on the history endpoint; prefer that instead.
# To re-enable the plugin (not recommended), uncomment the line below:
# export TERMINUSDB_PLUGINS_PATH=/opt/terminusdb-plugins

exec /app/terminusdb/terminusdb serve
