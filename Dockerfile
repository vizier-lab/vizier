# ====================
# Stage 1: Extract pre-built binaries from release tarball
# ====================
FROM ubuntu:24.04 AS build

ARG TARGET_DIR

COPY vizier-${TARGET_DIR}.tar.gz /tmp/

RUN mkdir -p /staging && \
  tar -xzf /tmp/vizier-${TARGET_DIR}.tar.gz -C /staging

# ====================
# Stage 2: Minimal runtime image
# ====================
FROM ubuntu:24.04 AS runtime

ARG TARGET_DIR

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3t64 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /staging/vizier-${TARGET_DIR}/vizier /usr/local/bin/vizier
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD []
