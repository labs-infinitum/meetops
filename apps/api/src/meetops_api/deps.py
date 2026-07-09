"""FastAPI dependency injection."""

from __future__ import annotations

from collections.abc import AsyncGenerator
from typing import Annotated

from fastapi import Depends, Request
from sqlalchemy.ext.asyncio import AsyncSession

from meetops_api.crypto.tokens import TokenCrypto
from meetops_api.errors import AppError
from meetops_common.db import get_session_factory
from meetops_common.models import User
from meetops_common.settings import Settings, get_settings


async def get_db() -> AsyncGenerator[AsyncSession, None]:
    factory = get_session_factory()
    async with factory() as session:
        try:
            yield session
            await session.commit()
        except Exception:
            await session.rollback()
            raise


def settings_dep() -> Settings:
    return get_settings()


def token_crypto_dep(settings: Annotated[Settings, Depends(settings_dep)]) -> TokenCrypto:
    return TokenCrypto(settings)


async def get_current_user_optional(
    request: Request,
    db: Annotated[AsyncSession, Depends(get_db)],
    settings: Annotated[Settings, Depends(settings_dep)],
) -> User | None:
    from meetops_api.services.auth import AuthService

    token = request.cookies.get(settings.meetops_session_cookie_name)
    if not token:
        return None
    auth = AuthService(db, settings)
    return await auth.get_user_for_session(token)


async def get_current_user(
    user: Annotated[User | None, Depends(get_current_user_optional)],
) -> User:
    if user is None:
        raise AppError("unauthorized", "Authentication required", 401)
    return user


DbSession = Annotated[AsyncSession, Depends(get_db)]
CurrentUser = Annotated[User, Depends(get_current_user)]
OptionalUser = Annotated[User | None, Depends(get_current_user_optional)]
AppSettings = Annotated[Settings, Depends(settings_dep)]
Crypto = Annotated[TokenCrypto, Depends(token_crypto_dep)]


def client_ip(request: Request) -> str:
    forwarded = request.headers.get("x-forwarded-for")
    if forwarded:
        return forwarded.split(",")[0].strip()
    return request.client.host if request.client else "unknown"