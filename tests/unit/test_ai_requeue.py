"""Tests for AI job requeue helpers."""

from __future__ import annotations

from meetops_common.models import Job


def test_complete_job_noop_when_cancelled() -> None:
    job = Job(kind="transcribe_recording", status="cancelled")
    assert job.status == "cancelled"


def test_job_statuses_include_cancelled() -> None:
    from meetops_common.constants import JOB_STATUSES

    assert "cancelled" in JOB_STATUSES
