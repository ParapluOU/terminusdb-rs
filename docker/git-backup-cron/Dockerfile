FROM terminusdb/terminusdb-server:v11.2.0-rc5

# Install git, cron, rust/cargo, and other dependencies (Debian-based)
RUN apt-get update && apt-get install -y \
    git \
    cron \
    bash \
    curl \
    build-essential \
    libssl-dev \
    pkg-config \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install Rust and Cargo
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install git-sync-rs globally
RUN /root/.cargo/bin/cargo install git-sync-rs

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
