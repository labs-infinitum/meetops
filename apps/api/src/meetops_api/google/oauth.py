"""Google OAuth helpers."""

from __future__ import annotations

import logging
import secrets
from datetime import UTC, datetime, timedelta
from urllib.parse import urlencode

import httpx
from sqlalchemy import delete, select
from sqlalchemy.ext.asyncio import AsyncSession

from meetops_api.crypto.tokens import TokenCrypto
from meetops_api.errors import AppError
from meetops_api.services.auth import new_id
from meetops_common.constants import GOOGLE_CALENDAR_SCOPES, OAUTH_STATE_TTL_SECS
from meetops_common.models import AppKv, CalendarConnection
from meetops_common.settings import Settings

logger = logging.getLogger(__name__)

AUTH_URL = "https://accounts.google.com/o/oauth2/v2/auth"
TOKEN_URL = "https://oauth2.googleapis.com/token"
USERINFO_URL = "https://www.googleapis.com/oauth2/v2/userinfo"


def _utcnow() -> datetime:
    return datetime.now(UTC)


class GoogleOAuth:
    def __init__(self, db: AsyncSession, settings: Settings, crypto: TokenCrypto) -> None:
        self.db = db
        self.settings = settings
        self.crypto = crypto

    def require_configured(self) -> None:
        if not self.settings.google_oauth_configured:
            raise AppError("google_not_configured", "Google OAuth is not configured", 503)

    async def start(self, user_id: str, *, label: str | None, color: str | None) -> str:
        self.require_configured()
        state = secrets.token_urlsafe(24)
        payload = f"{user_id}|{label or ''}|{color or ''}|{_utcnow().isoformat()}"
        self.db.add(AppKv(key=f"oauth_state:{state}", value=payload, updated_at=_utcnow()))
        await self.db.flush()
        params = {
            "client_id": self.settings.google_client_id,
            "redirect_uri": self.settings.google_redirect_uri,
            "response_type": "code",
            "scope": " ".join(GOOGLE_CALENDAR_SCOPES),
            "access_type": "offline",
            "prompt": "consent",
            "state": state,
        }
        return f"{AUTH_URL}?{urlencode(params)}"

    async def handle_callback(self, code: str, state: str) -> tuple[str, str]:
        """Exchange code, upsert connection. Returns (frontend_redirect, connection_id)."""
        self.require_configured()
        kv = await self.db.get(AppKv, f"oauth_state:{state}")
        if kv is None:
            raise AppError("invalid_state", "Invalid OAuth state", 400)
        await self.db.execute(delete(AppKv).where(AppKv.key == f"oauth_state:{state}"))
        parts = kv.value.split("|", 3)
        if len(parts) < 4:
            raise AppError("invalid_state", "Invalid OAuth state", 400)
        user_id, label, color, created = parts
        try:
            created_at = datetime.fromisoformat(created)
            if created_at.tzinfo is None:
                created_at = created_at.replace(tzinfo=UTC)
            if (_utcnow() - created_at).total_seconds() > OAUTH_STATE_TTL_SECS:
                raise AppError("invalid_state", "OAuth state expired", 400)
        except ValueError as exc:
            raise AppError("invalid_state", "Invalid OAuth state", 400) from exc

        async with httpx.AsyncClient(timeout=30) as client:
            token_resp = await client.post(
                TOKEN_URL,
                data={
                    "code": code,
                    "client_id": self.settings.google_client_id,
                    "client_secret": self.settings.google_client_secret,
                    "redirect_uri": self.settings.google_redirect_uri,
                    "grant_type": "authorization_code",
                },
            )
            if token_resp.status_code >= 400:
                logger.error("Google token exchange failed status=%s", token_resp.status_code)
                raise AppError("oauth_failed", "Google authorization failed", 400)
            tokens = token_resp.json()
            access = tokens.get("access_token")
            refresh = tokens.get("refresh_token")
            expires_in = int(tokens.get("expires_in", 3600))
            if not access or not refresh:
                raise AppError("oauth_failed", "Google did not return refresh token", 400)

            info_resp = await client.get(
                USERINFO_URL, headers={"Authorization": f"Bearer {access}"}
            )
            if info_resp.status_code >= 400:
                raise AppError("oauth_failed", "Failed to fetch Google profile", 400)
            email = info_resp.json().get("email")
            if not email:
                raise AppError("oauth_failed", "Google account has no email", 400)

        existing = await self.db.scalar(
            select(CalendarConnection).where(
                CalendarConnection.provider == "google",
                CalendarConnection.email == email,
            )
        )
        expires_at = _utcnow() + timedelta(seconds=expires_in)
        enc_access = self.crypto.encrypt(access)
        enc_refresh = self.crypto.encrypt(refresh)
        if existing:
            if existing.user_id != user_id:
                raise AppError(
                    "calendar_taken",
                    "This Google calendar is already connected to another account",
                    409,
                )
            existing.access_token = enc_access
            existing.refresh_token = enc_refresh
            existing.expires_at = expires_at
            existing.scope = " ".join(GOOGLE_CALENDAR_SCOPES)
            if label:
                existing.label = label
            if color:
                existing.color = color
            conn_id = existing.id
        else:
            conn_id = new_id()
            self.db.add(
                CalendarConnection(
                    id=conn_id,
                    user_id=user_id,
                    provider="google",
                    email=email,
                    label=label or email,
                    color=color or "#4285F4",
                    access_token=enc_access,
                    refresh_token=enc_refresh,
                    expires_at=expires_at,
                    scope=" ".join(GOOGLE_CALENDAR_SCOPES),
                    bot_session_status="unknown",
                )
            )
        await self.db.flush()
        redirect = (
            f"{self.settings.google_frontend_redirect_url}"
            f"?oauth=connected&connection_id={conn_id}"
        )
        return redirect, conn_id

    async def refresh_if_needed(self, conn: CalendarConnection) -> str:
        access = self.crypto.decrypt(conn.access_token)
        if conn.expires_at and conn.expires_at > _utcnow() + timedelta(seconds=60):
            return access
        refresh = self.crypto.decrypt(conn.refresh_token)
        async with httpx.AsyncClient(timeout=30) as client:
            resp = await client.post(
                TOKEN_URL,
                data={
                    "client_id": self.settings.google_client_id,
                    "client_secret": self.settings.google_client_secret,
                    "refresh_token": refresh,
                    "grant_type": "refresh_token",
                },
            )
            if resp.status_code >= 400:
                logger.error("Google refresh failed status=%s", resp.status_code)
                raise AppError("oauth_refresh_failed", "Failed to refresh Google token", 401)
            data = resp.json()
            access = data["access_token"]
            expires_in = int(data.get("expires_in", 3600))
            conn.access_token = self.crypto.encrypt(access)
            conn.expires_at = _utcnow() + timedelta(seconds=expires_in)
            await self.db.flush()
            return access