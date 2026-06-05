import type { CalendarConnectError, CalendarConnection, CalendarProvider } from "@/types"

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
    default: {
      const _exhaustive: never = code
      return _exhaustive
    }
  }
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
