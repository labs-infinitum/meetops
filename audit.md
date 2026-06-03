# Deep Audit — AI Meeting Notetaker

This document records a code/architecture audit covering the Rust backend (auth,
crypto, gRPC, OAuth, config, bot/recording, workers, storage, db), the React
frontend, and the build/infra layer (Docker, Turbo, CI).

Findings are ordered by severity. A build/test run was **not** performed, so the
few "would error" items are flagged for confirmation.

---

## 🔴 Critical / High

### 1. Graceful shutdown abandons in-flight recordings and orphans Chrome
`apps/backend/src/main.rs` (L65–122), `apps/backend/src/workers/scheduler.rs`,
`apps/backend/src/workers/bot_runner.rs`

Recordings run as detached `tokio::spawn` tasks, but graceful shutdown is wired
**only** into the axum HTTP server. On SIGTERM/SIGINT, `main` returns and the
runtime is dropped mid-recording. The recorder loop in `recorder.rs` only listens
for `ctrl_c` (never SIGTERM), so it never finalizes/uploads. Result: the
recording is lost, the `bot_runs` row is stuck in `recording`/`in_meeting`, and
Chrome can be orphaned (holding the shared profile's `SingletonLock`).

**Fix:** On shutdown, request-stop every active run, await recording tasks with a
bounded timeout, and make the recorder react to SIGTERM.

### 2. `MeetingSession` has no `Drop`; cancellation leaks Chrome + handler task
`apps/backend/src/bot/chromium/mod.rs` (L89–119)

Cleanup (browser close + `handler_task.abort()`) only happens in the explicit
`close(self)`. Any cancellation/panic drops the session without it, leaking the
spawned handler task and an orphaned Chrome that locks the shared
`--user-data-dir`.

**Fix:** Implement `Drop` to abort the handler and best-effort kill the browser
child.

### 3. Auth token stored in `localStorage` (XSS-exfiltratable bearer credential)
`apps/frontend/src/lib/auth-context.tsx` (L20, L37–52)

The full session including the long-lived (30-day) bearer `token` is persisted in
`localStorage`, readable by any injected script and impossible to mark
`HttpOnly`. Combined with the 30-day TTL and no rotation
(`config.rs` `session_ttl_secs`), a single XSS yields a portable, long-lived
credential.

**Fix:** Prefer an `HttpOnly`/`Secure`/`SameSite` cookie set by the backend;
shorten token lifetime; keep server-side revocation.

### 4. Secondary-artifact upload failure marks a successful recording as Failed
`apps/backend/src/storage/recordings.rs` (L108–131),
`apps/backend/src/workers/bot_runner.rs` (L78–101)

After the main `raw.webm` uploads successfully, audio/tab-capture uploads use
`?`. If those fail, `persist_bot_recording` returns `Err`, the run is marked
**Failed with no `recording_path`**, the staging dir is never cleaned, and the
user can never play a recording that is actually in S3.

**Fix:** Treat the main video upload as the success boundary; attempt secondary
artifacts best-effort (log + continue).

### 5. Containers run as root; RustDesk debug can fall back to public relays with a weak default password
`apps/backend/Dockerfile`, `apps/frontend/Dockerfile`,
`apps/backend/docker/configure-rustdesk-server.sh` (L18–22),
`debug.env.example` (L4)

Neither final image sets `USER`, so the backend runs Chrome/`xdotool` as root and
writes root-owned files into the host `./data` bind mount. Separately, if
`RUSTDESK_ID_SERVER` is unset, the debug entrypoint still starts RustDesk and
"uses public servers" — exposing a remote desktop into a container holding a
logged-in Google session and OAuth tokens, with the shipped example password
`ChooseAStrongPassword`.

**Fix:** Add a non-root `appuser` and `chown` data/binaries; hard-fail RustDesk
startup when no self-hosted ID server/key/password is configured.

### 6. Mock data still wired into shipped UI (misleading/non-functional features)
`apps/frontend/src/pages/{NotesPage,TodosPage,SettingsPage,DashboardPage}.tsx`

`NotesPage`/`TodosPage` render `mock-data.ts`; `SettingsPage` is entirely
non-functional (`toast.success("Profile saved (demo)")`); the dashboard
"Open todos" stat is hardcoded `0`. These present fabricated data as real.
(Roadmap confirms these are pending.)

**Fix:** Wire to real services or clearly gate as "coming soon"; delete
`mock-data.ts` when done.

---

## 🟠 Medium

### 7. SQLite pool: no WAL, no `busy_timeout`, but 5 connections + a write on every request
`apps/backend/src/db/mod.rs` (L20–29),
`apps/backend/src/auth/service.rs` (`get_session` → `touch_session`)

The pool allows 5 concurrent connections with rollback-journal mode and no busy
timeout, while writers run concurrently (timeline inserts per media chunk,
scheduler claim/finalize, calendar sync, session cleanup). Worse, **every
authenticated gRPC call** runs `touch_session` (an UPDATE) via
`require_user_id`. Under concurrency this yields immediate `SQLITE_BUSY` errors
(e.g. dropped timeline events, failed finalize).

**Fix:** Enable WAL + `busy_timeout(5s)` + `synchronous=Normal`; throttle
`touch_session` (only update if `last_used_at` is stale).

### 8. Login allows username enumeration via timing + distinct signup errors
`apps/backend/src/auth/service.rs` (L60–76, `validate_sign_up`)

When the username doesn't exist, `verify_password` (argon2) is skipped, so the
missing-user path is dramatically faster than the wrong-password path — a
reliable username-enumeration oracle. Signup also returns distinct
`UsernameTaken` vs `EmailTaken`. There's no rate limiting/lockout on login or
signup, enabling brute force.

**Fix:** Run a dummy argon2 verify on the missing-user path; add rate limiting;
consider a generic "invalid credentials".

### 9. OAuth `state` has no TTL and is never garbage-collected
`apps/backend/src/google/oauth.rs` (`start_url`, `verify_state`)

State is stored in `app_kv` and only deleted on a successful callback. Abandoned
flows leak rows forever, and a leaked/logged state URL remains replayable
indefinitely (no expiry check).

**Fix:** Store a timestamp with the state, reject states older than a few
minutes, and sweep expired entries.

### 10. App locks out / hangs when backend is unreachable
`apps/frontend/src/lib/auth-context.tsx` (L75–127),
`apps/frontend/src/routes/guards.tsx`

Every load blocks on `apiGetSession` with no timeout, rendering a full-screen
"Loading…" via both guards. If the backend is down, the user is stuck, then
bounced to `/login` despite a locally valid session. There's also no proactive
session-expiry timer (expiry only detected on next API call).

**Fix:** Optimistically trust a locally-valid stored session, validate in the
background, and only downgrade on an explicit auth error (not a network failure);
add an `expiresAt` timeout.

### 11. Panic in a recording task strands the run and leaks state
`apps/backend/src/workers/bot_runner.rs` (L52–131)

`unregister`/`finalize_run` run only after the body returns. A panic in the
join/record path leaks the `ActiveRecordings` entry and leaves the `bot_runs` row
non-terminal forever; a later `stop_recording` "succeeds" against a dead
receiver.

**Fix:** Use an RAII guard for unregister/finalize; add startup reconciliation
that fails non-terminal runs.

### 12. Recording-detail poll has a stale closure and no unmount guard
`apps/frontend/src/pages/RecordingsPage.tsx` (L201–237)

`loadRun` is omitted from effect deps and has no `cancelled` guard, so the 3s
interval fires `setState` on unmounted components and can use stale closures.

**Fix:** `useCallback` with proper deps; add a `cancelled` flag cleared in
cleanup.

### 13. Internal error strings leaked to clients
`apps/backend/src/grpc/recording.rs` `to_proto_error` (and peers)

Wrapped `sqlx`/internal errors are stringified into `RecordingError.message` and
sent verbatim to the browser, leaking schema/internal details.

**Fix:** Return a generic message for `Internal`; log the detail server-side.

### Additional medium infra items
- `tower_http=debug` baked into the prod image risks logging OAuth codes/tokens
  (`apps/backend/Dockerfile`).
- `backend-debug` exposes no host ports (OAuth callback unreachable) and ignores
  the `PUBLIC_*` CORS/redirect overrides (`docker-compose.yml`).
- `backend` and `backend-debug` share one Chrome profile volume (corruption if
  run together).
- CI sets `should_push=true` even on `pull_request`
  (`.github/workflows/docker-images.yml`).

---

## 🟡 Low / Notable

- **`set_auto_join_for_user` may reference an undefined alias `m` in an UPDATE**
  (`apps/backend/src/db/meetings.rs` ~L113–143) — needs a confirming test; if
  real, the auto-join toggle is broken.
- **`int64 → Number` narrowing** for IDs
  (`apps/frontend/src/lib/proto-fields.ts`) loses precision above 2^53; large
  auto-increment `runId`s could resolve to the wrong record.
- **One bad media chunk kills the writer** (`apps/backend/src/bot/chromium/mod.rs`
  ~L1280–1339) — base64/write errors use `?`; should log-and-continue.
- **Timed-out child processes orphaned** (xdotool/ffmpeg/import) — no
  `.kill_on_drop(true)`; leaks processes over many meetings.
- **Blocking `std::process`/`std::fs` on the async runtime**
  (`apps/backend/src/bot/x11_display.rs`, profile-lock cleanup).
- **Queued (pre-permit) runs can't be stopped** — `register` happens after
  semaphore acquisition.
- **Staging dirs leak on failed runs**; single-part `PutObject` caps recordings
  at 5 GiB with no retry.
- **Module-scope `now`** in `MeetingsPage.tsx` mis-classifies meetings until
  reload.
- **Dependency versions far ahead of GA** in `apps/frontend/package.json`
  (vite ^8, eslint ^10, typescript ~6, react ^19.2.6) — verify they resolve;
  `tsconfig.app.json` is missing `"strict": true`.
- **Turbo:** `build` depends on `proto:generate` globally (gates the backend,
  which generates Rust stubs itself) and `proto:generate` is `cache:false`, so
  `build` is never a full cache hit.
- **`debug.env` not in `.dockerignore`**; image bloat from leftover
  `wget`/`gnupg`.

---

## ✅ Verified sound

- Per-user authorization is correctly enforced on recordings/meetings/calendars
  (queries scoped by `user_id`).
- OAuth tokens **are** AES-256-GCM encrypted at rest when a key is set
  (`apps/backend/src/db/calendar_connections.rs`); CSRF state is high-entropy
  random.
- `get_session` correctly filters expired sessions; argon2 password hashing is
  correct.
- The single-recording semaphore (`acquire_owned` for scheduler queueing,
  `try_acquire_owned` → `BotBusy` for ad-hoc) and `INSERT OR IGNORE` claim are
  race-safe; `X11Session` cleans up Xvfb/openbox on drop.
- No committed secrets/data; no `dangerouslySetInnerHTML`; route guards don't
  flash protected content.

---

## Suggested fix order

Top 5 to address first:

1. #1 — Shutdown loses recordings / orphans Chrome.
2. #3 — Token in `localStorage`.
3. #7 — SQLite WAL + `busy_timeout` and per-request write.
4. #4 — Failed secondary upload mismarks success.
5. #5 — Root containers / RustDesk default password.

Lowest-risk, self-contained wins: #7 (WAL + `busy_timeout`, throttle
`touch_session`), #4 (best-effort secondary uploads), #13 (generic internal
error message).
