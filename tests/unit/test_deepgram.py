"""Unit tests for Deepgram parsing and speaker attribution."""

from __future__ import annotations

from meetops_api.ai.deepgram_client import parse_deepgram_response
from meetops_api.ai.speaker_attribution import (
    extract_diarized_utterances,
    format_speaker_transcript,
    map_diarized_speakers,
    timeline_to_intervals,
)


def test_parse_deepgram_response_utterances() -> None:
    payload = {
        "metadata": {"detected_language": "en"},
        "results": {
            "channels": [
                {
                    "alternatives": [
                        {
                            "transcript": "Hello there How are you",
                            "words": [],
                        }
                    ]
                }
            ],
            "utterances": [
                {
                    "speaker": 0,
                    "transcript": "Hello there",
                    "start": 0.0,
                    "end": 1.2,
                    "confidence": 0.9,
                },
                {
                    "speaker": 1,
                    "transcript": "How are you",
                    "start": 1.3,
                    "end": 2.0,
                    "confidence": 0.8,
                },
            ],
        },
    }
    result = parse_deepgram_response(payload, model="nova-3")
    assert result["language"] == "en"
    assert result["text"] == "Hello there How are you"
    assert result["segments"]["provider"] == "deepgram"
    assert len(result["segments"]["utterances"]) == 2
    assert result["segments"]["utterances"][1]["speaker"] == 1


def test_parse_deepgram_falls_back_to_words() -> None:
    payload = {
        "results": {
            "channels": [
                {
                    "alternatives": [
                        {
                            "transcript": "",
                            "words": [
                                {
                                    "word": "hi",
                                    "punctuated_word": "Hi",
                                    "start": 0.0,
                                    "end": 0.3,
                                    "speaker": 0,
                                },
                                {
                                    "word": "alice",
                                    "punctuated_word": "Alice",
                                    "start": 0.3,
                                    "end": 0.6,
                                    "speaker": 0,
                                },
                                {
                                    "word": "hello",
                                    "punctuated_word": "Hello",
                                    "start": 1.0,
                                    "end": 1.4,
                                    "speaker": 1,
                                },
                            ],
                        }
                    ]
                }
            ],
            "utterances": [],
        },
    }
    result = parse_deepgram_response(payload, model="nova-3")
    turns = result["segments"]["utterances"]
    assert len(turns) == 2
    assert turns[0]["text"] == "Hi Alice"
    assert turns[0]["speaker"] == 0
    assert turns[1]["speaker"] == 1
    assert turns[1]["text"] == "Hello"


def test_map_diarized_speakers_uses_timeline_overlap() -> None:
    utterances = [
        {"speaker": 0, "text": "Welcome everyone", "start": 0.0, "end": 2.0},
        {"speaker": 1, "text": "Thanks for having me", "start": 2.5, "end": 4.0},
        {"speaker": 0, "text": "Let's begin", "start": 4.5, "end": 5.5},
    ]
    timeline = [
        {"relative_ms": 0, "participant_name": "Alex"},
        {"relative_ms": 2400, "participant_name": "Blake"},
        {"relative_ms": 4300, "participant_name": "Alex"},
    ]
    turns = map_diarized_speakers(utterances, timeline)
    assert [t["speaker"] for t in turns] == ["Alex", "Blake", "Alex"]
    assert turns[0]["speaker_id"] == 0
    text = format_speaker_transcript(turns)
    assert "Alex: Welcome everyone" in text
    assert "Blake: Thanks for having me" in text


def test_map_diarized_speakers_fallback_labels() -> None:
    utterances = [
        {"speaker": 0, "text": "Hello", "start": 0.0, "end": 1.0},
        {"speaker": 2, "text": "World", "start": 1.0, "end": 2.0},
    ]
    turns = map_diarized_speakers(utterances, timeline=None)
    assert turns[0]["speaker"] == "Speaker 0"
    assert turns[1]["speaker"] == "Speaker 2"


def test_timeline_to_intervals() -> None:
    intervals = timeline_to_intervals(
        [
            {"relative_ms": 1000, "participant_name": "A"},
            {"relative_ms": 3000, "participant_name": "B"},
        ]
    )
    assert intervals[0] == (1.0, 3.0, "A")
    assert intervals[1][0] == 3.0
    assert intervals[1][2] == "B"
    assert intervals[1][1] == float("inf")


def test_extract_diarized_utterances() -> None:
    assert extract_diarized_utterances([{"start": 0}]) is None
    assert extract_diarized_utterances({"provider": "openai"}) is None
    assert (
        extract_diarized_utterances(
            {"provider": "deepgram", "utterances": [{"speaker": 0, "text": "x"}]}
        )
        is not None
    )
