# Task: MeetOps.Pro rebrand — landing page & project-wide

**Status:** Not started  
**Domain:** [MeetOps.Pro](https://meetops.pro) (use `meetops.pro` in URLs, `MeetOps` or `MeetOps.Pro` in UI copy)  
**Owner:** —  
**Target:** Public marketing + app chrome aligned with brand before production deploy

---

## Overview

Rebrand the product from generic **“AI Meeting Notetaker” / “Notetaker”** to **MeetOps.Pro**: update the marketing landing page, in-app naming, SEO metadata, and deployment/OAuth configuration so the experience matches the new domain. Internal repo/package names (`ai-meeting-notetaker-rs`, `@ai-meeting-notetaker/*`, `notetaker.v1` protos) can stay for now to limit churn; user-visible and ops-facing surfaces must change.

---

## Specification / scope

### In scope

| Area | What to change |
|------|----------------|
| **Landing page** | Hero, features, pricing, CTA, mock browser URL (`app.meetops.pro`), value prop tuned to “meeting ops” positioning |
| **Marketing chrome** | `MarketingNav`, `MarketingFooter`, auth pages (`Login`, `Signup`) if they show product name |
| **App chrome** | `AppShell` sidebar/header — “Notetaker” → MeetOps |
| **HTML / SEO** | `index.html` title, meta description, favicon, optional OG image |
| **Legal placeholders** | Footer links: Privacy, Terms (routes or external URLs on `meetops.pro`) |
| **Docs** | README title/intro, ROADMAP product name (keep repo path as-is) |
| **Production config** | `GMR_GRPC__ALLOW_ORIGIN`, Google OAuth redirect URIs, `frontend_redirect_url`, CORS for `https://meetops.pro` and `https://app.meetops.pro` (or chosen host layout) |
| **Deploy docs** | README / ROADMAP production section: DNS, TLS, env examples for MeetOps.Pro |

### Out of scope (follow-up tasks)

- Renaming npm packages (`@ai-meeting-notetaker/*`) or Rust crate `google-meeting-recorder`
- Renaming protobuf package `notetaker.v1` (breaking API; only if you want a v2)
- Renaming GitHub repo / Docker image names
- Custom logo design system (use wordmark + existing favicon until brand assets exist)
- Full privacy policy / terms legal copy (lawyer-reviewed) — use placeholders + “coming soon”

---

## Technical approach

### Brand guidelines (working)

- **Product name (UI):** `MeetOps` or `MeetOps.Pro` (footer/legal: `MeetOps.Pro`)
- **Domain:** `meetops.pro`
- **Suggested hosts:**
  - `https://meetops.pro` — marketing + SPA (current Vite app) **or**
  - `https://app.meetops.pro` — authenticated app; `https://meetops.pro` — marketing only  
  Pick one layout and document it in README.
- **Tagline (draft):** “Meeting ops for Google Meet — record, timeline, and AI notes in one place.”
- **Do not** claim Outlook/Apple calendar sync on landing until backend ships (ROADMAP: Calendars later).

### Landing page (`apps/frontend/src/pages/LandingPage.tsx`)

- [ ] Replace hero headline/subcopy with MeetOps positioning (bot + recordings + timeline + future AI notes).
- [ ] Update feature grid: emphasize **auto-join bot**, **live timeline** (join/leave, screen share), **recording library**; mark AI summaries/todos as “coming soon” or soften if still mock-backed.
- [ ] Replace `app.notetaker.io/dashboard` mock URL with production hostname (e.g. `app.meetops.pro`).
- [ ] Pricing section: keep structure; add “MeetOps” in plan copy if needed.
- [ ] CTA section: “Get started on MeetOps.Pro”.
- [ ] Optional: add link to live demo / docs when available.

### Layout & auth UI

- [ ] `MarketingLayout.tsx` — nav logo text, footer copyright `© {year} MeetOps.Pro`.
- [ ] `AppShell.tsx` — sidebar brand `MeetOps` (optional `.Pro` on marketing only).
- [ ] `JoinAdHocMeetingDialog.tsx` — “notetaker bot” → “MeetOps bot”.
- [ ] Login/Signup pages — product name in heading if present.

### Metadata & assets

- [ ] `apps/frontend/index.html` — `<title>`, `<meta name="description">`, theme-color.
- [ ] Add `public/og-image.png` or SVG + meta tags when asset exists.
- [ ] `public/favicon.svg` — update to “M” / MeetOps mark if desired.

### Backend / ops (production readiness)

- [ ] `apps/backend/config.toml.example` — comment block for production:
  - `redirect_uri = "https://api.meetops.pro/auth/google/callback"` (or same host as API)
  - `frontend_redirect_url = "https://app.meetops.pro/app/calendars"` (or `https://meetops.pro/app/...`)
  - `GMR_GRPC__ALLOW_ORIGIN=https://meetops.pro,https://app.meetops.pro`
- [ ] Google Cloud Console — authorized redirect URIs + JavaScript origins for MeetOps.Pro hosts.
- [ ] S3 CORS (ROADMAP) — allow `GET` from MeetOps.Pro origins for playback.
- [ ] Docker Compose / deploy notes — env file template `config.meetops.pro.toml` (optional).

### Documentation

- [ ] `README.md` — product name MeetOps.Pro; repo name can stay `ai-meeting-notetaker-rs`.
- [ ] `docs/ROADMAP.md` — title “MeetOps.Pro” or subtitle; production checklist references domain.
- [ ] `apps/frontend/README.md` — dev URLs unchanged for localhost.

### Verification checklist

- [ ] `bun run build` (frontend) passes.
- [ ] `cargo test` (backend) passes.
- [ ] Grep repo for stale user-facing strings: `Notetaker`, `notetaker.io`, `AI Meeting Notetaker` (exclude `notetaker.v1` proto paths and package names unless intentionally renamed).
- [ ] Manual: open `/`, `/login`, `/app` — consistent MeetOps branding.
- [ ] Staging deploy with production OAuth redirect URIs tested once DNS is live.

---

## Phases

### Phase 1 — User-visible rebrand (1–2 days)

Landing page + marketing/app shell + `index.html` metadata. No infra changes.

### Phase 2 — Copy & honesty pass (0.5 day)

Align landing claims with ROADMAP (calendar providers, AI notes, live preview). Remove or qualify unimplemented features.

### Phase 3 — Production domain wiring (1 day, blocked on DNS/TLS)

OAuth, CORS, env examples, deploy doc. Requires DNS for `meetops.pro` / `app.meetops.pro` and TLS certs.

### Phase 4 — Optional deep rename (later)

Package/proto/repo rename if you want full alignment with MeetOps brand in code.

---

## Dependencies

- DNS control for `meetops.pro` (and subdomains).
- Google OAuth client updated with new redirect URIs.
- Decision: single host vs `app.` subdomain for SPA + API.

## Risks

| Risk | Mitigation |
|------|------------|
| OAuth break after URI change | Update Console + env together; test on staging subdomain first |
| Landing promises unshipped features | Phase 2 copy audit against ROADMAP |
| Split API/app origins | Document `ALLOW_ORIGIN` and vite proxy for grpc-web |

---

## Acceptance criteria

1. Landing page and marketing footer show **MeetOps.Pro** / **MeetOps**, not “AI Meeting Notetaker” or generic “Notetaker”.
2. Logged-in app shell shows **MeetOps** branding.
3. Browser tab title and meta description reference MeetOps.Pro.
4. Production config examples document `meetops.pro` URLs.
5. No misleading “multi-calendar” claims beyond what the app actually supports (at minimum: Google calendar connected).

---

## Key files

| File | Purpose |
|------|---------|
| `apps/frontend/src/pages/LandingPage.tsx` | Marketing home |
| `apps/frontend/src/components/layout/MarketingLayout.tsx` | Nav + footer |
| `apps/frontend/src/components/layout/AppShell.tsx` | App sidebar brand |
| `apps/frontend/index.html` | Title / meta |
| `apps/backend/config.toml.example` | OAuth + CORS examples |
| `README.md`, `docs/ROADMAP.md` | Docs |
