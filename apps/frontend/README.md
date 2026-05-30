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

Shared protobuf definitions live in `../../packages/proto`. Regenerate TypeScript clients after proto changes:

```bash
bun run proto:generate
```

From the repo root: `bun run proto:generate`
