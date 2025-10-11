FROM terminusdb/terminusdb-server

# Copy the changeset-sse plugin into the image
COPY plugins/changeset-sse.pl /opt/terminusdb-plugins/changeset-sse.pl

# Copy custom entrypoint that sets TERMINUSDB_PLUGINS_PATH before serving
# This ensures plugins are NOT loaded during store init, only when serving
COPY docker/changeset-sse/entrypoint.sh /opt/terminusdb-entrypoint.sh
RUN chmod +x /opt/terminusdb-entrypoint.sh

ENTRYPOINT ["/opt/terminusdb-entrypoint.sh"]
CMD []
