import { useState } from "react"
import { Link2, Plus } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { PageHeader } from "@/components/shared/PageHeader"
import {
  ConnectedCalendarCard,
  ConnectedCalendarsSummary,
} from "@/components/calendars/ConnectedCalendarCard"
import { ConnectCalendarDialog } from "@/components/calendars/ConnectCalendarDialog"
import { ProviderIcon, providerMeta } from "@/components/calendars/ProviderIcon"
import {
  ConnectionErrorAlert,
  ConnectionErrorDialog,
} from "@/components/errors/ConnectionError"
import { mockCalendars } from "@/lib/mock-data"
import { groupCalendarsByProvider } from "@/lib/calendar-connect"
import { useAuth } from "@/lib/auth-context"
import type { CalendarConnectError, CalendarConnection, CalendarProvider } from "@/types"
import { toast } from "sonner"

const addProviders: CalendarProvider[] = ["google", "outlook", "apple"]

function providerSectionTitle(provider: CalendarProvider, count: number) {
  const name = providerMeta[provider].name
  if (provider === "google" && count > 1) {
    return `${name} · ${count} accounts`
  }
  return name
}

export function CalendarsPage() {
  const { session } = useAuth()
  const [calendars, setCalendars] = useState(mockCalendars)
  const [syncing, setSyncing] = useState<string | null>(null)
  const [connectProvider, setConnectProvider] = useState<CalendarProvider | null>(null)
  const [dialogError, setDialogError] = useState<CalendarConnectError | null>(null)
  const [errorDialogOpen, setErrorDialogOpen] = useState(false)
  const [pageError, setPageError] = useState<CalendarConnectError | null>(null)

  const grouped = groupCalendarsByProvider(calendars)
  const connectedCount = calendars.filter((c) => c.connected).length

  function openConnect(provider: CalendarProvider) {
    setPageError(null)
    setConnectProvider(provider)
  }

  function handleConnectSuccess(connection: CalendarConnection) {
    setCalendars((prev) => [...prev, connection])
    toast.success(`${connection.email} connected`, {
      description: "Meetings from this calendar will appear alongside your other calendars.",
    })
  }

  function handleConnectError(error: CalendarConnectError) {
    if (
      error.code === "already_connected_by_other_user" ||
      error.code === "oauth_failed"
    ) {
      setDialogError(error)
      setErrorDialogOpen(true)
    }
  }

  async function handleSync(id: string) {
    setSyncing(id)
    await new Promise((r) => setTimeout(r, 1200))
    setCalendars((prev) =>
      prev.map((c) =>
        c.id === id ? { ...c, lastSyncedAt: Math.floor(Date.now() / 1000) } : c,
      ),
    )
    setSyncing(null)
    toast.success("Calendar synced")
  }

  function handleDisconnect(cal: CalendarConnection) {
    setCalendars((prev) => prev.filter((c) => c.id !== cal.id))
    toast.info(`Disconnected ${cal.email}`)
  }

  return (
    <div className="space-y-8">
      <PageHeader
        title="Calendars"
        description="Connect multiple accounts — including several Google emails — and sync all meetings in one place."
      />

      {pageError && (
        <ConnectionErrorAlert
          error={pageError}
          onDismiss={() => setPageError(null)}
        />
      )}

      {connectedCount > 0 && (
        <>
          <ConnectedCalendarsSummary calendars={calendars} />

          <section className="space-y-6">
            <h2 className="text-sm font-medium">Connected calendars</h2>

            {grouped.google.length > 0 && (
              <CalendarProviderSection
                title={providerSectionTitle("google", grouped.google.length)}
                description={
                  grouped.google.length > 1
                    ? "Each Google account syncs independently; meetings are merged in Meetings."
                    : undefined
                }
                action={
                  <Button variant="outline" size="sm" onClick={() => openConnect("google")}>
                    <Plus className="size-4" />
                    Add Google account
                  </Button>
                }
              >
                {grouped.google.map((cal) => (
                  <ConnectedCalendarCard
                    key={cal.id}
                    calendar={cal}
                    syncing={syncing === cal.id}
                    onSync={() => handleSync(cal.id)}
                    onDisconnect={() => handleDisconnect(cal)}
                  />
                ))}
              </CalendarProviderSection>
            )}

            {grouped.outlook.length > 0 && (
              <CalendarProviderSection title={providerSectionTitle("outlook", grouped.outlook.length)}>
                {grouped.outlook.map((cal) => (
                  <ConnectedCalendarCard
                    key={cal.id}
                    calendar={cal}
                    syncing={syncing === cal.id}
                    onSync={() => handleSync(cal.id)}
                    onDisconnect={() => handleDisconnect(cal)}
                  />
                ))}
              </CalendarProviderSection>
            )}

            {grouped.apple.length > 0 && (
              <CalendarProviderSection title={providerSectionTitle("apple", grouped.apple.length)}>
                {grouped.apple.map((cal) => (
                  <ConnectedCalendarCard
                    key={cal.id}
                    calendar={cal}
                    syncing={syncing === cal.id}
                    onSync={() => handleSync(cal.id)}
                    onDisconnect={() => handleDisconnect(cal)}
                  />
                ))}
              </CalendarProviderSection>
            )}
          </section>
        </>
      )}

      <section className="space-y-4">
        <div>
          <h2 className="text-sm font-medium">Add a calendar</h2>
          <p className="text-muted-foreground mt-1 text-sm">
            Google supports multiple accounts per user. Outlook and Apple can be added the same way.
          </p>
        </div>
        <div className="grid gap-4 sm:grid-cols-3">
          {addProviders.map((provider) => {
            const meta = providerMeta[provider]
            const connectedForProvider =
              provider === "google"
                ? grouped.google.length
                : provider === "outlook"
                  ? grouped.outlook.length
                  : grouped.apple.length

            return (
              <Card key={provider}>
                <CardHeader>
                  <ProviderIcon provider={provider} />
                  <CardTitle className="mt-3 text-base">{meta.name}</CardTitle>
                  <CardDescription>{meta.description}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-3">
                  {connectedForProvider > 0 && (
                    <p className="text-muted-foreground text-xs">
                      {connectedForProvider} account{connectedForProvider !== 1 ? "s" : ""} connected
                    </p>
                  )}
                  <Button className="w-full" onClick={() => openConnect(provider)}>
                    <Link2 className="size-4" />
                    {provider === "google" && connectedForProvider > 0
                      ? "Add Google account"
                      : `Connect ${meta.name.split(" ")[0]}`}
                  </Button>
                </CardContent>
              </Card>
            )
          })}
        </div>
      </section>

      <ConnectCalendarDialog
        provider={connectProvider}
        open={connectProvider !== null}
        onOpenChange={(open) => {
          if (!open) setConnectProvider(null)
        }}
        existingCalendars={calendars}
        currentUserId={session?.user.id ?? "user-1"}
        onSuccess={handleConnectSuccess}
        onError={handleConnectError}
      />

      <ConnectionErrorDialog
        error={dialogError}
        open={errorDialogOpen}
        onOpenChange={setErrorDialogOpen}
        onTryAnotherAccount={() => {
          setErrorDialogOpen(false)
          setDialogError(null)
          if (dialogError?.provider) openConnect(dialogError.provider)
        }}
        onRetry={() => {
          setErrorDialogOpen(false)
          if (dialogError?.provider) openConnect(dialogError.provider)
        }}
      />
    </div>
  )
}

function CalendarProviderSection({
  title,
  description,
  action,
  children,
}: {
  title: string
  description?: string
  action?: React.ReactNode
  children: React.ReactNode
}) {
  return (
    <div className="space-y-3">
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h3 className="text-sm font-medium">{title}</h3>
          {description && (
            <p className="text-muted-foreground text-xs">{description}</p>
          )}
        </div>
        {action}
      </div>
      <div className="grid gap-4 sm:grid-cols-2">{children}</div>
    </div>
  )
}
