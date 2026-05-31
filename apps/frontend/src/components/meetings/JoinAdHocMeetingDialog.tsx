import { useState } from "react"
import { Loader2, Video } from "lucide-react"
import { useNavigate } from "react-router-dom"
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
import {
  apiStartAdHocRecording,
  RecordingApiError,
  recordingErrorMessage,
} from "@/lib/recording-api"
import { SESSION_EXPIRED_MESSAGE } from "@/lib/session-guard"
import { toast } from "sonner"

type JoinAdHocMeetingDialogProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
  sessionToken?: string
}

function isLikelyMeetUrl(value: string): boolean {
  const trimmed = value.trim()
  return (
    trimmed.includes("meet.google.com/") &&
    !trimmed.endsWith("meet.google.com/") &&
    !trimmed.endsWith("meet.google.com")
  )
}

export function JoinAdHocMeetingDialog({
  open,
  onOpenChange,
  sessionToken,
}: JoinAdHocMeetingDialogProps) {
  const navigate = useNavigate()
  const [meetUrl, setMeetUrl] = useState("")
  const [loading, setLoading] = useState(false)
  const [fieldError, setFieldError] = useState<string | null>(null)

  function resetForm() {
    setMeetUrl("")
    setFieldError(null)
    setLoading(false)
  }

  function handleOpenChange(next: boolean) {
    if (!next) resetForm()
    onOpenChange(next)
  }

  async function submit() {
    if (!sessionToken) {
      toast.error(SESSION_EXPIRED_MESSAGE)
      return
    }

    if (!isLikelyMeetUrl(meetUrl)) {
      setFieldError("Enter a Google Meet link like https://meet.google.com/abc-defg-hij")
      return
    }

    setLoading(true)
    setFieldError(null)

    try {
      const run = await apiStartAdHocRecording(sessionToken, meetUrl.trim())
      toast.success("Bot is joining the meeting")
      handleOpenChange(false)
      navigate(`/app/recordings/${run.id}`)
    } catch (error) {
      setLoading(false)
      if (error instanceof RecordingApiError && error.code === "invalid_meet_url") {
        setFieldError("That doesn't look like a valid Google Meet link.")
        return
      }
      toast.error(recordingErrorMessage(error))
    }
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Join meeting with bot</DialogTitle>
          <DialogDescription>
            Paste a Google Meet link that is not on your calendar. The notetaker bot will join
            immediately and start recording.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-2">
          <Label htmlFor="adhoc-meet-url">Meeting link</Label>
          <Input
            id="adhoc-meet-url"
            placeholder="https://meet.google.com/abc-defg-hij"
            value={meetUrl}
            onChange={(e) => {
              setMeetUrl(e.target.value)
              if (fieldError) setFieldError(null)
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !loading) submit()
            }}
            autoFocus
          />
          {fieldError && (
            <p className="text-destructive text-sm">{fieldError}</p>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => handleOpenChange(false)} disabled={loading}>
            Cancel
          </Button>
          <Button onClick={() => submit()} disabled={loading || !meetUrl.trim()}>
            {loading ? <Loader2 className="animate-spin" /> : <Video className="size-4" />}
            Send bot
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export function JoinAdHocMeetingButton({
  sessionToken,
  variant = "default",
}: {
  sessionToken?: string
  variant?: "default" | "outline"
}) {
  const [open, setOpen] = useState(false)

  return (
    <>
      <Button variant={variant} onClick={() => setOpen(true)}>
        <Video className="size-4" />
        Join unlisted meet
      </Button>
      <JoinAdHocMeetingDialog
        open={open}
        onOpenChange={setOpen}
        sessionToken={sessionToken}
      />
    </>
  )
}
