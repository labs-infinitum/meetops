import type {
  ActionItem,
  BotRun,
  CalendarConnection,
  Meeting,
  Note,
  User,
} from "@/types"

const now = Math.floor(Date.now() / 1000)
const hour = 3600
const day = 86400

export const mockUser: User = {
  id: "user-1",
  name: "Alex Chen",
  email: "alex@acme.co",
  username: "alex",
}

export const mockCalendars: CalendarConnection[] = [
  {
    id: "cal-1",
    provider: "google",
    email: "alex@acme.co",
    label: "Work Calendar",
    connected: true,
    lastSyncedAt: now - 120,
    meetingCount: 24,
    color: "#4285F4",
  },
  {
    id: "cal-2",
    provider: "google",
    email: "alex.personal@gmail.com",
    label: "Personal",
    connected: true,
    lastSyncedAt: now - 900,
    meetingCount: 8,
    color: "#34A853",
  },
]

export const mockMeetings: Meeting[] = [
  {
    id: "evt-1",
    title: "Product sync — Q2 roadmap",
    meetUrl: "https://meet.google.com/abc-defg-hij",
    startAt: now + hour,
    endAt: now + hour + 1800,
    autoRecord: true,
    calendarId: "cal-1",
    attendees: ["alex@acme.co", "sarah@acme.co", "mike@acme.co"],
    description: "Weekly product alignment on Q2 deliverables.",
  },
  {
    id: "evt-2",
    title: "Design review: onboarding flow",
    meetUrl: "https://meet.google.com/klm-nopq-rst",
    startAt: now + 3 * hour,
    endAt: now + 3 * hour + 3600,
    autoRecord: true,
    calendarId: "cal-1",
    attendees: ["alex@acme.co", "jess@acme.co"],
  },
  {
    id: "evt-3",
    title: "1:1 with Sarah",
    meetUrl: "https://meet.google.com/uvw-xyz-abc",
    startAt: now + day,
    endAt: now + day + 1800,
    autoRecord: false,
    calendarId: "cal-1",
    attendees: ["alex@acme.co", "sarah@acme.co"],
  },
  {
    id: "evt-4",
    title: "Investor update prep",
    meetUrl: "https://meet.google.com/def-ghi-jkl",
    startAt: now + 2 * day,
    endAt: now + 2 * day + 3600,
    autoRecord: true,
    calendarId: "cal-2",
    attendees: ["alex.personal@gmail.com", "founder@startup.io"],
  },
  {
    id: "evt-5",
    title: "Engineering standup",
    meetUrl: "https://meet.google.com/mno-pqr-stu",
    startAt: now - 2 * hour,
    endAt: now - hour,
    autoRecord: true,
    calendarId: "cal-1",
    attendees: ["alex@acme.co", "dev-team@acme.co"],
  },
]

export const mockBotRuns: BotRun[] = [
  {
    id: 42,
    meetingId: "evt-past-1",
    meetingTitle: "Sprint planning — Cycle 14",
    status: "completed",
    meetingStartAt: now - 2 * day,
    startedAt: now - 2 * day + 30,
    endedAt: now - 2 * day + 3930,
    createdAt: now - 2 * day - 60,
    recordingPath: "recordings/run-42/raw.webm",
    durationSecs: 3900,
  },
  {
    id: 41,
    meetingId: "evt-past-2",
    meetingTitle: "Customer feedback session",
    status: "completed",
    meetingStartAt: now - 3 * day,
    startedAt: now - 3 * day + 45,
    endedAt: now - 3 * day + 2745,
    createdAt: now - 3 * day - 60,
    recordingPath: "recordings/run-41/raw.webm",
    durationSecs: 2700,
  },
  {
    id: 40,
    meetingId: "evt-5",
    meetingTitle: "Engineering standup",
    status: "completed",
    meetingStartAt: now - 2 * hour,
    startedAt: now - 2 * hour + 20,
    endedAt: now - hour - 60,
    createdAt: now - 2 * hour - 120,
    recordingPath: "recordings/run-40/raw.webm",
    durationSecs: 3540,
  },
  {
    id: 39,
    meetingId: "evt-past-3",
    meetingTitle: "All-hands meeting",
    status: "failed",
    error: "Bot could not be admitted to the meeting within timeout",
    meetingStartAt: now - 5 * day,
    startedAt: now - 5 * day,
    endedAt: now - 5 * day + 300,
    createdAt: now - 5 * day - 60,
  },
  {
    id: 38,
    meetingId: "evt-past-4",
    meetingTitle: "Partnership discussion",
    status: "recording",
    meetingStartAt: now - 900,
    startedAt: now - 870,
    createdAt: now - 960,
    recordingPath: "recordings/run-38/raw.webm",
  },
]

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

export function getDashboardStats() {
  const upcoming = mockMeetings.filter((m) => m.startAt > now).length
  const recordings = mockBotRuns.filter((r) => r.status === "completed").length
  const openTodos = mockActionItems.filter((t) => !t.completed).length
  const connectedCalendars = mockCalendars.filter((c) => c.connected).length

  return { upcoming, recordings, openTodos, connectedCalendars }
}
