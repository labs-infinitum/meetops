import { Link } from "react-router-dom"
import { FileText, ListChecks } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Separator } from "@/components/ui/separator"
import { PageHeader } from "@/components/shared/PageHeader"
import { mockNotes } from "@/lib/mock-data"
import { formatDate } from "@/lib/format"

export function NotesPage() {
  const notes = [...mockNotes].sort((a, b) => b.recordedAt - a.recordedAt)

  return (
    <div className="space-y-8">
      <PageHeader
        title="Notes"
        description="AI-generated summaries and transcripts from your meetings."
      />

      <div className="grid gap-4">
        {notes.map((note) => (
          <Card key={note.id} className="transition-shadow hover:shadow-md">
            <CardHeader className="pb-3">
              <div className="flex flex-wrap items-start justify-between gap-2">
                <div>
                  <CardTitle className="text-base">
                    <Link to={`/app/notes/${note.id}`} className="hover:underline">
                      {note.meetingTitle}
                    </Link>
                  </CardTitle>
                  <CardDescription>{formatDate(note.recordedAt)}</CardDescription>
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {note.tags.map((tag) => (
                    <Badge key={tag} variant="outline" className="text-xs">
                      {tag}
                    </Badge>
                  ))}
                </div>
              </div>
            </CardHeader>
            <CardContent>
              <p className="text-muted-foreground text-sm leading-relaxed">{note.tldr}</p>
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
    </div>
  )
}

export function NoteDetailPage({ noteId }: { noteId: string }) {
  const note = mockNotes.find((n) => n.id === noteId)

  if (!note) {
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
            <Link to="/app/todos">View todos</Link>
          </Button>
        }
      />

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <FileText className="size-4" />
            TL;DR
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="leading-relaxed">{note.tldr}</p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Full summary</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="prose prose-sm dark:prose-invert max-w-none">
            {note.summaryMd.split("\n").map((line, i) => {
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
            })}
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
              <div key={item.id}>
                {i > 0 && <Separator className="mb-3" />}
                <div className="flex items-start justify-between gap-4">
                  <p className={`text-sm ${item.completed ? "text-muted-foreground line-through" : ""}`}>
                    {item.text}
                  </p>
                  {item.assignee && (
                    <Badge variant="secondary" className="shrink-0 text-xs">
                      {item.assignee}
                    </Badge>
                  )}
                </div>
              </div>
            ))}
          </CardContent>
        </Card>
      )}
    </div>
  )
}
