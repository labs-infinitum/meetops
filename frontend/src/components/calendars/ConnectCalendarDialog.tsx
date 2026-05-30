import { useState } from "react"
import { Loader2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ConnectionErrorAlert } from "@/components/errors/ConnectionError"
import { ProviderIcon, providerMeta } from "@/components/calendars/ProviderIcon"
import {
  apiStartGoogleOAuth,
  CalendarApiError,
  calendarErrorMessage,
} from "@/lib/calendar-api"
import { buildConnectError, nextGoogleColor } from "@/lib/calendar-connect"
import type { CalendarConnectError, CalendarConnection, CalendarProvider } from "@/types"

type ConnectCalendarDialogProps = {
  provider: CalendarProvider | null
  open: boolean
  onOpenChange: (open: boolean) => void
  existingCalendars: CalendarConnection[]
  sessionToken?: string
  onError: (error: CalendarConnectError) => void
}

export function ConnectCalendarDialog({
  provider,
  open,
  onOpenChange,
  existingCalendars,
  sessionToken,
  onError,
}: ConnectCalendarDialogProps) {
  const [label, setLabel] = useState("")
  const [loading, setLoading] = useState(false)
  const [inlineError, setInlineError] = useState<CalendarConnectError | null>(null)

  if (!provider) return null

  const activeProvider = provider
  const meta = providerMeta[activeProvider]
  const googleCount = existingCalendars.filter(
    (c) => c.connected && c.provider === "google",
  ).length

  function resetForm() {
    setLabel("")
    setInlineError(null)
    setLoading(false)
  }

  function handleOpenChange(next: boolean) {
    if (!next) resetForm()
    onOpenChange(next)
  }

  async function submit() {
    if (!sessionToken) {
      const error = buildConnectError("oauth_failed", activeProvider)
      setInlineError(error)
      onError(error)
      return
    }

    if (activeProvider !== "google") {
      const error = buildConnectError("provider_unavailable", activeProvider)
      setInlineError(error)
      onError(error)
      return
    }

    setLoading(true)
    setInlineError(null)

    try {
      const color = nextGoogleColor(existingCalendars)
      const redirectUrl = await apiStartGoogleOAuth(
        sessionToken,
        label.trim() || undefined,
        color,
      )
      window.location.href = redirectUrl
    } catch (error) {
      setLoading(false)
      const connectError =
        error instanceof CalendarApiError
          ? error.toConnectError()
          : buildConnectError("oauth_failed", activeProvider, {
              email: undefined,
            })
      connectError.message = calendarErrorMessage(error)
      setInlineError(connectError)
      onError(connectError)
    }
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <div className="flex items-center gap-3">
            <ProviderIcon provider={activeProvider} />
            <div>
              <DialogTitle>
                {activeProvider === "google" && googleCount > 0
                  ? "Add another Google account"
                  : `Connect ${meta.name}`}
              </DialogTitle>
              <DialogDescription>
                {activeProvider === "google"
                  ? "Sign in with Google to grant calendar access. You can connect multiple Google accounts."
                  : meta.description}
              </DialogDescription>
            </div>
          </div>
        </DialogHeader>

        <div className="space-y-4">
          {activeProvider === "google" && googleCount > 0 && (
            <p className="text-muted-foreground rounded-lg border border-dashed px-3 py-2 text-xs leading-relaxed">
              You have {googleCount} Google account{googleCount !== 1 ? "s" : ""} connected.
              Add another email to merge all meetings into your unified schedule.
            </p>
          )}

          {inlineError && (
            <ConnectionErrorAlert
              error={inlineError}
              onDismiss={() => setInlineError(null)}
            />
          )}

          {activeProvider === "google" && (
            <div className="space-y-2">
              <Label htmlFor="calendar-label">
                Display name <span className="text-muted-foreground font-normal">(optional)</span>
              </Label>
              <Input
                id="calendar-label"
                placeholder="Work, Personal, Client projects…"
                value={label}
                onChange={(e) => setLabel(e.target.value)}
              />
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => handleOpenChange(false)} disabled={loading}>
            Cancel
          </Button>
          <Button onClick={() => submit()} disabled={loading}>
            {loading && <Loader2 className="animate-spin" />}
            {activeProvider === "google" ? "Continue with Google" : "Connect"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
