import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react"
import type { AuthSession, User } from "@/types"
import { mockUser } from "@/lib/mock-data"

const STORAGE_KEY = "amn-auth-session"

type AuthContextValue = {
  session: AuthSession | null
  isAuthenticated: boolean
  login: (username: string, password: string) => Promise<{ ok: true } | { ok: false; error: string }>
  signup: (name: string, email: string, username: string, password: string) => Promise<{ ok: true } | { ok: false; error: string }>
  logout: () => void
}

const AuthContext = createContext<AuthContextValue | null>(null)

function loadSession(): AuthSession | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return null
    return JSON.parse(raw) as AuthSession
  } catch {
    return null
  }
}

function saveSession(session: AuthSession | null) {
  if (session) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(session))
  } else {
    localStorage.removeItem(STORAGE_KEY)
  }
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [session, setSession] = useState<AuthSession | null>(() => loadSession())

  const login = useCallback(async (username: string, password: string) => {
    await new Promise((r) => setTimeout(r, 600))

    if (!username.trim() || !password.trim()) {
      return { ok: false as const, error: "Username and password are required." }
    }

    if (password.length < 6) {
      return { ok: false as const, error: "Invalid credentials." }
    }

    const user: User = {
      ...mockUser,
      username: username.trim(),
      name: username.trim() === "alex" ? mockUser.name : username.trim(),
      email: username.trim() === "alex" ? mockUser.email : `${username.trim()}@example.com`,
    }

    const next: AuthSession = { user, token: "mock-token-" + Date.now() }
    setSession(next)
    saveSession(next)
    return { ok: true as const }
  }, [])

  const signup = useCallback(
    async (name: string, email: string, username: string, password: string) => {
      await new Promise((r) => setTimeout(r, 800))

      if (!name.trim() || !email.trim() || !username.trim() || !password.trim()) {
        return { ok: false as const, error: "All fields are required." }
      }

      if (password.length < 8) {
        return { ok: false as const, error: "Password must be at least 8 characters." }
      }

      const user: User = {
        id: "user-" + Date.now(),
        name: name.trim(),
        email: email.trim(),
        username: username.trim(),
      }

      const next: AuthSession = { user, token: "mock-token-" + Date.now() }
      setSession(next)
      saveSession(next)
      return { ok: true as const }
    },
    [],
  )

  const logout = useCallback(() => {
    setSession(null)
    saveSession(null)
  }, [])

  const value = useMemo(
    () => ({
      session,
      isAuthenticated: session !== null,
      login,
      signup,
      logout,
    }),
    [session, login, signup, logout],
  )

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>
}

export function useAuth() {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error("useAuth must be used within AuthProvider")
  return ctx
}
