FROM terminusdb/terminusdb-server

# Copy the plugin into the image at a temporary location
COPY plugins/changeset-sse.pl /opt/terminusdb-plugins/changeset-sse.pl

# Copy custom entrypoint script that will inject the plugin at runtime
COPY docker/changeset-sse/entrypoint.sh /opt/terminusdb-entrypoint.sh
RUN chmod +x /opt/terminusdb-entrypoint.sh

# Use custom entrypoint
ENTRYPOINT ["/opt/terminusdb-entrypoint.sh"]
CMD []
