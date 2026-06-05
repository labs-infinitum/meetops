// Static fixtures for Notes/Todos until those APIs exist.

import type { ActionItem, Note } from "@/types"

const now = Math.floor(Date.now() / 1000)
const hour = 3600
const day = 86400

const actionItems: ActionItem[] = [
  {
    id: "ai-1",
    text: "Share updated Q2 roadmap doc with stakeholders",
    assignee: "Alex Chen",
    dueDate: now + 2 * day,
    completed: false,
    meetingId: "evt-past-1",
    meetingTitle: "Sprint planning — Cycle 14",
  },
  {
    id: "ai-2",
    text: "Schedule follow-up with design on onboarding v2",
    assignee: "Sarah Kim",
    dueDate: now + day,
    completed: false,
    meetingId: "evt-past-1",
    meetingTitle: "Sprint planning — Cycle 14",
  },
  {
    id: "ai-3",
    text: "Send customer interview summary to product channel",
    assignee: "Alex Chen",
    completed: true,
    meetingId: "evt-past-2",
    meetingTitle: "Customer feedback session",
  },
  {
    id: "ai-4",
    text: "Fix flaky CI pipeline before next release",
    assignee: "Mike Torres",
    dueDate: now + 3 * day,
    completed: false,
    meetingId: "evt-5",
    meetingTitle: "Engineering standup",
  },
  {
    id: "ai-5",
    text: "Review PR #847 — auth middleware refactor",
    assignee: "Alex Chen",
    dueDate: now + day,
    completed: false,
    meetingId: "evt-5",
    meetingTitle: "Engineering standup",
  },
]

export const mockActionItems = actionItems

export const mockNotes: Note[] = [
  {
    id: "note-1",
    meetingId: "evt-past-1",
    meetingTitle: "Sprint planning — Cycle 14",
    recordedAt: now - 2 * day,
    tldr: "Team committed to shipping onboarding v2 and API v3 by end of cycle. Design review moved to Thursday.",
    summaryMd: `## Sprint planning — Cycle 14

### Key decisions
- **Onboarding v2** is the top priority for this cycle
- API v3 beta launch targeted for May 15
- Design review rescheduled to Thursday 2pm

### Discussion highlights
- Sarah presented updated wireframes for the onboarding flow
- Mike raised concerns about CI stability — agreed to dedicate 2 days to fixes
- Team velocity is up 12% from last cycle

### Next steps
- Alex to share roadmap doc with stakeholders
- Sarah to schedule design follow-up
- Mike to investigate flaky test suite`,
    actionItems: actionItems.filter((a) => a.meetingId === "evt-past-1"),
    tags: ["planning", "product", "sprint"],
  },
  {
    id: "note-2",
    meetingId: "evt-past-2",
    meetingTitle: "Customer feedback session",
    recordedAt: now - 3 * day,
    tldr: "Three enterprise customers want better calendar integration and faster transcript turnaround.",
    summaryMd: `## Customer feedback session

### Customer pain points
1. Calendar sync delays cause missed recordings
2. Transcript processing takes too long (>10 min)
3. Action items aren't easily exportable to task tools

### Positive feedback
- Summary quality rated 4.5/5 across participants
- Auto-join bot reliability praised by 2 of 3 customers

### Follow-ups
- Prioritize multi-calendar support in Q2
- Investigate faster transcription pipeline`,
    actionItems: actionItems.filter((a) => a.meetingId === "evt-past-2"),
    tags: ["customers", "feedback"],
  },
  {
    id: "note-3",
    meetingId: "evt-5",
    meetingTitle: "Engineering standup",
    recordedAt: now - 2 * hour,
    tldr: "CI fixes in progress. Auth middleware PR ready for review. No blockers reported.",
    summaryMd: `## Engineering standup

- **Mike**: Working on CI pipeline — 3 flaky tests identified
- **Alex**: Auth middleware refactor PR #847 ready for review
- **Sarah**: Onboarding prototype feedback incorporated

No blockers. Next standup tomorrow 9am.`,
    actionItems: actionItems.filter((a) => a.meetingId === "evt-5"),
    tags: ["engineering", "standup"],
  },
]
