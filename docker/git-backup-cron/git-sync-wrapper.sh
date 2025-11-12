#!/usr/bin/env bash
set -e

# Load environment variables
source /backup/scripts/backup-env.sh

echo "Starting git-sync-rs for repository: $GIT_REPO_DIR"

# Export git-sync-rs environment variables
# These are compatible with the original git-sync implementation
export GIT_SYNC_DIRECTORY="$GIT_REPO_DIR"
export GIT_SYNC_INTERVAL="${GIT_SYNC_INTERVAL:-60}"
export GIT_SYNC_NEW_FILES="${GIT_SYNC_NEW_FILES:-true}"  # Include untracked files
export GIT_SYNC_REMOTE="${GIT_SYNC_REMOTE:-origin}"
export GIT_SYNC_COMMIT_MESSAGE="${GIT_SYNC_COMMIT_MESSAGE:-TerminusDB backup sync from {hostname} at {timestamp}}"

echo "  Directory: $GIT_SYNC_DIRECTORY"
echo "  Interval: $GIT_SYNC_INTERVAL seconds"
echo "  Remote: $GIT_SYNC_REMOTE"
echo "  Include new files: $GIT_SYNC_NEW_FILES"

# Run git-sync-rs in watch mode
# Using environment variables for configuration
# The watch command is default if no command specified
exec git-sync-rs \
    --new-files=true \
    --remote "$GIT_SYNC_REMOTE" \
    --directory "$GIT_SYNC_DIRECTORY" \
    watch \
    --interval "$GIT_SYNC_INTERVAL" \
    --debounce "${GIT_SYNC_DEBOUNCE:-2}" \
    --min-interval "${GIT_SYNC_MIN_INTERVAL:-5}"
