import { useEffect, useState } from "react"
import { Link } from "react-router-dom"
import { AlertCircle, FileText, ListChecks, Loader2, Mic2, Sparkles } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Separator } from "@/components/ui/separator"
import { PageHeader } from "@/components/shared/PageHeader"
import { EmptyState } from "@/components/shared/EmptyState"
import { formatDate } from "@/lib/format"
import { useAuth } from "@/lib/auth-context"
import {
  apiGetGeneratedNote,
  apiListGeneratedNotes,
  recordingErrorMessage,
} from "@/lib/recording-api"
import type { GeneratedNote } from "@/types"
import { toast } from "sonner"

function statusVariant(status?: string): "default" | "secondary" | "destructive" | "outline" {
  switch (status) {
    case "completed":
    case "cleaned":
      return "default"
    case "failed":
      return "destructive"
    case "processing":
    case "transcribed":
      return "secondary"
    default:
      return "outline"
  }
}

function noteSummary(note: GeneratedNote): string {
  if (note.tldr) return note.tldr
  if (note.error) return note.error
  if (note.summaryStatus === "processing" || note.transcriptStatus === "processing") {
    return "AI notes are still processing. The page will show the transcript and summary when ready."
  }
  if (note.cleanedTranscript) return "Cleaned transcript is ready. Meeting notes are still pending."
  if (note.rawTranscript) return "Raw transcription is ready. Speaker cleanup and notes are still pending."
  return "Transcription has been queued for this recording."
}

function renderMarkdownLite(markdown: string) {
  return markdown.split("\n").map((line, i) => {
    if (line.startsWith("## ")) {
      return (
        <h2 key={i} className="mt-6 mb-2 text-lg font-semibold first:mt-0">
          {line.slice(3)}
        </h2>
      )
    }
    if (line.startsWith("### ")) {
      return (
        <h3 key={i} className="mt-4 mb-1 text-base font-medium">
          {line.slice(4)}
        </h3>
      )
    }
    if (line.startsWith("- ")) {
      return (
        <p key={i} className="text-muted-foreground ml-4 text-sm">
          • {line.slice(2)}
        </p>
      )
    }
    if (/^\d+\./.test(line)) {
      return (
        <p key={i} className="text-muted-foreground ml-4 text-sm">
          {line}
        </p>
      )
    }
    if (line.trim() === "") return <br key={i} />
    return (
      <p key={i} className="text-muted-foreground text-sm leading-relaxed">
        {line}
      </p>
    )
  })
}

