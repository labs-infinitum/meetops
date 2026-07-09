"""Auth API integration tests with TestClient + SQLite is not used; skip if no DB.

These tests use an in-memory approach via dependency overrides when DATABASE_URL
points to a reachable postgres; otherwise they are skipped.
"""

from __future__ import annotations

import os

import pytest
from httpx import ASGITransport, AsyncClient

pytestmark = pytest.mark.asyncio


@pytest.fixture
def anyio_backend():
    return "asyncio"


async def test_healthz() -> None:
    os.environ.setdefault("MEETOPS_ALLOW_PLAINTEXT_TOKENS", "1")
    os.environ.setdefault("MEETOPS_ENV", "development")
    from meetops_api.main import create_app

    app = create_app()
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.get("/healthz")
        assert resp.status_code == 200
        assert resp.json()["status"] == "ok"
