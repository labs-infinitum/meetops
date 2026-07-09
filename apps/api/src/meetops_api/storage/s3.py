"""S3 multipart upload + presigned URLs."""

from __future__ import annotations

import logging
import time
from datetime import UTC, datetime, timedelta
from pathlib import Path

import boto3
from botocore.client import Config
from botocore.exceptions import BotoCoreError, ClientError

from meetops_common.settings import Settings

logger = logging.getLogger(__name__)

MULTIPART_THRESHOLD = 8 * 1024 * 1024  # 8 MiB
PART_SIZE = 8 * 1024 * 1024
MAX_RETRIES = 5


class S3Storage:
    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        kwargs: dict = {
            "region_name": settings.aws_region,
            "config": Config(signature_version="s3v4", retries={"max_attempts": 3}),
        }
        if settings.aws_access_key_id and settings.aws_secret_access_key:
            kwargs["aws_access_key_id"] = settings.aws_access_key_id
            kwargs["aws_secret_access_key"] = settings.aws_secret_access_key
        self._client = boto3.client("s3", **kwargs)

    def recording_key(self, user_id: str, run_id: int | str, name: str = "raw.webm") -> str:
        return f"{self.settings.s3_prefix}/{user_id}/{run_id}/{name}"

    def upload_file(self, local_path: Path, key: str, *, content_type: str = "video/webm") -> None:
        if not self.settings.s3_bucket:
            raise RuntimeError("S3_BUCKET not configured")
        size = local_path.stat().st_size
        if size < MULTIPART_THRESHOLD:
            self._put_with_retry(local_path, key, content_type)
        else:
            self._multipart_upload(local_path, key, content_type)

    def _put_with_retry(self, local_path: Path, key: str, content_type: str) -> None:
        delay = 1.0
        for attempt in range(MAX_RETRIES):
            try:
                self._client.put_object(
                    Bucket=self.settings.s3_bucket,
                    Key=key,
                    Body=local_path.read_bytes(),
                    ContentType=content_type,
                )
                return
            except (BotoCoreError, ClientError):
                if attempt == MAX_RETRIES - 1:
                    raise
                logger.warning("S3 put retry attempt=%s key=%s", attempt + 1, key)
                time.sleep(delay)
                delay *= 2

    def _multipart_upload(self, local_path: Path, key: str, content_type: str) -> None:
        bucket = self.settings.s3_bucket
        assert bucket
        delay = 1.0
        for attempt in range(MAX_RETRIES):
            try:
                mpu = self._client.create_multipart_upload(
                    Bucket=bucket, Key=key, ContentType=content_type
                )
                upload_id = mpu["UploadId"]
                parts = []
                part_number = 1
                with local_path.open("rb") as f:
                    while True:
                        data = f.read(PART_SIZE)
                        if not data:
                            break
                        resp = self._client.upload_part(
                            Bucket=bucket,
                            Key=key,
                            PartNumber=part_number,
                            UploadId=upload_id,
                            Body=data,
                        )
                        parts.append({"ETag": resp["ETag"], "PartNumber": part_number})
                        part_number += 1
                self._client.complete_multipart_upload(
                    Bucket=bucket,
                    Key=key,
                    UploadId=upload_id,
                    MultipartUpload={"Parts": parts},
                )
                return
            except (BotoCoreError, ClientError):
                if attempt == MAX_RETRIES - 1:
                    raise
                logger.warning("S3 multipart retry attempt=%s key=%s", attempt + 1, key)
                time.sleep(delay)
                delay *= 2

    def presign_get(self, key: str) -> tuple[str, datetime]:
        if not self.settings.s3_bucket:
            raise RuntimeError("S3_BUCKET not configured")
        ttl = self.settings.meetops_presigned_url_ttl_secs
        url = self._client.generate_presigned_url(
            "get_object",
            Params={"Bucket": self.settings.s3_bucket, "Key": key},
            ExpiresIn=ttl,
        )
        expires = datetime.now(UTC) + timedelta(seconds=ttl)
        return url, expires

    def download_file(self, key: str, dest: Path) -> None:
        if not self.settings.s3_bucket:
            raise RuntimeError("S3_BUCKET not configured")
        dest.parent.mkdir(parents=True, exist_ok=True)
        self._client.download_file(self.settings.s3_bucket, key, str(dest))