//! Tracks detached bot recording tasks so graceful shutdown can await them.

use std::{future::Future, sync::Mutex, time::Duration};

use tokio::task::JoinHandle;

/// Max time to wait for in-flight recordings to finalize after a stop request.
pub const DRAIN_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Default)]
pub struct RecordingTasks {
    handles: Mutex<Vec<JoinHandle<()>>>,
}

impl RecordingTasks {
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let handle = tokio::spawn(future);
        self.handles
            .lock()
            .expect("recording tasks lock")
            .push(handle);
    }

    pub async fn drain(&self, timeout: Duration) {
        let mut pending: Vec<JoinHandle<()>> = {
            let mut guard = self.handles.lock().expect("recording tasks lock");
            std::mem::take(&mut *guard)
        };

        if pending.is_empty() {
            return;
        }

        let count = pending.len();
        tracing::info!(count, "awaiting in-flight recording tasks");

        let finished = tokio::time::timeout(timeout, async {
            while let Some(handle) = pending.pop() {
                if let Err(error) = handle.await {
                    if error.is_cancelled() {
                        tracing::debug!("recording task cancelled during shutdown");
                    } else {
                        tracing::warn!(%error, "recording task panicked");
                    }
                }
            }
        })
        .await;

        match finished {
            Ok(()) => tracing::info!("all recording tasks finished"),
            Err(_) => {
                let remaining = pending.len();
                for handle in pending {
                    handle.abort();
                }
                tracing::warn!(
                    remaining,
                    timeout_secs = timeout.as_secs(),
                    "recording drain timed out; aborted remaining tasks"
                );
            }
        }
    }
}
