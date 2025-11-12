FROM terminusdb/terminusdb-server:v11.2.0-rc5

# Install git, cronie (Alpine's cron), rust/cargo, and other dependencies
RUN apk add --no-cache \
    git \
    cronie \
    bash \
    curl \
    build-base \
    openssl-dev \
    ca-certificates

# Install git-sync-rs globally
RUN cargo install git-sync-rs

# Create directories for git repo and scripts
RUN mkdir -p /backup/repo \
    /backup/scripts \
    /var/log/backup

# Copy backup scripts
COPY docker/git-backup-cron/backup-triples.sh /backup/scripts/backup-triples.sh
COPY docker/git-backup-cron/git-sync-wrapper.sh /backup/scripts/git-sync-wrapper.sh
COPY docker/git-backup-cron/entrypoint.sh /backup/scripts/entrypoint.sh

# Make scripts executable
RUN chmod +x /backup/scripts/*.sh

# Set up cron log file
RUN touch /var/log/backup/cron.log

# Set entrypoint
ENTRYPOINT ["/backup/scripts/entrypoint.sh"]
CMD []
