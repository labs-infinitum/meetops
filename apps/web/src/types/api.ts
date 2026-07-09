export interface ApiErrorBody {
  error: {
    code: string;
    message: string;
  };
}

export interface User {
  id: string;
  username: string;
  email: string;
  name: string;
  email_verified_at: string | null;
  created_at: string;
}

export interface MeResponse {
  user: User;
}

export interface LoginResponse {
  user: User;
}

export interface SignupResponse {
  user: User;
  email_verification_required: boolean;
}

export interface MessageResponse {
  message: string;
}

export interface Settings {
  name: string;
  email: string;
  username: string;
  auto_record_default: boolean;
  join_lead_time_secs: number;
  bot_display_name: string | null;
  notify_email_on_notes_ready: boolean;
  notify_email_on_recording_failed: boolean;
}

export interface CalendarConnection {
  id: string;
  provider: string;
  email: string;
  label: string | null;
  color: string | null;
  last_synced_at: string | null;
  bot_session_status: string;
  created_at: string;
}

export interface Meeting {
  id: string;
  title: string;
  description: string | null;
  meet_url: string | null;
  start_at: string;
  end_at: string;
  auto_join: boolean;
  calendar_connection_id: string;
  attendees_json: unknown;
}

export interface Dashboard {
  upcoming_count: number;
  recordings_count: number;
  notes_count: number;
  open_todos_count: number;
  upcoming: Meeting[];
  latest_summary: {
    id: string;
    tldr: string | null;
    summary_md: string | null;
    meeting_title: string | null;
    bot_run_id: string;
    created_at: string;
    action_items: Todo[];
  } | null;
}

export interface BotRun {
  id: string;
  meeting_id: string | null;
  status: string;
  error: string | null;
  trigger: string;
  recording_path: string | null;
  meet_url: string | null;
  meeting_title: string | null;
  meeting_start_at: string | null;
  duration_secs: number | null;
  started_at: string | null;
  ended_at: string | null;
  created_at: string;
}

export interface PlaybackUrl {
  url: string;
  expires_at: string;
}

export interface TimelineEvent {
  id: string;
  event_type: string;
  occurred_at: string;
  relative_ms: number | null;
  participant_id: string | null;
  participant_name: string | null;
  payload_json: unknown;
}

export interface Note {
  id: string;
  transcript_id: string;
  bot_run_id: string;
  status: string;
  transcript_status: string | null;
  summary_status: string | null;
  tldr: string | null;
  summary_md: string | null;
  raw_transcript: string | null;
  cleaned_transcript: string | null;
  meeting_title: string | null;
  error: string | null;
  created_at: string;
  action_items: Todo[];
}

export interface Todo {
  id: string;
  text: string;
  owner: string | null;
  due: string | null;
  completed: boolean;
  completed_at: string | null;
  bot_run_id: string;
  created_at: string;
}

export interface SyncResult {
  seen: number;
  upserted: number;
  deleted: number;
  last_synced_at: string | null;
}

export interface OAuthStartResponse {
  redirect_url: string;
}
