use std::{error::Error, time::Duration};

use serde::{Deserialize, Serialize};

use crate::config::GroqConfig;

const GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1";
const GROQ_TRANSCRIPTION_MAX_BYTES: usize = 100 * 1024 * 1024;
const CLEAN_TRANSCRIPT_RAW_MAX_CHARS: usize = 12_000;
const CLEAN_TRANSCRIPT_SEGMENTS_MAX_CHARS: usize = 4_000;
const CLEAN_TRANSCRIPT_TIMELINE_MAX_CHARS: usize = 4_000;
const MEETING_NOTES_TRANSCRIPT_MAX_CHARS: usize = 14_000;

#[derive(Clone)]
pub struct GroqClient {
    http: reqwest::Client,
    api_key: String,
    transcription_model: String,
    summarization_model: String,
}

#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    pub language: Option<String>,
    pub segments_json: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CleanTranscriptResult {
    pub cleaned_transcript: String,
    #[serde(default)]
    pub speaker_turns: Vec<SpeakerTurn>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpeakerTurn {
    pub speaker: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MeetingNotesResult {
    pub tldr: String,
    pub summary_md: String,
    #[serde(default)]
    pub action_items: Vec<ActionItem>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActionItem {
    pub text: String,
    pub owner: Option<String>,
    pub due: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GroqTranscriptionResponse {
    text: String,
    language: Option<String>,
    segments: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
}

impl GroqClient {
    pub fn from_config(http: reqwest::Client, config: &GroqConfig) -> anyhow::Result<Self> {
        let api_key = std::env::var(&config.api_key_env)
            .map_err(|_| anyhow::anyhow!("{} is not set", config.api_key_env))?;
        if api_key.trim().is_empty() {
            anyhow::bail!("{} is empty", config.api_key_env);
        }
        let http = reqwest::Client::builder()
            .http1_only()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(180))
            .build()
            .unwrap_or(http);
        Ok(Self {
            http,
            api_key,
            transcription_model: config.transcription_model.clone(),
            summarization_model: config.summarization_model.clone(),
        })
    }

    pub async fn transcribe_audio(
        &self,
        audio_webm: Vec<u8>,
    ) -> anyhow::Result<TranscriptionResult> {
        let audio_bytes = audio_webm.len();
        if audio_bytes == 0 {
            anyhow::bail!("recording audio artifact is empty");
        }
        if audio_bytes > GROQ_TRANSCRIPTION_MAX_BYTES {
            anyhow::bail!(
                "recording audio artifact is too large for direct Groq transcription upload: {} bytes > {} bytes",
                audio_bytes,
                GROQ_TRANSCRIPTION_MAX_BYTES
            );
        }
        let response = match self
            .send_transcription_request(&audio_webm, audio_bytes)
            .await
        {
            Ok(response) => response,
            Err(first_error) if is_transient_send_error(&first_error) => {
                tracing::warn!(
                    %first_error,
                    model = %self.transcription_model,
                    audio_bytes,
                    "Groq transcription request send failed; retrying once"
                );
                self.send_transcription_request(&audio_webm, audio_bytes)
                    .await?
            }
            Err(error) => return Err(error),
        };
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            anyhow::bail!("Groq transcription failed ({status}): {body}");
        }

        let parsed: GroqTranscriptionResponse = serde_json::from_str(&body).map_err(|error| {
            anyhow::anyhow!("invalid Groq transcription response: {error}; body={body}")
        })?;
        let segments_json = parsed.segments.map(|value| value.to_string());
        Ok(TranscriptionResult {
            text: parsed.text,
            language: parsed.language,
            segments_json,
        })
    }

    async fn send_transcription_request(
        &self,
        audio_webm: &[u8],
        audio_bytes: usize,
    ) -> anyhow::Result<reqwest::Response> {
        let file_part = reqwest::multipart::Part::bytes(audio_webm.to_vec())
            .file_name("audio.webm")
            .mime_str("audio/webm")?;
        let form = reqwest::multipart::Form::new()
            .text("model", self.transcription_model.clone())
            .text("response_format", "verbose_json")
            .part("file", file_part);

        self.http
            .post(format!("{GROQ_BASE_URL}/audio/transcriptions"))
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|error| {
                transcription_send_error(error, &self.transcription_model, audio_bytes)
            })
    }

    pub async fn clean_transcript(
        &self,
        raw_text: &str,
        segments_json: Option<&str>,
        timeline_events_json: &str,
    ) -> anyhow::Result<CleanTranscriptResult> {
        let raw_text_for_prompt = truncate_for_prompt(raw_text, CLEAN_TRANSCRIPT_RAW_MAX_CHARS);
        let segments_for_prompt = segments_json
            .and_then(compact_segments_for_prompt)
            .map(|segments| truncate_for_prompt(&segments, CLEAN_TRANSCRIPT_SEGMENTS_MAX_CHARS));
        let timeline_for_prompt =
            truncate_for_prompt(timeline_events_json, CLEAN_TRANSCRIPT_TIMELINE_MAX_CHARS);
        if raw_text_for_prompt.truncated
            || segments_for_prompt
                .as_ref()
                .is_some_and(|segments| segments.truncated)
            || timeline_for_prompt.truncated
        {
            tracing::warn!(
                raw_chars = raw_text.chars().count(),
                raw_prompt_chars = raw_text_for_prompt.text.chars().count(),
                segments_prompt_chars = segments_for_prompt
                    .as_ref()
                    .map(|segments| segments.text.chars().count())
                    .unwrap_or(0),
                timeline_chars = timeline_events_json.chars().count(),
                timeline_prompt_chars = timeline_for_prompt.text.chars().count(),
                "truncated Groq transcript-cleaning prompt to fit on-demand TPM limits"
            );
        }
        let prompt = format!(
            "Create a speaker-attributed cleaned transcript from the raw transcription and meeting timeline events.\n\
             Use active_speaker_changed event times to infer speaker turns. Preserve meaning, remove filler where safe, and do not invent speakers.\n\
             Return strict JSON with keys: cleaned_transcript (markdown string), speaker_turns (array of objects with speaker, start_ms, end_ms, text).\n\n\
             Raw transcription{}:\n{}\n\n\
             Compact transcription segments JSON{}:\n{}\n\n\
             Active-speaker timeline JSON{}:\n{}",
            truncation_suffix(raw_text_for_prompt.truncated),
            raw_text_for_prompt.text,
            truncation_suffix(
                segments_for_prompt
                    .as_ref()
                    .is_some_and(|segments| segments.truncated)
            ),
            segments_for_prompt
                .as_ref()
                .map(|segments| segments.text.as_str())
                .unwrap_or("null"),
            truncation_suffix(timeline_for_prompt.truncated),
            timeline_for_prompt.text
        );
        let content = self
            .chat_json(
                "You are a precise meeting transcript editor. Return only valid JSON.",
                &prompt,
            )
            .await?;
        parse_json_content(&content)
    }

    pub async fn generate_meeting_notes(
        &self,
        cleaned_transcript: &str,
    ) -> anyhow::Result<MeetingNotesResult> {
        let cleaned_transcript_for_prompt =
            truncate_for_prompt(cleaned_transcript, MEETING_NOTES_TRANSCRIPT_MAX_CHARS);
        if cleaned_transcript_for_prompt.truncated {
            tracing::warn!(
                transcript_chars = cleaned_transcript.chars().count(),
                prompt_chars = cleaned_transcript_for_prompt.text.chars().count(),
                "truncated Groq meeting-notes prompt to fit on-demand TPM limits"
            );
        }
        let prompt = format!(
            "Create meeting notes from this speaker-attributed cleaned transcript.\n\
             Return strict JSON with keys: tldr (string), summary_md (markdown string), action_items (array of objects with text, owner, due).\n\
             Use null for unknown owner or due. Do not invent action items.\n\n\
             Cleaned transcript{}:\n{}",
            truncation_suffix(cleaned_transcript_for_prompt.truncated),
            cleaned_transcript_for_prompt.text
        );
        let content = self
            .chat_json(
                "You turn meeting transcripts into concise, faithful meeting notes. Return only valid JSON.",
                &prompt,
            )
            .await?;
        parse_json_content(&content)
    }

    async fn chat_json(&self, system: &str, user: &str) -> anyhow::Result<String> {
        match self.chat_json_request(system, user, true).await {
            Ok(content) => Ok(content),
            Err(error) if error.to_string().contains("json_validate_failed") => {
                tracing::warn!(
                    %error,
                    model = %self.summarization_model,
                    "Groq JSON mode failed validation; retrying without response_format"
                );
                self.chat_json_request(system, user, false).await
            }
            Err(error) => Err(error),
        }
    }

    async fn chat_json_request(
        &self,
        system: &str,
        user: &str,
        json_mode: bool,
    ) -> anyhow::Result<String> {
        let body = self.chat_request_body(system, user, json_mode);

        let response = self
            .http
            .post(format!("{GROQ_BASE_URL}/chat/completions"))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            anyhow::bail!("Groq chat completion failed ({status}): {body}");
        }

        let parsed: ChatCompletionResponse = serde_json::from_str(&body)
            .map_err(|error| anyhow::anyhow!("invalid Groq chat response: {error}; body={body}"))?;
        parsed
            .choices
            .into_iter()
            .find_map(|choice| choice.message.content)
            .ok_or_else(|| anyhow::anyhow!("Groq chat response contained no content"))
    }

    fn chat_request_body(&self, system: &str, user: &str, json_mode: bool) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.summarization_model,
            "messages": [
                {
                    "role": "system",
                    "content": format!("{system}\nReturn only valid JSON. Do not include markdown fences or explanatory text.")
                },
                { "role": "user", "content": user }
            ],
            "temperature": 0.2
        });
        if json_mode {
            body["response_format"] = serde_json::json!({ "type": "json_object" });
        }
        if self.summarization_model.contains("qwen3") {
            // Groq docs: Qwen3 supports `none` or `default`; structured JSON output should
            // run with reasoning disabled so thinking text cannot break JSON validation.
            body["reasoning_effort"] = serde_json::json!("none");
        }
        body
    }
}

