"""Aggregate the in-page audio mixer's `audio_health` reports for one recording."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

# Below this share of seconds with audible signal on the recorded bus, a recording
# is flagged as silent (speech normally fills a large share of a meeting).
SILENT_SIGNAL_RATIO = 0.02
MIN_SECS_FOR_VERDICT = 120


@dataclass
class AudioHealthTracker:
    reports: int = 0
    total_secs: int = 0
    active_signal_secs: int = 0
    active_secs: dict[str, int] = field(default_factory=dict)
    max_switches: int = 0
    last: dict[str, Any] | None = None

    def add(self, payload: dict[str, Any]) -> None:
        self.reports += 1
        self.last = payload
        secs = _as_int(payload.get("windowSecs"))
        self.total_secs += secs
        active = payload.get("active")
        bus = payload.get(active) if active in ("meet", "tab") else None
        if isinstance(bus, dict):
            self.active_signal_secs += _as_int(bus.get("signalSecs"))
        if isinstance(active, str):
            self.active_secs[active] = self.active_secs.get(active, 0) + secs
        self.max_switches = max(self.max_switches, _as_int(payload.get("switches")))

    @property
    def signal_ratio(self) -> float | None:
        if self.total_secs <= 0:
            return None
        return self.active_signal_secs / self.total_secs

    def looks_silent(self) -> bool:
        ratio = self.signal_ratio
        return (
            ratio is not None
            and self.total_secs >= MIN_SECS_FOR_VERDICT
            and ratio < SILENT_SIGNAL_RATIO
        )

    def summary(self) -> dict[str, Any]:
        ratio = self.signal_ratio
        return {
            "reports": self.reports,
            "total_secs": self.total_secs,
            "active_signal_secs": self.active_signal_secs,
            "signal_ratio": round(ratio, 3) if ratio is not None else None,
            "active_secs": self.active_secs,
            "switches": self.max_switches,
        }


def _as_int(value: Any) -> int:
    try:
        return int(value)
    except (TypeError, ValueError):
        return 0
