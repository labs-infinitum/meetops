use std::error::Error;

use serde::{Deserialize, Serialize};

use crate::config::GroqConfig;

const GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1";
const GROQ_TRANSCRIPTION_MAX_BYTES: usize = 100 * 1024 * 1024;

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
        let file_part = reqwest::multipart::Part::bytes(audio_webm)
            .file_name("audio.webm")
            .mime_str("audio/webm")?;
        let form = reqwest::multipart::Form::new()
            .text("model", self.transcription_model.clone())
            .text("response_format", "verbose_json")
            .part("file", file_part);

        let response = self
            .http
            .post(format!("{GROQ_BASE_URL}/audio/transcriptions"))
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|error| {
                anyhow::anyhow!(
                    "failed to send Groq transcription request: model={} audio_bytes={} timeout={} connect={} request={} body={} source={}",
                    self.transcription_model,
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
            })?;
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

    pub async fn clean_transcript(
        &self,
        raw_text: &str,
        segments_json: Option<&str>,
        timeline_events_json: &str,
    ) -> anyhow::Result<CleanTranscriptResult> {
        let prompt = format!(
            "Create a speaker-attributed cleaned transcript from the raw transcription and meeting timeline events.\n\
             Use active_speaker_changed event times to infer speaker turns. Preserve meaning, remove filler where safe, and do not invent speakers.\n\
             Return strict JSON with keys: cleaned_transcript (markdown string), speaker_turns (array of objects with speaker, start_ms, end_ms, text).\n\n\
             Raw transcription:\n{raw_text}\n\n\
             Transcription segments JSON:\n{}\n\n\
             Timeline events JSON:\n{timeline_events_json}",
            segments_json.unwrap_or("null")
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
        let prompt = format!(
            "Create meeting notes from this speaker-attributed cleaned transcript.\n\
             Return strict JSON with keys: tldr (string), summary_md (markdown string), action_items (array of objects with text, owner, due).\n\
             Use null for unknown owner or due. Do not invent action items.\n\n\
             Cleaned transcript:\n{cleaned_transcript}"
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

#[cfg(test)]
mod tests {
    use super::{CleanTranscriptResult, GroqClient, parse_json_content};

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
}