export function NotesPage() {
  const { session } = useAuth()
  const [notes, setNotes] = useState<GeneratedNote[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    if (!session?.token) return
    apiListGeneratedNotes(session.token)
      .then(setNotes)
      .catch((error) => toast.error(recordingErrorMessage(error)))
      .finally(() => setLoading(false))
  }, [session?.token])

  return (
    <div className="space-y-8">
      <PageHeader
        title="Notes"
        description="AI-generated summaries and transcripts from your meetings."
      />

      {loading ? (
        <div className="text-muted-foreground flex items-center gap-2 text-sm">
          <Loader2 className="size-4 animate-spin" />
          Loading AI notes…
        </div>
      ) : notes.length === 0 ? (
        <EmptyState
          icon={FileText}
          title="No AI notes yet"
          description="Completed recordings will appear here once transcription starts."
          action={
            <Button asChild>
              <Link to="/app/recordings">View recordings</Link>
            </Button>
          }
        />
      ) : (
        <div className="grid gap-4">
          {notes.map((note) => (
            <Card key={note.transcriptId} className="transition-shadow hover:shadow-md">
              <CardHeader className="pb-3">
                <div className="flex flex-wrap items-start justify-between gap-2">
                  <div>
                    <CardTitle className="text-base">
                      <Link to={`/app/notes/${note.transcriptId}`} className="hover:underline">
                        {note.meetingTitle}
                      </Link>
                    </CardTitle>
                    <CardDescription>{formatDate(note.recordedAt)}</CardDescription>
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    <Badge variant={statusVariant(note.transcriptStatus)} className="text-xs">
                      Transcript {note.transcriptStatus}
                    </Badge>
                    {note.summaryStatus && (
                      <Badge variant={statusVariant(note.summaryStatus)} className="text-xs">
                        Notes {note.summaryStatus}
                      </Badge>
                    )}
                  </div>
                </div>
              </CardHeader>
              <CardContent>
                <p className="text-muted-foreground text-sm leading-relaxed">{noteSummary(note)}</p>
                {note.actionItems.length > 0 && (
                  <p className="text-muted-foreground mt-3 flex items-center gap-1.5 text-xs">
                    <ListChecks className="size-3.5" />
                    {note.actionItems.length} action item{note.actionItems.length !== 1 ? "s" : ""}
                  </p>
                )}
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  )
}

export function NoteDetailPage({ noteId }: { noteId: string }) {
  const { session } = useAuth()
  const [note, setNote] = useState<GeneratedNote | null>(null)
  const [loading, setLoading] = useState(true)
  const [notFound, setNotFound] = useState(false)
  const transcriptId = Number(noteId)
  const invalidNoteId = Number.isNaN(transcriptId)

  useEffect(() => {
    if (!session?.token || invalidNoteId) return
    apiGetGeneratedNote(session.token, transcriptId)
      .then((loaded) => {
        setNote(loaded)
        setNotFound(false)
      })
      .catch((error) => {
        setNotFound(true)
        toast.error(recordingErrorMessage(error))
      })
      .finally(() => setLoading(false))
  }, [session?.token, transcriptId, invalidNoteId])

  if (!invalidNoteId && loading) {
    return (
      <div className="text-muted-foreground flex items-center gap-2 text-sm">
        <Loader2 className="size-4 animate-spin" />
        Loading AI note…
      </div>
    )
  }

  if (invalidNoteId || notFound || !note) {
    return (
      <div className="space-y-4">
        <PageHeader title="Note not found" />
        <Button asChild variant="outline">
          <Link to="/app/notes">Back to notes</Link>
        </Button>
      </div>
    )
  }

  return (
    <div className="space-y-8">
      <PageHeader
        title={note.meetingTitle}
        description={`Recorded ${formatDate(note.recordedAt)}`}
        action={
          <Button variant="outline" asChild>
            <Link to={`/app/recordings/${note.botRunId}`}>View recording</Link>
          </Button>
        }
      />

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardContent className="p-4">
            <p className="text-muted-foreground text-xs">Transcript</p>
            <Badge className="mt-2" variant={statusVariant(note.transcriptStatus)}>
              {note.transcriptStatus}
            </Badge>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <p className="text-muted-foreground text-xs">Notes</p>
            <Badge className="mt-2" variant={statusVariant(note.summaryStatus)}>
              {note.summaryStatus || "pending"}
            </Badge>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <p className="text-muted-foreground text-xs">Action items</p>
            <p className="mt-2 text-2xl font-semibold">{note.actionItems.length}</p>
          </CardContent>
        </Card>
      </div>

      {note.error && (
        <Card className="border-destructive/30 bg-destructive/5">
          <CardContent className="flex gap-3 p-4">
            <AlertCircle className="text-destructive mt-0.5 size-4 shrink-0" />
            <p className="text-destructive text-sm">{note.error}</p>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <Sparkles className="size-4" />
            TL;DR
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="leading-relaxed">
            {note.tldr || "TL;DR will appear here after meeting notes are generated."}
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Full summary</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="prose prose-sm dark:prose-invert max-w-none">
            {note.summaryMd ? (
              renderMarkdownLite(note.summaryMd)
            ) : (
              <p className="text-muted-foreground text-sm">
                Summary is not available yet.
              </p>
            )}
          </div>
        </CardContent>
      </Card>

      {note.actionItems.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <ListChecks className="size-4" />
              Action items
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {note.actionItems.map((item, i) => (
              <div key={`${item.text}-${i}`}>
                {i > 0 && <Separator className="mb-3" />}
                <div className="flex items-start justify-between gap-4">
                  <p className="text-sm">{item.text}</p>
                  {item.owner && (
                    <Badge variant="secondary" className="shrink-0 text-xs">
                      {item.owner}
                    </Badge>
                  )}
                </div>
              </div>
            ))}
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <Mic2 className="size-4" />
            Speaker-attributed transcript
          </CardTitle>
          <CardDescription>
            Cleaned transcript generated from audio plus active-speaker timeline events.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {note.cleanedTranscript ? (
            <pre className="bg-muted text-muted-foreground max-h-[520px] overflow-auto rounded-lg border p-4 whitespace-pre-wrap text-sm leading-relaxed">
              {note.cleanedTranscript}
            </pre>
          ) : (
            <p className="text-muted-foreground text-sm">
              Cleaned transcript is not available yet.
            </p>
          )}
        </CardContent>
      </Card>

      {note.rawTranscript && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Raw transcription</CardTitle>
            <CardDescription>Original speech-to-text output before speaker cleanup.</CardDescription>
          </CardHeader>
          <CardContent>
            <pre className="bg-muted text-muted-foreground max-h-80 overflow-auto rounded-lg border p-4 whitespace-pre-wrap text-xs leading-relaxed">
              {note.rawTranscript}
            </pre>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
