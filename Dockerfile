# syntax=docker/dockerfile:1
#
# RushDB server image.
#
#   docker build -t rushdb .                      # build the binaries from source
#   docker buildx build --build-arg BINARIES=prebuilt ...  # release: package verified
#                                                  # binaries from bin/<arch>/ in the context
#
# Run (a token is required: the server refuses the default token off loopback):
#
#   docker run -d -p 127.0.0.1:3000:3000 -v rushdb-data:/data \
#     -e RUSHDB_TOKEN=change-me rushdb/rushdb

ARG BINARIES=source

# The data directory, owned by the runtime's non-root user (uid 65532).
FROM --platform=$BUILDPLATFORM busybox:1.37 AS data
RUN mkdir -p /data && chown 65532:65532 /data

FROM rust:1.98.1-bookworm AS source
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p rushdb-server --bin rushdb \
    && mkdir /out \
    && cp target/release/rushdb /out/

FROM scratch AS prebuilt
ARG TARGETARCH
COPY bin/${TARGETARCH}/rushdb /out/

FROM ${BINARIES} AS binaries

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=binaries --chmod=0755 /out/rushdb /usr/local/bin/
COPY --from=data --chown=65532:65532 /data /data
# Bind every interface inside the container; which host interfaces reach it is
# decided by `docker run -p`. The cleartext transports need the explicit
# opt-in off loopback: terminate TLS in front of the container, or mount a
# certificate and set RUSHDB_TLS_CERT / RUSHDB_TLS_KEY.
ENV RUSHDB_LISTEN=0.0.0.0 \
    RUSHDB_DATA_DIR=/data \
    RUSHDB_ALLOW_INSECURE_LISTEN=true
VOLUME ["/data"]
# HTTP and PostgreSQL wire.
EXPOSE 3000 5432
HEALTHCHECK --interval=10s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/rushdb", "health"]
ENTRYPOINT ["/usr/local/bin/rushdb", "serve"]
