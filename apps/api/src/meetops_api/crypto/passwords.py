"""Password hashing with Argon2id."""

from __future__ import annotations

from argon2 import PasswordHasher
from argon2.exceptions import VerifyMismatchError

from meetops_common.constants import DUMMY_PASSWORD_HASH

_ph = PasswordHasher(time_cost=3, memory_cost=65536, parallelism=4)


def hash_password(password: str) -> str:
    return _ph.hash(password)


def verify_password(password_hash: str, password: str) -> bool:
    try:
        return _ph.verify(password_hash, password)
    except VerifyMismatchError:
        return False
    except Exception:
        return False


def dummy_verify(password: str) -> None:
    """Run a verify against a dummy hash to reduce timing oracles on missing users."""
    try:
        _ph.verify(DUMMY_PASSWORD_HASH, password)
    except Exception:
        pass


def validate_password_strength(password: str) -> str | None:
    if len(password) < 8:
        return "Password must be at least 8 characters"
    return None