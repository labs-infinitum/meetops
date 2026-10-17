"""Local recording storage (MEETOPS_STORAGE_BACKEND=local)."""

from __future__ import annotations

from pathlib import Path

import pytest
from cryptography.fernet import Fernet

import meetops_api.storage as storage_pkg
from meetops_api.storage import LocalStorage, get_storage
from meetops_common.settings import Settings


def _settings(tmp_path: Path, **kw: object) -> Settings:
    return Settings(_env_file=None, meetops_data_dir=tmp_path, **kw)  # type: ignore[call-arg]


def test_get_storage_picks_backend(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    class FakeS3:  # the real client would resolve AWS credentials
        def __init__(self, settings: Settings) -> None:
            self.settings = settings

    monkeypatch.setattr(storage_pkg, "S3Storage", FakeS3)
    assert isinstance(get_storage(_settings(tmp_path, meetops_storage_backend="local")), LocalStorage)
    assert isinstance(get_storage(_settings(tmp_path, s3_bucket="b")), FakeS3)
    assert get_storage(_settings(tmp_path)) is None


def test_upload_and_download_roundtrip(tmp_path: Path) -> None:
    store = LocalStorage(_settings(tmp_path, meetops_storage_backend="local"))
    src = tmp_path / "staging" / "raw.webm"
    src.parent.mkdir()
    src.write_bytes(b"webm-bytes")
    key = store.recording_key("user-1", 42)
    assert key == "recordings/user-1/42/raw.webm"

    store.upload_file(src, key)
    stored = store.path_for(key)
    assert stored == tmp_path.resolve() / "recordings" / "store" / key
    assert stored.read_bytes() == b"webm-bytes"
    assert not stored.with_name("raw.webm.part").exists()

    out = tmp_path / "out" / "a.webm"
    store.download_file(key, out)
    assert out.read_bytes() == b"webm-bytes"


@pytest.mark.parametrize("key", ["../secret", "recordings/../../x", "/etc/passwd"])
def test_keys_cannot_leave_the_store(tmp_path: Path, key: str) -> None:
    store = LocalStorage(_settings(tmp_path, meetops_storage_backend="local"))
    with pytest.raises(ValueError):
        store.path_for(key)


def test_production_needs_no_bucket_with_local_storage(tmp_path: Path) -> None:
    key = Fernet.generate_key().decode()
    _settings(
        tmp_path, meetops_env="production", meetops_storage_backend="local", meetops_token_encryption_key=key
    )
    with pytest.raises(ValueError, match="S3_BUCKET"):
        _settings(tmp_path, meetops_env="production", meetops_token_encryption_key=key)
