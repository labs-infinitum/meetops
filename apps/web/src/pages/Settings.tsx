import { useEffect, useState } from "react";
import { Alert } from "../components/Alert";
import { PageLoader, Spinner } from "../components/Spinner";
import { ApiError, api } from "../lib/api";
import type { Settings } from "../types/api";

export function SettingsPage() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState<string | null>(null);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(
    null,
  );

  useEffect(() => {
    void api.get<Settings>("/settings/").then((s) => {
      setSettings(s);
      setLoading(false);
    });
  }, []);

  async function saveProfile(name: string) {
    setSaving("profile");
    setMessage(null);
    try {
      const updated = await api.patch<Settings>("/settings/profile", { name });
      setSettings(updated);
      setMessage({ type: "success", text: "Profile updated." });
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof ApiError ? err.message : "Failed to save.",
      });
    } finally {
      setSaving(null);
    }
  }

  async function saveRecording(data: {
    auto_record_default?: boolean;
    join_lead_time_secs?: number;
    bot_display_name?: string | null;
  }) {
    setSaving("recording");
    setMessage(null);
    try {
      const updated = await api.patch<Settings>("/settings/recording", data);
      setSettings(updated);
      setMessage({ type: "success", text: "Recording settings updated." });
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof ApiError ? err.message : "Failed to save.",
      });
    } finally {
      setSaving(null);
    }
  }

  async function saveNotifications(data: {
    notify_email_on_notes_ready?: boolean;
    notify_email_on_recording_failed?: boolean;
  }) {
    setSaving("notifications");
    setMessage(null);
    try {
      const updated = await api.patch<Settings>("/settings/notifications", data);
      setSettings(updated);
      setMessage({ type: "success", text: "Notification settings updated." });
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof ApiError ? err.message : "Failed to save.",
      });
    } finally {
      setSaving(null);
    }
  }

  if (loading || !settings) return <PageLoader />;

  return (
    <div>
      <h1 className="font-display text-2xl font-bold">Settings</h1>
      <p className="mt-1 text-sm text-surface-500">Manage your account and preferences</p>

      {message && (
        <div className="mt-4">
          <Alert variant={message.type === "success" ? "success" : "error"}>{message.text}</Alert>
        </div>
      )}

      <ProfileSection
        settings={settings}
        saving={saving === "profile"}
        onSave={(name) => void saveProfile(name)}
      />

      <RecordingSection
        settings={settings}
        saving={saving === "recording"}
        onSave={(data) => void saveRecording(data)}
      />

      <NotificationsSection
        settings={settings}
        saving={saving === "notifications"}
        onSave={(data) => void saveNotifications(data)}
      />
    </div>
  );
}

function ProfileSection({
  settings,
  saving,
  onSave,
}: {
  settings: Settings;
  saving: boolean;
  onSave: (name: string) => void;
}) {
  const [name, setName] = useState(settings.name);

  useEffect(() => {
    setName(settings.name);
  }, [settings.name]);

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        onSave(name);
      }}
      className="card mt-8"
    >
      <h2 className="font-display text-lg font-semibold">Profile</h2>
      <div className="mt-4 space-y-4">
        <div>
          <label htmlFor="name" className="label">
            Display name
          </label>
          <input
            id="name"
            className="input"
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
          />
        </div>
        <div>
          <label className="label">Email</label>
          <input className="input" value={settings.email} disabled />
        </div>
        <div>
          <label className="label">Username</label>
          <input className="input" value={settings.username} disabled />
        </div>
      </div>
      <button type="submit" disabled={saving} className="btn-primary mt-4">
        {saving ? <Spinner /> : "Save profile"}
      </button>
    </form>
  );
}

function RecordingSection({
  settings,
  saving,
  onSave,
}: {
  settings: Settings;
  saving: boolean;
  onSave: (data: {
    auto_record_default?: boolean;
    join_lead_time_secs?: number;
    bot_display_name?: string | null;
  }) => void;
}) {
  const [autoRecord, setAutoRecord] = useState(settings.auto_record_default);
  const [leadTime, setLeadTime] = useState(String(settings.join_lead_time_secs));
  const [botName, setBotName] = useState(settings.bot_display_name ?? "");

  useEffect(() => {
    setAutoRecord(settings.auto_record_default);
    setLeadTime(String(settings.join_lead_time_secs));
    setBotName(settings.bot_display_name ?? "");
  }, [settings]);

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        onSave({
          auto_record_default: autoRecord,
          join_lead_time_secs: parseInt(leadTime, 10),
          bot_display_name: botName || null,
        });
      }}
      className="card mt-6"
    >
      <h2 className="font-display text-lg font-semibold">Recording</h2>
      <div className="mt-4 space-y-4">
        <label className="flex items-center gap-3">
          <input
            type="checkbox"
            checked={autoRecord}
            onChange={(e) => setAutoRecord(e.target.checked)}
            className="h-4 w-4 rounded border-surface-300 text-brand-600"
          />
          <span className="text-sm">Auto-record new synced meetings by default</span>
        </label>
        <div>
          <label htmlFor="leadTime" className="label">
            Join lead time (seconds before meeting start)
          </label>
          <input
            id="leadTime"
            type="number"
            min={0}
            max={3600}
            className="input"
            value={leadTime}
            onChange={(e) => setLeadTime(e.target.value)}
          />
        </div>
        <div>
          <label htmlFor="botName" className="label">
            Bot display name in Meet
          </label>
          <input
            id="botName"
            className="input"
            value={botName}
            onChange={(e) => setBotName(e.target.value)}
            placeholder="MeetOps Notetaker"
            maxLength={100}
          />
        </div>
      </div>
      <button type="submit" disabled={saving} className="btn-primary mt-4">
        {saving ? <Spinner /> : "Save recording settings"}
      </button>
    </form>
  );
}

function NotificationsSection({
  settings,
  saving,
  onSave,
}: {
  settings: Settings;
  saving: boolean;
  onSave: (data: {
    notify_email_on_notes_ready?: boolean;
    notify_email_on_recording_failed?: boolean;
  }) => void;
}) {
  const [notesReady, setNotesReady] = useState(settings.notify_email_on_notes_ready);
  const [recFailed, setRecFailed] = useState(settings.notify_email_on_recording_failed);

  useEffect(() => {
    setNotesReady(settings.notify_email_on_notes_ready);
    setRecFailed(settings.notify_email_on_recording_failed);
  }, [settings]);

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        onSave({
          notify_email_on_notes_ready: notesReady,
          notify_email_on_recording_failed: recFailed,
        });
      }}
      className="card mt-6"
    >
      <h2 className="font-display text-lg font-semibold">Notifications</h2>
      <div className="mt-4 space-y-3">
        <label className="flex items-center gap-3">
          <input
            type="checkbox"
            checked={notesReady}
            onChange={(e) => setNotesReady(e.target.checked)}
            className="h-4 w-4 rounded border-surface-300 text-brand-600"
          />
          <span className="text-sm">Email me when meeting notes are ready</span>
        </label>
        <label className="flex items-center gap-3">
          <input
            type="checkbox"
            checked={recFailed}
            onChange={(e) => setRecFailed(e.target.checked)}
            className="h-4 w-4 rounded border-surface-300 text-brand-600"
          />
          <span className="text-sm">Email me when a recording fails</span>
        </label>
      </div>
      <button type="submit" disabled={saving} className="btn-primary mt-4">
        {saving ? <Spinner /> : "Save notification settings"}
      </button>
    </form>
  );
}
