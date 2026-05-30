use std::path::{Path, PathBuf};
use std::time::Duration;

use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client as S3Client;

use crate::config::{RecordingsBackend, StorageConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingStorageKind {
    Local,
    S3,
}

impl RecordingStorageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::S3 => "s3",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "s3" => Self::S3,
            _ => Self::Local,
        }
    }
}

#[derive(Clone)]
pub struct RecordingsStore {
    config: StorageConfig,
    s3: Option<S3Client>,
}

impl RecordingsStore {
    pub async fn connect(config: &StorageConfig) -> anyhow::Result<Self> {
        let s3 = if config.recordings.backend == RecordingsBackend::S3
            && !config.recordings.s3.bucket.is_empty()
        {
            let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
            if let Some(region) = config.recordings.s3.region.as_deref() {
                loader = loader.region(aws_config::Region::new(region.to_owned()));
            }
            let shared = loader.load().await;
            Some(S3Client::new(&shared))
        } else {
            None
        };

        Ok(Self {
            config: config.clone(),
            s3,
        })
    }

    pub fn uses_s3(&self) -> bool {
        self.s3.is_some()
    }

    pub fn presigned_url_ttl_secs(&self) -> u64 {
        self.config.recordings.s3.presigned_url_ttl_secs
    }

    pub fn staging_dir(&self, user_id: &str, run_id: i64) -> PathBuf {
        self.config
            .data_dir
            .join("recordings")
            .join("staging")
            .join(sanitize_path_segment(user_id))
            .join(run_id.to_string())
    }

    pub fn manual_staging_dir(&self, recording_id: &str) -> PathBuf {
        self.config
            .data_dir
            .join("recordings")
            .join("staging")
            .join("manual")
            .join(sanitize_path_segment(recording_id))
    }

    pub fn object_key_for_bot_run(&self, user_id: &str, run_id: i64) -> String {
        let prefix = normalized_prefix(&self.config.recordings.s3.prefix);
        format!(
            "{prefix}/{}/{}/raw.webm",
            sanitize_path_segment(user_id),
            run_id
        )
    }

    pub fn object_key_for_manual(&self, recording_id: &str) -> String {
        let prefix = normalized_prefix(&self.config.recordings.s3.prefix);
        format!(
            "{prefix}/manual/{}/raw.webm",
            sanitize_path_segment(recording_id)
        )
    }

    pub async fn persist_bot_recording(
        &self,
        local_path: &Path,
        user_id: &str,
        run_id: i64,
    ) -> anyhow::Result<(String, RecordingStorageKind)> {
        if let Some(client) = &self.s3 {
            let key = self.object_key_for_bot_run(user_id, run_id);
            self.upload_file(client, local_path, &key).await?;
            if self.config.recordings.delete_local_after_upload {
                self.remove_local_artifacts(local_path).await;
            }
            Ok((key, RecordingStorageKind::S3))
        } else {
            let rel = self.local_relative_path(local_path)?;
            Ok((rel, RecordingStorageKind::Local))
        }
    }

    pub async fn persist_manual_recording(
        &self,
        local_path: &Path,
        recording_id: &str,
    ) -> anyhow::Result<(String, RecordingStorageKind)> {
        if let Some(client) = &self.s3 {
            let key = self.object_key_for_manual(recording_id);
            self.upload_file(client, local_path, &key).await?;
            if self.config.recordings.delete_local_after_upload {
                self.remove_local_artifacts(local_path).await;
            }
            Ok((key, RecordingStorageKind::S3))
        } else {
            let rel = self.local_relative_path(local_path)?;
            Ok((rel, RecordingStorageKind::Local))
        }
    }

    pub async fn playback_url(
        &self,
        storage: RecordingStorageKind,
        stored_path: &str,
    ) -> anyhow::Result<String> {
        match storage {
            RecordingStorageKind::S3 => {
                let client = self
                    .s3
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("S3 is not configured"))?;
                let ttl = Duration::from_secs(self.config.recordings.s3.presigned_url_ttl_secs);
                let presigned = client
                    .get_object()
                    .bucket(&self.config.recordings.s3.bucket)
                    .key(stored_path)
                    .presigned(PresigningConfig::expires_in(ttl)?)
                    .await?;
                Ok(presigned.uri().to_string())
            }
            RecordingStorageKind::Local => {
                anyhow::bail!("local recording playback is not exposed via URL; configure S3")
            }
        }
    }

    async fn upload_file(&self, client: &S3Client, local_path: &Path, key: &str) -> anyhow::Result<()> {
        let body = ByteStream::from_path(local_path).await?;
        client
            .put_object()
            .bucket(&self.config.recordings.s3.bucket)
            .key(key)
            .content_type("video/webm")
            .body(body)
            .send()
            .await?;
        let bytes = tokio::fs::metadata(local_path).await.ok().map(|m| m.len());
        tracing::info!(
            bucket = %self.config.recordings.s3.bucket,
            key = %key,
            ?bytes,
            "uploaded recording to S3"
        );
        Ok(())
    }

    fn local_relative_path(&self, local_path: &Path) -> anyhow::Result<String> {
        local_path
            .strip_prefix(&self.config.data_dir)
            .map(|path| path.to_string_lossy().into_owned())
            .map_err(|_| anyhow::anyhow!("recording path is outside data_dir"))
    }

    async fn remove_local_artifacts(&self, local_path: &Path) {
        if let Err(error) = tokio::fs::remove_file(local_path).await {
            tracing::warn!(%error, path = %local_path.display(), "failed to delete local recording after S3 upload");
        }
        if let Some(parent) = local_path.parent() {
            let _ = tokio::fs::remove_dir_all(parent).await;
        }
    }
}

fn normalized_prefix(prefix: &str) -> String {
    prefix.trim().trim_matches('/').to_owned()
}

fn sanitize_path_segment(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "unknown".to_owned();
    }
    trimmed
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
