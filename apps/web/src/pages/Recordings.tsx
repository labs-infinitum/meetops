import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { ChevronRight, Film, Loader2 } from "lucide-react";
import { toast } from "sonner";
import { Alert } from "@/components/Alert";
import { EmptyState } from "@/components/EmptyState";
import { PageLoader } from "@/components/Spinner";
import { PageHeader } from "@/components/shared/PageHeader";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { api } from "@/lib/api";
import { formatDate, formatDateTime, formatDuration } from "@/lib/format";
import type { BotRun } from "@/types/api";

function recordingTitle(run: BotRun) {
  return run.meeting_title || run.meet_url || `Recording #${run.id}`;
}

export function RecordingsPage() {
  const [recordings, setRecordings] = useState<BotRun[]>([]);
  const [loading, setLoading] = useState(true);
  const [meetUrl, setMeetUrl] = useState("");
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    void api
      .get<BotRun[]>("/recordings/")
      .then(setRecordings)
      .catch(() => toast.error("Failed to load recordings"))
      .finally(() => setLoading(false));
  }, []);

  async function startAdhoc(e: React.FormEvent) {
    e.preventDefault();
    setError("");
    setStarting(true);
    try {
      const run = await api.post<BotRun>("/recordings/adhoc", { meet_url: meetUrl });
      setRecordings((prev) => [run, ...prev]);
      setMeetUrl("");
      toast.success("Ad-hoc recording queued");
    } catch {
      setError("Failed to start recording. Check the Meet URL.");
    } finally {
      setStarting(false);
    }
  }

  if (loading) return <PageLoader />;

  return (
    <div className="space-y-8">
      <PageHeader
        title="Recordings"
        description="Bot recording activity for your meetings."
      />

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Record a Meet now</CardTitle>
          <CardDescription>
            Paste a Google Meet URL to start an ad-hoc recording
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={(e) => void startAdhoc(e)} className="space-y-3">
            {error && <Alert>{error}</Alert>}
            <div className="flex gap-2">
              <Input
                placeholder="https://meet.google.com/abc-defg-hij"
                value={meetUrl}
                onChange={(e) => setMeetUrl(e.target.value)}
                required
              />
              <Button type="submit" disabled={starting} className="shrink-0">
                {starting && <Loader2 className="animate-spin" />}
                Start
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>

      {recordings.length === 0 ? (
        <EmptyState
          icon={Film}
          title="No recordings yet"
          description="Start an ad-hoc recording or wait for a scheduled meeting to be recorded."
          action={
            <Button asChild>
              <Link to="/app/meetings">View meetings</Link>
            </Button>
          }
        />
      ) : (
        <Card>
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Meeting</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead className="hidden md:table-cell">Started</TableHead>
                  <TableHead className="hidden sm:table-cell">Duration</TableHead>
                  <TableHead className="w-10" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {recordings.map((r) => (
                  <TableRow key={r.id}>
                    <TableCell>
                      <div>
                        <p className="font-medium">{recordingTitle(r)}</p>
                        <p className="text-muted-foreground text-xs">
                          {formatDate(r.meeting_start_at || r.created_at)}
                        </p>
                      </div>
                    </TableCell>
                    <TableCell>
                      <StatusBadge status={r.status} />
                    </TableCell>
                    <TableCell className="text-muted-foreground hidden text-sm md:table-cell">
                      {r.started_at ? formatDateTime(r.started_at) : "—"}
                    </TableCell>
                    <TableCell className="text-muted-foreground hidden text-sm sm:table-cell">
                      {r.duration_secs != null ? formatDuration(r.duration_secs) : "—"}
                    </TableCell>
                    <TableCell>
                      <Button variant="ghost" size="icon" asChild>
                        <Link to={`/app/recordings/${r.id}`}>
                          <ChevronRight className="size-4" />
                        </Link>
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
