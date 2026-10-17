"""Unit tests: per-slot X displays and recording audio-health verdicts."""

from __future__ import annotations

from meetops_api.bot.audio_health import AudioHealthTracker
from meetops_api.bot.slots import slot_displays


def test_slot_displays_distinct_with_configured_display() -> None:
    displays = slot_displays(":99", 3)
    assert displays == [(":99", False), (":100", True), (":101", True)]
    assert len({d for d, _ in displays}) == 3


def test_slot_displays_default() -> None:
    assert slot_displays(None, 2) == [(":10", True), (":11", True)]


def _report(active: str, signal_secs: int, window: int = 15) -> dict[str, object]:
    quiet = {"tracks": 1, "avgDb": -95, "maxDb": -80, "signalSecs": 0}
    loud = {"tracks": 1, "avgDb": -30, "maxDb": -5, "signalSecs": signal_secs}
    return {
        "kind": "audio_health",
        "active": active,
        "switches": 0,
        "windowSecs": window,
        "meet": loud if active == "meet" else quiet,
        "tab": loud if active == "tab" else quiet,
    }


def test_audio_health_flags_silent_recording() -> None:
    tracker = AudioHealthTracker()
    for _ in range(20):  # 5 minutes, one stray UI pop
        tracker.add(_report("tab", 0))
    tracker.add(_report("tab", 1))
    assert tracker.looks_silent()
    assert tracker.summary()["total_secs"] == 315


def test_audio_health_accepts_normal_speech() -> None:
    tracker = AudioHealthTracker()
    for _ in range(20):
        tracker.add(_report("meet", 9))
    assert not tracker.looks_silent()
    assert tracker.summary()["active_secs"] == {"meet": 300}


def test_audio_health_no_verdict_for_short_recordings() -> None:
    tracker = AudioHealthTracker()
    tracker.add(_report("meet", 0))
    assert not tracker.looks_silent()


def test_recorder_collects_audio_health_reports(tmp_path) -> None:  # type: ignore[no-untyped-def]
    import asyncio
    import json

    from meetops_api.bot.recorder import MeetRecorder
    from meetops_api.bot.slots import BotSlot
    from meetops_common.settings import Settings

    recorder = MeetRecorder(
        Settings(meetops_env="development"),
        BotSlot(index=0, display=":99", profile_dir=tmp_path, owns_display=False),
        session_factory=None,  # type: ignore[arg-type]
        bot_run_id=1,
        meet_url="https://meet.google.com/abc-defg-hij",
        display_name="Bot",
        stop_event=asyncio.Event(),
        staging_dir=tmp_path,
    )
    recorder._on_chunk(json.dumps(_report("meet", 12)))
    assert recorder.audio_health.reports == 1
    assert recorder.audio_health.active_signal_secs == 12


def test_chrome_args_pin_meet_language() -> None:
    from meetops_api.bot.browser import chrome_args
    from meetops_common.settings import Settings

    args = chrome_args(Settings(_env_file=None))  # type: ignore[call-arg]
    assert "--lang=en-US" in args
    assert "--accept-lang=en-US,en" in args
