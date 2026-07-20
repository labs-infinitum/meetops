import { useCallback, useEffect, useRef, useState } from "react";
import { Link, useParams } from "react-router-dom";
import {
  AlertCircle,
  FileText,
  Film,
  ListChecks,
  Loader2,
  LogIn,
  LogOut,
  Mic2,
  MonitorOff,
  MonitorUp,
  RefreshCw,
  Sparkles,
  Square,
  Users,
  type LucideIcon,
} from "lucide-react";
import { toast } from "sonner";
import { Alert } from "@/components/Alert";
import { PageLoader } from "@/components/Spinner";
import { PageHeader } from "@/components/shared/PageHeader";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { ApiError, api } from "@/lib/api";
import { formatDate, formatDateTime, formatDuration, formatRelativeMs } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { BotRun, Note, PlaybackUrl, TimelineEvent } from "@/types/api";

const ACTIVE_STATUSES = new Set([
  "queued",
  "joining",
  "in_meeting",
  "recording",
  "uploading",
]);

function isActiveStatus(status: string) {
  return ACTIVE_STATUSES.has(status);
}

function recordingTitle(run: BotRun) {
  return run.meeting_title || run.meet_url || `Recording #${run.id}`;
}

function snapshotParticipantCount(payload: unknown): number | undefined {
  if (!payload || typeof payload !== "object") return undefined;
  const count = (payload as { count?: unknown; participant_count?: unknown }).count
    ?? (payload as { participant_count?: unknown }).participant_count;
  return typeof count === "number" ? count : undefined;
}

function eventTitle(event: TimelineEvent): string {
  switch (event.event_type) {
    case "participant_joined":
      return `${event.participant_name || "Someone"} joined`;
    case "participant_left":
      return `${event.participant_name || "Someone"} left`;
    case "screen_share_started":
      return `${event.participant_name || "Someone"} started presenting`;
    case "screen_share_stopped":
      return `${event.participant_name || "Someone"} stopped presenting`;
    case "active_speaker_changed":
      return `${event.participant_name || "Unknown speaker"} started speaking`;
    case "active_speaker_sample":
      return `${event.participant_name || "Unknown speaker"} speaking`;
    case "participant_snapshot": {
      const count = snapshotParticipantCount(event.payload_json);
      return count === undefined ? "Participant snapshot" : `${count} participants in meeting`;
    }
    case "collector_warning":
      return "Collector warning";
    default:
      return event.event_type.replaceAll("_", " ");
  }
}

function eventIcon(eventType: string): LucideIcon {
  switch (eventType) {
    case "participant_joined":
      return LogIn;
    case "participant_left":
      return LogOut;
    case "screen_share_started":
      return MonitorUp;
    case "screen_share_stopped":
      return MonitorOff;
    case "active_speaker_changed":
    case "active_speaker_sample":
      return Mic2;
    default:
      return Users;
  }
}

function timelineEventsToShow(events: TimelineEvent[]): TimelineEvent[] {
  const withoutNoise = events.filter(
    (event) =>
      event.event_type !== "participant_snapshot" &&
      event.event_type !== "active_speaker_sample",
  );
  return (withoutNoise.length > 0 ? withoutNoise : events).slice(0, 100);
}

function latestActiveSpeakerEvent(events: TimelineEvent[]): TimelineEvent | undefined {
  return events
    .filter((event) => event.event_type === "active_speaker_changed" && event.participant_name)
    .at(-1);
}

function aiStatusVariant(status?: string | null): "default" | "secondary" | "destructive" | "outline" {
  switch (status) {
    case "completed":
    case "cleaned":
      return "default";
    case "failed":
      return "destructive";
    case "processing":
    case "transcribed":
      return "secondary";
    default:
      return "outline";
  }
}

function isAiBusy(note: Note | null): boolean {
  if (!note) return false;
  const transcript = note.transcript_status;
  const summary = note.summary_status;
  if (transcript === "failed" || summary === "failed") return false;
  if (transcript === "processing" || transcript === "transcribed") return true;
  if (transcript === "cleaned" && summary !== "completed") return true;
  return summary === "processing";
}

