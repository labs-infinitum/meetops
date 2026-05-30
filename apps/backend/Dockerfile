# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder

WORKDIR /app
COPY packages/proto packages/proto
COPY apps/backend apps/backend

WORKDIR /app/apps/backend
RUN cargo build --release

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
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY apps/backend/config.docker.toml /app/config.toml
COPY --from=builder /app/apps/backend/target/release/google-meeting-recorder /usr/local/bin/google-meeting-recorder
COPY --from=builder /app/apps/backend/target/release/find-chrome-tab-allow-click /usr/local/bin/find-chrome-tab-allow-click

RUN mkdir -p /data /data/chrome-profile /data/recordings

ENV RUST_LOG=info,google_meeting_recorder=debug,tower_http=debug,tonic=info,chromiumoxide::handler=error

EXPOSE 8080 50051

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["google-meeting-recorder"]
CMD []
