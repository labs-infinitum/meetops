import { useCallback, useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { Alert } from "../components/Alert";
import { EmptyState } from "../components/EmptyState";
import { PageLoader, Spinner } from "../components/Spinner";
import { api } from "../lib/api";
import type { CalendarConnection, OAuthStartResponse, SyncResult } from "../types/api";

function formatDate(iso: string | null) {
  if (!iso) return "Never";
  return new Date(iso).toLocaleString();
}

export function CalendarsPage() {
  const [connections, setConnections] = useState<CalendarConnection[]>([]);
  const [loading, setLoading] = useState(true);
  const [connecting, setConnecting] = useState(false);
  const [syncing, setSyncing] = useState<string | null>(null);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(
    null,
  );
  const [searchParams, setSearchParams] = useSearchParams();

  const load = useCallback(async () => {
    const data = await api.get<CalendarConnection[]>("/calendars/");
    setConnections(data);
    setLoading(false);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    const oauth = searchParams.get("oauth");
    if (oauth === "success") {
      setMessage({ type: "success", text: "Google Calendar connected successfully." });
      setSearchParams({}, { replace: true });
      void load();
    } else if (oauth === "error") {
      const reason = searchParams.get("reason") ?? "unknown";
      setMessage({ type: "error", text: `Calendar connection failed (${reason}).` });
      setSearchParams({}, { replace: true });
    }
  }, [searchParams, setSearchParams, load]);

  async function connectGoogle() {
    setConnecting(true);
    setMessage(null);
    try {
      const data = await api.post<OAuthStartResponse>("/calendars/oauth/start", {
        provider: "google",
      });
      window.location.href = data.redirect_url;
    } catch {
      setMessage({ type: "error", text: "Failed to start Google OAuth." });
      setConnecting(false);
    }
  }

  async function syncConnection(id: string) {
    setSyncing(id);
    try {
      const result = await api.post<SyncResult>(`/calendars/${id}/sync`);
      setMessage({
        type: "success",
        text: `Synced: ${result.upserted} updated, ${result.deleted} removed.`,
      });
      await load();
    } catch {
      setMessage({ type: "error", text: "Sync failed." });
    } finally {
      setSyncing(null);
    }
  }

  async function disconnect(id: string) {
    if (!confirm("Disconnect this calendar?")) return;
    try {
      await api.delete(`/calendars/${id}`);
      await load();
    } catch {
      setMessage({ type: "error", text: "Failed to disconnect." });
    }
  }

  if (loading) return <PageLoader />;

  const googleConnections = connections.filter((c) => c.provider === "google");

  return (
    <div>
      <h1 className="font-display text-2xl font-bold">Calendars</h1>
      <p className="mt-1 text-sm text-surface-500">
        Connect calendars to sync Google Meet meetings
      </p>

      {message && (
        <div className="mt-4">
          <Alert variant={message.type === "success" ? "success" : "error"}>{message.text}</Alert>
        </div>
      )}

      <div className="mt-8 grid gap-4 md:grid-cols-3">
        <div className="card">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-white shadow-sm">
              <svg viewBox="0 0 24 24" className="h-6 w-6" aria-hidden="true">
                <path
                  fill="#4285F4"
                  d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 01-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z"
                />
                <path
                  fill="#34A853"
                  d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
                />
                <path
                  fill="#FBBC05"
                  d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
                />
                <path
                  fill="#EA4335"
                  d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
                />
              </svg>
            </div>
            <div>
              <h3 className="font-medium">Google Calendar</h3>
              <p className="text-xs text-surface-500">Available now</p>
            </div>
          </div>
          <button
            type="button"
            onClick={() => void connectGoogle()}
            disabled={connecting}
            className="btn-primary mt-4 w-full"
          >
            {connecting ? <Spinner /> : "Connect Google"}
          </button>
        </div>

        <div className="card opacity-60">
          <h3 className="font-medium">Outlook Calendar</h3>
          <p className="mt-1 text-xs text-surface-500">Coming soon</p>
          <button type="button" disabled className="btn-secondary mt-4 w-full">
            Coming soon
          </button>
        </div>

        <div className="card opacity-60">
          <h3 className="font-medium">Apple Calendar</h3>
          <p className="mt-1 text-xs text-surface-500">Coming soon</p>
          <button type="button" disabled className="btn-secondary mt-4 w-full">
            Coming soon
          </button>
        </div>
      </div>

      <section className="mt-10">
        <h2 className="font-display text-lg font-semibold">Connected accounts</h2>
        {googleConnections.length === 0 ? (
          <div className="mt-4">
            <EmptyState
              title="No calendars connected"
              description="Connect Google Calendar to start syncing your meetings."
            />
          </div>
        ) : (
          <div className="mt-4 space-y-3">
            {googleConnections.map((c) => (
              <div key={c.id} className="card flex flex-wrap items-center justify-between gap-4">
                <div>
                  <p className="font-medium">{c.email}</p>
                  <p className="text-sm text-surface-500">
                    Last synced: {formatDate(c.last_synced_at)} · Session: {c.bot_session_status}
                  </p>
                </div>
                <div className="flex gap-2">
                  <button
                    type="button"
                    onClick={() => void syncConnection(c.id)}
                    disabled={syncing === c.id}
                    className="btn-secondary"
                  >
                    {syncing === c.id ? <Spinner /> : "Sync now"}
                  </button>
                  <button
                    type="button"
                    onClick={() => void disconnect(c.id)}
                    className="btn-ghost text-red-600"
                  >
                    Disconnect
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