fn parse_json_content<T>(content: &str) -> anyhow::Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(content)
        .or_else(|first_error| {
            let Some(start) = content.find('{') else {
                return Err(first_error);
            };
            let Some(end) = content.rfind('}') else {
                return Err(first_error);
            };
            serde_json::from_str(&content[start..=end])
        })
        .map_err(|error| anyhow::anyhow!("LLM returned invalid JSON: {error}; content={content}"))
}

#[derive(Debug)]
struct PromptText {
    text: String,
    truncated: bool,
}

fn truncate_for_prompt(value: &str, max_chars: usize) -> PromptText {
    if value.chars().count() <= max_chars {
        return PromptText {
            text: value.to_owned(),
            truncated: false,
        };
    }

    let mut text: String = value.chars().take(max_chars).collect();
    text.push_str("\n[TRUNCATED]");
    PromptText {
        text,
        truncated: true,
    }
}

fn truncation_suffix(truncated: bool) -> &'static str {
    if truncated { " (truncated)" } else { "" }
}

fn compact_segments_for_prompt(segments_json: &str) -> Option<String> {
    let segments = serde_json::from_str::<serde_json::Value>(segments_json).ok()?;
    let array = segments.as_array()?;
    let compact: Vec<serde_json::Value> = array
        .iter()
        .filter_map(|segment| {
            let text = segment.get("text")?.as_str()?.trim();
            if text.is_empty() {
                return None;
            }
            Some(serde_json::json!({
                "start": segment.get("start").and_then(|value| value.as_f64()),
                "end": segment.get("end").and_then(|value| value.as_f64()),
                "text": text,
            }))
        })
        .collect();
    if compact.is_empty() {
        None
    } else {
        serde_json::to_string(&compact).ok()
    }
}

