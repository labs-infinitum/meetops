# MeetOps.Pro — Product Vision & Marketing

**Domain:** [meetops.pro](https://meetops.pro)  
**Product name:** MeetOps (UI) · MeetOps.Pro (legal, marketing)  
**Internal repo:** `ai-meeting-notetaker-rs` (may differ from public brand)

This document is the **complete product vision** — what MeetOps.Pro is meant to become, who it serves, and how it wins. For engineering execution order, see [ROADMAP.md](./ROADMAP.md). For rebrand implementation details, see [tasks/meetops-pro-rebrand.md](./tasks/meetops-pro-rebrand.md).

---

## One-line pitch

**MeetOps.Pro turns every meeting into finished work** — it records your calls, understands what happened, and automates the follow-up: tasks in your project tools, draft messages ready to send, and context synced to the systems your team already uses.

---

## Taglines (working)

| Context | Copy |
|---------|------|
| Primary | **Meeting ops, automated.** |
| Hero (long) | Never take meeting notes again — and never lose the follow-up. |
| Sub-hero | Auto-records your calls, extracts decisions and action items, and pushes them into your workflow. |
| Developer angle | From standup to shipped — meeting intelligence that connects to your repo. |
| Ops angle | The system of record for what was decided in the room. |

---

## The problem

Meetings create work, but the work stays trapped in people's heads, scattered notes, or half-finished Slack threads.

**After every call, someone has to:**

1. Remember what was said
2. Write a summary
3. Extract action items
4. Create tasks in Linear, Jira, GitHub, or Notion
5. Draft follow-up emails or Slack recaps
6. Make sure nothing falls through the cracks

This is repetitive, error-prone, and almost never done consistently. Important decisions get lost. Action items never become tickets. Follow-ups are delayed or forgotten.

**Existing tools solve half the problem:**

- **Notetakers** (Otter, Fireflies, Granola) capture and summarize — but stop at the note
- **Task managers** (Linear, Asana) track work — but don't know what happened in the meeting
- **Automation tools** (Zapier) connect systems — but need manual triggers and brittle rules

**MeetOps.Pro closes the loop:** capture → understand → act.

---

## Vision

MeetOps.Pro is the **operating system for meeting follow-up**.

Not just a notetaker. Not just a task manager. A platform that:

1. **Joins and records** your video meetings automatically
2. **Understands** what happened — decisions, owners, deadlines, blockers
3. **Automates workflows** after every call — creating drafts and syncing to your tools
4. **Keeps context alive** — searchable history tied to meetings, people, and projects

The north star user experience:

> Connect your calendar once. After every meeting, open MeetOps and find a summary, extracted todos, draft follow-up messages, and tasks already staged in your tools — ready for one-click approval.

---

## Positioning

### Category

**Meeting operations platform** — sits between meeting intelligence (notetakers) and work management (PM tools).

### What we are

- An **auto-join recording bot** for Google Meet (expand to Zoom, Teams)
- An **AI meeting analyst** — transcripts, summaries, action items, decisions
- A **workflow engine** — post-meeting automations to task systems, messaging, and dev tools
- A **meeting memory** — searchable library with timeline, playback, and linked artifacts

### What we are not

- A video conferencing platform (we join existing calls)
- A replacement for Linear/Jira/Notion (we push into them)
- A generic Zapier clone (workflows are meeting-native, not generic triggers)
- A passive transcript dump (we produce structured, actionable output)

### Positioning statement

For **teams that live in meetings and ship in tools**, MeetOps.Pro is the **meeting ops platform** that **automates everything after the call** — unlike standalone notetakers that stop at summaries, MeetOps connects meeting intelligence directly to your task systems, repos, and communication channels.

---

## Target users

### Primary personas

| Persona | Role | Pain | MeetOps value |
|---------|------|------|---------------|
| **Engineering lead** | Runs standups, planning, reviews | Decisions don't become tickets; PRs discussed in calls get forgotten | Auto-create GitHub issues, link PRs mentioned, draft standup recap |
| **Product manager** | Customer calls, roadmap syncs | Scattered notes; stakeholders miss context | Structured summaries, action items with owners, Notion export |
| **Founder / operator** | Back-to-back calls all day | No time to follow up; things slip | One dashboard for open todos across all meetings |
| **Sales / CS** | Discovery and account calls | CRM updates lag; next steps unclear | Draft follow-up emails, extract commitments, CRM sync (future) |

### Ideal customer profile (ICP)

- **Size:** 5–100 person teams (Starter → Pro sweet spot)
- **Meeting load:** 10+ hours/week in video calls
- **Stack:** Google Workspace, Google Meet, Linear or GitHub, Slack
- **Behavior:** Already tries to take notes; frustrated when follow-up doesn't happen
- **Buyer:** Team lead or ops-minded IC who feels the follow-up pain

### Secondary ICP (Enterprise)

- Regulated industries needing retention controls
- IT teams wanting on-prem or SSO
- Orgs with 100+ concurrent meetings (multi-tenant bot fleet)

---

## Core product pillars

### 1. Capture — never miss a meeting

The bot joins your calls so you don't have to think about recording.

- **Multi-calendar sync** — Google Calendar today; Outlook, Apple, CalDAV planned
- **Auto-join bot** — joins before meeting start (configurable lead time)
- **Graceful admission** — handles waiting rooms, host approval, lobby states
- **Ad-hoc recording** — paste a Meet link and start immediately
- **Recording library** — status tracking (queued → recording → completed → failed)
- **Live timeline** — join/leave, screen share, active speaker, snapshots during the call
- **Live preview** — see what the bot sees while recording (planned)
- **Playback** — full recording with presigned S3 streaming

### 2. Understand — AI meeting intelligence

Turn raw conversation into structured knowledge.

- **Transcription** — full transcript with timestamps and speaker segments
- **TL;DR summary** — 2–3 sentence executive summary
- **Detailed notes** — markdown summary with sections (decisions, discussion, blockers)
- **Action items** — extracted todos with suggested assignee and due date
- **Decision log** — explicit decisions surfaced from conversation
- **Meeting timeline correlation** — link transcript moments to join/leave and screen share events
- **Search** — find meetings, notes, and todos by keyword, person, or date
- **Tags & organization** — auto-tag by calendar, project, or custom rules (future)

### 3. Act — post-meeting workflows

The differentiator. Automate what happens after the call ends.

**Workflow triggers:**

- Recording completed
- Summary ready
- Action items extracted
- Manual trigger from meeting detail page

**Workflow actions:**

| Action | Description |
|--------|-------------|
| **Create tasks** | Push action items to Linear, Jira, Asana, GitHub Issues, Notion database |
| **Draft messages** | Generate follow-up email, Slack recap, or stakeholder update — stored as draft for review |
| **Sync to GitHub** | Create issues from discussed bugs/features; link mentioned PRs; optional branch/commit context |
| **Post to Slack** | Send summary + todos to a channel or DM |
| **Export to Notion** | Create a page or append to a database with summary and action items |
| **Webhook** | Generic HTTP POST for Zapier, Make, or custom integrations |
| **Notify** | Email or Slack alert when summary is ready or workflow completes |

**Workflow principles:**

- **Draft-first by default** — nothing sends or creates without user review unless explicitly configured
- **Meeting-native context** — every action carries meeting title, date, attendees, and link back to recording
- **Per-calendar or global rules** — "Engineering standups → Linear team X" vs "Customer calls → Notion CRM database"
- **Audit trail** — every workflow run logged with status, errors, and created artifacts

### 4. Operate — team visibility and control

One place to see meeting ops across the org.

- **Dashboard** — upcoming meetings, recent recordings, open todos, latest summary
- **Unified todo inbox** — all action items across meetings; complete, assign, set due dates
- **Notes library** — browse summaries and transcripts by meeting
- **Settings** — profile, recording prefs, notification prefs, integration connections
- **Team sharing** (Pro) — shared recordings and notes within workspace
- **Retention policies** (Enterprise) — configurable recording and transcript retention

---

## User journey

### First-time setup (< 5 minutes)

1. Sign up at meetops.pro
2. Connect Google Calendar
3. Enable auto-record (default on for new synced meetings)
4. Optional: connect Linear, Slack, or GitHub for workflows
5. Invite the bot's Google account to meetings (or use workspace bot user)

### Every meeting (automatic)

1. Calendar sync detects meeting with video link
2. Scheduler queues bot run ~60s before start
3. Bot joins Google Meet, handles admission, starts recording
4. Timeline events collected (joins, screen share, speakers)
5. Meeting ends → bot stops, uploads recording to S3
6. AI pipeline: transcribe → summarize → extract action items
7. Configured workflows run (create drafts, push tasks, notify)
8. User gets notification: "Sprint planning summary ready — 4 action items, 2 drafts"

### After the meeting (review & approve)

1. Open dashboard or notification link
2. Read TL;DR and full summary
3. Review extracted todos — edit, assign, complete, or push to tool
4. Review draft Slack message or follow-up email — edit and send
5. Approve staged Linear/GitHub issues — one click to create
6. Search later: "What did we decide about onboarding v2?"

---

## Feature vision by area

### Calendars & meetings

| Feature | Status (vision) | Notes |
|---------|-----------------|-------|
| Google Calendar OAuth + sync | **Shipped** | Production-ready |
| Outlook / Microsoft 365 | Planned | Microsoft Graph |
| Apple Calendar / CalDAV | Planned | Lower priority |
| Per-meeting auto-record toggle | **Shipped** | |
| Multi-calendar per user | **Shipped** | Google only today |
| Meeting detail with attendees | **Shipped** | |
| Ad-hoc join by URL | **Shipped** | |

### Recording & bot

| Feature | Status (vision) | Notes |
|---------|-----------------|-------|
| Google Meet auto-join | **Shipped** | |
| Tab capture (audio + video) | **Shipped** | |
| S3 storage + playback | **Shipped** | |
| Meeting timeline (join/leave, screen share) | **Shipped** | |
| Live preview (bot view) | Planned | CDP screenshots |
| Concurrent multi-meeting recording | Planned | Isolated Xvfb + Chrome profiles |
| Automated bot Google sign-in | Planned | No manual CLI profile step |
| Zoom / Teams support | Future | Platform expansion |
| Bot display name customization | Planned | Settings API |

### AI & notes

| Feature | Status (vision) | Notes |
|---------|-----------------|-------|
| Transcription (Groq Whisper) | Planned | Schema + config exist |
| TL;DR + markdown summary | Planned | |
| Action item extraction | Planned | |
| Notes library UI | UI exists | Mock data today |
| Todos UI | UI exists | Mock data today |
| Transcript viewer with timestamps | Planned | |
| Search across notes | Future | |
| Custom summary templates | Future | Per team or calendar |
| Speaker diarization | Future | Depends on model quality |

### Workflows & integrations

| Feature | Status (vision) | Notes |
|---------|-----------------|-------|
| Workflow engine (triggers + actions) | Future | Core differentiator |
| Slack — post summary / notify | Planned | Webhook first |
| Linear — create issues from todos | Planned | First PM integration |
| GitHub Issues — create from action items | Planned | Dev team focus |
| GitHub — link PRs mentioned in transcript | Future | |
| Notion — export page/database row | Planned | Pro tier feature |
| Draft follow-up email | Planned | LLM-generated, user approves |
| Draft Slack recap | Planned | |
| Generic webhook | Planned | Zapier/Make escape hatch |
| Jira, Asana, HubSpot | Future | By demand |

### Platform & enterprise

| Feature | Status (vision) | Notes |
|---------|-----------------|-------|
| Email/password auth | **Shipped** | |
| Settings (profile, prefs) | Planned | Schema exists |
| Team workspaces | Future | Pro tier |
| SSO / SAML | Future | Enterprise |
| On-prem deployment | Future | Enterprise |
| Custom retention | Future | Enterprise |
| API access | Future | Pro+ |
| Audit logs | Future | Enterprise |

---

## Integrations roadmap

### Tier 1 — launch workflows (MVP integrations)

1. **Slack** — notifications + draft recap posts
2. **Linear** — create issues from action items
3. **Webhook** — generic automation

### Tier 2 — expand reach

4. **GitHub Issues** — dev team task creation
5. **Notion** — page/database export
6. **Email (SMTP)** — summary ready + draft follow-up delivery

### Tier 3 — depth

7. **Jira** — enterprise PM
8. **Asana** — general PM
9. **GitHub** — PR linking, repo context, optional code-aware summaries
10. **HubSpot / Salesforce** — sales meeting follow-up (CRM sync)

### Integration design principles

- OAuth where possible; API keys as fallback
- Minimal scopes — only what's needed for the action
- User connects integrations in Settings; admin controls for teams (future)
- Every created artifact links back to the MeetOps meeting record
- Failed integrations surface clearly on recording detail + operator alerts

---

## Workflow examples

### Engineering standup

**Trigger:** Summary ready  
**Actions:**

1. Extract todos → create draft Linear issues in "Engineering" team
2. Generate Slack recap → post to `#eng-standup` as draft for review
3. If PR number mentioned → add comment linking to meeting summary

### Sprint planning

**Trigger:** Summary ready  
**Actions:**

1. Extract commitments → create Linear cycle issues with labels
2. Export full summary to Notion "Sprint Notes" database
3. Notify PM via Slack DM with TL;DR

### Customer discovery call

**Trigger:** Summary ready  
**Actions:**

1. Generate follow-up email draft ("Thanks for your time…")
2. Extract feature requests → tag and store in MeetOps; optional Notion CRM row
3. Notify CS lead

### Ad-hoc bug triage

**Trigger:** Manual — user clicks "Run workflows" on recording  
**Actions:**

1. Create GitHub issue in `org/repo` with repro steps from transcript
2. Draft Slack message for `#bugs` channel

---

## Pricing & packaging (vision)

| Plan | Price | Target | Includes |
|------|-------|--------|----------|
| **Starter** | Free | Individuals | 1 calendar, 10 recordings/mo, AI summaries, basic todos in-app |
| **Pro** | $19/mo | Professionals & small teams | Unlimited calendars & recordings, priority transcription, team sharing, integrations (Notion, Slack, Linear, GitHub), workflows |
| **Enterprise** | Custom | Organizations | SSO/SAML, custom retention, dedicated support, on-prem option, SLA |

### Packaging principles

- **Free tier** proves the loop: record → summarize → todos in-app
- **Pro unlocks automation** — integrations and workflows are the upgrade driver
- **Enterprise** sells compliance, scale, and control — not features individuals need

### Usage limits (Starter)

- 10 recordings/month
- 1 calendar connection
- In-app todos only (no external sync)
- 7-day recording retention (optional constraint)

---

## Competitive landscape

| Competitor | Strength | Gap MeetOps fills |
|------------|----------|-------------------|
| **Otter.ai** | Transcription, live notes | No workflow automation; weak task tool sync |
| **Fireflies.ai** | Summaries, CRM integrations | Integrations are shallow; not dev-tool native |
| **Granola** | Beautiful AI notes | No bot recording; manual meeting presence |
| **Fathom** | Free bot, good summaries | Limited workflow automation |
| **Linear / Notion AI** | Native AI in tools | Don't join and record meetings |
| **Zapier** | Connect anything | Not meeting-aware; manual setup |

### MeetOps differentiation

1. **Meeting-native workflows** — triggers and actions understand meeting context out of the box
2. **Dev-tool first** — GitHub and Linear as first-class citizens, not afterthoughts
3. **Draft-first automation** — powerful but safe; user approves before anything sends
4. **Open, self-hostable path** — Docker, Rust backend; on-prem for Enterprise
5. **Rich timeline** — not just transcript; who joined, who presented, when

---

## Brand voice & messaging

### Voice

- **Direct** — say what it does, not buzzwords
- **Confident** — we solve a real, daily pain
- **Technical when needed** — dev teams respect honesty about how the bot works
- **Human** — meetings are about people; automation serves them, not replaces them

### Words we use

- Meeting ops, follow-up, action items, workflows, automate, draft, sync, capture, timeline
- "After every call" / "when the meeting ends"
- Bot, recording, summary, todos

### Words we avoid

- "Revolutionary," "game-changing," "synergy"
- Overclaiming AI (" understands everything perfectly")
- Implying we replace PM tools or video platforms

### Honesty rules (marketing ↔ product)

- Do not claim calendar providers until backend ships
- Do not claim AI summaries on landing until pipeline is live (mark "coming soon" or ship first)
- Do not claim integrations until OAuth + action is implemented
- Under-promise on timeline; over-deliver on reliability

---

## Go-to-market phases

Aligned with [ROADMAP.md](./ROADMAP.md) engineering phases.

### Phase A — Credible notetaker (now → ~8 weeks)

**Message:** "Auto-records Google Meet and gives you AI summaries with action items."

- Ship AI pipeline (transcribe → summarize → todos)
- Wire Notes/Todos/Dashboard to real data
- MeetOps.Pro rebrand + production deploy
- Google Calendar only; honest landing page

**Goal:** First 50 users complete the full loop without mock data.

### Phase B — Meeting ops (8 → 16 weeks)

**Message:** "Your meetings create tasks automatically."

- Workflow engine v1
- Slack notifications + draft recaps
- Linear integration (create issues from todos)
- Webhook for power users

**Goal:** Pro tier launch; users run at least one workflow after every meeting.

### Phase C — Platform (16 → 32 weeks)

**Message:** "The system of record for meeting follow-up."

- GitHub Issues + Notion export
- Team workspaces
- Outlook calendar
- Concurrent recording
- Search across notes

**Goal:** Team adoption; 3+ integrations connected per Pro user.

### Phase D — Enterprise (32+ weeks)

**Message:** "Meeting ops at scale, on your terms."

- SSO/SAML
- On-prem
- CRM integrations
- Custom retention and audit

---

## Success metrics

### Activation

- Calendar connected within 24h of signup
- First recording completed within 7 days
- First summary viewed within 24h of recording

### Engagement

- Recordings per user per week
- Action items completed in-app vs pushed to tools
- Workflows run per recording (Phase B+)
- Draft messages approved and sent (Phase B+)

### Retention

- Week-4 retention (users with ≥1 recording in week 4)
- Integration connection rate (Pro)
- Upgrade rate Starter → Pro

### Quality

- Transcription accuracy (sampled review)
- Action item precision (user edits before accept)
- Bot join success rate
- Workflow success rate

---

## Key messages by channel

### Landing page hero

> **Never take meeting notes again.**  
> MeetOps.Pro auto-records your Google Meet calls, extracts action items with AI, and automates follow-up — tasks in Linear, drafts in Slack, context in Notion.

### App store / directory listing

> MeetOps — meeting ops for teams. Auto-join bot, AI summaries, workflow automation to your task tools.

### Developer community (Hacker News, Twitter)

> We built an open-source Rust bot that joins Google Meet, records calls, and pushes standup todos to Linear. Self-hostable with Docker.

### Sales one-pager (Enterprise)

> MeetOps.Pro gives your organization a auditable system of record for meeting decisions — with retention controls, SSO, and on-prem deployment.

---

## Appendix: current vs vision gap

For internal planning — what exists in the repo today vs this vision.

| Area | Today | Vision |
|------|-------|--------|
| Recording bot | ✅ Google Meet, S3, timeline | + live preview, concurrency, Zoom |
| Calendar | ✅ Google only | + Outlook, Apple |
| AI pipeline | ❌ Schema only | Full transcribe → summarize → todos |
| Notes/Todos UI | ⚠️ Mock data | Real API-backed |
| Workflows | ❌ Not started | Core product differentiator |
| Integrations | ❌ Marketing only | Slack, Linear, GitHub, Notion, webhook |
| Settings | ⚠️ Demo UI | Full API + integration connections |
| Production | ⚠️ Docker local | meetops.pro HTTPS deploy |

---

## Related documents

- [ROADMAP.md](./ROADMAP.md) — engineering execution order
- [tasks/meetops-pro-rebrand.md](./tasks/meetops-pro-rebrand.md) — brand implementation checklist
- [audit.md](../audit.md) — reliability and security findings to address before scale
- [README.md](../README.md) — developer setup and architecture

---

*Last updated: 2026-06-03*
