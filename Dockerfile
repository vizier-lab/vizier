# ====================
# Stage 1: Extract pre-built binary from release tarball
# ====================
FROM ubuntu:24.04 AS build

ARG VERSION
ARG TARGETARCH

ADD vizier-v${VERSION}-${TARGETARCH}.tar.gz /staging/

# ====================
# Stage 2: Runtime image with Node.js + Python
# ====================
FROM ubuntu:24.04 AS runtime

ARG VERSION
ARG TARGETARCH

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3t64 \
        curl \
        wget \
        python3 \
        python3-pip \
        python3-venv \
        pipx \
    && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /staging/vizier-v${VERSION}-${TARGETARCH}/vizier /usr/local/bin/vizier
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD []
