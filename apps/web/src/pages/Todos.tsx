import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { CheckSquare } from "lucide-react";
import { EmptyState } from "@/components/EmptyState";
import { PageLoader } from "@/components/Spinner";
import { PageHeader } from "@/components/shared/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { api } from "@/lib/api";
import type { Todo } from "@/types/api";

export function TodosPage() {
  const [todos, setTodos] = useState<Todo[]>([]);
  const [loading, setLoading] = useState(true);
  const [patching, setPatching] = useState<string | null>(null);

  useEffect(() => {
    void api.get<Todo[]>("/todos/").then((data) => {
      setTodos(data);
      setLoading(false);
    });
  }, []);

  async function toggleTodo(todo: Todo) {
    setPatching(todo.id);
    try {
      const updated = await api.patch<Todo>(`/todos/${todo.id}`, {
        completed: !todo.completed,
      });
      setTodos((prev) => prev.map((t) => (t.id === updated.id ? updated : t)));
    } finally {
      setPatching(null);
    }
  }

  if (loading) return <PageLoader />;

  const open = todos.filter((t) => !t.completed);
  const done = todos.filter((t) => t.completed);

  return (
    <div className="space-y-8">
      <PageHeader
        title="Todos"
        description="Action items extracted from your meeting notes."
      />

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
            <TodoList items={open} patching={patching} onToggle={toggleTodo} />
          )}
        </TabsContent>

        <TabsContent value="done" className="mt-4">
          {done.length === 0 ? (
            <p className="text-muted-foreground text-sm">No completed items yet.</p>
          ) : (
            <TodoList items={done} patching={patching} onToggle={toggleTodo} />
          )}
        </TabsContent>
      </Tabs>
    </div>
  );
}

function TodoList({
  items,
  patching,
  onToggle,
}: {
  items: Todo[];
  patching: string | null;
  onToggle: (todo: Todo) => void;
}) {
  return (
    <div className="space-y-2">
      {items.map((todo) => (
        <Card key={todo.id}>
          <CardContent className="flex items-start gap-4 p-4">
            <Checkbox
              checked={todo.completed}
              disabled={patching === todo.id}
              onCheckedChange={() => void onToggle(todo)}
              className="mt-0.5"
            />
            <div className="min-w-0 flex-1">
              <p
                className={`text-sm font-medium ${todo.completed ? "text-muted-foreground line-through" : ""}`}
              >
                {todo.text}
              </p>
              <div className="mt-2 flex flex-wrap items-center gap-2">
                <Link
                  to={`/app/recordings/${todo.bot_run_id}`}
                  className="text-muted-foreground hover:text-foreground text-xs hover:underline"
                >
                  Recording #{todo.bot_run_id}
                </Link>
                {todo.owner && (
                  <Badge variant="outline" className="text-xs">
                    {todo.owner}
                  </Badge>
                )}
                {todo.due && (
                  <Badge variant="secondary" className="text-xs">
                    Due {todo.due}
                  </Badge>
                )}
              </div>
            </div>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}
