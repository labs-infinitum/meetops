"""Google Calendar sync."""

from __future__ import annotations

import logging
import re
from datetime import UTC, datetime, timedelta
from typing import Any

import httpx
from sqlalchemy import delete, select
from sqlalchemy.ext.asyncio import AsyncSession

from meetops_api.crypto.tokens import TokenCrypto
from meetops_api.google.oauth import GoogleOAuth
from meetops_common.models import CalendarConnection, Meeting, UserSettings
from meetops_common.settings import Settings

logger = logging.getLogger(__name__)

MEET_URL_RE = re.compile(
    r"https://meet\.google\.com/[a-z0-9\-]+",
    re.IGNORECASE,
)
CALENDAR_API = "https://www.googleapis.com/calendar/v3"


def _utcnow() -> datetime:
    return datetime.now(UTC)


def extract_meet_url(event: dict[str, Any]) -> str | None:
    if hangout := event.get("hangoutLink"):
        return hangout
    conf = event.get("conferenceData") or {}
    for ep in conf.get("entryPoints") or []:
        if ep.get("entryPointType") == "video" and ep.get("uri"):
            return ep["uri"]
    desc = event.get("description") or ""
    m = MEET_URL_RE.search(desc)
    return m.group(0) if m else None


def parse_event_time(value: dict[str, str] | None) -> datetime | None:
    if not value:
        return None
    if "dateTime" in value:
        dt = datetime.fromisoformat(value["dateTime"].replace("Z", "+00:00"))
        if dt.tzinfo is None:
            dt = dt.replace(tzinfo=UTC)
        return dt
    # all-day — skip
    return None


class CalendarSync:
    def __init__(self, db: AsyncSession, settings: Settings, crypto: TokenCrypto) -> None:
        self.db = db
        self.settings = settings
        self.oauth = GoogleOAuth(db, settings, crypto)

    async def sync_connection(self, conn: CalendarConnection) -> dict[str, int | datetime | None]:
        access = await self.oauth.refresh_if_needed(conn)
        settings_row = await self.db.get(UserSettings, conn.user_id)
        auto_default = settings_row.auto_record_default if settings_row else True

        seen = 0
        upserted = 0
        deleted = 0
        headers = {"Authorization": f"Bearer {access}"}
        params: dict[str, str] = {
            "singleEvents": "true",
            "showDeleted": "true",
            "maxResults": "250",
        }

        use_sync_token = bool(conn.sync_token)
        if use_sync_token and conn.sync_token:
            params["syncToken"] = conn.sync_token
        else:
            now = _utcnow()
            params["timeMin"] = now.isoformat()
            params["timeMax"] = (now + timedelta(days=14)).isoformat()
            params["orderBy"] = "startTime"

        next_sync_token: str | None = None
        page_token: str | None = None
        events: list[dict[str, Any]] = []

        async with httpx.AsyncClient(timeout=60) as client:
            while True:
                page_params = dict(params)
                if page_token:
                    page_params["pageToken"] = page_token
                resp = await client.get(
                    f"{CALENDAR_API}/calendars/primary/events",
                    headers=headers,
                    params=page_params,
                )
                if resp.status_code == 410 and use_sync_token:
                    logger.info("Calendar syncToken expired; full resync conn=%s", conn.id)
                    conn.sync_token = None
                    await self.db.flush()
                    return await self.sync_connection(conn)
                if resp.status_code >= 400:
                    logger.error(
                        "Calendar sync failed conn=%s status=%s", conn.id, resp.status_code
                    )
                    raise RuntimeError(f"Calendar sync failed: {resp.status_code}")
                data = resp.json()
                events.extend(data.get("items") or [])
                page_token = data.get("nextPageToken")
                next_sync_token = data.get("nextSyncToken") or next_sync_token
                if not page_token:
                    break

        seen_ids: set[str] = set()
        for event in events:
            seen += 1
            google_event_id = event.get("id")
            if not google_event_id:
                continue
            meeting_id = f"{conn.id}:{google_event_id}"
            seen_ids.add(meeting_id)
            status = event.get("status")
            if status == "cancelled":
                result = await self.db.execute(delete(Meeting).where(Meeting.id == meeting_id))
                deleted += result.rowcount or 0
                continue

            start = parse_event_time(event.get("start"))
            end = parse_event_time(event.get("end"))
            if start is None or end is None:
                logger.debug("Skipping all-day or untimed event %s", google_event_id)
                continue

            meet_url = extract_meet_url(event)
            attendees = [
                {"email": a.get("email"), "displayName": a.get("displayName")}
                for a in (event.get("attendees") or [])
                if a.get("email")
            ]
            existing = await self.db.get(Meeting, meeting_id)
            if existing:
                existing.title = event.get("summary") or "(No title)"
                existing.description = event.get("description")
                existing.meet_url = meet_url
                existing.start_at = start
                existing.end_at = end
                existing.attendees_json = attendees
                existing.calendar_etag = event.get("etag")
                upserted += 1
            else:
                self.db.add(
                    Meeting(
                        id=meeting_id,
                        user_id=conn.user_id,
                        calendar_connection_id=conn.id,
                        google_event_id=google_event_id,
                        title=event.get("summary") or "(No title)",
                        description=event.get("description"),
                        meet_url=meet_url,
                        start_at=start,
                        end_at=end,
                        auto_join=auto_default,
                        attendees_json=attendees,
                        calendar_etag=event.get("etag"),
                    )
                )
                upserted += 1

        if not use_sync_token:
            # Full sync: delete meetings for this connection not seen in window
            rows = await self.db.scalars(
                select(Meeting).where(Meeting.calendar_connection_id == conn.id)
            )
            for m in rows:
                if m.id not in seen_ids and m.start_at >= _utcnow():
                    await self.db.delete(m)
                    deleted += 1

        if next_sync_token:
            conn.sync_token = next_sync_token
        conn.last_synced_at = _utcnow()
        await self.db.flush()
        return {
            "seen": seen,
            "upserted": upserted,
            "deleted": deleted,
            "last_synced_at": conn.last_synced_at,
        }

    async def sync_all(self) -> None:
        rows = await self.db.scalars(select(CalendarConnection))
        for conn in rows:
            try:
                await self.sync_connection(conn)
            except Exception:
                logger.exception("Calendar sync failed for conn=%s", conn.id)