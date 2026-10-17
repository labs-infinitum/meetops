"""Recordings on the local disk (MEETOPS_STORAGE_BACKEND=local).

Same interface as S3Storage, keyed the same way, so a recording's key looks identical in both
backends. Playback goes through the API (`GET /api/v1/recordings/{id}/media`), which checks the
signed-in user owns the recording, instead of a presigned URL.
"""

from __future__ import annotations

import os
import shutil
from pathlib import Path

from meetops_common.settings import Settings


class LocalStorage:
    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self.root = settings.recordings_store_dir.resolve()

    def recording_key(self, user_id: str, run_id: int | str, name: str = "raw.webm") -> str:
        return f"{self.settings.s3_prefix}/{user_id}/{run_id}/{name}"

    def path_for(self, key: str) -> Path:
        """The file for `key`; refuses keys that would leave the store (e.g. `../`)."""
        path = (self.root / key).resolve()
        if not path.is_relative_to(self.root):
            raise ValueError(f"recording key outside the store: {key!r}")
        return path

    def upload_file(self, local_path: Path, key: str, *, content_type: str = "video/webm") -> None:
        dest = self.path_for(key)
        dest.parent.mkdir(parents=True, exist_ok=True)
        part = dest.with_name(dest.name + ".part")
        shutil.copyfile(local_path, part)
        os.replace(part, dest)  # readers never see a half-written file

    def download_file(self, key: str, dest: Path) -> None:
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(self.path_for(key), dest)
