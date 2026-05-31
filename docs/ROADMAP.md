# Roadmap — AI Meeting Notetaker

Planned work for the monorepo (`apps/backend`, `apps/frontend`, `packages/proto`). For setup and architecture, see the root [README](../README.md).

**North star:** Full product loop (record → transcribe → summarize → notes/todos), settings persisted per user, and **N concurrent recordings** on one machine (isolated Xvfb displays + Chrome profiles).

---

## Recommended order

1. **AI pipeline** — Groq workers, then Notes/Todos/Dashboard APIs
2. **Frontend wiring** — Replace remaining mocks; delete `mock-data.ts`
3. **Settings API** — Profile and prefs end-to-end
4. **Concurrent recording** — Raise `concurrency_max` incrementally (1 → 2 → N)
5. **Production** — HTTPS, prod OAuth/CORS, deploy scripts, CI
6. **Hardening** — Rate limits, S3 CORS/retention, optional Postgres

---

## APIs & protos

- [ ] **`NoteService`** — `ListNotes`, `GetNote` (tldr, summary markdown, tags)
- [ ] **`TodoService`** — `ListActionItems`, `UpdateActionItem` (complete, assignee, due date)
- [ ] **`SettingsService`** — `GetSettings`, `UpdateProfile`, `UpdateRecordingPrefs`, `UpdateNotificationPrefs`
- [ ] Production **`GMR_GRPC__ALLOW_ORIGIN`** and Google OAuth redirect URIs per deploy environment

---

## Frontend (wire real data)

- [ ] **Dashboard** — open todos count and “Latest AI summary” from API (meetings + bot runs already live)
- [ ] **Notes** — list/detail from `summaries` (+ meeting title)
- [ ] **Todos** — action items from API; completion persists
- [ ] **Settings** — load/save profile and prefs via API (session profile only today)
- [ ] Remove or gate mocks behind `import.meta.env.DEV`
- [ ] Delete `apps/frontend/src/lib/mock-data.ts` once the above pages are API-backed

---

## AI pipeline (notes & todos)

Schema exists (`transcripts`, `summaries`, `jobs`); workers and RPCs are not built.

### Groq

- [ ] Implement client using `config.groq` (Whisper + summarization model)
- [ ] Read `GROQ_API_KEY` from env

### Job worker

- [ ] Poll `jobs` where `status = pending` and `run_after <= now`
- [ ] Job kinds:
  - `transcribe` — `bot_run_id` → `transcripts`
  - `summarize` — `transcript_id` → `summaries` (+ `action_items_json`)
- [ ] Retry/backoff via `attempts`, `last_error`, `run_after`
- [ ] On bot run `completed`: enqueue `transcribe` automatically

### Product surfacing

- [ ] Dashboard “Latest AI summary” from most recent user summary
- [ ] Notes/Todos pages backed by `summaries` / `action_items_json`

---

## Concurrent multi-meeting recording

Today: one Chrome profile, `Semaphore(1)`. Target: **N parallel runs**, each with its own Xvfb display and Chrome user-data dir.

```
Scheduler → RecordingSlotPool (size N)
  ├── Slot 0: DISPLAY :10, profile user-A, bot_run …
  ├── Slot 1: DISPLAY :11, profile user-B, bot_run …
  └── …
```

### Backend

- [ ] Replace global `Semaphore(1)` with **`recording.concurrency_max`** (default 1; raise after testing)
- [ ] **Display pool** — allocate/free `:10`–`:99`; track `{ display, user_id, bot_run_id }`; kill orphaned Xvfb/openbox on timeout
- [ ] **Chrome profile pool** — e.g. `data/chrome-profiles/{user_id}/`; document one-time bot sign-in per user
- [ ] Scheduler: acquire slot before spawn; queue excess; optional fair scheduling; dedupe shared Meet URLs
- [ ] Pass `DISPLAY` per child only (no process-global mutation)
- [ ] Supervisor for zombie Xvfb/Chrome after crashes
- [ ] Logging: active slots, queued runs, display allocations

### Config (`apps/backend/config.toml.example`)

- [ ] `bot.concurrency_max = 1`
- [ ] `bot.display_range_start = 10`, `bot.display_range_end = 99`
- [ ] `bot.chrome_profiles_dir = "./data/chrome-profiles"`
- [ ] `bot.per_run_display_isolation = true`

### Ops & tests

- [ ] Document per-user bot Google account setup (invite bot email to meetings)
- [ ] Document minimum RAM/CPU for N concurrent captures
- [ ] Integration test: two users, overlapping meetings, two displays, S3 artifacts per run

---

## Recording bot & storage

- [ ] **Stop while `queued`** — handle race before worker starts (`not_active`; optional retry once `joining`)
- [ ] **S3 bucket CORS** — `GET`/`HEAD` from frontend origins for presigned playback
- [ ] **S3 lifecycle** — retention policy for old recordings
- [ ] Optional playback **poster/thumbnail** from first frame

---

## Security & reliability

- [ ] Rate limiting on auth endpoints
- [ ] Per-user Chrome profiles (today: one global profile + concurrency 1)
- [ ] Optional: session tokens in httpOnly cookies instead of `localStorage`

---

## Calendars (later)

- [ ] Register production redirect URIs in Google Cloud Console
- [ ] Outlook / Apple / CalDAV — UI placeholders exist; backend not started

---

## Production & CI

### Deploy

- [ ] Production Compose or EC2 script pulling ECR images
- [ ] Optional: serve React without Docker (Axum static or CDN + API host)
- [ ] HTTPS termination; grpc-web over HTTPS
- [ ] Secrets management (Google, Groq, AWS, `GMR_STORAGE__TOKEN_ENCRYPTION_KEY`)
- [ ] Optional: SQLite → Postgres when running multiple API instances

### Testing

- [ ] Backend gRPC integration tests (signup/login over wire)
- [ ] Frontend smoke tests for auth flow
- [ ] CI workflow: `cargo test`, `cargo clippy`, `bun run build`
- [ ] Manual QA script: calendar → meeting → record → stop → note appears

### Tooling

- [ ] ESLint: ignore or relax `packages/proto/src/gen/**` so `bun run lint` is usable
- [ ] knip (or similar) for unused TS once Node ≥20.19 / ESM issue resolved
- [ ] `ThemeProvider` for `sonner` or drop `next-themes` usage

---

## Cleanup

- [ ] Add `apps/backend/config.toml.example` alongside `config.docker.toml`
- [ ] Align frontend `isActiveBotRun` with backend `is_stoppable_status` (shared constants or proto enum)
- [ ] Proto `enum` for `BotRunStatus` (today: string on wire)

---

## Key paths (for upcoming work)

| Area | Path |
|------|------|
| Protos | `packages/proto/notetaker/v1/` |
| gRPC handlers | `apps/backend/src/grpc/` |
| Job worker (new) | `apps/backend/src/workers/` |
| Bot runner / scheduler | `apps/backend/src/workers/bot_runner.rs`, `scheduler.rs` |
| Display isolation | `apps/backend/src/bot/x11_display.rs`, `chromium/mod.rs` |
| Frontend mocks | `apps/frontend/src/lib/mock-data.ts` |
| Frontend pages | `apps/frontend/src/pages/` |
