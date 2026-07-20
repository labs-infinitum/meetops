"""OpenAI Whisper + LLM client for transcription and notes."""

from __future__ import annotations

import json
import logging
from pathlib import Path
from typing import Any

import httpx
from pydantic import BaseModel, Field

from meetops_common.settings import Settings

logger = logging.getLogger(__name__)

OPENAI_BASE = "https://api.openai.com/v1"
# Whisper / gpt-4o-transcribe upload limit is typically 25 MiB.
_MAX_AUDIO_BYTES = 25 * 1024 * 1024


class ActionItemDraft(BaseModel):
    text: str
    owner: str | None = None
    due: str | None = None


class NotesDraft(BaseModel):
    tldr: str
    summary_md: str
    action_items: list[ActionItemDraft] = Field(default_factory=list)


class CleanedTranscript(BaseModel):
    cleaned_text: str
    speaker_turns: list[dict[str, Any]] = Field(default_factory=list)


def _truncate(text: str, limit: int) -> str:
    if len(text) <= limit:
        return text
    return text[: limit - 20] + "\n...[truncated]..."


def _audio_content_type(path: Path) -> str:
    suffix = path.suffix.lower()
    return {
        ".webm": "audio/webm",
        ".wav": "audio/wav",
        ".mp3": "audio/mpeg",
        ".mp4": "audio/mp4",
        ".m4a": "audio/mp4",
        ".ogg": "audio/ogg",
        ".flac": "audio/flac",
        ".mpeg": "audio/mpeg",
        ".mpga": "audio/mpeg",
    }.get(suffix, "application/octet-stream")


class OpenAIClient:
    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self.api_key = settings.openai_api_key
        base = (settings.openai_base_url or OPENAI_BASE).strip().rstrip("/")
        self.base_url = base
        self.transcribe_model = settings.openai_transcribe_model
        self.llm_model = settings.openai_model

    @property
    def enabled(self) -> bool:
        return bool(self.api_key)

    def _headers(self) -> dict[str, str]:
        return {"Authorization": f"Bearer {self.api_key or ''}"}

    def _transcribe_formats(self) -> list[str]:
        # gpt-4o(-mini)-transcribe only accept json/text; whisper-1 supports verbose_json.
        model = (self.transcribe_model or "").lower()
        if "whisper" in model:
            return ["verbose_json", "json"]
        return ["json"]

    async def transcribe(
        self,
        audio_path: Path,
        *,
        languages: list[str] | None = None,
    ) -> dict[str, Any]:
        if not self.enabled:
            raise RuntimeError("OPENAI_API_KEY not configured")
        size = audio_path.stat().st_size
        if size > _MAX_AUDIO_BYTES:
            raise RuntimeError(
                f"Audio file exceeds {_MAX_AUDIO_BYTES} byte OpenAI transcription limit"
            )
        if size == 0:
            raise RuntimeError("Audio file is empty")
        from meetops_api.ai.languages import openai_transcription_language

        content_type = _audio_content_type(audio_path)
        language = openai_transcription_language(languages)
        async with httpx.AsyncClient(timeout=300) as client:
            last_error = ""
            for response_format in self._transcribe_formats():
                with audio_path.open("rb") as f:
                    files = {"file": (audio_path.name, f, content_type)}
                    data: dict[str, str] = {
                        "model": self.transcribe_model,
                        "response_format": response_format,
                    }
                    if language:
                        data["language"] = language
                    resp = await client.post(
                        f"{self.base_url}/audio/transcriptions",
                        headers=self._headers(),
                        files=files,
                        data=data,
                    )
                if resp.status_code < 400:
                    payload = resp.json()
                    if isinstance(payload, str):
                        return {"text": payload}
                    return payload
                last_error = (resp.text or "")[:500]
                if resp.status_code in {400, 404, 422} and response_format == "verbose_json":
                    logger.info(
                        "OpenAI transcribe verbose_json rejected; retrying json status=%s body=%s",
                        resp.status_code,
                        last_error,
                    )
                    continue
                logger.error(
                    "OpenAI transcribe failed status=%s body=%s",
                    resp.status_code,
                    last_error,
                )
                raise RuntimeError(f"Transcription failed: {resp.status_code} {last_error}")
            raise RuntimeError(f"Transcription failed: {last_error}")

    async def clean_transcript(
        self,
        *,
        raw_text: str,
        segments: list[dict[str, Any]] | None,
        timeline: list[dict[str, Any]] | None,
        languages: list[str] | None = None,
    ) -> CleanedTranscript:
        from meetops_api.ai.languages import language_prompt_hint

        prompt = {
            "raw_text": _truncate(raw_text or "", 12_000),
            "segments": _truncate(json.dumps(segments or [])[:4000], 4000),
            "active_speaker_timeline": _truncate(json.dumps(timeline or [])[:4000], 4000),
            "languages": languages or [],
            "language_hint": language_prompt_hint(languages),
            "instructions": (
                "Clean the transcript, attribute speakers using the timeline when possible. "
                "Preserve wording in the expected languages when provided. "
                "Return JSON: {cleaned_text: string, speaker_turns: [{speaker, text, start, end}]}"
            ),
        }
        data = await self._chat_json(
            system="You clean meeting transcripts. Respond with JSON only.",
            user=json.dumps(prompt),
        )
        return CleanedTranscript.model_validate(data)

    async def generate_notes(
        self,
        *,
        cleaned_text: str,
        title: str | None = None,
        languages: list[str] | None = None,
    ) -> NotesDraft:
        from meetops_api.ai.languages import language_prompt_hint

        prompt = {
            "title": title or "Meeting",
            "transcript": _truncate(cleaned_text or "", 14_000),
            "languages": languages or [],
            "language_hint": language_prompt_hint(languages),
            "instructions": (
                "Produce meeting notes. Write the TL;DR and summary in the primary expected "
                "language when provided (first in languages), keeping proper nouns intact. "
                "Return JSON: "
                "{tldr: string, summary_md: string, action_items: [{text, owner, due}]}"
            ),
        }
        data = await self._chat_json(
            system="You write concise meeting notes. Respond with JSON only.",
            user=json.dumps(prompt),
        )
        return NotesDraft.model_validate(data)

    async def _chat_json(self, *, system: str, user: str) -> dict[str, Any]:
        if not self.enabled:
            raise RuntimeError("OPENAI_API_KEY not configured")
        async with httpx.AsyncClient(timeout=120) as client:
            resp = await client.post(
                f"{self.base_url}/chat/completions",
                headers={
                    **self._headers(),
                    "Content-Type": "application/json",
                },
                json={
                    "model": self.llm_model,
                    "messages": [
                        {"role": "system", "content": system},
                        {"role": "user", "content": user},
                    ],
                    "temperature": 0.2,
                    "response_format": {"type": "json_object"},
                },
            )
            if resp.status_code >= 400:
                detail = (resp.text or "")[:500]
                logger.error(
                    "OpenAI chat failed status=%s body=%s",
                    resp.status_code,
                    detail,
                )
                raise RuntimeError(f"LLM request failed: {resp.status_code} {detail}")
            content = resp.json()["choices"][0]["message"]["content"]
            if isinstance(content, list):
                content = "".join(
                    part.get("text", "") if isinstance(part, dict) else str(part)
                    for part in content
                )
            return json.loads(content)
