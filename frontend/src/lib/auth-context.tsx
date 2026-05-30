import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react"
import type { AuthSession } from "@/types"
import {
  apiGetSession,
  apiLogin,
  apiLogout,
  apiSignUp,
  authErrorMessage,
} from "@/lib/auth-api"

const STORAGE_KEY = "amn-auth-session"

type AuthContextValue = {
  session: AuthSession | null
  isAuthenticated: boolean
  isLoading: boolean
  login: (username: string, password: string) => Promise<{ ok: true } | { ok: false; error: string }>
  signup: (name: string, email: string, username: string, password: string) => Promise<{ ok: true } | { ok: false; error: string }>
  logout: () => Promise<void>
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
  const [isLoading, setIsLoading] = useState(true)

  useEffect(() => {
    let cancelled = false

    async function validateStoredSession() {
      const stored = loadSession()
      if (!stored?.token) {
        if (!cancelled) setIsLoading(false)
        return
      }

      try {
        const validated = await apiGetSession(stored.token)
        if (!cancelled) {
          setSession(validated)
          saveSession(validated)
        }
      } catch {
        if (!cancelled) {
          setSession(null)
          saveSession(null)
        }
      } finally {
        if (!cancelled) setIsLoading(false)
      }
    }

    validateStoredSession()
    return () => {
      cancelled = true
    }
  }, [])

  const login = useCallback(async (username: string, password: string) => {
    try {
      const next = await apiLogin(username, password)
      setSession(next)
      saveSession(next)
      return { ok: true as const }
    } catch (error) {
      return { ok: false as const, error: authErrorMessage(error) }
    }
  }, [])

  const signup = useCallback(
    async (name: string, email: string, username: string, password: string) => {
      try {
        const next = await apiSignUp(name, email, username, password)
        setSession(next)
        saveSession(next)
        return { ok: true as const }
      } catch (error) {
        return { ok: false as const, error: authErrorMessage(error) }
      }
    },
    [],
  )

  const logout = useCallback(async () => {
    const token = session?.token
    setSession(null)
    saveSession(null)
    if (token) {
      try {
        await apiLogout(token)
      } catch {
        // Local session already cleared.
      }
    }
  }, [session?.token])

  const value = useMemo(
    () => ({
      session,
      isAuthenticated: session !== null,
      isLoading,
      login,
      signup,
      logout,
    }),
    [session, isLoading, login, signup, logout],
  )

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>
}

export function useAuth() {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error("useAuth must be used within AuthProvider")
  return ctx
}
