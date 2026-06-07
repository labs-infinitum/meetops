use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::watch;

#[derive(Clone, Default)]
pub struct ActiveRecordings {
    inner: std::sync::Arc<Mutex<HashMap<i64, watch::Sender<bool>>>>,
}

impl ActiveRecordings {
    pub fn register(&self, run_id: i64) -> watch::Receiver<bool> {
        let (tx, rx) = watch::channel(false);
        self.inner
            .lock()
            .expect("active recordings lock")
            .insert(run_id, tx);
        rx
    }

    pub fn unregister(&self, run_id: i64) {
        self.inner
            .lock()
            .expect("active recordings lock")
            .remove(&run_id);
    }

    pub fn request_stop(&self, run_id: i64) -> bool {
        let guard = self.inner.lock().expect("active recordings lock");
        if let Some(tx) = guard.get(&run_id) {
            tx.send(true).is_ok()
        } else {
            false
        }
    }

    /// Request stop on every in-flight recording (used during process shutdown).
    pub fn request_stop_all(&self) -> usize {
        let guard = self.inner.lock().expect("active recordings lock");
        guard.values().filter(|tx| tx.send(true).is_ok()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::ActiveRecordings;

    #[test]
    fn request_stop_all_signals_every_registered_run() {
        let active = ActiveRecordings::default();
        let mut first = active.register(1);
        let mut second = active.register(2);

        assert_eq!(active.request_stop_all(), 2);
        assert!(*first.borrow_and_update());
        assert!(*second.borrow_and_update());
        assert!(active.request_stop(1));
        assert!(active.request_stop(2));
    }
}
