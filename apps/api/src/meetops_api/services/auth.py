"""Authentication service: signup, login, sessions, email tokens."""

from __future__ import annotations

import logging
import secrets
from datetime import UTC, datetime, timedelta

from sqlalchemy import delete, func, select
from sqlalchemy.ext.asyncio import AsyncSession

from meetops_api.crypto.passwords import (
    dummy_verify,
    hash_password,
    validate_password_strength,
    verify_password,
)
from meetops_api.email.sender import EmailSender
from meetops_api.errors import AppError
from meetops_common.constants import SESSION_TOUCH_THROTTLE_SECS
from meetops_common.models import AuthEmailToken, Session, User, UserSettings
from meetops_common.settings import Settings

logger = logging.getLogger(__name__)


def _utcnow() -> datetime:
    return datetime.now(UTC)


def new_id(prefix: str = "") -> str:
    import uuid

    return f"{prefix}{uuid.uuid4()}" if prefix else str(uuid.uuid4())


def new_token(nbytes: int = 32) -> str:
    return secrets.token_urlsafe(nbytes)


class AuthService:
    def __init__(self, db: AsyncSession, settings: Settings) -> None:
        self.db = db
        self.settings = settings
        self.email = EmailSender(settings)

    async def signup(
        self,
        *,
        name: str,
        email: str,
        username: str,
        password: str,
    ) -> tuple[User, bool, str | None]:
        err = validate_password_strength(password)
        if err:
            raise AppError("invalid_password", err, 400)

        username_l = username.strip()
        email_l = email.strip().lower()
        if not username_l or not email_l:
            raise AppError("invalid_request", "Invalid signup data", 400)

        existing = await self.db.scalar(
            select(User.id).where(
                (func.lower(User.username) == username_l.lower())
                | (func.lower(User.email) == email_l)
            )
        )
        if existing:
            logger.info("Signup rejected: username or email taken")
            raise AppError("signup_failed", "Could not create account", 400)

        user = User(
            id=new_id(),
            username=username_l,
            email=email_l,
            name=name.strip(),
            password_hash=hash_password(password),
            email_verified_at=None if self.settings.meetops_require_email_verification else _utcnow(),
        )
        self.db.add(user)
        self.db.add(
            UserSettings(
                user_id=user.id,
                auto_record_default=True,
                join_lead_time_secs=self.settings.meetops_bot_join_lead_time_secs,
                bot_display_name=self.settings.meetops_bot_display_name,
                languages_json=["en"],
            )
        )
        await self.db.flush()

        session_token: str | None = None
        if self.settings.meetops_require_email_verification:
            await self._issue_email_token(user, "verify_email")
        else:
            session_token = await self._create_session(user)

        return user, self.settings.meetops_require_email_verification, session_token

    async def login(
        self,
        *,
        username: str,
        password: str,
        user_agent: str | None = None,
        ip: str | None = None,
    ) -> tuple[User, str]:
        user = await self.db.scalar(
            select(User).where(func.lower(User.username) == username.strip().lower())
        )
        if user is None:
            dummy_verify(password)
            raise AppError("invalid_credentials", "Invalid username or password", 401)

        if not verify_password(user.password_hash, password):
            raise AppError("invalid_credentials", "Invalid username or password", 401)

        if (
            self.settings.meetops_require_email_verification
            and user.email_verified_at is None
        ):
            raise AppError(
                "email_not_verified",
                "Please verify your email before logging in",
                403,
            )

        token = await self._create_session(user, user_agent=user_agent, ip=ip)
        return user, token

    async def logout(self, token: str | None) -> None:
        if not token:
            return
        await self.db.execute(delete(Session).where(Session.token == token))

    async def get_user_for_session(self, token: str) -> User | None:
        row = await self.db.execute(
            select(Session, User)
            .join(User, User.id == Session.user_id)
            .where(Session.token == token)
        )
        result = row.first()
        if result is None:
            return None
        session, user = result
        now = _utcnow()
        if session.expires_at <= now:
            await self.db.execute(delete(Session).where(Session.token == token))
            return None
        # Throttle last_used_at updates
        if (now - session.last_used_at).total_seconds() >= SESSION_TOUCH_THROTTLE_SECS:
            session.last_used_at = now
        return user

    async def request_password_reset(self, email: str) -> None:
        user = await self.db.scalar(
            select(User).where(func.lower(User.email) == email.strip().lower())
        )
        if user is None:
            logger.info("Password reset requested for unknown email")
            return
        await self._issue_email_token(user, "reset_password")

    async def confirm_password_reset(self, token: str, new_password: str) -> None:
        err = validate_password_strength(new_password)
        if err:
            raise AppError("invalid_password", err, 400)
        email_token = await self._consume_email_token(token, "reset_password")
        user = await self.db.get(User, email_token.user_id)
        if user is None:
            raise AppError("invalid_token", "Invalid or expired token", 400)
        user.password_hash = hash_password(new_password)
        # Revoke all sessions
        await self.db.execute(delete(Session).where(Session.user_id == user.id))

    async def verify_email(self, token: str) -> tuple[User, str]:
        email_token = await self._consume_email_token(token, "verify_email")
        user = await self.db.get(User, email_token.user_id)
        if user is None:
            raise AppError("invalid_token", "Invalid or expired token", 400)
        user.email_verified_at = _utcnow()
        session_token = await self._create_session(user)
        return user, session_token

    async def resend_verification(self, email: str) -> None:
        user = await self.db.scalar(
            select(User).where(func.lower(User.email) == email.strip().lower())
        )
        if user is None or user.email_verified_at is not None:
            return
        await self._issue_email_token(user, "verify_email")

    async def _create_session(
        self,
        user: User,
        *,
        user_agent: str | None = None,
        ip: str | None = None,
    ) -> str:
        token = new_token()
        now = _utcnow()
        self.db.add(
            Session(
                token=token,
                user_id=user.id,
                expires_at=now + timedelta(seconds=self.settings.meetops_session_ttl_secs),
                created_at=now,
                last_used_at=now,
                user_agent=user_agent,
                ip=ip,
            )
        )
        await self.db.flush()
        return token

    async def _issue_email_token(self, user: User, purpose: str) -> None:
        await self.db.execute(
            delete(AuthEmailToken).where(
                AuthEmailToken.user_id == user.id,
                AuthEmailToken.purpose == purpose,
            )
        )
        token = new_token()
        ttl = (
            self.settings.meetops_email_verify_ttl_secs
            if purpose == "verify_email"
            else self.settings.meetops_password_reset_ttl_secs
        )
        self.db.add(
            AuthEmailToken(
                token=token,
                user_id=user.id,
                purpose=purpose,
                expires_at=_utcnow() + timedelta(seconds=ttl),
            )
        )
        await self.db.flush()
        if purpose == "verify_email":
            url = f"{self.settings.meetops_app_base_url}/verify-email?token={token}"
            await self.email.send_verification(user.email, user.name, url)
        else:
            url = f"{self.settings.meetops_app_base_url}/reset-password?token={token}"
            await self.email.send_password_reset(user.email, user.name, url)

    async def _consume_email_token(self, token: str, purpose: str) -> AuthEmailToken:
        row = await self.db.get(AuthEmailToken, token)
        if row is None or row.purpose != purpose or row.expires_at <= _utcnow():
            raise AppError("invalid_token", "Invalid or expired token", 400)
        await self.db.execute(delete(AuthEmailToken).where(AuthEmailToken.token == token))
        await self.db.flush()
        return row

    def set_session_cookie(self, response: object, token: str) -> None:
        from starlette.responses import Response

        assert isinstance(response, Response)
        response.set_cookie(
            key=self.settings.meetops_session_cookie_name,
            value=token,
            httponly=True,
            secure=bool(self.settings.meetops_session_cookie_secure),
            samesite=self.settings.meetops_session_cookie_samesite,
            max_age=self.settings.meetops_session_ttl_secs,
            path="/",
        )

    def clear_session_cookie(self, response: object) -> None:
        from starlette.responses import Response

        assert isinstance(response, Response)
        response.delete_cookie(
            key=self.settings.meetops_session_cookie_name,
            path="/",
            samesite=self.settings.meetops_session_cookie_samesite,
            secure=bool(self.settings.meetops_session_cookie_secure),
        )