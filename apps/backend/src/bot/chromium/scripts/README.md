# Meet tab injection scripts

Each `.js` file is the **full function expression** passed to Chromium CDP `Page::evaluate_function` (e.g. `async function() { … }`). Rust embeds them at compile time via `include_str!` in `mod.rs`.

## Files

| File | When it runs |
|------|----------------|
| `attempt_join.js` | Poll loop while joining Meet (`__DISPLAY_NAME_JSON__` replaced with JSON string) |
| `install_recording_control.js` | Once before recording: MediaRecorder, timeline collector, hidden start button |
| `recording_status.js` | Poll `window.__gmrRecordingStatus` |
| `recording_runtime_status.js` | Poll status + upload counters |
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

## Participant list (`gmrListMeetParticipants`)

Queries `[role="listitem"][data-participant-id]` anywhere in the document (Meet’s participant roster, not only when the People side sheet is visibly open). In practice Meet usually keeps these nodes mounted while in a call, so a collapsed/hidden People UI still returns correct ids/names—matching manual testing.

Excludes **(You)** and lobby rows (**Admit** / **Deny** / **Ask to join**). Join/leave diffs run on each **video chunk (~1s)** and on a **5s** interval (snapshots + active speaker).

**Edge cases (rare):** false `participant_left` bursts are still possible if Meet unmounts roster nodes (navigation, reconnection, layout experiments) or the roster is empty before anyone else joins. No need to force-open the People panel for normal recordings.

## Global state (`window.__gmr*`)

Set by `install_recording_control.js`, read by other scripts in the **same tab**. Do not call `stop_tab_recording.js` without a prior successful install in that page.
