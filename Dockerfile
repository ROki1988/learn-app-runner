FROM rust:1.52.1-slim as build

WORKDIR /app

ENV HOME=/app
ENV SCCACHE_CACHE_SIZE="1G"
ENV SCCACHE_DIR=$HOME/.cache/sccache
ENV RUSTC_WRAPPER="/usr/local/cargo/bin/sccache"

ENV LINK="https://github.com/mozilla/sccache/releases/download"
ENV SCCACHE_VERSION="v0.2.15"
ENV SCACHE_SHA256="e5d03a9aa3b9fac7e490391bbe22d4f42c840d31ef9eaf127a03101930cbb7ca"
ENV SCCACHE_FILE="sccache-$SCCACHE_VERSION-x86_64-unknown-linux-musl"

RUN apt-get update \
  && apt install -y wget
RUN wget -q "$LINK/$SCCACHE_VERSION/$SCCACHE_FILE.tar.gz"  \
  && echo "$SCACHE_SHA256 *$SCCACHE_FILE.tar.gz" | sha256sum -c - \
  && tar xvf $SCCACHE_FILE.tar.gz \
  && mv -f $SCCACHE_FILE/sccache $RUSTC_WRAPPER \
  && chmod +x $RUSTC_WRAPPER \
  && rm -rf $SCCACHE_FILE sccache.tar.gz
WORKDIR $HOME

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/.cache/sccache \
    sccache --start-server \
    && sccache --show-stats \
    && cargo build --release \
    && (sccache --stop-server || true)

# our final base
FROM gcr.io/distroless/cc

# copy the build artifact from the build stage
COPY --from=build /app/target/release/learn-app-runner .

# set the startup command to run your binary
ENTRYPOINT ["./learn-app-runner"]