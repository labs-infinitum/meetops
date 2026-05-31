use std::path::{Path, PathBuf};
use std::time::Duration;

use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;

use crate::config::StorageConfig;

#[derive(Clone)]
pub struct RecordingsStore {
    config: StorageConfig,
    s3: S3Client,
}

impl RecordingsStore {
    pub async fn connect(config: &StorageConfig) -> anyhow::Result<Self> {
        let bucket = config.recordings.s3.bucket.trim();
        if bucket.is_empty() {
            anyhow::bail!(
                "S3 bucket is required for recordings — set storage.recordings.s3.bucket or S3_BUCKET"
            );
        }

        let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
        if let Some(region) = config.recordings.s3.region.as_deref() {
            loader = loader.region(aws_config::Region::new(region.to_owned()));
        }
        let shared = loader.load().await;
        let s3 = S3Client::new(&shared);

        Ok(Self {
            config: config.clone(),
            s3,
        })
    }

    pub fn bucket(&self) -> &str {
        &self.config.recordings.s3.bucket
    }

    pub fn presigned_url_ttl_secs(&self) -> u64 {
        self.config.recordings.s3.presigned_url_ttl_secs
    }

    /// Temporary on-disk directory while Chrome captures the Meet tab; uploaded to S3 after the run.
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

    pub fn object_key_for_tab_capture(&self, user_id: &str, run_id: i64) -> String {
        let prefix = normalized_prefix(&self.config.recordings.s3.prefix);
        format!(
            "{prefix}/{}/{}/tab-capture.png",
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
    ) -> anyhow::Result<String> {
        let key = self.object_key_for_bot_run(user_id, run_id);
        self.upload_file(&self.s3, local_path, &key, "video/webm")
            .await?;
        if let Some(staging_dir) = local_path.parent() {
            self.persist_tab_capture_if_present(staging_dir, user_id, run_id)
                .await?;
        }
        if self.config.recordings.delete_local_after_upload {
            self.remove_staging_artifacts(local_path).await;
        }
        Ok(key)
    }

    /// Uploads `tab-capture.png` from a bot run staging directory when present.
    pub async fn persist_tab_capture_if_present(
        &self,
        staging_dir: &Path,
        user_id: &str,
        run_id: i64,
    ) -> anyhow::Result<()> {
        let local_path = staging_dir.join("tab-capture.png");
        if tokio::fs::metadata(&local_path).await.is_err() {
            return Ok(());
        }
        let key = self.object_key_for_tab_capture(user_id, run_id);
        self.upload_file(&self.s3, &local_path, &key, "image/png")
            .await?;
        Ok(())
    }

    pub async fn persist_manual_recording(
        &self,
        local_path: &Path,
        recording_id: &str,
    ) -> anyhow::Result<String> {
        let key = self.object_key_for_manual(recording_id);
        self.upload_file(&self.s3, local_path, &key, "video/webm")
            .await?;
        if self.config.recordings.delete_local_after_upload {
            self.remove_staging_artifacts(local_path).await;
        }
        Ok(key)
    }

    pub async fn playback_url(&self, object_key: &str) -> anyhow::Result<String> {
        let ttl = Duration::from_secs(self.config.recordings.s3.presigned_url_ttl_secs);
        let presigned = self
            .s3
            .get_object()
            .bucket(&self.config.recordings.s3.bucket)
            .key(object_key)
            .presigned(PresigningConfig::expires_in(ttl)?)
            .await?;
        Ok(presigned.uri().to_string())
    }

    async fn upload_file(
        &self,
        client: &S3Client,
        local_path: &Path,
        key: &str,
        content_type: &str,
    ) -> anyhow::Result<()> {
        let body = ByteStream::from_path(local_path).await?;
        client
            .put_object()
            .bucket(&self.config.recordings.s3.bucket)
            .key(key)
            .content_type(content_type)
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

    async fn remove_staging_artifacts(&self, local_path: &Path) {
        if let Err(error) = tokio::fs::remove_file(local_path).await {
            tracing::warn!(%error, path = %local_path.display(), "failed to delete staging file after S3 upload");
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
