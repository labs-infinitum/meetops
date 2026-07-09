import { useEffect, useState } from "react";
import { EmptyState } from "../components/EmptyState";
import { PageLoader, Spinner } from "../components/Spinner";
import { api } from "../lib/api";
import type { Todo } from "../types/api";

export function TodosPage() {
  const [todos, setTodos] = useState<Todo[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<"all" | "open" | "done">("all");
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

  const filtered = todos.filter((t) => {
    if (filter === "open") return !t.completed;
    if (filter === "done") return t.completed;
    return true;
  });

  const openCount = todos.filter((t) => !t.completed).length;

  return (
    <div>
      <h1 className="font-display text-2xl font-bold">Todos</h1>
      <p className="mt-1 text-sm text-surface-500">
        Action items from your meeting notes · {openCount} open
      </p>

      <div className="mt-6 flex gap-2">
        {(["all", "open", "done"] as const).map((f) => (
          <button
            key={f}
            type="button"
            onClick={() => setFilter(f)}
            className={`rounded-lg px-3 py-1.5 text-sm font-medium capitalize ${
              filter === f
                ? "bg-brand-600 text-white"
                : "bg-surface-100 text-surface-600 dark:bg-surface-800 dark:text-surface-400"
            }`}
          >
            {f}
          </button>
        ))}
      </div>

      {filtered.length === 0 ? (
        <div className="mt-8">
          <EmptyState
            title={filter === "done" ? "No completed todos" : "No action items yet"}
            description="Action items are extracted automatically from meeting notes."
          />
        </div>
      ) : (
        <ul className="mt-6 space-y-2">
          {filtered.map((todo) => (
            <li key={todo.id} className="card flex items-start gap-3">
              <button
                type="button"
                onClick={() => void toggleTodo(todo)}
                disabled={patching === todo.id}
                className={`mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded border transition-colors ${
                  todo.completed
                    ? "border-brand-600 bg-brand-600 text-white"
                    : "border-surface-300 dark:border-surface-600"
                }`}
                aria-label={todo.completed ? "Mark incomplete" : "Mark complete"}
              >
                {patching === todo.id ? (
                  <Spinner className="h-3 w-3" />
                ) : todo.completed ? (
                  "✓"
                ) : null}
              </button>
              <div className="flex-1">
                <p className={todo.completed ? "text-surface-400 line-through" : ""}>
                  {todo.text}
                </p>
                <p className="mt-1 text-xs text-surface-500">
                  {todo.owner && <span>{todo.owner} · </span>}
                  {todo.due && <span>Due {todo.due} · </span>}
                  Recording #{todo.bot_run_id}
                </p>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
