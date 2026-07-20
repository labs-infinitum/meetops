"""Meeting language codes for transcription / notes hints."""

from __future__ import annotations

# ISO 639-1 codes (compatible with Whisper; Deepgram accepts these and BCP-47).
LANGUAGE_OPTIONS: tuple[tuple[str, str], ...] = (
    ("en", "English"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("de", "German"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
    ("pl", "Polish"),
    ("uk", "Ukrainian"),
    ("ru", "Russian"),
    ("hi", "Hindi"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("zh", "Chinese"),
    ("ar", "Arabic"),
    ("tr", "Turkish"),
    ("sv", "Swedish"),
    ("da", "Danish"),
    ("no", "Norwegian"),
    ("fi", "Finnish"),
    ("cs", "Czech"),
    ("el", "Greek"),
    ("he", "Hebrew"),
    ("th", "Thai"),
    ("vi", "Vietnamese"),
    ("id", "Indonesian"),
    ("ro", "Romanian"),
    ("hu", "Hungarian"),
)

LANGUAGE_CODES: frozenset[str] = frozenset(code for code, _ in LANGUAGE_OPTIONS)
DEFAULT_LANGUAGES: list[str] = ["en"]
MAX_LANGUAGES = 8


def normalize_languages(raw: object) -> list[str]:
    """Validate and de-dupe language codes; empty means auto-detect."""
    if raw is None:
        return list(DEFAULT_LANGUAGES)
    if not isinstance(raw, (list, tuple)):
        raise ValueError("languages must be a list of language codes")
    seen: set[str] = set()
    out: list[str] = []
    for item in raw:
        if not isinstance(item, str):
            raise ValueError("each language must be a string code")
        code = item.strip().lower().replace("_", "-")
        # Accept en-US → en for Whisper compatibility when base is supported.
        base = code.split("-", 1)[0]
        if base not in LANGUAGE_CODES:
            raise ValueError(f"unsupported language code: {item}")
        if base in seen:
            continue
        seen.add(base)
        out.append(base)
        if len(out) > MAX_LANGUAGES:
            raise ValueError(f"at most {MAX_LANGUAGES} languages allowed")
    return out


def deepgram_language_param(languages: list[str] | None) -> str | None:
    """
    Deepgram language query value.
    - none/empty → omit (provider default / detection)
    - one → that code
    - many → multilingual codeswitching (`multi`)
    """
    langs = [c for c in (languages or []) if c]
    if not langs:
        return None
    if len(langs) == 1:
        return langs[0]
    return "multi"


def openai_transcription_language(languages: list[str] | None) -> str | None:
    """Whisper accepts a single ISO-639-1 code; omit for auto-detect when multi."""
    langs = [c for c in (languages or []) if c]
    if len(langs) == 1:
        return langs[0]
    return None


def language_prompt_hint(languages: list[str] | None) -> str | None:
    langs = [c for c in (languages or []) if c]
    if not langs:
        return None
    labels = dict(LANGUAGE_OPTIONS)
    named = [labels.get(c, c) for c in langs]
    if len(named) == 1:
        return f"Expected meeting language: {named[0]} ({langs[0]})."
    joined = ", ".join(f"{n} ({c})" for n, c in zip(named, langs, strict=True))
    return f"Expected meeting languages (may mix): {joined}."
