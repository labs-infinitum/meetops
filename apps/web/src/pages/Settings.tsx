import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { Alert } from "@/components/Alert";
import { PageLoader } from "@/components/Spinner";
import { PageHeader } from "@/components/shared/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { ApiError, api } from "@/lib/api";
import { cn } from "@/lib/utils";
import type { Settings } from "@/types/api";

const LANGUAGE_OPTIONS: { code: string; label: string }[] = [
  { code: "en", label: "English" },
  { code: "es", label: "Spanish" },
  { code: "fr", label: "French" },
  { code: "de", label: "German" },
  { code: "it", label: "Italian" },
  { code: "pt", label: "Portuguese" },
  { code: "nl", label: "Dutch" },
  { code: "pl", label: "Polish" },
  { code: "uk", label: "Ukrainian" },
  { code: "ru", label: "Russian" },
  { code: "hi", label: "Hindi" },
  { code: "ja", label: "Japanese" },
  { code: "ko", label: "Korean" },
  { code: "zh", label: "Chinese" },
  { code: "ar", label: "Arabic" },
  { code: "tr", label: "Turkish" },
  { code: "sv", label: "Swedish" },
  { code: "da", label: "Danish" },
  { code: "no", label: "Norwegian" },
  { code: "fi", label: "Finnish" },
  { code: "cs", label: "Czech" },
  { code: "el", label: "Greek" },
  { code: "he", label: "Hebrew" },
  { code: "th", label: "Thai" },
  { code: "vi", label: "Vietnamese" },
  { code: "id", label: "Indonesian" },
  { code: "ro", label: "Romanian" },
  { code: "hu", label: "Hungarian" },
];

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

  async function saveLanguages(languages: string[]) {
    setSaving("languages");
    setMessage(null);
    try {
      const updated = await api.patch<Settings>("/settings/languages", { languages });
      setSettings(updated);
      setMessage({ type: "success", text: "Language settings updated." });
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
    <div className="space-y-8">
      <PageHeader
        title="Settings"
        description="Manage your account and recording preferences."
      />

      {message && (
        <Alert variant={message.type === "success" ? "success" : "error"}>{message.text}</Alert>
      )}

      <div className="grid max-w-2xl gap-6">
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

        <LanguagesSection
          settings={settings}
          saving={saving === "languages"}
          onSave={(languages) => void saveLanguages(languages)}
        />

        <NotificationsSection
          settings={settings}
          saving={saving === "notifications"}
          onSave={(data) => void saveNotifications(data)}
        />
      </div>
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
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Profile</CardTitle>
        <CardDescription>Your account information</CardDescription>
      </CardHeader>
      <CardContent>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            onSave(name);
          }}
          className="space-y-4"
        >
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="name">Display name</Label>
              <Input
                id="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                required
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="username">Username</Label>
              <Input id="username" value={settings.username} disabled />
            </div>
          </div>
          <div className="space-y-2">
            <Label htmlFor="email">Email</Label>
            <Input id="email" type="email" value={settings.email} disabled />
          </div>
          <Button type="submit" size="sm" disabled={saving}>
            {saving && <Loader2 className="animate-spin" />}
            Save profile
          </Button>
        </form>
      </CardContent>
    </Card>
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
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Recording</CardTitle>
        <CardDescription>Bot behavior for auto-recorded meetings</CardDescription>
      </CardHeader>
      <CardContent>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            onSave({
              auto_record_default: autoRecord,
              join_lead_time_secs: parseInt(leadTime, 10),
              bot_display_name: botName || null,
            });
          }}
          className="space-y-6"
        >
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm font-medium">Auto-record by default</p>
              <p className="text-muted-foreground text-xs">
                New synced meetings will have auto-record enabled
              </p>
            </div>
            <Switch checked={autoRecord} onCheckedChange={setAutoRecord} />
          </div>
          <Separator />
          <div className="space-y-2">
            <Label htmlFor="leadTime">Join lead time (seconds)</Label>
            <Input
              id="leadTime"
              type="number"
              min={0}
              max={3600}
              value={leadTime}
              onChange={(e) => setLeadTime(e.target.value)}
              className="max-w-32"
            />
            <p className="text-muted-foreground text-xs">
              How early the bot joins before meeting start
            </p>
          </div>
          <div className="space-y-2">
            <Label htmlFor="botName">Bot display name</Label>
            <Input
              id="botName"
              value={botName}
              onChange={(e) => setBotName(e.target.value)}
              placeholder="AI Notetaker"
              maxLength={100}
            />
          </div>
          <Button type="submit" size="sm" disabled={saving}>
            {saving && <Loader2 className="animate-spin" />}
            Save recording settings
          </Button>
        </form>
      </CardContent>
    </Card>
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
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Notifications</CardTitle>
        <CardDescription>When to notify you about recordings</CardDescription>
      </CardHeader>
      <CardContent>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            onSave({
              notify_email_on_notes_ready: notesReady,
              notify_email_on_recording_failed: recFailed,
            });
          }}
          className="space-y-4"
        >
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm font-medium">Summary ready</p>
              <p className="text-muted-foreground text-xs">When AI notes are generated</p>
            </div>
            <Switch checked={notesReady} onCheckedChange={setNotesReady} />
          </div>
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm font-medium">Recording failed</p>
              <p className="text-muted-foreground text-xs">
                When bot cannot join or record
              </p>
            </div>
            <Switch checked={recFailed} onCheckedChange={setRecFailed} />
          </div>
          <Button type="submit" size="sm" disabled={saving}>
            {saving && <Loader2 className="animate-spin" />}
            Save notification settings
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}

function LanguagesSection({
  settings,
  saving,
  onSave,
}: {
  settings: Settings;
  saving: boolean;
  onSave: (languages: string[]) => void;
}) {
  const [selected, setSelected] = useState<string[]>(settings.languages ?? ["en"]);

  useEffect(() => {
    setSelected(settings.languages?.length ? settings.languages : ["en"]);
  }, [settings.languages]);

  function toggle(code: string) {
    setSelected((prev) => {
      if (prev.includes(code)) {
        const next = prev.filter((c) => c !== code);
        return next.length === 0 ? prev : next;
      }
      if (prev.length >= 8) return prev;
      return [...prev, code];
    });
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Languages</CardTitle>
        <CardDescription>
          Languages spoken in your meetings. Used to improve transcription accuracy and notes.
          Select one for a fixed language, or several for multilingual meetings.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            onSave(selected);
          }}
          className="space-y-4"
        >
          <div className="flex flex-wrap gap-2">
            {LANGUAGE_OPTIONS.map((opt) => {
              const active = selected.includes(opt.code);
              return (
                <button
                  key={opt.code}
                  type="button"
                  onClick={() => toggle(opt.code)}
                  className="rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  aria-pressed={active}
                >
                  <Badge
                    variant={active ? "default" : "outline"}
                    className={cn(
                      "h-7 cursor-pointer px-2.5 text-xs",
                      !active && "bg-background",
                    )}
                  >
                    {opt.label}
                  </Badge>
                </button>
              );
            })}
          </div>
          <p className="text-muted-foreground text-xs">
            {selected.length === 1
              ? "Transcription will target this language."
              : "Transcription will use multilingual mode; notes prefer the first selected language."}
          </p>
          <Button type="submit" size="sm" disabled={saving}>
            {saving && <Loader2 className="animate-spin" />}
            Save languages
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}
