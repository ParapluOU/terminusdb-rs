# TerminusDB 12 (latest released image).
FROM terminusdb/terminusdb-server:v12.0.6

# DEPRECATED: the changeset-sse plugin is our own, unstable, and unmaintained,
# so it is NOT copied into / loaded by the image anymore (the entrypoint no
# longer sets TERMINUSDB_PLUGINS_PATH). This image is now just a plain
# TerminusDB 12 server. Prefer v12's native diff+streaming history endpoint.
# To re-enable the plugin (not recommended), uncomment the COPY below and the
# corresponding line in entrypoint.sh:
# COPY plugins/changeset-sse.pl /opt/terminusdb-plugins/changeset-sse.pl

# Custom entrypoint (store init, then serve).
COPY docker/changeset-sse/entrypoint.sh /opt/terminusdb-entrypoint.sh
RUN chmod +x /opt/terminusdb-entrypoint.sh

ENTRYPOINT ["/opt/terminusdb-entrypoint.sh"]
CMD []
