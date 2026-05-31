# Frontend

React + Vite UI for AI Meeting Notetaker. Talks to the Rust backend via grpc-web (proxied to `:50051` in dev).

## Development

From the repo root:

```bash
bun run dev --filter=@ai-meeting-notetaker/frontend
```

Or from this directory:

```bash
bun dev
```

Ensure the backend is running:

```bash
cd ../backend && cargo run
```

## Proto clients

TypeScript clients are generated in `../../packages/proto` and imported as `@ai-meeting-notetaker/proto/notetaker/v1/*`. Regenerate after `.proto` changes:

```bash
# from repo root
bun run proto:generate
```
