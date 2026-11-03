//! A stream of events for a lease or a key prefix.

use tokio::sync::mpsc;

/// Events from a `watch`. Dropping it stops watching.
pub struct Watch<T> {
    rx: mpsc::UnboundedReceiver<T>,
}

/// The sending end, held by a backend.
pub struct WatchSender<T> {
    tx: mpsc::UnboundedSender<T>,
}

impl<T> Clone for WatchSender<T> {
    fn clone(&self) -> Self {
        Self { tx: self.tx.clone() }
    }
}

impl<T> Watch<T> {
    pub fn channel() -> (WatchSender<T>, Watch<T>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (WatchSender { tx }, Watch { rx })
    }

    /// The next event, or `None` once the backend is gone.
    pub async fn recv(&mut self) -> Option<T> {
        self.rx.recv().await
    }

    pub fn try_recv(&mut self) -> Option<T> {
        self.rx.try_recv().ok()
    }
}

impl<T> WatchSender<T> {
    /// `false` once the watcher is gone.
    pub fn send(&self, event: T) -> bool {
        self.tx.send(event).is_ok()
    }

    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}
