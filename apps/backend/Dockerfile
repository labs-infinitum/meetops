# syntax=docker/dockerfile:1

# cargo-chef splits dependency compilation from app compilation so `docker compose build`
# reuses cached deps when only Rust sources change (Cargo.lock/Cargo.toml unchanged).
FROM lukemathwalker/cargo-chef:latest-rust-1-bookworm AS chef

FROM chef AS planner
WORKDIR /app
COPY packages/proto packages/proto
COPY apps/backend apps/backend
WORKDIR /app/apps/backend
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS cacher
WORKDIR /app
COPY packages/proto packages/proto
WORKDIR /app/apps/backend
COPY apps/backend/Cargo.toml apps/backend/Cargo.lock apps/backend/build.rs ./
COPY apps/backend/migrations ./migrations
COPY --from=planner /app/apps/backend/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder
WORKDIR /app
COPY packages/proto packages/proto
COPY apps/backend apps/backend
WORKDIR /app/apps/backend
COPY --from=cacher /app/apps/backend/target ./target
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release --bin google-meeting-recorder --bin find-chrome-tab-allow-click && \
    test "$(stat -c%s target/release/google-meeting-recorder)" -gt 1000000 && \
    install -D target/release/google-meeting-recorder /app/bin/google-meeting-recorder && \
    install -D target/release/find-chrome-tab-allow-click /app/bin/find-chrome-tab-allow-click

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        gnupg \
        wget \
    && wget -q -O - https://dl.google.com/linux/linux_signing_key.pub \
        | gpg --dearmor -o /usr/share/keyrings/google-chrome.gpg \
    && echo "deb [arch=amd64 signed-by=/usr/share/keyrings/google-chrome.gpg] https://dl.google.com/linux/chrome/deb/ stable main" \
        > /etc/apt/sources.list.d/google-chrome.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
        ffmpeg \
        google-chrome-stable \
        imagemagick \
        openbox \
        xvfb \
        xdotool \
        x11-utils \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY apps/backend/config.docker.toml /app/config.toml
COPY apps/backend/docker/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
COPY --from=builder /app/bin/google-meeting-recorder /usr/local/bin/google-meeting-recorder
COPY --from=builder /app/bin/find-chrome-tab-allow-click /usr/local/bin/find-chrome-tab-allow-click

RUN mkdir -p /data /data/chrome-profile /data/recordings \
    && chmod +x /usr/local/bin/docker-entrypoint.sh

ENV RUST_LOG=info,google_meeting_recorder=debug,tower_http=debug,tonic=info,chromiumoxide::handler=error

EXPOSE 8080 50051

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD []

# Debug image: RustDesk + pinned Xvfb display (:99) for remote viewing during profile/debug.
FROM runtime AS debug

ARG RUSTDESK_VERSION=1.4.6

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        dbus-x11 \
        libegl1 \
        libgbm1 \
        libgl1 \
        libglib2.0-0 \
        libxcb1 \
        libxfixes3 \
        libxrandr2 \
        x11-utils \
    && wget -q "https://github.com/rustdesk/rustdesk/releases/download/${RUSTDESK_VERSION}/rustdesk-${RUSTDESK_VERSION}-x86_64.deb" \
        -O /tmp/rustdesk.deb \
    && apt-get install -y /tmp/rustdesk.deb \
    && rm -f /tmp/rustdesk.deb \
    && rm -rf /var/lib/apt/lists/*

COPY apps/backend/config.docker.debug.toml /app/config.toml
COPY apps/backend/docker/debug-entrypoint.sh /usr/local/bin/debug-entrypoint.sh
COPY apps/backend/docker/configure-rustdesk-server.sh /usr/local/bin/configure-rustdesk-server.sh
RUN chmod +x /usr/local/bin/debug-entrypoint.sh /usr/local/bin/configure-rustdesk-server.sh

ENV DEBUG_X11_DISPLAY=:99 \
    GMR_BOT__X11_DISPLAY=:99 \
    GMR_BOT__X11_AUTO_VIRTUAL=false

HEALTHCHECK NONE

EXPOSE 21115 21116 21116/udp 21117

ENTRYPOINT ["/usr/local/bin/debug-entrypoint.sh"]
CMD []
