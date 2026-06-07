import { useEffect, useState } from "react"
import { Link } from "react-router-dom"
import { CheckSquare, Loader2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { PageHeader } from "@/components/shared/PageHeader"
import { EmptyState } from "@/components/shared/EmptyState"
import { useAuth } from "@/lib/auth-context"
import { apiListGeneratedNotes, recordingErrorMessage } from "@/lib/recording-api"
import { toast } from "sonner"

type TodoItem = {
  id: string
  text: string
  owner?: string
  due?: string
  completed: boolean
  transcriptId: number
  meetingTitle: string
}

export function TodosPage() {
  const { session } = useAuth()
  const [items, setItems] = useState<TodoItem[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    if (!session?.token) return
    apiListGeneratedNotes(session.token, 100)
      .then((notes) => {
        setItems(
          notes.flatMap((note) =>
            note.actionItems.map((item, index) => ({
              id: `${note.transcriptId}:${index}:${item.text}`,
              text: item.text,
              owner: item.owner,
              due: item.due,
              completed: false,
              transcriptId: note.transcriptId,
              meetingTitle: note.meetingTitle,
            })),
          ),
        )
      })
      .catch((error) => toast.error(recordingErrorMessage(error)))
      .finally(() => setLoading(false))
  }, [session?.token])

  function toggleItem(id: string) {
    setItems((prev) =>
      prev.map((item) =>
        item.id === id ? { ...item, completed: !item.completed } : item,
      ),
    )
  }

  const open = items.filter((i) => !i.completed)
  const done = items.filter((i) => i.completed)

  return (
    <div className="space-y-8">
      <PageHeader
        title="Todos"
        description="Action items extracted from your meeting notes."
      />

      {loading ? (
        <div className="text-muted-foreground flex items-center gap-2 text-sm">
          <Loader2 className="size-4 animate-spin" />
          Loading action items…
        </div>
      ) : (
      <Tabs defaultValue="open">
        <TabsList>
          <TabsTrigger value="open">Open ({open.length})</TabsTrigger>
          <TabsTrigger value="done">Completed ({done.length})</TabsTrigger>
        </TabsList>

        <TabsContent value="open" className="mt-4">
          {open.length === 0 ? (
            <EmptyState
              icon={CheckSquare}
              title="All caught up"
              description="No open action items. Great work!"
              action={
                <Button variant="outline" asChild>
                  <Link to="/app/notes">Browse notes</Link>
                </Button>
              }
            />
          ) : (
            <TodoList items={open} onToggle={toggleItem} />
          )}
        </TabsContent>

        <TabsContent value="done" className="mt-4">
          {done.length === 0 ? (
            <p className="text-muted-foreground text-sm">No completed items yet.</p>
          ) : (
            <TodoList items={done} onToggle={toggleItem} />
          )}
        </TabsContent>
      </Tabs>
      )}
    </div>
  )
}

function TodoList({
  items,
  onToggle,
}: {
  items: TodoItem[]
  onToggle: (id: string) => void
}) {
  return (
    <div className="space-y-2">
      {items.map((item) => (
        <Card key={item.id}>
          <CardContent className="flex items-start gap-4 p-4">
            <Checkbox
              checked={item.completed}
              onCheckedChange={() => {
                onToggle(item.id)
                toast.success(item.completed ? "Marked as open" : "Marked as done")
              }}
              className="mt-0.5"
            />
            <div className="min-w-0 flex-1">
              <p
                className={`text-sm font-medium ${item.completed ? "text-muted-foreground line-through" : ""}`}
              >
                {item.text}
              </p>
              <div className="mt-2 flex flex-wrap items-center gap-2">
                <Link
                  to={`/app/notes/${item.transcriptId}`}
                  className="text-muted-foreground hover:text-foreground text-xs hover:underline"
                >
                  {item.meetingTitle}
                </Link>
                {item.owner && (
                  <Badge variant="outline" className="text-xs">
                    {item.owner}
                  </Badge>
                )}
                {item.due && (
                  <Badge variant="secondary" className="text-xs">
                    Due {item.due}
                  </Badge>
                )}
              </div>
            </div>
          </CardContent>
        </Card>
      ))}
    </div>
  )
}
