import { NavLink, Outlet, useNavigate } from "react-router-dom";
import { useAuth } from "../context/AuthProvider";
import { api } from "../lib/api";
import { Logo } from "./Logo";

const navItems = [
  { to: "/app", label: "Dashboard", end: true },
  { to: "/app/meetings", label: "Meetings" },
  { to: "/app/recordings", label: "Recordings" },
  { to: "/app/notes", label: "Notes" },
  { to: "/app/todos", label: "Todos" },
  { to: "/app/calendars", label: "Calendars" },
  { to: "/app/settings", label: "Settings" },
];

export function AppLayout() {
  const { user, setUser } = useAuth();
  const navigate = useNavigate();

  async function handleLogout() {
    try {
      await api.post("/auth/logout");
    } catch {
      // still clear local state
    }
    setUser(null);
    navigate("/login");
  }

  return (
    <div className="flex min-h-screen bg-surface-50 dark:bg-surface-950">
      <aside className="fixed inset-y-0 left-0 z-30 flex w-60 flex-col border-r border-surface-200 bg-white dark:border-surface-800 dark:bg-surface-900">
        <div className="border-b border-surface-200 px-5 py-4 dark:border-surface-800">
          <Logo className="text-surface-900 dark:text-white" />
        </div>
        <nav className="flex-1 space-y-1 px-3 py-4">
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.end}
              className={({ isActive }) =>
                `block rounded-lg px-3 py-2 text-sm font-medium transition-colors ${
                  isActive
                    ? "bg-brand-50 text-brand-700 dark:bg-brand-950 dark:text-brand-300"
                    : "text-surface-600 hover:bg-surface-100 dark:text-surface-400 dark:hover:bg-surface-800"
                }`
              }
            >
              {item.label}
            </NavLink>
          ))}
        </nav>
        <div className="border-t border-surface-200 px-4 py-4 dark:border-surface-800">
          <p className="truncate text-sm font-medium text-surface-900 dark:text-surface-100">
            {user?.name}
          </p>
          <p className="truncate text-xs text-surface-500">{user?.email}</p>
          <button type="button" onClick={() => void handleLogout()} className="btn-ghost mt-2 px-0">
            Sign out
          </button>
        </div>
      </aside>
      <main className="ml-60 flex-1">
        <div className="mx-auto max-w-5xl px-6 py-8">
          <Outlet />
        </div>
      </main>
    </div>
  );
}
