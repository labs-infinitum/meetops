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
        self.inner.lock().expect("active recordings lock").insert(run_id, tx);
        rx
    }

    pub fn unregister(&self, run_id: i64) {
        self.inner.lock().expect("active recordings lock").remove(&run_id);
    }

    pub fn request_stop(&self, run_id: i64) -> bool {
        let guard = self.inner.lock().expect("active recordings lock");
        if let Some(tx) = guard.get(&run_id) {
            tx.send(true).is_ok()
        } else {
            false
        }
    }
}
