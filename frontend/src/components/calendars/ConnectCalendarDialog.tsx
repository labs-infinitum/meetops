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
import type { CalendarConnectError, CalendarConnection, CalendarProvider } from "@/types"
import { mockConnectCalendar } from "@/lib/calendar-connect"
import { mockCalendarAccountClaims } from "@/lib/calendar-connect"

type ConnectCalendarDialogProps = {
  provider: CalendarProvider | null
  open: boolean
  onOpenChange: (open: boolean) => void
  existingCalendars: CalendarConnection[]
  currentUserId: string
  onSuccess: (connection: CalendarConnection) => void
  onError: (error: CalendarConnectError) => void
}

export function ConnectCalendarDialog({
  provider,
  open,
  onOpenChange,
  existingCalendars,
  currentUserId,
  onSuccess,
  onError,
}: ConnectCalendarDialogProps) {
  const [email, setEmail] = useState("")
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
    setEmail("")
    setLabel("")
    setInlineError(null)
    setLoading(false)
  }

  function handleOpenChange(next: boolean) {
    if (!next) resetForm()
    onOpenChange(next)
  }

  async function submit(overrideEmail?: string) {
    const targetEmail = overrideEmail ?? email
    setLoading(true)
    setInlineError(null)

    const result = await mockConnectCalendar(
      { provider: activeProvider, email: targetEmail, label: label || undefined },
      existingCalendars,
      currentUserId,
    )

    setLoading(false)

    if (result.ok) {
      onSuccess(result.connection)
      handleOpenChange(false)
    } else {
      setInlineError(result.error)
      onError(result.error)
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
              <DialogDescription>{meta.description}</DialogDescription>
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

          <div className="space-y-2">
            <Label htmlFor="calendar-email">Account email</Label>
            <Input
              id="calendar-email"
              type="email"
              placeholder={
                activeProvider === "google"
                  ? "you@company.com or personal@gmail.com"
                  : "you@company.com"
              }
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              autoComplete="email"
            />
          </div>

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

          <div className="rounded-lg border border-dashed p-3">
            <p className="text-muted-foreground mb-2 text-xs font-medium">Demo scenarios</p>
            <div className="flex flex-wrap gap-2">
              <Button
                type="button"
                variant="outline"
                size="xs"
                disabled={loading}
                onClick={() => submit("sarah@acme.co")}
              >
                Account taken by another user
              </Button>
              <Button
                type="button"
                variant="outline"
                size="xs"
                disabled={loading}
                onClick={() => {
                  const mine = existingCalendars.find(
                    (c) => c.connected && c.provider === activeProvider,
                  )
                  if (mine) submit(mine.email)
                  else
                    setInlineError({
                      code: "invalid_account",
                      provider: activeProvider,
                    title: "No existing account",
                    message: "Connect an account first to test the duplicate error.",
                  })
                }}
              >
                Already on your account
              </Button>
            </div>
            <p className="text-muted-foreground mt-2 text-[11px] leading-relaxed">
              Try <span className="font-mono">{Object.keys(mockCalendarAccountClaims)[0]}</span> to
              simulate another user&apos;s linked calendar.
            </p>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => handleOpenChange(false)} disabled={loading}>
            Cancel
          </Button>
          <Button onClick={() => submit()} disabled={loading || !email.trim()}>
            {loading && <Loader2 className="animate-spin" />}
            {activeProvider === "google" ? "Connect Google account" : "Connect"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
