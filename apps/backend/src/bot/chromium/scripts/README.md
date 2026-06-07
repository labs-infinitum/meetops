# Meet tab injection scripts

Each `.js` file is the **full function expression** passed to Chromium CDP `Page::evaluate_function` (e.g. `async function() { … }`). Rust embeds them at compile time via `include_str!` in `mod.rs`.

## Files

| File | When it runs |
|------|----------------|
| `attempt_join.js` | Poll loop while joining Meet (`__DISPLAY_NAME_JSON__` replaced with JSON string) |
| `dismiss_meet_overlays.js` | Best-effort click on informational modals (`Got it`, `OK`, …) in dialogs or when known overlay copy is visible |
| `prime_participant_roster.js` | After host admits (up to ~20s): wait out lobby → open People → wait for roster → close |
| `install_recording_control.js` | Once before recording: MediaRecorder, timeline collector, hidden start button |
| `recording_runtime_status.js` | Poll `window.__gmrRecordingStatus` + upload counters |
| `meet_participant_count.js` | Solo-leave / admission checks from Rust (`recorder.rs`) |
| `stop_tab_recording.js` | Stop recorders, flush timeline, send `{ kind: 'done' }` |

## Validate syntax

```bash
node -e "
const fs = require('fs');
const dir = '.';
for (const f of fs.readdirSync(dir).filter((x) => x.endsWith('.js'))) {
  const src = fs.readFileSync(dir + '/' + f, 'utf8');
  new Function('return (' + src + ')');
  console.log('OK', f);
}
"
```

`cargo test` also runs `scripts::tests::all_scripts_are_valid_function_expressions`.

## Selector policy (Meet vs bot-owned)

| OK to use | Avoid |
|-----------|--------|
| ARIA roles/states: `role`, `aria-label`, `aria-labelledby`, `aria-expanded`, `aria-haspopup` | `jsname`, `jscontroller`, `jsaction` (obfuscated per build) |
| Stable Meet data attrs: `data-participant-id`, `data-requested-participant-id`, `data-tile-media-id` | Random `class` names, dynamic element `id` literals (e.g. `DPUxh-nav9Xe`) |
| Visible copy: “People”, “Show everyone”, “Leave call”, … | CSS module hashes / `class="JgybGf"` style tokens |
| Bot-owned: `#gmr-start-recording`, `window.__gmr*` | — |

Runtime `getElementById` is only used to resolve **whatever** id Meet puts in `aria-labelledby` at that moment—we never hardcode those ids in selectors.

## Participant list (`gmrListMeetParticipants`)

Queries `[role="listitem"][data-participant-id]` in the document. Meet must **hydrate the roster at least once** per call: `prime_participant_roster.js` runs after `attempt_join.js` reports `joined` (in-call, not lobby). It polls until the “Please wait until a meeting host…” lobby copy disappears, finds the People control via a **“People”** label / `aria-labelledby`, toggles open, waits for rows, toggles closed (budget **~20s**). After that, diffs work with the panel collapsed.

Excludes **(You)**, lobby rows (**Admit** / **Deny** / **Ask to join**), and **presentation** rows (subtitle “Presentation”, Safari “Visitor” + “Presentation”, or any control `aria-label` mentioning “presentation”, including “You can't unmute someone else's presentation”). Those rows emit **`screen_share_started`** / **`screen_share_stopped`**, not join/leave. Events set top-level **`participantId`** to the presenter’s person row when resolved (`presenterParticipantId` in payload); **`presentationId`** is the presentation listitem id. Duplicate names are disambiguated via Meet space prefix, roster avatar URL, then nearest person row above the presentation row. Join/leave diffs run on each **video chunk (~1s)** and on a **5s** interval. Participant snapshots run on the 5s interval.

**Edge cases:** if the People toggle cannot be found or `rosterCount` stays 0, join still succeeds but timeline participant events may be wrong until the panel is opened manually.

## Active speaker

`install_recording_control.js` detects active speaker changes from Meet tiles every **1s**. It locates tiles with `data-participant-id` + `data-tile-media-id`, then checks for the visible speaking-ring effect via computed `::before` / `::after` styles on large tile overlays. Meet renders that effect on at most one tile at a time, so the collector treats the first matching tile as the active speaker and records no speaker when no tile has the effect. This intentionally avoids hardcoding Meet's obfuscated speaker classes while still following the visual state Meet renders.

## Global state (`window.__gmr*`)

Set by `install_recording_control.js`, read by other scripts in the **same tab**. Do not call `stop_tab_recording.js` without a prior successful install in that page.
