"""Groq Whisper + LLM client."""

from __future__ import annotations

import json
import logging
from pathlib import Path
from typing import Any

import httpx
from pydantic import BaseModel, Field

from meetops_common.settings import Settings

logger = logging.getLogger(__name__)

GROQ_BASE = "https://api.groq.com/openai/v1"


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


class GroqClient:
    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self.api_key = settings.groq_api_key

    @property
    def enabled(self) -> bool:
        return bool(self.api_key)

    async def transcribe(self, audio_path: Path) -> dict[str, Any]:
        if not self.api_key:
            raise RuntimeError("GROQ_API_KEY not configured")
        size = audio_path.stat().st_size
        if size > 100 * 1024 * 1024:
            raise RuntimeError("Audio file exceeds 100 MiB Groq limit")
        async with httpx.AsyncClient(timeout=300) as client:
            with audio_path.open("rb") as f:
                files = {"file": (audio_path.name, f, "audio/webm")}
                data = {
                    "model": self.settings.meetops_groq_transcribe_model,
                    "response_format": "verbose_json",
                }
                resp = await client.post(
                    f"{GROQ_BASE}/audio/transcriptions",
                    headers={"Authorization": f"Bearer {self.api_key}"},
                    files=files,
                    data=data,
                )
            if resp.status_code >= 400:
                logger.error("Groq transcribe failed status=%s", resp.status_code)
                raise RuntimeError(f"Transcription failed: {resp.status_code}")
            return resp.json()

    async def clean_transcript(
        self,
        *,
        raw_text: str,
        segments: list[dict[str, Any]] | None,
        timeline: list[dict[str, Any]] | None,
    ) -> CleanedTranscript:
        prompt = {
            "raw_text": _truncate(raw_text or "", 12_000),
            "segments": _truncate(json.dumps(segments or [])[:4000], 4000),
            "active_speaker_timeline": _truncate(json.dumps(timeline or [])[:4000], 4000),
            "instructions": (
                "Clean the transcript, attribute speakers using the timeline when possible. "
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
    ) -> NotesDraft:
        prompt = {
            "title": title or "Meeting",
            "transcript": _truncate(cleaned_text or "", 14_000),
            "instructions": (
                "Produce meeting notes. Return JSON: "
                "{tldr: string, summary_md: string, action_items: [{text, owner, due}]}"
            ),
        }
        data = await self._chat_json(
            system="You write concise meeting notes. Respond with JSON only.",
            user=json.dumps(prompt),
        )
        return NotesDraft.model_validate(data)

    async def _chat_json(self, *, system: str, user: str) -> dict[str, Any]:
        if not self.api_key:
            raise RuntimeError("GROQ_API_KEY not configured")
        async with httpx.AsyncClient(timeout=120) as client:
            resp = await client.post(
                f"{GROQ_BASE}/chat/completions",
                headers={
                    "Authorization": f"Bearer {self.api_key}",
                    "Content-Type": "application/json",
                },
                json={
                    "model": self.settings.meetops_groq_llm_model,
                    "messages": [
                        {"role": "system", "content": system},
                        {"role": "user", "content": user},
                    ],
                    "temperature": 0.2,
                    "response_format": {"type": "json_object"},
                },
            )
            if resp.status_code >= 400:
                logger.error("Groq chat failed status=%s", resp.status_code)
                raise RuntimeError(f"LLM request failed: {resp.status_code}")
            content = resp.json()["choices"][0]["message"]["content"]
            return json.loads(content)