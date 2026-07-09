import { NavLink, Outlet, useNavigate } from "react-router-dom";
import {
  Calendar,
  CalendarDays,
  CheckSquare,
  FileText,
  LayoutDashboard,
  LogOut,
  Mic2,
  Settings,
  Video,
} from "lucide-react";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useAuth } from "@/context/AuthProvider";
import { api } from "@/lib/api";
import { cn } from "@/lib/utils";

const navItems = [
  { to: "/app", label: "Dashboard", icon: LayoutDashboard, end: true },
  { to: "/app/calendars", label: "Calendars", icon: Calendar },
  { to: "/app/meetings", label: "Meetings", icon: CalendarDays },
  { to: "/app/recordings", label: "Recordings", icon: Video },
  { to: "/app/notes", label: "Notes", icon: FileText },
  { to: "/app/todos", label: "Todos", icon: CheckSquare },
  { to: "/app/settings", label: "Settings", icon: Settings },
];

function SidebarNav({ onNavigate }: { onNavigate?: () => void }) {
  return (
    <nav className="space-y-1 px-3 py-4">
      {navItems.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          end={item.end}
          onClick={onNavigate}
          className={({ isActive }) =>
            cn(
              "flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-colors",
              isActive
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:bg-muted hover:text-foreground",
            )
          }
        >
          <item.icon className="size-4 shrink-0" />
          {item.label}
        </NavLink>
      ))}
    </nav>
  );
}

export function AppLayout() {
  const { user, setUser } = useAuth();
  const navigate = useNavigate();
  const initials = (user?.name ?? "?")
    .split(" ")
    .map((n) => n[0])
    .join("")
    .slice(0, 2)
    .toUpperCase();

  async function handleLogout() {
    try {
      await api.post("/auth/logout");
    } catch {
      // still clear local state
    }
    setUser(null);
    navigate("/");
  }

  return (
    <div className="bg-muted/30 flex h-svh overflow-hidden">
      <aside className="bg-background hidden h-svh w-64 shrink-0 flex-col border-r md:flex">
        <div className="flex h-16 shrink-0 items-center gap-2.5 border-b px-6">
          <div className="bg-primary flex size-8 items-center justify-center rounded-lg">
            <Mic2 className="text-primary-foreground size-4" />
          </div>
          <span className="font-semibold tracking-tight">Notetaker</span>
        </div>
        <ScrollArea className="min-h-0 flex-1">
          <SidebarNav />
        </ScrollArea>
        <div className="shrink-0 border-t p-3">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" className="h-auto w-full justify-start gap-3 px-3 py-2">
                <Avatar className="size-8">
                  <AvatarFallback className="text-xs">{initials}</AvatarFallback>
                </Avatar>
                <div className="min-w-0 text-left">
                  <p className="truncate text-sm font-medium">{user?.name}</p>
                  <p className="text-muted-foreground truncate text-xs">{user?.email}</p>
                </div>
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-56">
              <DropdownMenuLabel>My account</DropdownMenuLabel>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={() => navigate("/app/settings")}>
                <Settings className="size-4" />
                Settings
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={() => void handleLogout()}>
                <LogOut className="size-4" />
                Log out
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </aside>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <header className="bg-background flex h-16 shrink-0 items-center justify-between border-b px-4 md:hidden">
          <div className="flex items-center gap-2">
            <div className="bg-primary flex size-8 items-center justify-center rounded-lg">
              <Mic2 className="text-primary-foreground size-4" />
            </div>
            <span className="font-semibold">Notetaker</span>
          </div>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon">
                <Avatar className="size-8">
                  <AvatarFallback className="text-xs">{initials}</AvatarFallback>
                </Avatar>
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-56">
              <DropdownMenuLabel>{user?.name}</DropdownMenuLabel>
              <DropdownMenuSeparator />
              {navItems.map((item) => (
                <DropdownMenuItem key={item.to} onClick={() => navigate(item.to)}>
                  <item.icon className="size-4" />
                  {item.label}
                </DropdownMenuItem>
              ))}
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={() => void handleLogout()}>
                <LogOut className="size-4" />
                Log out
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </header>

        <main className="min-h-0 flex-1 overflow-y-auto">
          <div className="mx-auto max-w-6xl px-4 py-8 sm:px-6">
            <Outlet />
          </div>
        </main>
      </div>
    </div>
  );
}

export function AuthLayout() {
  return (
    <div className="flex min-h-svh">
      <AuthBrandPanel />
      <div className="flex flex-1 flex-col items-center justify-center px-4 py-12 sm:px-6">
        <div className="mb-8 flex items-center gap-2 lg:hidden">
          <div className="bg-primary flex size-8 items-center justify-center rounded-lg">
            <Mic2 className="text-primary-foreground size-4" />
          </div>
          <span className="font-semibold">Notetaker</span>
        </div>
        <div className="w-full max-w-sm">
          <Outlet />
        </div>
      </div>
    </div>
  );
}

function AuthBrandPanel() {
  return (
    <div className="relative hidden overflow-hidden bg-zinc-950 p-10 text-white lg:flex lg:w-1/2 lg:flex-col lg:justify-between">
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_top_right,_var(--tw-gradient-stops))] from-violet-600/30 via-transparent to-transparent" />
      <div className="absolute inset-0 bg-[linear-gradient(to_right,#ffffff08_1px,transparent_1px),linear-gradient(to_bottom,#ffffff08_1px,transparent_1px)] bg-[size:32px_32px]" />

      <div className="relative flex items-center gap-2.5">
        <div className="flex size-9 items-center justify-center rounded-lg bg-white/10 backdrop-blur">
          <Mic2 className="size-4" />
        </div>
        <span className="font-semibold">Notetaker</span>
      </div>

      <div className="relative space-y-6">
        <blockquote className="text-2xl leading-snug font-medium tracking-tight">
          &ldquo;We stopped losing action items in meeting notes. Every call is captured, summarized,
          and searchable.&rdquo;
        </blockquote>
        <div>
          <p className="font-medium">Sarah Kim</p>
          <p className="text-sm text-white/60">Head of Product, Acme Co</p>
        </div>
      </div>

      <p className="relative text-sm text-white/40">
        Auto-records, transcribes, and summarizes your meetings.
      </p>
    </div>
  );
}