function renderMarkdownLite(markdown: string) {
  return markdown.split("\n").map((line, i) => {
    if (line.startsWith("## ")) {
      return (
        <h2 key={i} className="mt-6 mb-2 text-lg font-semibold first:mt-0">
          {line.slice(3)}
        </h2>
      );
    }
    if (line.startsWith("### ")) {
      return (
        <h3 key={i} className="mt-4 mb-1 text-base font-medium">
          {line.slice(4)}
        </h3>
      );
    }
    if (line.startsWith("- ")) {
      return (
        <p key={i} className="text-muted-foreground ml-4 text-sm">
          • {line.slice(2)}
        </p>
      );
    }
    if (/^\d+\./.test(line)) {
      return (
        <p key={i} className="text-muted-foreground ml-4 text-sm">
          {line}
        </p>
      );
    }
    if (line.trim() === "") return <br key={i} />;
    return (
      <p key={i} className="text-muted-foreground text-sm leading-relaxed">
        {line}
      </p>
    );
  });
}

async function fetchRecordingDetail(
  id: string,
  options: { fetchPlayback?: boolean } = {},
) {
  const fetchPlayback = options.fetchPlayback !== false;
  const [loaded, timelineEvents, generatedNote] = await Promise.all([
    api.get<BotRun>(`/recordings/${id}`),
    api.get<TimelineEvent[]>(`/recordings/${id}/timeline`),
    api.get<Note | null>(`/recordings/${id}/ai`).catch(() => null),
  ]);
  let playbackUrl: string | null = null;
  if (fetchPlayback && loaded.status === "completed" && loaded.recording_path) {
    try {
      const playback = await api.get<PlaybackUrl>(`/recordings/${id}/playback-url`);
      playbackUrl = playback.url;
    } catch {
      playbackUrl = null;
    }
  }
  return { loaded, timelineEvents, generatedNote, playbackUrl };
}

