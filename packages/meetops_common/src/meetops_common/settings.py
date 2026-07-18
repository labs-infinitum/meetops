"""MeetOps configuration via environment variables (flat MEETOPS_* style)."""

from __future__ import annotations

from functools import lru_cache
from pathlib import Path
from typing import Literal

from pydantic import field_validator, model_validator
from pydantic_settings import BaseSettings, SettingsConfigDict


def resolve_database_url_from_secret(
    secret_arn: str, host: str, port: int, database: str
) -> str:
    """Build an asyncpg DATABASE_URL from a Secrets Manager secret.

    The secret must contain {"username": ..., "password": ...} (the shape of
    RDS-managed master secrets). Uses the ambient AWS identity (instance role
    or env credentials); the region is taken from the ARN itself.
    """
    import json
    from urllib.parse import quote

    import boto3  # deferred: only needed when the secret ARN is configured

    region = secret_arn.split(":")[3]
    client = boto3.client("secretsmanager", region_name=region)
    secret = json.loads(client.get_secret_value(SecretId=secret_arn)["SecretString"])
    username = quote(secret["username"], safe="")
    password = quote(secret["password"], safe="")
    return f"postgresql+asyncpg://{username}:{password}@{host}:{port}/{database}"


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        extra="ignore",
        case_sensitive=False,
    )

    # Server
    meetops_env: Literal["development", "staging", "production"] = "development"
    meetops_api_host: str = "0.0.0.0"
    meetops_api_port: int = 8000
    meetops_cors_origins: str = "http://localhost:5173,http://localhost:3000"
    meetops_app_base_url: str = "http://localhost:5173"
    meetops_public_api_url: str = "http://localhost:8000"

    # Database
    database_url: str = "postgresql+asyncpg://meetops:meetops@localhost:5432/meetops"
    database_pool_size: int = 10
    database_max_overflow: int = 10
    # Alternative to DATABASE_URL: resolve credentials from an AWS Secrets Manager
    # secret ({"username": ..., "password": ...}, e.g. an RDS-managed master secret).
    # Requires host/name below; overrides DATABASE_URL when set.
    meetops_database_secret_arn: str | None = None
    meetops_database_host: str | None = None
    meetops_database_port: int = 5432
    meetops_database_name: str = "meetops"

    # Auth
    meetops_session_ttl_secs: int = 2_592_000
    meetops_session_cookie_name: str = "meetops_session"
    meetops_session_cookie_secure: bool | None = None
    meetops_session_cookie_samesite: Literal["lax", "strict", "none"] = "lax"
    meetops_require_email_verification: bool = True
    meetops_email_verify_ttl_secs: int = 86_400
    meetops_password_reset_ttl_secs: int = 3_600
    meetops_auth_rate_limit: str = "10/minute"

    # Email
    resend_api_key: str | None = None
    meetops_email_from: str | None = None
    meetops_email_from_name: str = "MeetOps"

    # Google
    google_client_id: str | None = None
    google_client_secret: str | None = None
    google_redirect_uri: str = "http://localhost:8000/auth/google/callback"
    google_frontend_redirect_url: str = "http://localhost:5173/app/calendars"
    meetops_calendar_sync_interval_secs: int = 300

    # Crypto
    meetops_token_encryption_key: str | None = None
    meetops_allow_plaintext_tokens: bool = False

    # Storage
    s3_bucket: str | None = None
    s3_prefix: str = "recordings"
    aws_region: str = "us-east-1"
    aws_access_key_id: str | None = None
    aws_secret_access_key: str | None = None
    meetops_presigned_url_ttl_secs: int = 3600
    meetops_delete_local_after_upload: bool = True
    meetops_data_dir: Path = Path("./data")
    meetops_staging_ttl_days: int = 7

    # Bot
    meetops_bot_display_name: str = "MeetOps Notetaker"
    meetops_bot_concurrency: int = 2
    meetops_bot_profiles_dir: Path | None = None
    meetops_bot_join_lead_time_secs: int = 60
    meetops_bot_admission_timeout_secs: int = 300
    meetops_bot_scheduler_poll_secs: int = 15
    meetops_bot_solo_leave_secs: int = 300
    meetops_bot_adhoc_duration_secs: int = 7200
    meetops_bot_x11_display: str | None = None
    meetops_browser_proxy: str | None = None
    meetops_patchright_channel: str = "bundled"
    meetops_bot_window_width: int = 1920
    meetops_bot_window_height: int = 1080
    meetops_use_tab_capture_auto_accept: bool = False

    # AI (OpenAI)
    openai_api_key: str | None = None
    openai_base_url: str = "https://api.openai.com/v1"
    openai_model: str = "gpt-4.1-mini"
    openai_transcribe_model: str = "whisper-1"

    # Observability
    meetops_log_level: str = "INFO"
    meetops_log_json: bool = False

    # Docs
    meetops_enable_openapi: bool | None = None

    @field_validator("meetops_allow_plaintext_tokens", mode="before")
    @classmethod
    def _parse_boolish(cls, v: object) -> object:
        if isinstance(v, str):
            return v.strip().lower() in {"1", "true", "yes", "on"}
        return v

    @model_validator(mode="after")
    def _defaults_and_validate(self) -> Settings:
        if self.meetops_database_secret_arn:
            if not self.meetops_database_host:
                raise ValueError(
                    "MEETOPS_DATABASE_HOST is required when MEETOPS_DATABASE_SECRET_ARN is set"
                )
            self.database_url = resolve_database_url_from_secret(
                self.meetops_database_secret_arn,
                self.meetops_database_host,
                self.meetops_database_port,
                self.meetops_database_name,
            )
        if self.meetops_session_cookie_secure is None:
            self.meetops_session_cookie_secure = self.meetops_env == "production"
        if self.meetops_enable_openapi is None:
            self.meetops_enable_openapi = self.meetops_env != "production"
        if self.meetops_bot_profiles_dir is None:
            self.meetops_bot_profiles_dir = self.meetops_data_dir / "chrome-profiles"
        if self.meetops_env == "production":
            missing: list[str] = []
            if not self.database_url:
                missing.append("DATABASE_URL")
            if not self.s3_bucket:
                missing.append("S3_BUCKET")
            if not self.meetops_token_encryption_key and not self.meetops_allow_plaintext_tokens:
                missing.append("MEETOPS_TOKEN_ENCRYPTION_KEY")
            if missing:
                raise ValueError(
                    f"Production requires: {', '.join(missing)}. "
                    "Set MEETOPS_ALLOW_PLAINTEXT_TOKENS=1 only for local dev."
                )
        if (
            self.meetops_env == "production"
            and self.meetops_allow_plaintext_tokens
            and not self.meetops_token_encryption_key
        ):
            raise ValueError(
                "MEETOPS_ALLOW_PLAINTEXT_TOKENS is not allowed in production without encryption key"
            )
        return self

    @property
    def cors_origin_list(self) -> list[str]:
        return [o.strip() for o in self.meetops_cors_origins.split(",") if o.strip()]

    @property
    def is_production(self) -> bool:
        return self.meetops_env == "production"

    @property
    def google_oauth_configured(self) -> bool:
        return bool(self.google_client_id and self.google_client_secret)

    @property
    def staging_dir(self) -> Path:
        return self.meetops_data_dir / "recordings" / "staging"


@lru_cache
def get_settings() -> Settings:
    return Settings()


def clear_settings_cache() -> None:
    get_settings.cache_clear()