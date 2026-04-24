# ====================
# Stage 1: Extract pre-built binaries from release tarball
# ====================
FROM debian:bookworm AS build

ARG TARGET_DIR

COPY vizier-${TARGET_DIR}.tar.gz /tmp/

RUN mkdir -p /staging && \
  tar -xzf /tmp/vizier-${TARGET_DIR}.tar.gz -C /staging

# ====================
# Stage 2: Minimal runtime image
# ====================
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
  ca-certificates \
  libssl3 \
  && rm -rf /var/lib/apt/lists/*

COPY --from=build /staging/vizier-${TARGET_DIR}/vizier /usr/local/bin/vizier

ENTRYPOINT ["vizier"]
