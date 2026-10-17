import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { ApiError, NetworkError, api, setOnUnauthorized } from "../lib/api";
import type { MeResponse, User } from "../types/api";

type AuthStatus = "loading" | "authenticated" | "unauthenticated" | "network_error";

interface AuthContextValue {
  user: User | null;
  status: AuthStatus;
  refresh: () => Promise<void>;
  setUser: (user: User | null) => void;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUserState] = useState<User | null>(null);
  const [status, setStatus] = useState<AuthStatus>("loading");

  // Keep status in step with the user: after login/signup the guard reads status, and a stale
  // "unauthenticated" from the initial /auth/me would bounce the user straight back to /login.
  const setUser = useCallback((next: User | null) => {
    setUserState(next);
    setStatus(next ? "authenticated" : "unauthenticated");
  }, []);

  const refresh = useCallback(async () => {
    try {
      const data = await api.get<MeResponse>("/auth/me");
      setUser(data.user);
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) {
        setUser(null);
        return;
      }
      if (err instanceof NetworkError) {
        setStatus((prev) => (prev === "authenticated" ? "authenticated" : "network_error"));
        return;
      }
      setUser(null);
    }
  }, [setUser]);

  useEffect(() => {
    setOnUnauthorized(() => setUser(null));
    void refresh();
    return () => setOnUnauthorized(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const value = useMemo(
    () => ({ user, status, refresh, setUser }),
    [user, status, refresh, setUser],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error("useAuth must be used within AuthProvider");
  }
  return ctx;
}
