"""Unit tests for speaker attribution mapping and renames."""

from __future__ import annotations

from meetops_api.ai.speaker_attribution import (
    apply_speaker_renames,
    extract_roster_names,
    format_speaker_transcript,
    map_diarized_speakers,
    timeline_to_intervals,
)


def test_timeline_to_intervals_applies_lag() -> None:
    intervals = timeline_to_intervals(
        [
            {"relative_ms": 1000, "participant_name": "A", "type": "active_speaker_changed"},
            {"relative_ms": 3000, "participant_name": "B", "type": "active_speaker_sample"},
        ]
    )
    assert intervals[0][0] == 0.6  # 1.0 - 0.4 lag
    assert intervals[0][1] == 3.4  # 3.0 + 0.4 lag
    assert intervals[0][2] == "A"
    assert intervals[1][0] == 2.6
    assert intervals[1][2] == "B"
    assert intervals[1][1] == float("inf")


def test_timeline_ignores_nameless_and_non_speaker_rows() -> None:
    intervals = timeline_to_intervals(
        [
            {"relative_ms": 0, "participant_name": "", "type": "active_speaker_changed"},
            {
                "relative_ms": 500,
                "participant_name": "Alex",
                "type": "participant_joined",
            },
            {
                "relative_ms": 1000,
                "participant_name": "Alex",
                "type": "active_speaker_changed",
            },
        ]
    )
    assert len(intervals) == 1
    assert intervals[0][2] == "Alex"


def test_map_diarized_speakers_uses_timeline_overlap() -> None:
    utterances = [
        {"speaker": 0, "text": "Welcome everyone", "start": 0.0, "end": 2.0},
        {"speaker": 1, "text": "Thanks for having me", "start": 2.5, "end": 4.0},
        {"speaker": 0, "text": "Let's begin", "start": 4.5, "end": 5.5},
    ]
    timeline = [
        {"relative_ms": 0, "participant_name": "Alex", "type": "active_speaker_changed"},
        {"relative_ms": 2400, "participant_name": "Blake", "type": "active_speaker_changed"},
        {"relative_ms": 4300, "participant_name": "Alex", "type": "active_speaker_sample"},
    ]
    result = map_diarized_speakers(utterances, timeline)
    assert [t["speaker"] for t in result.turns] == ["Alex", "Blake", "Alex"]
    assert result.mapped == 2
    assert result.unmapped == 0
    assert not result.needs_llm_fallback
    text = format_speaker_transcript(result.turns)
    assert "Alex: Welcome everyone" in text
    assert "Blake: Thanks for having me" in text


def test_map_rejects_weak_overlap() -> None:
    utterances = [
        {"speaker": 0, "text": "Hello", "start": 0.0, "end": 0.5},
    ]
    # Named activity only near t=10s — no meaningful overlap with early speech.
    timeline = [
        {
            "relative_ms": 10000,
            "participant_name": "Alex",
            "type": "active_speaker_changed",
        },
        {
            "relative_ms": 10100,
            "participant_name": "Blake",
            "type": "active_speaker_changed",
        },
    ]
    result = map_diarized_speakers(utterances, timeline)
    assert result.turns[0]["speaker"] == "Speaker 0"
    assert result.mapped == 0
    assert result.unmapped == 1
    assert result.needs_llm_fallback


def test_map_diarized_speakers_fallback_labels() -> None:
    utterances = [
        {"speaker": 0, "text": "Hello", "start": 0.0, "end": 1.0},
        {"speaker": 2, "text": "World", "start": 1.0, "end": 2.0},
    ]
    result = map_diarized_speakers(utterances, timeline=None)
    assert result.turns[0]["speaker"] == "Speaker 0"
    assert result.turns[1]["speaker"] == "Speaker 2"
    assert result.needs_llm_fallback


def test_extract_roster_names_from_snapshot() -> None:
    timeline = [
        {
            "type": "participant_snapshot",
            "relative_ms": 0,
            "payload": {
                "participants": [
                    {"id": "1", "name": "Alex"},
                    {"id": "2", "name": "Blake"},
                ],
                "activeSpeakerName": "Alex",
            },
        },
        {
            "type": "participant_joined",
            "relative_ms": 10,
            "participant_name": "Casey",
        },
    ]
    assert extract_roster_names(timeline) == ["Alex", "Blake", "Casey"]


def test_apply_speaker_renames_constrained_to_roster() -> None:
    turns = [
        {"speaker": "Speaker 0", "speaker_id": 0, "text": "Hi", "start": 0, "end": 1},
        {"speaker": "Speaker 1", "speaker_id": 1, "text": "Yo", "start": 1, "end": 2},
    ]
    renamed = apply_speaker_renames(
        turns,
        {"0": "alex", "1": "Invented"},
        roster=["Alex", "Blake"],
    )
    assert renamed[0]["speaker"] == "Alex"
    assert renamed[1]["speaker"] == "Speaker 1"  # Invented rejected
