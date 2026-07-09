"""Meet URL validation helpers."""

from __future__ import annotations

import re

from meetops_api.errors import AppError

MEET_URL_RE = re.compile(
    r"^https://meet\.google\.com/[a-z0-9\-]+(?:\?.*)?$",
    re.IGNORECASE,
)


def normalize_meet_url(url: str) -> str:
    url = url.strip()
    if not MEET_URL_RE.match(url):
        raise AppError("invalid_meet_url", "Invalid Google Meet URL", 400)
    # strip query for storage consistency
    return url.split("?", 1)[0]