fn transcription_send_error(
    error: reqwest::Error,
    model: &str,
    audio_bytes: usize,
) -> anyhow::Error {
    anyhow::anyhow!(
        "failed to send Groq transcription request: model={} audio_bytes={} timeout={} connect={} request={} body={} source={}",
        model,
        audio_bytes,
        error.is_timeout(),
        error.is_connect(),
        error.is_request(),
        error.is_body(),
        error
            .source()
            .map(|source| source.to_string())
            .unwrap_or_else(|| "none".to_owned())
    )
}

fn is_transient_send_error(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.contains("timeout=true")
        || message.contains("connect=true")
        || message.contains("request=true")
        || message.contains("SendRequest")
}

#[cfg(test)]
mod tests {
    use super::{CleanTranscriptResult, GroqClient, is_transient_send_error, parse_json_content};

    #[test]
    fn parses_json_inside_model_text() {
        let parsed: CleanTranscriptResult = parse_json_content(
            r#"Here is JSON:
            {"cleaned_transcript":"Alice: Hello","speaker_turns":[]}"#,
        )
        .expect("json should parse");
        assert_eq!(parsed.cleaned_transcript, "Alice: Hello");
    }

    #[test]
    fn qwen3_json_request_disables_reasoning() {
        let client = GroqClient {
            http: reqwest::Client::new(),
            api_key: "test".to_owned(),
            transcription_model: "whisper-large-v3-turbo".to_owned(),
            summarization_model: "qwen/qwen3-32b".to_owned(),
        };
        let body = client.chat_request_body("system", "user", true);
        assert_eq!(body["reasoning_effort"], "none");
        assert_eq!(body["response_format"]["type"], "json_object");
        assert!(
            body["messages"][0]["content"]
                .as_str()
                .expect("system message")
                .contains("valid JSON")
        );
    }

