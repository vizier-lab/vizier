FROM alpine:3.20

ARG BINARY_PATH=dist/vizier
COPY ${BINARY_PATH} /usr/local/bin/vizier

ENV RUST_BACKTRACE=1

ENTRYPOINT ["vizier"]
