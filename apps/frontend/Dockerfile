# syntax=docker/dockerfile:1

FROM oven/bun:1 AS builder

WORKDIR /app

COPY package.json bun.lock turbo.json ./
COPY apps/frontend/package.json apps/frontend/
COPY packages/proto/package.json packages/proto/
COPY apps/backend/package.json apps/backend/

RUN bun install --frozen-lockfile

COPY packages/proto packages/proto
COPY apps/frontend apps/frontend

WORKDIR /app/apps/frontend
RUN bun run proto:generate && bun run build

FROM caddy:2-alpine AS runtime

RUN apk add --no-cache curl

COPY apps/frontend/Caddyfile /etc/caddy/Caddyfile
COPY --from=builder /app/apps/frontend/dist /srv

EXPOSE 80

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1/ >/dev/null || exit 1
