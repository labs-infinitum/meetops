ARG BUN_IMAGE=oven/bun:1
ARG CADDY_IMAGE=caddy:2-alpine
FROM ${BUN_IMAGE} AS builder
WORKDIR /app
COPY apps/web/package.json apps/web/bun.lock* ./
RUN bun install --frozen-lockfile || bun install
COPY apps/web/ ./
RUN bun run build

FROM ${CADDY_IMAGE}
COPY --from=builder /app/dist /srv
COPY apps/web/Caddyfile /etc/caddy/Caddyfile
EXPOSE 80
CMD ["caddy", "run", "--config", "/etc/caddy/Caddyfile"]
