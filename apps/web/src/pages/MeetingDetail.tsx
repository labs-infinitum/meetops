import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { ExternalLink, Loader2 } from "lucide-react";
import { Alert } from "@/components/Alert";
import { PageLoader } from "@/components/Spinner";
import { PageHeader } from "@/components/shared/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { ApiError, api } from "@/lib/api";
import type { Meeting } from "@/types/api";

function formatDate(iso: string) {
  return new Date(iso).toLocaleString();
}

export function MeetingDetailPage() {
  const { id } = useParams<{ id: string }>();
  const [meeting, setMeeting] = useState<Meeting | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!id) return;
    void api
      .get<Meeting>(`/meetings/${id}`)
      .then((m) => {
        setMeeting(m);
        setLoading(false);
      })
      .catch((err) => {
        if (err instanceof ApiError) setError(err.message);
        setLoading(false);
      });
  }, [id]);

  async function toggleAutoRecord(checked: boolean) {
    if (!meeting) return;
    setSaving(true);
    setError("");
    try {
      const updated = await api.patch<Meeting>(`/meetings/${meeting.id}`, {
        auto_record: checked,
      });
      setMeeting(updated);
    } catch (err) {
      if (err instanceof ApiError) setError(err.message);
    } finally {
      setSaving(false);
    }
  }

  if (loading) return <PageLoader />;

  if (!meeting) {
    return (
      <div className="space-y-4">
        <PageHeader title="Meeting not found" />
        <Alert>{error || "Meeting not found"}</Alert>
        <Button asChild variant="outline">
          <Link to="/app/meetings">Back to meetings</Link>
        </Button>
      </div>
    );
  }

  const isPast = new Date(meeting.end_at).getTime() < Date.now();

  return (
    <div className="space-y-8">
      <PageHeader
        title={meeting.title}
        description={`${formatDate(meeting.start_at)} – ${formatDate(meeting.end_at)}`}
        action={
          meeting.meet_url ? (
            <Button asChild>
              <a href={meeting.meet_url} target="_blank" rel="noreferrer">
                <ExternalLink className="size-4" />
                Open in Meet
              </a>
            </Button>
          ) : undefined
        }
      />

      {error && <Alert>{error}</Alert>}

      <div className="grid gap-6 lg:grid-cols-3">
        <Card className="lg:col-span-2">
          <CardContent className="space-y-4 p-6">
            {meeting.description && (
              <div>
                <h3 className="text-sm font-medium">Description</h3>
                <p className="text-muted-foreground mt-1 whitespace-pre-wrap text-sm">
                  {meeting.description}
                </p>
              </div>
            )}
            {meeting.meet_url && (
              <div>
                <h3 className="text-sm font-medium">Meet link</h3>
                <a
                  href={meeting.meet_url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-primary mt-1 block text-sm hover:underline"
                >
                  {meeting.meet_url}
                </a>
              </div>
            )}
            {!meeting.description && !meeting.meet_url && (
              <p className="text-muted-foreground text-sm">No additional details.</p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardContent className="space-y-4 p-6">
            <div>
              <p className="text-muted-foreground text-xs">Status</p>
              <p className="text-sm font-medium">{isPast ? "Completed" : "Upcoming"}</p>
            </div>
            <div>
              <p className="text-muted-foreground text-xs">Auto-record</p>
              <div className="mt-1">
                <Badge variant={meeting.auto_join ? "default" : "secondary"}>
                  {meeting.auto_join ? "Enabled" : "Disabled"}
                </Badge>
              </div>
            </div>
            <div className="flex items-center justify-between gap-3 border-t pt-4">
              <div className="space-y-0.5">
                <Label htmlFor="auto-record">Auto-record</Label>
                <p className="text-muted-foreground text-xs">
                  Bot joins and records this meeting
                </p>
              </div>
              <div className="flex items-center gap-2">
                {saving && <Loader2 className="text-muted-foreground size-4 animate-spin" />}
                <Switch
                  id="auto-record"
                  checked={meeting.auto_join}
                  disabled={saving}
                  onCheckedChange={(checked) => void toggleAutoRecord(checked)}
                />
              </div>
            </div>
            {isPast && (
              <Button variant="outline" className="w-full" asChild>
                <Link to="/app/notes">View notes</Link>
              </Button>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
