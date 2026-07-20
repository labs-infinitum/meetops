"""Map diarized speaker IDs onto Meet display names using the active-speaker timeline."""

from __future__ import annotations

from collections import defaultdict
from typing import Any


def _overlap_seconds(a_start: float, a_end: float, b_start: float, b_end: float) -> float:
    return max(0.0, min(a_end, b_end) - max(a_start, b_start))


def timeline_to_intervals(
    timeline: list[dict[str, Any]] | None,
) -> list[tuple[float, float, str]]:
    """Convert active_speaker_changed events into [start, end) intervals in seconds."""
    if not timeline:
        return []
    events = sorted(
        (
            e
            for e in timeline
            if e.get("participant_name") is not None or e.get("relative_ms") is not None
        ),
        key=lambda e: float(e.get("relative_ms") or 0),
    )
    intervals: list[tuple[float, float, str]] = []
    for i, event in enumerate(events):
        name = (event.get("participant_name") or "").strip()
        if not name:
            continue
        start = float(event.get("relative_ms") or 0) / 1000.0
        end = (
            float(events[i + 1].get("relative_ms") or 0) / 1000.0
            if i + 1 < len(events)
            else float("inf")
        )
        if end < start:
            end = start
        intervals.append((start, end, name))
    return intervals


def map_diarized_speakers(
    utterances: list[dict[str, Any]],
    timeline: list[dict[str, Any]] | None,
) -> list[dict[str, Any]]:
    """
    Assign Meet display names to Deepgram speaker indices via overlap with the
    Meet active-speaker timeline. Falls back to \"Speaker N\" when unknown.
    """
    intervals = timeline_to_intervals(timeline)
    overlap_scores: dict[int, dict[str, float]] = defaultdict(lambda: defaultdict(float))

    for utt in utterances:
        speaker = int(utt.get("speaker") or 0)
        start = float(utt.get("start") or 0.0)
        end = float(utt.get("end") or start)
        if end < start:
            end = start
        for iv_start, iv_end, name in intervals:
            ov = _overlap_seconds(start, end, iv_start, iv_end)
            if ov > 0:
                overlap_scores[speaker][name] += ov

    speaker_names: dict[int, str] = {}
    for speaker, scores in overlap_scores.items():
        if not scores:
            continue
        best_name, best_score = max(scores.items(), key=lambda item: item[1])
        if best_score > 0:
            speaker_names[speaker] = best_name

    turns: list[dict[str, Any]] = []
    for utt in utterances:
        speaker = int(utt.get("speaker") or 0)
        text = (utt.get("text") or "").strip()
        if not text:
            continue
        turns.append(
            {
                "speaker": speaker_names.get(speaker, f"Speaker {speaker}"),
                "speaker_id": speaker,
                "text": text,
                "start": float(utt.get("start") or 0.0),
                "end": float(utt.get("end") or 0.0),
            }
        )
    return turns


def format_speaker_transcript(turns: list[dict[str, Any]]) -> str:
    """Render speaker turns as a readable segregated transcript."""
    lines: list[str] = []
    for turn in turns:
        speaker = (turn.get("speaker") or "Speaker").strip() or "Speaker"
        text = (turn.get("text") or "").strip()
        if not text:
            continue
        lines.append(f"{speaker}: {text}")
    return "\n\n".join(lines)


def extract_diarized_utterances(segments: Any) -> list[dict[str, Any]] | None:
    """Return Deepgram-style utterances from segments_json, or None if not diarized."""
    if not isinstance(segments, dict):
        return None
    if segments.get("provider") != "deepgram":
        return None
    utterances = segments.get("utterances")
    if not isinstance(utterances, list) or not utterances:
        return None
    return utterances
