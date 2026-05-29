import type { CalendarConnectError, CalendarConnection, CalendarProvider } from "@/types"

/** Accounts already linked to other Notetaker users (mock platform registry). */
export const mockCalendarAccountClaims: Record<
  string,
  { userId: string; displayName: string }
> = {
  "sarah@acme.co": { userId: "user-2", displayName: "Sarah Kim" },
  "team@company.com": { userId: "user-3", displayName: "Mike Torres" },
  "founder@startup.io": { userId: "user-4", displayName: "Jordan Lee" },
}

const providerLabels: Record<CalendarProvider, string> = {
  google: "Google Calendar",
  outlook: "Microsoft Outlook",
  apple: "Apple Calendar",
  caldav: "CalDAV",
}

const googleColors = ["#4285F4", "#34A853", "#FBBC04", "#EA4335", "#1A73E8"]

export function nextGoogleColor(existing: CalendarConnection[]) {
  const used = new Set(existing.filter((c) => c.provider === "google").map((c) => c.color))
  return googleColors.find((c) => !used.has(c)) ?? googleColors[0]
}

export function normalizeEmail(email: string) {
  return email.trim().toLowerCase()
}

export function buildConnectError(
  code: CalendarConnectError["code"],
  provider: CalendarProvider,
  context?: { email?: string; otherAccountHint?: string },
): CalendarConnectError {
  const label = providerLabels[provider]
  const email = context?.email

  switch (code) {
    case "already_connected_by_other_user":
      return {
        code,
        provider,
        email,
        otherAccountHint: context?.otherAccountHint,
        title: "Calendar already linked",
        message: email
          ? `${email} is already connected to another Notetaker account. Each calendar can only be linked to one user. Disconnect it from the other account first, or sign in with that account instead.`
          : `This ${label} account is already connected to another Notetaker user.`,
      }
    case "already_connected_by_you":
      return {
        code,
        provider,
        email,
        title: "Already connected",
        message: email
          ? `${email} is already linked to your account. Meetings from this calendar are already syncing.`
          : `This ${label} account is already connected to your account.`,
      }
    case "oauth_denied":
      return {
        code,
        provider,
        title: "Connection cancelled",
        message: `You declined access to ${label}. Grant calendar permissions to sync your meetings.`,
      }
    case "oauth_failed":
      return {
        code,
        provider,
        title: "Connection failed",
        message: `We couldn't complete sign-in with ${label}. Check your connection and try again.`,
      }
    case "invalid_account":
      return {
        code,
        provider,
        email,
        title: "Invalid account",
        message: "Enter a valid email address for the calendar you want to connect.",
      }
    case "provider_unavailable":
      return {
        code,
        provider,
        title: `${label} unavailable`,
        message: `${label} is temporarily unavailable. Try again in a few minutes.`,
      }
    case "sync_failed":
      return {
        code,
        provider,
        email,
        title: "Sync failed",
        message: email
          ? `Connected ${email}, but the initial sync failed. Try syncing again from your calendar list.`
          : "The calendar connected, but the initial sync failed.",
      }
  }
}

export type ConnectCalendarInput = {
  provider: CalendarProvider
  email: string
  label?: string
}

export type ConnectCalendarResult =
  | { ok: true; connection: CalendarConnection }
  | { ok: false; error: CalendarConnectError }

export async function mockConnectCalendar(
  input: ConnectCalendarInput,
  existing: CalendarConnection[],
  currentUserId: string,
): Promise<ConnectCalendarResult> {
  await new Promise((r) => setTimeout(r, 900))

  const email = normalizeEmail(input.email)
  const { provider } = input

  if (!email || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
    return { ok: false, error: buildConnectError("invalid_account", provider, { email }) }
  }

  const alreadyMine = existing.find(
    (c) => c.connected && c.provider === provider && normalizeEmail(c.email) === email,
  )
  if (alreadyMine) {
    return {
      ok: false,
      error: buildConnectError("already_connected_by_you", provider, { email }),
    }
  }

  const claim = mockCalendarAccountClaims[email]
  if (claim && claim.userId !== currentUserId) {
    return {
      ok: false,
      error: buildConnectError("already_connected_by_other_user", provider, {
        email,
        otherAccountHint: claim.displayName,
      }),
    }
  }

  const color =
    provider === "google"
      ? nextGoogleColor(existing)
      : provider === "outlook"
        ? "#0078D4"
        : provider === "apple"
          ? "#FF2D55"
          : "#6B7280"

  const connection: CalendarConnection = {
    id: `cal-${Date.now()}`,
    provider,
    email,
    label: input.label?.trim() || email.split("@")[0],
    connected: true,
    lastSyncedAt: Math.floor(Date.now() / 1000),
    meetingCount: Math.floor(Math.random() * 12) + 3,
    color,
  }

  return { ok: true, connection }
}

export function groupCalendarsByProvider(calendars: CalendarConnection[]) {
  const connected = calendars.filter((c) => c.connected)
  return {
    google: connected.filter((c) => c.provider === "google"),
    outlook: connected.filter((c) => c.provider === "outlook"),
    apple: connected.filter((c) => c.provider === "apple"),
    caldav: connected.filter((c) => c.provider === "caldav"),
  }
}

export function totalMeetingsAcrossCalendars(calendars: CalendarConnection[]) {
  return calendars.filter((c) => c.connected).reduce((sum, c) => sum + c.meetingCount, 0)
}
