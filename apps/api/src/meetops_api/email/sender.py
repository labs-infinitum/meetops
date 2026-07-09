"""Email sending via Resend (or log links in development)."""

from __future__ import annotations

import logging

from meetops_common.settings import Settings

logger = logging.getLogger(__name__)


class EmailSender:
    def __init__(self, settings: Settings) -> None:
        self.settings = settings

    async def send_verification(self, to: str, name: str, url: str) -> None:
        subject = "Verify your MeetOps email"
        body = f"Hi {name},\n\nVerify your email: {url}\n\n— MeetOps"
        await self._send(to, subject, body, url)

    async def send_password_reset(self, to: str, name: str, url: str) -> None:
        subject = "Reset your MeetOps password"
        body = f"Hi {name},\n\nReset your password: {url}\n\n— MeetOps"
        await self._send(to, subject, body, url)

    async def send_notes_ready(self, to: str, name: str, meeting_title: str, url: str) -> None:
        subject = f"Notes ready: {meeting_title}"
        body = f"Hi {name},\n\nYour meeting notes for “{meeting_title}” are ready: {url}\n"
        await self._send(to, subject, body, url)

    async def _send(self, to: str, subject: str, body: str, link: str) -> None:
        if not self.settings.resend_api_key or not self.settings.meetops_email_from:
            if self.settings.meetops_env == "development":
                logger.info("Email (dev log) to=%s subject=%s link=%s", to, subject, link)
            else:
                logger.warning("Email not configured; skipping send to=%s subject=%s", to, subject)
            return
        try:
            import resend

            resend.api_key = self.settings.resend_api_key
            from_addr = f"{self.settings.meetops_email_from_name} <{self.settings.meetops_email_from}>"
            resend.Emails.send(
                {
                    "from": from_addr,
                    "to": [to],
                    "subject": subject,
                    "text": body,
                }
            )
        except Exception:
            logger.exception("Failed to send email to=%s subject=%s", to, subject)