    #[tokio::test]
    async fn rejects_empty_audio_before_upload() {
        let client = GroqClient {
            http: reqwest::Client::new(),
            api_key: "test".to_owned(),
            transcription_model: "whisper-large-v3-turbo".to_owned(),
            summarization_model: "qwen/qwen3-32b".to_owned(),
        };
        let error = client
            .transcribe_audio(Vec::new())
            .await
            .expect_err("empty audio should fail before request");
        assert!(error.to_string().contains("empty"));
    }

    #[test]
    fn classifies_send_request_as_transient() {
        let error = anyhow::anyhow!(
            "failed to send Groq transcription request: model=whisper-large-v3-turbo audio_bytes=10 timeout=false connect=false request=true body=false source=client error (SendRequest)"
        );
        assert!(is_transient_send_error(&error));
    }

    #[test]
    fn compact_segments_removes_verbose_fields() {
        let compact = super::compact_segments_for_prompt(
            r#"[{"id":0,"start":1.25,"end":2.5,"text":" Hello ","tokens":[1,2],"words":[{"word":"Hello"}]}]"#,
        )
        .expect("segments should compact");
        let parsed: serde_json::Value = serde_json::from_str(&compact).expect("valid json");
        assert_eq!(parsed[0]["start"], 1.25);
        assert_eq!(parsed[0]["end"], 2.5);
        assert_eq!(parsed[0]["text"], "Hello");
        assert!(!compact.contains("tokens"));
        assert!(!compact.contains("words"));
    }

    #[test]
    fn truncate_for_prompt_is_utf8_safe() {
        let prompt = super::truncate_for_prompt("hello नमस्ते", 8);
        assert!(prompt.truncated);
        assert!(prompt.text.ends_with("[TRUNCATED]"));
    }
}
