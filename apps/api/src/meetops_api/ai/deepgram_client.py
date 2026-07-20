"""Deepgram pre-recorded transcription with speaker diarization."""

from __future__ import annotations

import logging
from pathlib import Path
from typing import Any

import httpx

from meetops_common.settings import Settings

logger = logging.getLogger(__name__)

DEEPGRAM_BASE = "https://api.deepgram.com/v1"
# Deepgram allows much larger uploads; keep a practical worker bound.
_MAX_AUDIO_BYTES = 500 * 1024 * 1024


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


def parse_deepgram_response(payload: dict[str, Any], *, model: str) -> dict[str, Any]:
    """Normalize a Deepgram listen response into MeetOps transcript shape."""
    results = payload.get("results") or {}
    channels = results.get("channels") or []
    alt: dict[str, Any] = {}
    if channels:
        alts = channels[0].get("alternatives") or []
        if alts:
            alt = alts[0]

    transcript_text = (alt.get("transcript") or "").strip()
    words = alt.get("words") or []
    language = None
    metadata = payload.get("metadata") or {}
    if isinstance(metadata.get("detected_language"), str):
        language = metadata["detected_language"]

    utterances_raw = results.get("utterances") or []
    utterances: list[dict[str, Any]] = []
    for u in utterances_raw:
        text = (u.get("transcript") or "").strip()
        if not text:
            continue
        speaker = u.get("speaker")
        utterances.append(
            {
                "speaker": int(speaker) if speaker is not None else 0,
                "text": text,
                "start": float(u.get("start") or 0.0),
                "end": float(u.get("end") or 0.0),
                "confidence": u.get("confidence"),
            }
        )

    if not utterances and words:
        utterances = _utterances_from_words(words)

    if not transcript_text and utterances:
        transcript_text = " ".join(u["text"] for u in utterances)

    return {
        "text": transcript_text,
        "language": language,
        "segments": {
            "provider": "deepgram",
            "model": model,
            "utterances": utterances,
            "words": words,
        },
    }


def _utterances_from_words(words: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Group consecutive words with the same speaker into utterance-like turns."""
    turns: list[dict[str, Any]] = []
    current: dict[str, Any] | None = None
    for w in words:
        speaker = int(w.get("speaker") or 0)
        token = (w.get("punctuated_word") or w.get("word") or "").strip()
        if not token:
            continue
        start = float(w.get("start") or 0.0)
        end = float(w.get("end") or start)
        if current is None or current["speaker"] != speaker:
            if current is not None:
                turns.append(current)
            current = {
                "speaker": speaker,
                "text": token,
                "start": start,
                "end": end,
                "confidence": w.get("speaker_confidence") or w.get("confidence"),
            }
        else:
            current["text"] = f"{current['text']} {token}"
            current["end"] = end
    if current is not None:
        turns.append(current)
    return turns


class DeepgramClient:
    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self.api_key = settings.deepgram_api_key
        base = (settings.deepgram_base_url or DEEPGRAM_BASE).strip().rstrip("/")
        self.base_url = base
        self.model = settings.deepgram_model
        self.diarize_model = settings.deepgram_diarize_model

    @property
    def enabled(self) -> bool:
        return bool(self.api_key)

    async def transcribe(
        self,
        audio_path: Path,
        *,
        languages: list[str] | None = None,
    ) -> dict[str, Any]:
        if not self.enabled:
            raise RuntimeError("DEEPGRAM_API_KEY not configured")
        size = audio_path.stat().st_size
        if size > _MAX_AUDIO_BYTES:
            raise RuntimeError(
                f"Audio file exceeds {_MAX_AUDIO_BYTES} byte Deepgram worker limit"
            )
        if size == 0:
            raise RuntimeError("Audio file is empty")

        from meetops_api.ai.languages import deepgram_language_param

        params: dict[str, str] = {
            "model": self.model,
            "smart_format": "true",
            "punctuate": "true",
            "utterances": "true",
            "diarize_model": self.diarize_model,
        }
        language = deepgram_language_param(languages)
        if language:
            params["language"] = language
        headers = {
            "Authorization": f"Token {self.api_key}",
            "Content-Type": _audio_content_type(audio_path),
        }
        async with httpx.AsyncClient(timeout=600) as client:
            with audio_path.open("rb") as f:
                resp = await client.post(
                    f"{self.base_url}/listen",
                    headers=headers,
                    params=params,
                    content=f.read(),
                )
        if resp.status_code >= 400:
            detail = (resp.text or "")[:500]
            logger.error(
                "Deepgram transcribe failed status=%s body=%s",
                resp.status_code,
                detail,
            )
            raise RuntimeError(f"Deepgram transcription failed: {resp.status_code} {detail}")
        payload = resp.json()
        if not isinstance(payload, dict):
            raise RuntimeError("Deepgram returned unexpected response")
        return parse_deepgram_response(payload, model=self.model)
