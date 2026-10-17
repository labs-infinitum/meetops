from meetops_api.storage.local import LocalStorage
from meetops_api.storage.s3 import S3Storage
from meetops_common.settings import Settings

__all__ = ["LocalStorage", "S3Storage", "get_storage"]


def get_storage(settings: Settings) -> LocalStorage | S3Storage | None:
    """The configured recording store, or None when nothing is configured (dev without S3)."""
    if settings.meetops_storage_backend == "local":
        return LocalStorage(settings)
    if settings.s3_bucket:
        return S3Storage(settings)
    return None
