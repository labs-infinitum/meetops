"""Tests for meeting language helpers."""

from __future__ import annotations

import pytest

from meetops_api.ai.languages import (
    deepgram_language_param,
    language_prompt_hint,
    normalize_languages,
    openai_transcription_language,
)


def test_normalize_languages_defaults_and_dedupes() -> None:
    assert normalize_languages(None) == ["en"]
    assert normalize_languages(["EN", "en-US", "hi"]) == ["en", "hi"]


def test_normalize_rejects_unknown() -> None:
    with pytest.raises(ValueError, match="unsupported"):
        normalize_languages(["xx"])


def test_provider_language_params() -> None:
    assert deepgram_language_param(["en"]) == "en"
    assert deepgram_language_param(["en", "hi"]) == "multi"
    assert deepgram_language_param([]) is None
    assert openai_transcription_language(["hi"]) == "hi"
    assert openai_transcription_language(["en", "hi"]) is None


def test_language_prompt_hint() -> None:
    hint = language_prompt_hint(["en", "hi"])
    assert hint is not None
    assert "English" in hint
    assert "Hindi" in hint
