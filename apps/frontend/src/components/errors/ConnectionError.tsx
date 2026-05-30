import {
  AlertCircle,
  AlertTriangle,
  Ban,
  Link2Off,
  RefreshCw,
  ServerCrash,
  ShieldAlert,
  X,
  type LucideIcon,
} from "lucide-react"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import type { CalendarConnectError, CalendarConnectErrorCode } from "@/types"
import { cn } from "@/lib/utils"

const errorIcons: Record<CalendarConnectErrorCode, LucideIcon> = {
  already_connected_by_other_user: ShieldAlert,
  already_connected_by_you: Link2Off,
  oauth_denied: Ban,
  oauth_failed: AlertCircle,
  invalid_account: AlertTriangle,
  provider_unavailable: ServerCrash,
  sync_failed: RefreshCw,
}

function errorVariant(code: CalendarConnectErrorCode): "destructive" | "default" {
  if (
    code === "already_connected_by_other_user" ||
    code === "oauth_failed" ||
    code === "provider_unavailable"
  ) {
    return "destructive"
  }
  return "default"
}

type ConnectionErrorAlertProps = {
  error: CalendarConnectError
  onDismiss?: () => void
  action?: React.ReactNode
  className?: string
}

export function ConnectionErrorAlert({
  error,
  onDismiss,
  action,
  className,
}: ConnectionErrorAlertProps) {
  const Icon = errorIcons[error.code]

  return (
    <Alert variant={errorVariant(error.code)} className={cn("relative", className)}>
      <Icon />
      <AlertTitle>{error.title}</AlertTitle>
      <AlertDescription className="space-y-2">
        <p>{error.message}</p>
        {error.otherAccountHint && (
          <p className="text-xs opacity-80">
            Linked to another account{error.otherAccountHint ? ` (${error.otherAccountHint})` : ""}.
          </p>
        )}
        {(action || onDismiss) && (
          <div className="flex flex-wrap gap-2 pt-1">
            {action}
            {onDismiss && (
              <Button variant="ghost" size="xs" onClick={onDismiss}>
                Dismiss
              </Button>
            )}
          </div>
        )}
      </AlertDescription>
      {onDismiss && (
        <Button
          variant="ghost"
          size="icon-xs"
          className="absolute top-2 right-2"
          onClick={onDismiss}
          aria-label="Dismiss"
        >
          <X />
        </Button>
      )}
    </Alert>
  )
}

type ConnectionErrorDialogProps = {
  error: CalendarConnectError | null
  open: boolean
  onOpenChange: (open: boolean) => void
  onRetry?: () => void
  onTryAnotherAccount?: () => void
}

export function ConnectionErrorDialog({
  error,
  open,
  onOpenChange,
  onRetry,
  onTryAnotherAccount,
}: ConnectionErrorDialogProps) {
  if (!error) return null

  const Icon = errorIcons[error.code]
  const isOtherUser = error.code === "already_connected_by_other_user"
  const isAlreadyYours = error.code === "already_connected_by_you"
  const canRetry =
    error.code === "oauth_failed" ||
    error.code === "provider_unavailable" ||
    error.code === "sync_failed"

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <div
            className={cn(
              "mb-2 flex size-11 items-center justify-center rounded-full",
              isOtherUser || error.code === "oauth_failed"
                ? "bg-destructive/10 text-destructive"
                : "bg-muted text-muted-foreground",
            )}
          >
            <Icon className="size-5" />
          </div>
          <DialogTitle>{error.title}</DialogTitle>
          <DialogDescription className="text-left leading-relaxed">
            {error.message}
          </DialogDescription>
        </DialogHeader>

        {error.email && (
          <div className="bg-muted rounded-lg px-3 py-2 font-mono text-sm">{error.email}</div>
        )}

        {isOtherUser && (
          <p className="text-muted-foreground bg-muted/50 rounded-lg px-3 py-2 text-sm leading-relaxed">
            Ask the other user to disconnect this calendar from their Notetaker settings, or contact
            support if you believe this is a mistake.
          </p>
        )}

        <DialogFooter className="flex-col gap-2 sm:flex-col">
          {isAlreadyYours && onOpenChange && (
            <Button className="w-full" onClick={() => onOpenChange(false)}>
              Got it
            </Button>
          )}
          {isOtherUser && onTryAnotherAccount && (
            <Button className="w-full" onClick={onTryAnotherAccount}>
              Try another account
            </Button>
          )}
          {canRetry && onRetry && (
            <Button className="w-full" onClick={onRetry}>
              Try again
            </Button>
          )}
          {!isAlreadyYours && (
            <Button variant="outline" className="w-full" onClick={() => onOpenChange(false)}>
              Close
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
