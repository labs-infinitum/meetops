import { Calendar } from "lucide-react"
import type { CalendarProvider } from "@/types"

export const providerMeta: Record<
  CalendarProvider,
  { name: string; description: string; icon: string; brandColor: string }
> = {
  google: {
    name: "Google Calendar",
    description: "Connect a Google Workspace or personal account. Add multiple Google emails — all meetings sync together.",
    icon: "G",
    brandColor: "#4285F4",
  },
  outlook: {
    name: "Microsoft Outlook",
    description: "Connect Outlook or Microsoft 365 calendars",
    icon: "O",
    brandColor: "#0078D4",
  },
  apple: {
    name: "Apple Calendar",
    description: "Sync via iCloud CalDAV",
    icon: "",
    brandColor: "#FF2D55",
  },
  caldav: {
    name: "CalDAV",
    description: "Generic CalDAV server",
    icon: "C",
    brandColor: "#6B7280",
  },
}

export function ProviderIcon({
  provider,
  color,
  size = "md",
}: {
  provider: CalendarProvider
  color?: string
  size?: "sm" | "md"
}) {
  const meta = providerMeta[provider]
  const dim = size === "sm" ? "size-8 text-xs" : "size-10 text-sm"

  return (
    <div
      className={`flex ${dim} shrink-0 items-center justify-center rounded-lg font-bold text-white`}
      style={{ backgroundColor: color ?? meta.brandColor }}
    >
      {provider === "apple" ? (
        <Calendar className={size === "sm" ? "size-4" : "size-5"} />
      ) : (
        meta.icon
      )}
    </div>
  )
}
