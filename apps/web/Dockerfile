FROM oven/bun:1 AS builder
WORKDIR /app
COPY apps/web/package.json apps/web/bun.lock* ./
RUN bun install --frozen-lockfile || bun install
COPY apps/web/ ./
RUN bun run build

FROM caddy:2-alpine
COPY --from=builder /app/dist /srv
COPY apps/web/Caddyfile /etc/caddy/Caddyfile
EXPOSE 80
CMD ["caddy", "run", "--config", "/etc/caddy/Caddyfile"]
