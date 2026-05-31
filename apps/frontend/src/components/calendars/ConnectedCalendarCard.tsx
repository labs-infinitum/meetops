import { Check, RefreshCw } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import type { CalendarConnection, CalendarProvider } from "@/types"
import { formatRelative } from "@/lib/format"
import { ProviderIcon } from "@/components/calendars/ProviderIcon"

type ConnectedCalendarCardProps = {
  calendar: CalendarConnection
  syncing: boolean
  onSync: () => void
  onDisconnect: () => void
}

export function ConnectedCalendarCard({
  calendar,
  syncing,
  onSync,
  onDisconnect,
}: ConnectedCalendarCardProps) {
  return (
    <Card>
      <CardHeader className="flex flex-row items-start gap-4 pb-3">
        <ProviderIcon provider={calendar.provider} color={calendar.color} />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <CardTitle className="text-base">{calendar.label}</CardTitle>
            <Badge variant="secondary" className="gap-1 text-xs">
              <Check className="size-3" />
              Connected
            </Badge>
          </div>
          <CardDescription>{calendar.email}</CardDescription>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="text-muted-foreground flex justify-between text-sm">
          <span>{calendar.meetingCount} meetings synced</span>
          {calendar.lastSyncedAt && (
            <span>Synced {formatRelative(calendar.lastSyncedAt)}</span>
          )}
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" disabled={syncing} onClick={onSync}>
            <RefreshCw className={syncing ? "animate-spin" : ""} />
            Sync now
          </Button>
          <Button variant="ghost" size="sm" onClick={onDisconnect}>
            Disconnect
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

export function ConnectedCalendarsSummary({
  calendars,
}: {
  calendars: CalendarConnection[]
}) {
  const connected = calendars.filter((c) => c.connected)
  const totalMeetings = connected.reduce((sum, c) => sum + c.meetingCount, 0)
  const byProvider = connected.reduce<Record<CalendarProvider, number>>(
    (acc, c) => {
      const provider = c.provider
      acc[provider] = (acc[provider] ?? 0) + 1
      return acc
    },
    {} as Record<CalendarProvider, number>,
  )

  const parts: string[] = []
  if (byProvider.google) parts.push(`${byProvider.google} Google`)
  if (byProvider.outlook) parts.push(`${byProvider.outlook} Outlook`)
  if (byProvider.apple) parts.push(`${byProvider.apple} Apple`)

  return (
    <div className="bg-muted/50 flex flex-col gap-1 rounded-lg border px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
      <div>
        <p className="text-sm font-medium">
          {connected.length} calendar{connected.length !== 1 ? "s" : ""} connected
        </p>
        <p className="text-muted-foreground text-xs">
          {parts.join(" · ") || "No providers"} — meetings sync together in one view
        </p>
      </div>
      <p className="text-muted-foreground text-sm">
        <span className="text-foreground font-semibold">{totalMeetings}</span> meetings total
      </p>
    </div>
  )
}
