# Roadmap — AI Meeting Notetaker

Planned work for the monorepo (`apps/backend`, `apps/frontend`, `packages/proto`). For setup and architecture, see the root [README](../README.md).

**North star:** Full product loop (record → transcribe → summarize → notes/todos), settings persisted per user, and **N concurrent recordings** on one machine (isolated Xvfb displays + Chrome profiles).

**Last reviewed:** 2026-05-31 — timeline collector, join/roster hardening, overlay dismiss, and proto workspace are in tree; AI pipeline, live preview, and Notes/Todos APIs still open.

---

## Recommended order

1. **AI pipeline** — Groq workers, then Notes/Todos/Dashboard APIs
2. **Live recording preview** — Bot-view frames on active runs (see [Recordings UI](#recordings-ui-playback--live-preview)); replaces “Recording in progress…” placeholder
3. **Frontend wiring** — Replace remaining mocks; delete `mock-data.ts`
4. **Settings API** — Profile and prefs end-to-end
5. **Concurrent recording** — Raise `concurrency_max` incrementally (1 → 2 → N)
6. **Bot Google sessions** — Profile health checks, automatic sign-in (no manual `profile` CLI)
7. **Telemetry & alerts** — Structured ops signals + developer notifications on failures
8. **Production** — HTTPS, prod OAuth/CORS, deploy scripts, broaden CI beyond Docker images
9. **Hardening** — Rate limits, S3 CORS/retention, optional Postgres

Meeting timeline and recordings detail (playback + events) are largely in place; reliability polish continues under [Meeting timeline](#meeting-timeline-in-call-events).

---

## APIs & protos

- [ ] **`NoteService`** — `ListNotes`, `GetNote` (tldr, summary markdown, tags)
- [ ] **`TodoService`** — `ListActionItems`, `UpdateActionItem` (complete, assignee, due date)
- [ ] **`SettingsService`** — `GetSettings`, `UpdateProfile`, `UpdateRecordingPrefs`, `UpdateNotificationPrefs`
- [ ] Production **`GMR_GRPC__ALLOW_ORIGIN`** and Google OAuth redirect URIs per deploy environment

`RecordingService` already exposes bot runs, playback URLs, ad-hoc start/stop, and meeting timeline events (`ListMeetingTimelineEvents`).

---

## Meeting timeline (in-call events)

Collect structured events during recording (join/leave, screen share, active speaker, periodic snapshots) for the recordings UI and future AI context.

### Backend & bot

- [x] SQLite `meeting_timeline_events` + `apps/backend/src/db/meeting_timeline.rs`
- [x] `RecordingService.ListMeetingTimelineEvents` (gRPC + grpc-web)
- [x] In-tab collector in `install_recording_control.js` (person join/leave diffs, active speaker, snapshots)
- [x] **Screen share events** — `screen_share_started` / `screen_share_stopped` for presentation listitems (not join/leave); `presenterParticipantId` + `presentationId` in payload; duplicate-name disambiguation (space prefix, avatar, roster order)
- [x] Post-join roster prime — `prime_participant_roster.js` (wait out host lobby up to ~20s, open/close People panel, hydrate `data-participant-id` rows)
- [x] Join lobby detection — `attempt_join.js` `waiting_for_host` (“Please wait until a meeting host…”) before treating UI as in-call; join loop retries prime on `timed_out_waiting_for_host`
- [x] Meet overlay dismiss — `dismiss_meet_overlays.js` (`Got it`, self-view hints, etc.) during join poll, before capture, and every ~5s while recording
- [x] Participant count helper — `meet_participant_count.js` (excludes presentation rows; solo-leave / admission checks from Rust)
- [x] Flush on stop — `stop_tab_recording.js`
- [x] Chromium scripts extracted from `chromium/mod.rs` into `scripts/*.js` + `include_str!` (`scripts/mod.rs`, syntax tests)
- [x] Chromium scripts documented — `apps/backend/src/bot/chromium/scripts/README.md`

### Frontend

- [x] Recordings detail — timeline list with typed payload parsing (`timeline-payload.ts`, `RecordingsPage.tsx`)
- [x] Timeline UI for screen share — “started/stopped presenting” + icons (`RecordingsPage.tsx`)

### Follow-ups

- [ ] Further harden People panel / roster hydration when `rosterCount` stays 0 (Meet UI variants) — see scripts README
- [ ] Optional: surface timeline summary on meeting detail or export (JSON/CSV)
- [ ] Optional: correlate timeline rows with transcript timestamps after AI pipeline ships

---

## Frontend (wire real data)

- [x] **Meetings, calendars, recordings list** — live gRPC (no mocks on those pages)
- [x] **Recordings detail** — presigned playback when `completed` + live timeline poll while active; placeholder while recording (no live video yet — see [Recordings UI](#recordings-ui-playback--live-preview))
- [x] **Proto clients** — generated TS in `packages/proto`, imported as `@ai-meeting-notetaker/proto` (Turbo `proto:generate` before frontend build)
- [ ] **Dashboard** — `GetDashboardSummary` wired for upcoming meetings, calendar count, completed recordings; **open todos** still hardcoded `0`; no “Latest AI summary” yet
- [ ] **Notes** — list/detail from `summaries` (+ meeting title); today uses `mock-data.ts`
- [ ] **Todos** — action items from API; completion persists; today uses `mock-data.ts`
- [ ] **Settings** — load/save profile and prefs via API (session profile + demo toasts only)
- [ ] Remove or gate mocks behind `import.meta.env.DEV`
- [ ] Delete `apps/frontend/src/lib/mock-data.ts` once Notes/Todos/Dashboard stats are API-backed

---

## Recordings UI (playback & live preview)

Today: completed runs use `GetPlaybackUrl` (S3 presigned `raw.webm`). Active runs show a static placeholder (“Recording in progress…”) while the bot appends chunks to staging on disk — nothing is exposed to the browser until finalize/upload.

**Not the same as embedding Google Meet** — users cannot “watch the call” as a Meet participant without a separate join flow. Live preview means **what the bot sees** (tab capture / Meet UI in headless Chrome).

### Planned (MVP: preview frames)

- [ ] Proto + `RecordingService.GetLivePreview` (or equivalent) — presigned JPEG URL + `captured_at` for active runs only
- [ ] Bot: periodic CDP screenshot during `joining` / `in_meeting` / `recording` (e.g. every 2–3s); write `staging/.../live-preview.jpg` (or in-memory + upload)
- [ ] Frontend: poll preview on `RecordingDetailPage` alongside existing 3s `GetBotRun` / timeline refresh; “Live (bot view)” badge; fall back to current placeholder on failure
- [ ] Auth + CORS for image URL; stop capture after run completes

### Later (optional)

- [ ] Near-live **video** from growing staging `raw.webm` (MSE / HLS / ffmpeg sidecar) — higher effort; many browsers won’t play incomplete WebM
- [ ] Playback poster/thumbnail from first frame (completed runs)

---

## AI pipeline (notes & todos)

Schema exists (`transcripts`, `summaries`, `jobs`); workers and RPCs are not built. Groq model names are configured in `config.toml.example` / `config.rs` but unused at runtime.

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
- [ ] Dashboard open-todos count from API
- [ ] Notes/Todos pages backed by `summaries` / `action_items_json`

---

## Concurrent multi-meeting recording

Today: one Chrome profile (`chrome_user_data_dir`), `Semaphore(1)` in `main.rs`. Target: **N parallel runs**, each with its own Xvfb display and Chrome user-data dir.

```
Scheduler → RecordingSlotPool (size N)
  ├── Slot 0: DISPLAY :10, profile user-A, bot_run …
  ├── Slot 1: DISPLAY :11, profile user-B, bot_run …
  └── …
```

### Backend

- [ ] Replace global `Semaphore(1)` with **`recording.concurrency_max`** (default 1; raise after testing)
- [ ] **Display pool** — allocate/free `:10`–`:99`; track `{ display, user_id, bot_run_id }`; kill orphaned Xvfb/openbox on timeout
- [ ] **Chrome profile pool** — e.g. `data/chrome-profiles/{user_id}/`; pair with [Bot Google session & automation](#bot-google-session--automation) (no manual `cargo run -- profile` flow long-term)
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

## Bot Google session & automation

Today developers run `cargo run -- profile` (or Docker `backend profile` / `backend-debug` with RustDesk — see root README). Target: **hands-off bot accounts** with session health checks before each run.

### Automatic sign-in (dev & prod)

- [ ] **Eliminate manual profile bootstrap** — no requirement to open the profile command and sign in interactively for routine dev/prod use
- [ ] **Automated Google login flow** for each bot Chrome profile (per user or per `calendar_connection`), using stored credentials or a supported auth mechanism (design TBD: service account, dedicated bot users + scripted login, or OAuth refresh tied to calendar tokens — must not conflate app-user OAuth with Meet bot identity)
- [ ] **First-run provisioning** — create profile dir, run login once programmatically, persist cookies/session state under `data/chrome-profiles/`
- [ ] Document security boundaries: which secrets live where, rotation, and least-privilege bot accounts

### Session health & recovery

- [ ] **Track profile session validity** per Google account — detect signed-out, expired, or challenged sessions (e.g. probe `accounts.google.com`, Meet join preflight, or cookie freshness heuristics)
- [ ] Expose status in API/DB (e.g. `calendar_connections.bot_session_status` or per-profile KV): `valid` | `expired` | `unknown` | `error` + `last_checked_at`
- [ ] **Before each bot run**: verify session; if invalid, attempt **automatic re-sign-in** before joining Meet
- [ ] On repeated failure: mark connection degraded, skip auto-record for that account, surface clear error on recordings/calendars UI
- [ ] Optional: scheduled background health check worker (not only on-demand at join time)

---

## Telemetry & operator alerts

When the bot, scheduler, or session automation fails, operators need visibility without tailing logs on the host.

### Telemetry

- [ ] **Structured telemetry** for bot lifecycle: slot allocation, display/profile used, join/leave, capture start/stop, tab-capture retries, S3 upload, session check outcomes
- [ ] Correlate events with `user_id`, `bot_run_id`, `calendar_connection_id`, and Chrome profile path
- [ ] Metrics hooks (latency, success/failure counts, queue depth) — export format TBD (OpenTelemetry, Prometheus, or structured JSON logs)
- [ ] Optional: persist recent events per run for debugging in the recordings detail UI

### Developer notifications

- [ ] **Alerting on actionable failures** — session expired and re-login failed, bot crash, stuck `joining`, S3 upload failure, repeated tab-capture denial, Xvfb/Chrome zombie detected
- [ ] Pluggable notification channels (start with one): email, Slack webhook, or generic HTTP webhook; config via env/`config.toml`
- [ ] Rate-limit / dedupe alerts (same connection failing N times in a window)
- [ ] Include enough context in the payload (run id, meet URL, error message, link to logs) for fast triage

---

## Recording bot & storage

- [ ] **Stop while `queued`** — `is_stoppable_status` includes `queued`, but `ActiveRecordings` only registers when the worker starts; cancel before spawn still returns `not_active` (optional: DB status transition + scheduler respect)
- [ ] **S3 bucket CORS** — `GET`/`HEAD` from frontend origins for presigned playback
- [ ] **S3 lifecycle** — retention policy for old recordings

Tab-capture Allow automation, staging uploads, Meet overlay dismiss, and tab-capture debug PNGs are implemented (see `config.toml.example` `[bot]` comments`). Live preview and playback posters: [Recordings UI](#recordings-ui-playback--live-preview).

---

## Security & reliability

- [ ] Rate limiting on auth endpoints
- [ ] Per-user Chrome profiles (today: one global profile + concurrency 1); see [Bot Google session & automation](#bot-google-session--automation)
- [ ] Optional: session tokens in httpOnly cookies instead of `localStorage`

---

## Calendars (later)

- [ ] Register production redirect URIs in Google Cloud Console
- [ ] Outlook / Apple / CalDAV — UI placeholders exist; backend not started

---

## Production & CI

### Deploy

- [x] **Docker Compose** — backend (Chrome/Xvfb), frontend, debug profile with RustDesk (root README)
- [x] **ECR image build** on `main` / PRs — `.github/workflows/docker-images.yml`
- [ ] Production Compose or EC2 script pulling ECR images (beyond local compose)
- [ ] Optional: serve React without Docker (Axum static or CDN + API host)
- [ ] HTTPS termination; grpc-web over HTTPS
- [ ] Secrets management (Google, Groq, AWS, `GMR_STORAGE__TOKEN_ENCRYPTION_KEY`)
- [ ] Optional: SQLite → Postgres when running multiple API instances

### Testing

- [ ] Backend gRPC integration tests (signup/login over wire)
- [ ] Frontend smoke tests for auth flow
- [ ] CI workflow: `cargo test`, `cargo clippy`, `bun run build` (in addition to Docker builds)
- [ ] Manual QA script: calendar → meeting → record → stop → note appears

### Tooling

- [ ] ESLint: ignore or relax `packages/proto/src/gen/**` so `bun run lint` is usable
- [ ] knip (or similar) for unused TS once Node ≥20.19 / ESM issue resolved
- [ ] `ThemeProvider` for `sonner` or drop `next-themes` usage

---

## Cleanup

- [x] `apps/backend/config.toml.example` (local dev; Docker uses `config.docker.toml`)
- [x] Proto generation in `packages/proto` with workspace exports (removed `apps/frontend/src/gen/`)
- [ ] Align frontend `isActiveBotRun` with backend `is_stoppable_status` (logic already matches; share via proto `BotRunStatus` enum instead of string literals)
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
| Meet injection scripts | `apps/backend/src/bot/chromium/scripts/*.js`, `scripts/README.md` |
| Overlay dismiss | `scripts/dismiss_meet_overlays.js`, `chromium/mod.rs` (`dismiss_meet_overlays_best_effort`) |
| Meeting timeline DB | `apps/backend/src/db/meeting_timeline.rs`, `migrations/0002_meeting_timeline_events.sql` |
| Profile CLI (today) | `cargo run -- profile` or `docker compose run … backend profile` |
| Frontend mocks | `apps/frontend/src/lib/mock-data.ts` |
| Frontend pages | `apps/frontend/src/pages/` |
| Timeline UI helpers | `apps/frontend/src/lib/timeline-payload.ts`, `types/timeline.ts` |
| Generated proto (TS) | `packages/proto/src/gen/`, `packages/proto/notetaker/v1/*.proto` |
