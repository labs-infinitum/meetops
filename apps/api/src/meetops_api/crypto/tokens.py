"""Token encryption (Fernet) — fail closed in production."""

from __future__ import annotations

import base64
import hashlib
import logging

from cryptography.fernet import Fernet, InvalidToken

from meetops_common.settings import Settings

logger = logging.getLogger(__name__)

_PLAIN_PREFIX = "plain:"


def _fernet_from_key(key_b64: str) -> Fernet:
    raw = base64.urlsafe_b64decode(key_b64.encode("utf-8"))
    if len(raw) != 32:
        # Derive a 32-byte key if user provided a passphrase-like secret
        raw = hashlib.sha256(key_b64.encode("utf-8")).digest()
    return Fernet(base64.urlsafe_b64encode(raw))


class TokenCrypto:
    def __init__(self, settings: Settings) -> None:
        self._settings = settings
        self._fernet: Fernet | None = None
        if settings.meetops_token_encryption_key:
            self._fernet = _fernet_from_key(settings.meetops_token_encryption_key)
        elif settings.is_production and not settings.meetops_allow_plaintext_tokens:
            raise RuntimeError("MEETOPS_TOKEN_ENCRYPTION_KEY is required in production")
        elif not settings.meetops_allow_plaintext_tokens and settings.meetops_env != "development":
            raise RuntimeError(
                "MEETOPS_TOKEN_ENCRYPTION_KEY required, or set MEETOPS_ALLOW_PLAINTEXT_TOKENS=1"
            )

    def encrypt(self, plaintext: str) -> str:
        if self._fernet is None:
            if not self._settings.meetops_allow_plaintext_tokens:
                raise RuntimeError("Token encryption key not configured")
            return _PLAIN_PREFIX + plaintext
        return self._fernet.encrypt(plaintext.encode("utf-8")).decode("utf-8")

    def decrypt(self, ciphertext: str) -> str:
        if ciphertext.startswith(_PLAIN_PREFIX):
            return ciphertext[len(_PLAIN_PREFIX) :]
        if self._fernet is None:
            if not self._settings.meetops_allow_plaintext_tokens:
                raise RuntimeError("Token encryption key not configured")
            return ciphertext
        try:
            return self._fernet.decrypt(ciphertext.encode("utf-8")).decode("utf-8")
        except InvalidToken as exc:
            raise ValueError("Failed to decrypt token") from exc


def generate_encryption_key() -> str:
    """Return a urlsafe base64-encoded 32-byte key suitable for MEETOPS_TOKEN_ENCRYPTION_KEY."""
    return Fernet.generate_key().decode("utf-8")