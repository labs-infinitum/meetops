"""Map diarized speaker IDs onto Meet display names using the active-speaker timeline."""

from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass, field
from typing import Any

# Meet UI speaking indicators lag audio slightly.
_LAG_SECONDS = 0.4
_MIN_OVERLAP_SECONDS = 1.0
_MIN_OVERLAP_FRACTION = 0.2
_MAPPED_COVERAGE_THRESHOLD = 0.7

_SPEAKER_EVENT_TYPES = frozenset({"active_speaker_changed", "active_speaker_sample"})


@dataclass
class AttributionResult:
    turns: list[dict[str, Any]]
    mapped: int = 0
    unmapped: int = 0
    method: str = "overlap"
    roster: list[str] = field(default_factory=list)

    @property
    def coverage(self) -> float:
        total = self.mapped + self.unmapped
        if total == 0:
            return 1.0
        return self.mapped / total

    @property
    def needs_llm_fallback(self) -> bool:
        if self.unmapped > 0:
            return True
        return self.coverage < _MAPPED_COVERAGE_THRESHOLD


def _overlap_seconds(a_start: float, a_end: float, b_start: float, b_end: float) -> float:
    return max(0.0, min(a_end, b_end) - max(a_start, b_start))


def timeline_to_intervals(
    timeline: list[dict[str, Any]] | None,
    *,
    lag_seconds: float = _LAG_SECONDS,
) -> list[tuple[float, float, str]]:
    """Convert named active-speaker events into [start, end) intervals in seconds."""
    if not timeline:
        return []

    def _is_speaker_row(event: dict[str, Any]) -> bool:
        etype = event.get("type") or event.get("event_type")
        name = (event.get("participant_name") or "").strip()
        if not name or event.get("relative_ms") is None:
            return False
        if etype in _SPEAKER_EVENT_TYPES:
            return True
        # Legacy rows / unit tests omit type.
        return etype is None

    events = sorted(
        (e for e in timeline if _is_speaker_row(e)),
        key=lambda e: float(e.get("relative_ms") or 0),
    )
    intervals: list[tuple[float, float, str]] = []
    for i, event in enumerate(events):
        name = (event.get("participant_name") or "").strip()
        start = max(0.0, float(event.get("relative_ms") or 0) / 1000.0 - lag_seconds)
        if i + 1 < len(events):
            end = max(
                start,
                float(events[i + 1].get("relative_ms") or 0) / 1000.0 + lag_seconds,
            )
        else:
            end = float("inf")
        intervals.append((start, end, name))
    return intervals


def extract_roster_names(timeline: list[dict[str, Any]] | None) -> list[str]:
    """Collect unique display names from snapshots / joins / speaker events."""
    if not timeline:
        return []
    seen: set[str] = set()
    names: list[str] = []

    def _add(name: object) -> None:
        if not isinstance(name, str):
            return
        cleaned = name.strip()
        if not cleaned or cleaned in seen:
            return
        seen.add(cleaned)
        names.append(cleaned)

    for event in timeline:
        etype = event.get("type") or event.get("event_type")
        _add(event.get("participant_name"))
        payload = event.get("payload") or event.get("payload_json") or {}
        if not isinstance(payload, dict):
            continue
        if etype == "participant_snapshot":
            for person in payload.get("participants") or []:
                if isinstance(person, dict):
                    _add(person.get("name"))
            _add(payload.get("activeSpeakerName"))
        _add(payload.get("presenterName"))
    return names


def map_diarized_speakers(
    utterances: list[dict[str, Any]],
    timeline: list[dict[str, Any]] | None,
) -> AttributionResult:
    """
    Assign Meet display names to Deepgram speaker indices via overlap with the
    Meet active-speaker timeline. Falls back to \"Speaker N\" when unknown.
    """
    intervals = timeline_to_intervals(timeline)
    roster = extract_roster_names(timeline)
    overlap_scores: dict[int, dict[str, float]] = defaultdict(lambda: defaultdict(float))
    speech_totals: dict[int, float] = defaultdict(float)

    for utt in utterances:
        speaker = int(utt.get("speaker") or 0)
        start = float(utt.get("start") or 0.0)
        end = float(utt.get("end") or start)
        if end < start:
            end = start
        duration = max(0.0, end - start)
        speech_totals[speaker] += duration
        for iv_start, iv_end, name in intervals:
            ov = _overlap_seconds(start, end, iv_start, iv_end)
            if ov > 0:
                overlap_scores[speaker][name] += ov

    speaker_names: dict[int, str] = {}
    for speaker, scores in overlap_scores.items():
        if not scores:
            continue
        best_name, best_score = max(scores.items(), key=lambda item: item[1])
        total = speech_totals.get(speaker) or 0.0
        fraction = (best_score / total) if total > 0 else 0.0
        if best_score >= _MIN_OVERLAP_SECONDS or fraction >= _MIN_OVERLAP_FRACTION:
            speaker_names[speaker] = best_name

    turns: list[dict[str, Any]] = []
    mapped_ids: set[int] = set()
    unmapped_ids: set[int] = set()
    for utt in utterances:
        speaker = int(utt.get("speaker") or 0)
        text = (utt.get("text") or "").strip()
        if not text:
            continue
        name = speaker_names.get(speaker)
        if name:
            mapped_ids.add(speaker)
            label = name
        else:
            unmapped_ids.add(speaker)
            label = f"Speaker {speaker}"
        turns.append(
            {
                "speaker": label,
                "speaker_id": speaker,
                "text": text,
                "start": float(utt.get("start") or 0.0),
                "end": float(utt.get("end") or 0.0),
            }
        )

    # Speakers with no utterances shouldn't count; use unique ids in turns.
    return AttributionResult(
        turns=turns,
        mapped=len(mapped_ids),
        unmapped=len(unmapped_ids),
        method="overlap",
        roster=roster,
    )


def apply_speaker_renames(
    turns: list[dict[str, Any]],
    renames: dict[str, str] | dict[int, str],
    *,
    roster: list[str] | None = None,
) -> list[dict[str, Any]]:
    """Apply speaker_id → name renames, optionally constrained to roster names."""
    roster_set = {n.strip() for n in (roster or []) if n and n.strip()}
    normalized: dict[int, str] = {}
    for key, value in renames.items():
        try:
            speaker_id = int(key)
        except (TypeError, ValueError):
            continue
        name = (value or "").strip()
        if not name:
            continue
        if roster_set and name not in roster_set:
            # Case-insensitive match against roster.
            match = next((r for r in roster_set if r.lower() == name.lower()), None)
            if match is None:
                continue
            name = match
        normalized[speaker_id] = name

    out: list[dict[str, Any]] = []
    for turn in turns:
        speaker_id = int(turn.get("speaker_id") or 0)
        updated = dict(turn)
        if speaker_id in normalized:
            updated["speaker"] = normalized[speaker_id]
        out.append(updated)
    return out


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
