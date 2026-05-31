# syntax=docker/dockerfile:1

ARG BUN_IMAGE=oven/bun:1@sha256:e10577f0db68676a7024391c6e5cb4b879ebd17188ab750cf10024a6d700e5c4
ARG CADDY_IMAGE=caddy:2-alpine@sha256:86deaf5e3d3408a6ccec08fbb79989783dd26e206ae10bcf78a801dc8c9ab794

FROM ${BUN_IMAGE} AS builder

WORKDIR /app

COPY package.json bun.lock turbo.json ./
COPY apps/frontend/package.json apps/frontend/
COPY packages/proto/package.json packages/proto/
COPY apps/backend/package.json apps/backend/

RUN --mount=type=cache,target=/root/.bun/install/cache \
    bun install --frozen-lockfile

COPY packages/proto packages/proto
COPY apps/frontend apps/frontend

WORKDIR /app/apps/frontend
RUN bun run proto:generate && bun run build

FROM ${CADDY_IMAGE} AS runtime

RUN apk add --no-cache curl

COPY apps/frontend/Caddyfile /etc/caddy/Caddyfile
COPY --from=builder /app/apps/frontend/dist /srv

EXPOSE 80

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1/ >/dev/null || exit 1
