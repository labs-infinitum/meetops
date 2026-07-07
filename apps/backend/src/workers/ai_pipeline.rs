use std::sync::Arc;
use std::time::Duration;

use crate::{
    ai::groq::GroqClient,
    config::Config,
    db::{
        self, DbPool, bot_runs, jobs, meeting_timeline,
        models::{Job, MeetingTimelineEvent},
        summaries, transcripts,
    },
    shutdown::AppShutdown,
    storage::RecordingsStore,
};

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const MAX_ATTEMPTS: i64 = 3;
const TIMELINE_EVENT_LIMIT: i64 = 10_000;

pub fn spawn(
    config: Arc<Config>,
    db: DbPool,
    http: reqwest::Client,
    recordings: Arc<RecordingsStore>,
    shutdown: Arc<AppShutdown>,
) {
    let client = match GroqClient::from_config(http, &config.groq) {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(%error, "AI pipeline worker disabled");
            return;
        }
    };

    let mut cancelled = shutdown.cancelled();
    tokio::spawn(async move {
        loop {
            if *cancelled.borrow() {
                tracing::info!("AI pipeline worker stopped for shutdown");
                break;
            }

            if let Err(error) = drain_ready_jobs(&db, &recordings, &client).await {
                tracing::error!(%error, "AI pipeline worker tick failed");
            }

            tokio::select! {
                _ = tokio::time::sleep(POLL_INTERVAL) => {}
                changed = cancelled.changed() => {
                    if changed.is_ok() && *cancelled.borrow() {
                        tracing::info!("AI pipeline worker stopped for shutdown");
                        break;
                    }
                }
            }
        }
    });
}

pub async fn enqueue_transcription(pool: &DbPool, bot_run_id: i64) -> sqlx::Result<Option<i64>> {
    jobs::enqueue_for_bot_run(
        pool,
        jobs::KIND_TRANSCRIBE_RECORDING,
        bot_run_id,
        None,
        None,
    )
    .await
}

async fn drain_ready_jobs(
    pool: &DbPool,
    recordings: &RecordingsStore,
    client: &GroqClient,
) -> anyhow::Result<()> {
    while let Some(job) = jobs::claim_next(pool).await? {
        process_claimed_job(pool, recordings, client, job).await;
    }
    Ok(())
}

async fn process_claimed_job(
    pool: &DbPool,
    recordings: &RecordingsStore,
    client: &GroqClient,
    job: Job,
) {
    let job_id = job.id;
    let kind = job.kind.clone();
    let result = match kind.as_str() {
        jobs::KIND_TRANSCRIBE_RECORDING => {
            transcribe_recording(pool, recordings, client, &job).await
        }
        jobs::KIND_CLEAN_TRANSCRIPT => clean_transcript(pool, client, &job).await,
        jobs::KIND_GENERATE_MEETING_NOTES => generate_meeting_notes(pool, client, &job).await,
        other => Err(anyhow::anyhow!("unknown AI pipeline job kind: {other}")),
    };

    match result {
        Ok(()) => {
            if let Err(error) = jobs::complete(pool, job_id).await {
                tracing::error!(%error, job_id, kind, "failed to complete AI pipeline job");
            }
        }
        Err(error) => {
            let message = error.to_string();
            mark_stage_failed_best_effort(pool, &job, &message).await;
            if let Err(update_error) = jobs::fail_or_retry(pool, &job, &message, MAX_ATTEMPTS).await
            {
                tracing::error!(
                    %update_error,
                    job_id,
                    kind,
                    "failed to mark AI pipeline job failed"
                );
            }
            tracing::warn!(%message, job_id, kind, attempts = job.attempts, "AI pipeline job failed");
        }
    }
}

async fn transcribe_recording(
    pool: &DbPool,
    recordings: &RecordingsStore,
    client: &GroqClient,
    job: &Job,
) -> anyhow::Result<()> {
    let bot_run_id = require_bot_run_id(job)?;
    let user_id = bot_runs::get_user_id(pool, bot_run_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("bot run {bot_run_id} has no user_id"))?;
    transcripts::upsert_processing(pool, bot_run_id).await?;
    let audio = recordings
        .download_bot_run_audio(&user_id, bot_run_id)
        .await?;
    let result = client.transcribe_audio(audio).await?;
    let transcript = transcripts::save_raw(
        pool,
        bot_run_id,
        &result.text,
        result.segments_json.as_deref(),
        result.language.as_deref(),
    )
    .await?;
    jobs::enqueue_for_bot_run(
        pool,
        jobs::KIND_CLEAN_TRANSCRIPT,
        bot_run_id,
        Some(transcript.id),
        None,
    )
    .await?;
    tracing::info!(
        bot_run_id,
        transcript_id = transcript.id,
        "transcribed recording audio"
    );
    let _ = transcript;
    Ok(())
}

