# ====================
# Stage 1: Extract pre-built binary from release tarball
# ====================
FROM ubuntu:24.04 AS build

ARG VERSION
ARG TARGETARCH

ADD vizier-v${VERSION}-${TARGETARCH}.tar.gz /staging/

# ====================
# Stage 2: Minimal runtime image
# ====================
FROM ubuntu:24.04 AS runtime

ARG VERSION
ARG TARGETARCH

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3t64 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /staging/vizier-v${VERSION}-${TARGETARCH}/vizier /usr/local/bin/vizier
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD []
