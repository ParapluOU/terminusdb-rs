FROM terminusdb/terminusdb-server

# Copy the changeset-sse plugin into the image
COPY plugins/changeset-sse.pl /opt/terminusdb-plugins/changeset-sse.pl

# Set plugins path to our bundled plugins directory
ENV TERMINUSDB_PLUGINS_PATH=/opt/terminusdb-plugins
