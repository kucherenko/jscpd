# syntax=docker/dockerfile:1
#
# jscpd container image.
#
# Stages
#   fetch    downloads the static musl release binary for the target
#            architecture from GitHub Releases and verifies it against the
#            release's checksums.txt
#   runtime  distroless image holding only the jscpd binary. This is what the
#            docker.yml workflow publishes to ghcr.io/kucherenko/jscpd:
#              docker build --target runtime --build-arg JSCPD_VERSION=5.1.1 .
#              docker run --rm -v "$PWD:/src" ghcr.io/kucherenko/jscpd .
#   mcp      (default) stdio MCP server, `jscpd --mcp`. Glama builds this
#            Dockerfile with no arguments to score the MCP listing (glama.json),
#            so it is the last stage.
#
# Build args
#   JSCPD_VERSION  release version, with or without the leading "v"
#                  (e.g. 5.1.1). Empty or "latest" resolves the latest
#                  GitHub release. Requires jscpd-linux-<arch>-musl.tar.gz on
#                  that release; arm64 musl binaries ship from 5.1.2 on.

ARG JSCPD_VERSION=latest

FROM --platform=$BUILDPLATFORM alpine:3.22@sha256:14358309a308569c32bdc37e2e0e9694be33a9d99e68afb0f5ff33cc1f695dce AS fetch
ARG JSCPD_VERSION
ARG TARGETARCH
RUN apk add --no-cache ca-certificates curl
WORKDIR /jscpd
RUN set -eu; \
    case "$TARGETARCH" in \
      amd64) arch=x64 ;; \
      arm64) arch=arm64 ;; \
      *) echo "unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    version="$JSCPD_VERSION"; \
    if [ -z "$version" ] || [ "$version" = "latest" ]; then \
      version=$(curl -fsSLI -o /dev/null -w '%{url_effective}' https://github.com/kucherenko/jscpd/releases/latest); \
      version="${version##*/tag/}"; \
    fi; \
    version="${version#v}"; \
    base="https://github.com/kucherenko/jscpd/releases/download/v${version}"; \
    asset="jscpd-linux-${arch}-musl.tar.gz"; \
    echo "Downloading ${base}/${asset}"; \
    curl -fsSL --retry 3 --retry-delay 5 -o "$asset" "${base}/${asset}"; \
    curl -fsSL --retry 3 --retry-delay 5 -o checksums.txt "${base}/checksums.txt"; \
    grep " ${asset}$" checksums.txt | sha256sum -c -; \
    tar -xzf "$asset" jscpd; \
    chmod 0755 jscpd

FROM gcr.io/distroless/static-debian12:latest@sha256:d75cdd72874d4790092fcb1b058493ecf6bb5bf2b2b897045b00ff01d91843f2 AS runtime
LABEL org.opencontainers.image.title="jscpd" \
      org.opencontainers.image.description="Copy/paste detector for programming source code" \
      org.opencontainers.image.url="https://jscpd.dev" \
      org.opencontainers.image.source="https://github.com/kucherenko/jscpd" \
      org.opencontainers.image.licenses="MIT"
COPY --from=fetch /jscpd/jscpd /usr/local/bin/jscpd
# Mount the project to scan here: docker run --rm -v "$PWD:/src" ghcr.io/kucherenko/jscpd .
WORKDIR /src
ENTRYPOINT ["jscpd"]

# Stdio MCP server (used by Glama). The server scans its cwd at startup, so it
# gets an empty workdir rather than the container's root filesystem.
FROM runtime AS mcp
WORKDIR /app
ENTRYPOINT ["jscpd", "--mcp"]
