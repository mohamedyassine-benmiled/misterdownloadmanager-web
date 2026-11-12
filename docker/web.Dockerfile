# Fork-owned Web image. Keep upstream's root Dockerfile available unchanged.
# Build with: docker build -f docker/web.Dockerfile -t webmisterdownloadmanager .
ARG PROFILE=release
FROM rust:1-trixie AS build
ARG PROFILE
RUN apt-get update \
 && apt-get install -y --no-install-recommends cmake libclang-dev \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
ENV CARGO_PROFILE_RELEASE_DEBUG=false
RUN set -e; \
    cargo build --locked --profile "$PROFILE" -p mdm-app -p mdm-cli; \
    out="target/$PROFILE"; if [ "$PROFILE" = dev ]; then out=target/debug; fi; \
    mkdir -p /out; cp "$out/misterdownloadmanager" "$out/misterdownloadmanager-cli" /out/; \
    strip /out/*; \
    rm -rf target /usr/local/cargo/registry /usr/local/cargo/git

FROM debian:trixie-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && mkdir -p /config /downloads && chown 1000:1000 /config /downloads
COPY --from=build /out/ /usr/local/bin/
ENV MDM_DATA_DIR=/config MDM_DOWNLOADS=/downloads
VOLUME ["/config", "/downloads"]
EXPOSE 8463
USER 1000:1000
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD ["misterdownloadmanager-cli", "health"]
# Flags supplied after the image name are passed to the existing app, not a second server.
ENTRYPOINT ["misterdownloadmanager", "--web"]
