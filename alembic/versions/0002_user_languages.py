"""Add languages_json to user_settings."""

from __future__ import annotations

import sqlalchemy as sa
from alembic import op
from sqlalchemy.dialects import postgresql

revision = "0002_user_languages"
down_revision = "0001_initial"
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.add_column(
        "user_settings",
        sa.Column(
            "languages_json",
            postgresql.JSONB(),
            nullable=False,
            server_default=sa.text("'[\"en\"]'::jsonb"),
        ),
    )


def downgrade() -> None:
    op.drop_column("user_settings", "languages_json")
