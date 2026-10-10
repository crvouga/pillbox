# syntax=docker/dockerfile:1
# Compile on the build host, including WASM. Cross-link only the Rust service
# when build and runtime architectures differ (avoids emulating the compiler).
FROM --platform=$BUILDPLATFORM rust:1.94.0-bookworm AS build
ARG TARGETARCH
ARG BUILDARCH
WORKDIR /build
ENV CARGO_BUILD_JOBS=1
RUN rustup target add wasm32-unknown-unknown
RUN if [ "$TARGETARCH" != "$BUILDARCH" ]; then \
      apt-get update && apt-get install -y --no-install-recommends gcc-x86-64-linux-gnu gcc-aarch64-linux-gnu && rm -rf /var/lib/apt/lists/*; \
    fi
RUN cargo install wasm-bindgen-cli --version 0.2.114 --locked
RUN if [ "$TARGETARCH" != "$BUILDARCH" ]; then apt-get update && apt-get install -y --no-install-recommends libc6-dev-amd64-cross libc6-dev-arm64-cross && rm -rf /var/lib/apt/lists/*; fi
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY apps ./apps
COPY scripts/build-web.sh ./scripts/build-web.sh
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
RUN --mount=type=cache,target=/build/target case "$TARGETARCH" in amd64) target=x86_64-unknown-linux-gnu ;; arm64) target=aarch64-unknown-linux-gnu ;; *) exit 1 ;; esac; \
    if [ "$TARGETARCH" = "$BUILDARCH" ]; then unset CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER CC_x86_64_unknown_linux_gnu CC_aarch64_unknown_linux_gnu; fi; \
    rustup target add "$target" && cargo build --locked --release -p pillbox-server --target "$target" && \
    mkdir /out && cp "target/$target/release/pillbox-server" /out/pillbox-server
RUN --mount=type=cache,target=/build/target ./scripts/build-web.sh
COPY web ./web

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /out/pillbox-server /usr/local/bin/pillbox-server
COPY --from=build /build/web ./web
ENV PORT=8080 ASSET_DIR=/app/web RUST_LOG=pillbox_server=info
USER 65532:65532
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s CMD curl -fsS "http://127.0.0.1:${PORT}/health" || exit 1
STOPSIGNAL SIGTERM
CMD ["pillbox-server"]