async fn clean_transcript(pool: &DbPool, client: &GroqClient, job: &Job) -> anyhow::Result<()> {
    let transcript = load_job_transcript(pool, job).await?;
    let raw_text = transcript
        .raw_text
        .as_deref()
        .or(transcript.text.as_deref())
        .ok_or_else(|| anyhow::anyhow!("transcript {} has no raw text", transcript.id))?;
    let events =
        meeting_timeline::list_for_run(pool, transcript.bot_run_id, TIMELINE_EVENT_LIMIT).await?;
    let events_json = serde_json::to_string(&timeline_events_for_prompt(&events))?;
    let cleaned = client
        .clean_transcript(raw_text, transcript.segments_json.as_deref(), &events_json)
        .await?;
    let speaker_turns_json = serde_json::to_string(&cleaned.speaker_turns)?;
    let transcript = transcripts::save_cleaned(
        pool,
        transcript.id,
        &cleaned.cleaned_transcript,
        Some(&speaker_turns_json),
    )
    .await?;
    jobs::enqueue_for_bot_run(
        pool,
        jobs::KIND_GENERATE_MEETING_NOTES,
        transcript.bot_run_id,
        Some(transcript.id),
        None,
    )
    .await?;
    tracing::info!(
        bot_run_id = transcript.bot_run_id,
        transcript_id = transcript.id,
        "cleaned speaker-attributed transcript"
    );
    Ok(())
}

async fn generate_meeting_notes(
    pool: &DbPool,
    client: &GroqClient,
    job: &Job,
) -> anyhow::Result<()> {
    let transcript = load_job_transcript(pool, job).await?;
    let cleaned_text = transcript
        .cleaned_text
        .as_deref()
        .or(transcript.text.as_deref())
        .ok_or_else(|| anyhow::anyhow!("transcript {} has no cleaned text", transcript.id))?;
    summaries::upsert_processing(pool, transcript.id).await?;
    let notes = client.generate_meeting_notes(cleaned_text).await?;
    let action_items_json = serde_json::to_string(&notes.action_items)?;
    summaries::save_completed(
        pool,
        transcript.id,
        &notes.tldr,
        &notes.summary_md,
        &action_items_json,
    )
    .await?;
    tracing::info!(
        bot_run_id = transcript.bot_run_id,
        transcript_id = transcript.id,
        "generated meeting notes"
    );
    Ok(())
}

async fn load_job_transcript(pool: &DbPool, job: &Job) -> anyhow::Result<db::models::Transcript> {
    if let Some(transcript_id) = job.transcript_id {
        return transcripts::get_by_id(pool, transcript_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("transcript {transcript_id} not found"));
    }
    let bot_run_id = require_bot_run_id(job)?;
    transcripts::get_by_bot_run(pool, bot_run_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("transcript for bot run {bot_run_id} not found"))
}

async fn mark_stage_failed_best_effort(pool: &DbPool, job: &Job, error: &str) {
    match job.kind.as_str() {
        jobs::KIND_TRANSCRIBE_RECORDING | jobs::KIND_CLEAN_TRANSCRIPT => {
            if let Ok(transcript) = load_job_transcript(pool, job).await {
                let _ = transcripts::mark_failed(pool, transcript.id, error).await;
            }
        }
        jobs::KIND_GENERATE_MEETING_NOTES => {
            if let Ok(transcript) = load_job_transcript(pool, job).await {
                let _ = summaries::mark_failed(pool, transcript.id, error).await;
            }
        }
        _ => {}
    }
}

fn require_bot_run_id(job: &Job) -> anyhow::Result<i64> {
    job.bot_run_id
        .ok_or_else(|| anyhow::anyhow!("job {} has no bot_run_id", job.id))
}

fn timeline_events_for_prompt(events: &[MeetingTimelineEvent]) -> Vec<serde_json::Value> {
    events
        .iter()
        .filter(|event| event.event_type == "active_speaker_changed")
        .map(|event| {
            serde_json::json!({
                "event_type": event.event_type,
                "relative_ms": event.relative_ms,
                "participant_id": event.participant_id,
                "participant_name": event.participant_name,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::db::models::MeetingTimelineEvent;

    use super::timeline_events_for_prompt;

    #[test]
    fn timeline_prompt_keeps_only_active_speaker_changes() {
        let events = vec![
            MeetingTimelineEvent {
                id: 1,
                bot_run_id: 2,
                event_type: "active_speaker_changed".to_owned(),
                occurred_at: 10,
                relative_ms: 1500,
                participant_id: Some("p1".to_owned()),
                participant_name: Some("Alice".to_owned()),
                payload_json: Some(r#"{"source":"tile_visual_effect"}"#.to_owned()),
                created_at: 10,
            },
            MeetingTimelineEvent {
                id: 2,
                bot_run_id: 2,
                event_type: "participant_snapshot".to_owned(),
                occurred_at: 11,
                relative_ms: 1600,
                participant_id: None,
                participant_name: None,
                payload_json: Some(r#"{"participants":[]}"#.to_owned()),
                created_at: 11,
            },
        ];
        let prompt_events = timeline_events_for_prompt(&events);
        assert_eq!(prompt_events.len(), 1);
        assert_eq!(prompt_events[0]["event_type"], "active_speaker_changed");
        assert!(prompt_events[0].get("payload").is_none());
    }
}
