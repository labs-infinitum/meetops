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
  AuthApiError,
  authErrorMessage,
} from "@/lib/auth-api"
import { parseStoredAuthSession } from "@/lib/domain-validators"
import {
  isSessionExpired,
  isSessionInvalidError,
  registerSessionInvalidHandler,
} from "@/lib/session-guard"

const STORAGE_KEY = "amn-auth-session"

type AuthContextValue = {
  session: AuthSession | null
  isAuthenticated: boolean
  isLoading: boolean
  login: (
    username: string,
    password: string,
  ) => Promise<
    | { ok: true }
    | { ok: false; error: string; emailNotVerified?: boolean }
  >
  signup: (
    name: string,
    email: string,
    username: string,
    password: string,
  ) => Promise<
    | { ok: true; verificationRequired?: boolean }
    | { ok: false; error: string }
  >
  logout: () => Promise<void>
  setSessionFromVerify: (session: AuthSession) => void
}

const AuthContext = createContext<AuthContextValue | null>(null)

function loadSession(): AuthSession | null {
  const raw = localStorage.getItem(STORAGE_KEY)
  if (!raw) return null
  const session = parseStoredAuthSession(raw)
  if (!session || isSessionExpired(session.expiresAt)) {
    return null
  }
  return session
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

  const logout = useCallback(async () => {
    const token = session?.token
    setSession(null)
    saveSession(null)
    if (token) {
      void apiLogout(token).catch(() => undefined)
    }
  }, [session?.token])

  useEffect(() => {
    registerSessionInvalidHandler(() => {
      void logout()
      if (window.location.pathname !== "/login") {
        window.location.assign("/login")
      }
    })
  }, [logout])

  useEffect(() => {
    let cancelled = false

    async function validateStoredSession() {
      const stored = loadSession()
      if (!stored?.token) {
        if (!cancelled) {
          setSession(null)
          saveSession(null)
          setIsLoading(false)
        }
        return
      }

      if (isSessionExpired(stored.expiresAt)) {
        if (!cancelled) {
          setSession(null)
          saveSession(null)
          setIsLoading(false)
        }
        return
      }

      try {
        const validated = await apiGetSession(stored.token)
        if (!cancelled) {
          setSession(validated)
          saveSession(validated)
        }
      } catch (error) {
        if (!cancelled) {
          if (isSessionInvalidError(error)) {
            setSession(null)
            saveSession(null)
            window.location.assign("/login")
          } else {
            // Network/server failure: keep the locally-valid session instead
            // of logging the user out; it will be re-validated on the next
            // API call and downgraded then if actually invalid.
            setSession(stored)
          }
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
      return {
        ok: false as const,
        error: authErrorMessage(error),
        emailNotVerified:
          error instanceof AuthApiError && error.code === "email_not_verified",
      }
    }
  }, [])

  const signup = useCallback(
    async (name: string, email: string, username: string, password: string) => {
      try {
        const result = await apiSignUp(name, email, username, password)
        if (result.kind === "session") {
          setSession(result.session)
          saveSession(result.session)
          return { ok: true as const }
        }
        return { ok: true as const, verificationRequired: true }
      } catch (error) {
        return { ok: false as const, error: authErrorMessage(error) }
      }
    },
    [],
  )

  const setSessionFromVerify = useCallback((next: AuthSession) => {
    setSession(next)
    saveSession(next)
  }, [])

  const value = useMemo(
    () => ({
      session,
      isAuthenticated: session !== null && !isSessionExpired(session.expiresAt),
      isLoading,
      login,
      signup,
      logout,
      setSessionFromVerify,
    }),
    [session, isLoading, login, signup, logout, setSessionFromVerify],
  )

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>
}

export function useAuth() {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error("useAuth must be used within AuthProvider")
  return ctx
}