export function RecordingDetailPage() {
  const { id } = useParams<{ id: string }>();
  const [run, setRun] = useState<BotRun | null>(null);
  const [playbackUrl, setPlaybackUrl] = useState<string | null>(null);
  const playbackUrlRef = useRef<string | null>(null);
  const [timelineEvents, setTimelineEvents] = useState<TimelineEvent[]>([]);
  const [generatedNote, setGeneratedNote] = useState<Note | null>(null);
  const [loading, setLoading] = useState(true);
  const [notFound, setNotFound] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [rerunningAi, setRerunningAi] = useState(false);
  const [error, setError] = useState("");

  const isActive = run ? isActiveStatus(run.status) : false;
  const aiBusy = isAiBusy(generatedNote);
  const canRerunAi = Boolean(run?.status === "completed" && run.recording_path);
  const activeSpeakerEvent = latestActiveSpeakerEvent(timelineEvents);
  const noteLink = generatedNote
    ? `/app/notes/${generatedNote.transcript_id}`
    : "/app/notes";

  const refresh = useCallback(async () => {
    if (!id) return;
    const detail = await fetchRecordingDetail(id, {
      // Keep the first presigned URL so the <video> element does not reload on polls.
      fetchPlayback: !playbackUrlRef.current,
    });
    setRun(detail.loaded);
    setNotFound(false);
    setTimelineEvents(detail.timelineEvents);
    setGeneratedNote(detail.generatedNote);
    if (detail.playbackUrl) {
      playbackUrlRef.current = detail.playbackUrl;
      setPlaybackUrl(detail.playbackUrl);
    }
  }, [id]);

  useEffect(() => {
    playbackUrlRef.current = null;
    setPlaybackUrl(null);
  }, [id]);

  useEffect(() => {
    if (!id) return;
    void refresh()
      .catch((err) => {
        if (err instanceof ApiError && err.status === 404) {
          setNotFound(true);
        } else {
          setError(err instanceof ApiError ? err.message : "Failed to load recording");
        }
      })
      .finally(() => setLoading(false));
  }, [id, refresh]);

  useEffect(() => {
    if (!id || (!isActive && !aiBusy)) return;
    const interval = window.setInterval(() => {
      void refresh().catch(() => {
        /* keep last good state while polling */
      });
    }, 3000);
    return () => window.clearInterval(interval);
  }, [id, isActive, aiBusy, refresh]);

  async function stopRecording() {
    if (!run) return;
    setStopping(true);
    try {
      await api.post<BotRun>(`/recordings/${run.id}/stop`);
      toast.success("Stopping bot and ending recording…");
      await refresh();
    } catch (err) {
      const message = err instanceof ApiError ? err.message : "Failed to stop recording";
      setError(message);
      toast.error(message);
    } finally {
      setStopping(false);
    }
  }

  async function rerunAi() {
    if (!run) return;
    setRerunningAi(true);
    setError("");
    try {
      const note = await api.post<Note>(`/recordings/${run.id}/ai/rerun`);
      setGeneratedNote(note);
      toast.success("AI pipeline queued — transcript and notes will refresh shortly");
      await refresh();
    } catch (err) {
      const message = err instanceof ApiError ? err.message : "Failed to re-run AI";
      setError(message);
      toast.error(message);
    } finally {
      setRerunningAi(false);
    }
  }

  if (loading) return <PageLoader />;

  if (notFound || !run) {
    return (
      <div className="space-y-4">
        <PageHeader title="Recording not found" />
        {error && <Alert>{error}</Alert>}
        <Button asChild variant="outline">
          <Link to="/app/recordings">Back to recordings</Link>
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <PageHeader
        title={recordingTitle(run)}
        description={`Run #${run.id} · ${formatDate(run.meeting_start_at || run.created_at)}`}
        action={
          isActive ? (
            <Button variant="destructive" onClick={() => void stopRecording()} disabled={stopping}>
              {stopping ? <Loader2 className="animate-spin" /> : <Square className="size-4" />}
              Stop bot
            </Button>
          ) : canRerunAi ? (
            <div className="flex flex-wrap gap-2">
              <Button
                variant="outline"
                onClick={() => void rerunAi()}
                disabled={rerunningAi || aiBusy}
              >
                {rerunningAi || aiBusy ? (
                  <Loader2 className="animate-spin" />
                ) : (
                  <RefreshCw className="size-4" />
                )}
                {aiBusy ? "AI running…" : "Re-run AI"}
              </Button>
              <Button asChild>
                <Link to={noteLink}>View AI notes</Link>
              </Button>
            </div>
          ) : undefined
        }
      />

      {error && <Alert>{error}</Alert>}

      <div className="grid gap-6 lg:grid-cols-3">
        <Card className="lg:col-span-2">
          <CardContent className="p-6">
            <div className="bg-muted overflow-hidden rounded-lg border">
              {playbackUrl ? (
                <video
                  src={playbackUrl}
                  controls
                  playsInline
                  className="aspect-video w-full bg-black object-contain"
                  preload="metadata"
                >
                  Your browser does not support video playback.
                </video>
              ) : isActive || run.status === "completed" ? (
                <div className="flex aspect-video items-center justify-center">
                  <div className="text-center">
                    <Film className="text-muted-foreground mx-auto size-12" />
                    <p className="text-muted-foreground mt-3 text-sm">
                      {run.status === "recording"
                        ? "Recording in progress…"
                        : run.status === "joining" || run.status === "in_meeting"
                          ? "Bot is joining the meeting…"
                          : isActive
                            ? "Bot is starting…"
                            : "Video is not available yet."}
                    </p>
                  </div>
                </div>
              ) : (
                <div className="flex aspect-video items-center justify-center">
                  <div className="text-center">
                    <AlertCircle className="text-muted-foreground mx-auto size-12" />
                    <p className="text-muted-foreground mt-3 text-sm">No recording available</p>
                  </div>
                </div>
              )}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="space-y-4 p-6">
            <div>
              <p className="text-muted-foreground text-xs">Status</p>
              <div className="mt-1">
                <StatusBadge status={run.status} />
              </div>
            </div>
            {run.started_at && (
              <div>
                <p className="text-muted-foreground text-xs">Started</p>
                <p className="text-sm">{formatDateTime(run.started_at)}</p>
              </div>
            )}
            {run.ended_at && (
              <div>
                <p className="text-muted-foreground text-xs">Ended</p>
                <p className="text-sm">{formatDateTime(run.ended_at)}</p>
              </div>
            )}
            {run.duration_secs != null && (
              <div>
                <p className="text-muted-foreground text-xs">Duration</p>
                <p className="text-sm">{formatDuration(run.duration_secs)}</p>
              </div>
            )}
            <div>
              <p className="text-muted-foreground text-xs">Trigger</p>
              <p className="text-sm">{run.trigger}</p>
            </div>
            {run.error && run.error !== "stop_requested" && (
              <div className="border-destructive/30 bg-destructive/5 rounded-lg border p-3">
                <p className="text-destructive text-xs font-medium">Error</p>
                <p className="text-destructive/80 mt-1 text-sm">{run.error}</p>
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <CardTitle className="flex items-center gap-2 text-base">
                <FileText className="size-4" />
                Transcript and AI outputs
              </CardTitle>
              <CardDescription>
                Transcription, speaker-attributed transcript, notes, and action items for this
                recording.
              </CardDescription>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              {generatedNote ? (
                <div className="flex flex-wrap gap-1.5">
                  <Badge variant={aiStatusVariant(generatedNote.transcript_status)}>
                    Transcript {generatedNote.transcript_status}
                  </Badge>
                  <Badge variant={aiStatusVariant(generatedNote.summary_status)}>
                    Notes {generatedNote.summary_status || "pending"}
                  </Badge>
                </div>
              ) : (
                <Badge variant="outline">Awaiting AI pipeline</Badge>
              )}
              {canRerunAi && (
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => void rerunAi()}
                  disabled={rerunningAi || aiBusy}
                >
                  {rerunningAi || aiBusy ? (
                    <Loader2 className="size-3.5 animate-spin" />
                  ) : (
                    <RefreshCw className="size-3.5" />
                  )}
                  {aiBusy ? "Running…" : "Re-run AI"}
                </Button>
              )}
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-6">
          {!generatedNote ? (
            <p className="text-muted-foreground text-sm">
              {run.status === "completed"
                ? "The recording is complete, but transcript data has not been created yet. Use Re-run AI to start the pipeline."
                : "Transcript data will appear here once the recording completes and the AI pipeline starts."}
            </p>
          ) : (
            <>
              {aiBusy && (
                <div className="text-muted-foreground flex items-center gap-2 text-sm">
                  <Loader2 className="size-4 animate-spin" />
                  AI pipeline in progress — this page refreshes automatically.
                </div>
              )}

              {generatedNote.error && (
                <div className="border-destructive/30 bg-destructive/5 flex gap-3 rounded-lg border p-3">
                  <AlertCircle className="text-destructive mt-0.5 size-4 shrink-0" />
                  <p className="text-destructive text-sm">{generatedNote.error}</p>
                </div>
              )}

              <div className="grid gap-4 lg:grid-cols-3">
                <div className="lg:col-span-2">
                  <div className="mb-2 flex items-center gap-2">
                    <Sparkles className="text-primary size-4" />
                    <h3 className="text-sm font-semibold">TL;DR</h3>
                  </div>
                  <p className="text-muted-foreground text-sm leading-relaxed">
                    {generatedNote.tldr || "TL;DR will appear after notes are generated."}
                  </p>
                </div>

                <div>
                  <div className="mb-2 flex items-center gap-2">
                    <ListChecks className="text-primary size-4" />
                    <h3 className="text-sm font-semibold">Action items</h3>
                  </div>
                  {generatedNote.action_items.length === 0 ? (
                    <p className="text-muted-foreground text-sm">No action items generated yet.</p>
                  ) : (
                    <div className="space-y-2">
                      {generatedNote.action_items.slice(0, 4).map((item, index) => (
                        <div key={`${item.id}-${index}`} className="rounded-md border p-2">
                          <p className="text-sm">{item.text}</p>
                          <div className="mt-1 flex flex-wrap gap-1.5">
                            {item.owner && <Badge variant="outline">{item.owner}</Badge>}
                            {item.due && <Badge variant="secondary">Due {item.due}</Badge>}
                          </div>
                        </div>
                      ))}
                      {generatedNote.action_items.length > 4 && (
                        <p className="text-muted-foreground text-xs">
                          +{generatedNote.action_items.length - 4} more in the Notes page
                        </p>
                      )}
                    </div>
                  )}
                </div>
              </div>

              {generatedNote.summary_md && (
                <>
                  <Separator />
                  <div>
                    <h3 className="mb-3 text-sm font-semibold">Meeting notes</h3>
                    <div className="prose prose-sm dark:prose-invert max-w-none">
                      {renderMarkdownLite(generatedNote.summary_md)}
                    </div>
                  </div>
                </>
              )}

              <Separator />

              <div className="grid gap-4 lg:grid-cols-2">
                <div>
                  <div className="mb-2 flex items-center gap-2">
                    <Mic2 className="text-primary size-4" />
                    <h3 className="text-sm font-semibold">Speaker-attributed transcript</h3>
                  </div>
                  {generatedNote.cleaned_transcript ? (
                    <pre className="bg-muted text-muted-foreground max-h-96 overflow-auto rounded-lg border p-4 whitespace-pre-wrap text-sm leading-relaxed">
                      {generatedNote.cleaned_transcript}
                    </pre>
                  ) : (
                    <p className="text-muted-foreground text-sm">
                      Cleaned speaker-attributed transcript is not available yet.
                    </p>
                  )}
                </div>

                <div>
                  <h3 className="mb-2 text-sm font-semibold">Raw transcription</h3>
                  {generatedNote.raw_transcript ? (
                    <pre className="bg-muted text-muted-foreground max-h-96 overflow-auto rounded-lg border p-4 whitespace-pre-wrap text-xs leading-relaxed">
                      {generatedNote.raw_transcript}
                    </pre>
                  ) : (
                    <p className="text-muted-foreground text-sm">
                      Raw transcription is not available yet.
                    </p>
                  )}
                </div>
              </div>
            </>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardContent className="p-6">
          <div className="flex items-center justify-between gap-4">
            <div>
              <h2 className="text-base font-semibold">Meeting activity</h2>
              <p className="text-muted-foreground text-sm">
                Participant changes and detected active speaker updates from the Meet UI.
              </p>
            </div>
            <Badge variant="secondary">{timelineEvents.length} events</Badge>
          </div>

          {activeSpeakerEvent && (
            <div className="bg-muted/60 mt-6 flex items-center gap-3 rounded-lg border p-3">
              <div className="bg-background flex size-9 shrink-0 items-center justify-center rounded-full border">
                <Mic2 className="text-primary size-4" />
              </div>
              <div className="min-w-0">
                <p className="text-sm font-medium">
                  Current active speaker: {activeSpeakerEvent.participant_name}
                </p>
                <p className="text-muted-foreground text-xs">
                  Updated at{" "}
                  {activeSpeakerEvent.relative_ms != null
                    ? formatRelativeMs(activeSpeakerEvent.relative_ms)
                    : formatDateTime(activeSpeakerEvent.occurred_at)}
                </p>
              </div>
            </div>
          )}

          {timelineEvents.length === 0 ? (
            <p className="text-muted-foreground mt-6 text-sm">
              No meeting activity has been captured for this recording yet.
            </p>
          ) : (
            <div className="mt-6 space-y-4">
              {timelineEventsToShow(timelineEvents).map((event) => {
                const Icon = eventIcon(event.event_type);
                const isSpeakerEvent = event.event_type === "active_speaker_changed";
                return (
                  <div key={event.id} className="flex gap-3">
                    <div
                      className={cn(
                        "mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-full",
                        isSpeakerEvent ? "bg-primary/10" : "bg-muted",
                      )}
                    >
                      <Icon
                        className={cn(
                          "size-4",
                          isSpeakerEvent ? "text-primary" : "text-muted-foreground",
                        )}
                      />
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <p className="text-sm font-medium">{eventTitle(event)}</p>
                        {isSpeakerEvent && <Badge variant="secondary">Active speaker</Badge>}
                        {event.relative_ms != null && (
                          <Badge variant="outline">{formatRelativeMs(event.relative_ms)}</Badge>
                        )}
                      </div>
                      <p className="text-muted-foreground mt-1 text-xs">
                        {formatDateTime(event.occurred_at)}
                        {event.participant_id ? ` · ${event.participant_id}` : ""}
                      </p>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
