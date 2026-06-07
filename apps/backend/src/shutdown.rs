//! Process-wide shutdown coordination for the API server.

use std::sync::Arc;

use tokio::sync::{Notify, broadcast, watch};

/// Signals scheduler/recorder shutdown and coordinates recording drain on SIGTERM/SIGINT.
#[derive(Clone)]
pub struct AppShutdown {
    cancelled: watch::Sender<bool>,
    signal: broadcast::Sender<()>,
    server_stop: Arc<Notify>,
}

/// User stop (`watch`) and/or process shutdown (`AppShutdown`) for the recording bot.
pub struct RecordingStop<'a> {
    pub stop_rx: &'a watch::Receiver<bool>,
    pub shutdown: Option<&'a AppShutdown>,
}

impl RecordingStop<'_> {
    pub fn requested(&self) -> bool {
        *self.stop_rx.borrow() || self.shutdown.is_some_and(AppShutdown::is_cancelled)
    }
}

impl AppShutdown {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            cancelled: watch::channel(false).0,
            signal: broadcast::channel(16).0,
            server_stop: Arc::new(Notify::new()),
        })
    }

    /// Stop accepting HTTP/gRPC and block new scheduler work (recordings keep running).
    pub fn request_server_stop(&self) {
        let _ = self.cancelled.send(true);
        self.server_stop.notify_waiters();
    }

    /// Wake in-flight recorders so they finalize before process exit.
    pub fn trigger_recordings(&self) {
        let _ = self.signal.send(());
    }

    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.borrow()
    }

    pub fn cancelled(&self) -> watch::Receiver<bool> {
        self.cancelled.subscribe()
    }

    pub fn subscribe_signal(&self) -> broadcast::Receiver<()> {
        self.signal.subscribe()
    }

    pub async fn wait_server_stop(&self) {
        self.server_stop.notified().await;
    }
}
