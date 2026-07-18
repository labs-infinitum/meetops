"""Unit tests: passwords, crypto, meet URL, JS scripts."""

from __future__ import annotations

from pathlib import Path

import pytest

from meetops_api.crypto.passwords import hash_password, validate_password_strength, verify_password
from meetops_api.crypto.tokens import TokenCrypto
from meetops_api.errors import AppError
from meetops_api.services.meet_url import normalize_meet_url
from meetops_common.settings import Settings, clear_settings_cache


def test_password_roundtrip() -> None:
    h = hash_password("secret-password")
    assert verify_password(h, "secret-password")
    assert not verify_password(h, "wrong")


def test_password_strength() -> None:
    assert validate_password_strength("short") is not None
    assert validate_password_strength("longenough") is None


def test_token_crypto_plaintext_dev() -> None:
    clear_settings_cache()
    settings = Settings(
        meetops_env="development",
        meetops_allow_plaintext_tokens=True,
        meetops_token_encryption_key=None,
    )
    crypto = TokenCrypto(settings)
    enc = crypto.encrypt("access-token")
    assert enc.startswith("plain:")
    assert crypto.decrypt(enc) == "access-token"


def test_token_crypto_fernet() -> None:
    from cryptography.fernet import Fernet

    clear_settings_cache()
    key = Fernet.generate_key().decode()
    settings = Settings(
        meetops_env="development",
        meetops_token_encryption_key=key,
        meetops_allow_plaintext_tokens=False,
    )
    crypto = TokenCrypto(settings)
    enc = crypto.encrypt("secret")
    assert not enc.startswith("plain:")
    assert crypto.decrypt(enc) == "secret"


def test_meet_url() -> None:
    assert normalize_meet_url("https://meet.google.com/abc-defg-hij") == (
        "https://meet.google.com/abc-defg-hij"
    )
    with pytest.raises(AppError):
        normalize_meet_url("https://example.com/not-meet")


def test_js_scripts_syntax() -> None:
    import subprocess

    scripts = Path(__file__).resolve().parents[2] / "apps/api/src/meetops_api/bot/scripts"
    for path in scripts.glob("*.js"):
        src = path.read_text()
        # Validate as function expression
        proc = subprocess.run(
            ["node", "-e", f"new Function('return (' + {src!r} + ')')"],
            capture_output=True,
            text=True,
        )
        assert proc.returncode == 0, f"{path.name}: {proc.stderr}"


def test_extract_meet_url() -> None:
    from meetops_api.google.calendar import extract_meet_url

    assert extract_meet_url({"hangoutLink": "https://meet.google.com/aaa-bbbb-ccc"})
    assert extract_meet_url(
        {"description": "Join https://meet.google.com/xxx-yyyy-zzz please"}
    )


def test_database_url_from_secret(monkeypatch) -> None:
    import meetops_common.settings as settings_mod

    def fake_resolver(secret_arn: str, host: str, port: int, database: str) -> str:
        assert secret_arn == "arn:aws:secretsmanager:us-east-1:1:secret:x"
        return f"postgresql+asyncpg://u:p@{host}:{port}/{database}"

    monkeypatch.setattr(settings_mod, "resolve_database_url_from_secret", fake_resolver)
    clear_settings_cache()
    settings = Settings(
        meetops_env="development",
        meetops_database_secret_arn="arn:aws:secretsmanager:us-east-1:1:secret:x",
        meetops_database_host="db.example.com",
        meetops_database_name="meetops",
    )
    assert settings.database_url == "postgresql+asyncpg://u:p@db.example.com:5432/meetops"


def test_database_secret_requires_host() -> None:
    import pytest

    clear_settings_cache()
    with pytest.raises(ValueError, match="MEETOPS_DATABASE_HOST"):
        Settings(
            meetops_env="development",
            meetops_database_secret_arn="arn:aws:secretsmanager:us-east-1:1:secret:x",
            meetops_database_host=None,
        )


def test_secret_url_encoding(monkeypatch) -> None:
    from meetops_common.settings import resolve_database_url_from_secret

    class FakeClient:
        def get_secret_value(self, SecretId: str) -> dict:
            return {"SecretString": '{"username": "admin", "password": "p@ss/w:rd"}'}

    import boto3

    monkeypatch.setattr(boto3, "client", lambda *a, **kw: FakeClient())
    url = resolve_database_url_from_secret(
        "arn:aws:secretsmanager:us-east-1:1:secret:x", "h", 5432, "meetops"
    )
    assert url == "postgresql+asyncpg://admin:p%40ss%2Fw%3Ard@h:5432/meetops"
