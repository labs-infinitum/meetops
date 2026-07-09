"""Auth routes."""

from __future__ import annotations

from fastapi import APIRouter, Request, Response

from meetops_api.deps import AppSettings, CurrentUser, DbSession, client_ip
from meetops_api.rate_limit import limiter
from meetops_api.schemas import (
    EmailResendRequest,
    EmailVerifyRequest,
    LoginRequest,
    LoginResponse,
    MeResponse,
    MessageResponse,
    PasswordResetConfirm,
    PasswordResetRequest,
    SignupRequest,
    SignupResponse,
    UserOut,
)
from meetops_api.services.auth import AuthService

router = APIRouter(prefix="/auth", tags=["auth"])


def _user_out(user: object) -> UserOut:
    return UserOut.model_validate(user)


@router.post("/signup", response_model=SignupResponse)
@limiter.limit("10/minute")
async def signup(
    request: Request,
    body: SignupRequest,
    db: DbSession,
    settings: AppSettings,
    response: Response,
) -> SignupResponse:
    auth = AuthService(db, settings)
    user, verify_required, session_token = await auth.signup(
        name=body.name,
        email=str(body.email),
        username=body.username,
        password=body.password,
    )
    if session_token:
        auth.set_session_cookie(response, session_token)
    return SignupResponse(user=_user_out(user), email_verification_required=verify_required)


@router.post("/login", response_model=LoginResponse)
@limiter.limit("10/minute")
async def login(
    request: Request,
    body: LoginRequest,
    db: DbSession,
    settings: AppSettings,
    response: Response,
) -> LoginResponse:
    auth = AuthService(db, settings)
    user, token = await auth.login(
        username=body.username,
        password=body.password,
        user_agent=request.headers.get("user-agent"),
        ip=client_ip(request),
    )
    auth.set_session_cookie(response, token)
    return LoginResponse(user=_user_out(user))


@router.post("/logout", response_model=MessageResponse)
async def logout(
    request: Request,
    db: DbSession,
    settings: AppSettings,
    response: Response,
) -> MessageResponse:
    auth = AuthService(db, settings)
    token = request.cookies.get(settings.meetops_session_cookie_name)
    await auth.logout(token)
    auth.clear_session_cookie(response)
    return MessageResponse(message="Logged out")


@router.get("/me", response_model=MeResponse)
async def me(user: CurrentUser) -> MeResponse:
    return MeResponse(user=_user_out(user))


@router.post("/password-reset/request", response_model=MessageResponse)
@limiter.limit("10/minute")
async def password_reset_request(
    request: Request,
    body: PasswordResetRequest,
    db: DbSession,
    settings: AppSettings,
) -> MessageResponse:
    auth = AuthService(db, settings)
    await auth.request_password_reset(str(body.email))
    return MessageResponse(message="If that email exists, a reset link was sent")


@router.post("/password-reset/confirm", response_model=MessageResponse)
@limiter.limit("10/minute")
async def password_reset_confirm(
    request: Request,
    body: PasswordResetConfirm,
    db: DbSession,
    settings: AppSettings,
) -> MessageResponse:
    auth = AuthService(db, settings)
    await auth.confirm_password_reset(body.token, body.new_password)
    return MessageResponse(message="Password updated")


@router.post("/email/verify", response_model=LoginResponse)
@limiter.limit("10/minute")
async def verify_email(
    request: Request,
    body: EmailVerifyRequest,
    db: DbSession,
    settings: AppSettings,
    response: Response,
) -> LoginResponse:
    auth = AuthService(db, settings)
    user, token = await auth.verify_email(body.token)
    auth.set_session_cookie(response, token)
    return LoginResponse(user=_user_out(user))


@router.post("/email/resend", response_model=MessageResponse)
@limiter.limit("10/minute")
async def resend_verification(
    request: Request,
    body: EmailResendRequest,
    db: DbSession,
    settings: AppSettings,
) -> MessageResponse:
    auth = AuthService(db, settings)
    await auth.resend_verification(str(body.email))
    return MessageResponse(message="If that email needs verification, a link was sent")