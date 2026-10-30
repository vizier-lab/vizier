//! One WebSocket connection's side of the client tool calls made to it
//! (specs/019-client-tools research R2).
//!
//! A call's frame is written straight to the socket, never through the session-event
//! broadcast, which may drop frames when a subscriber lags. Answers are looked up in this
//! connection's own `pending` map, so an answer from any other socket is "not pending" by
//! construction.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use tokio::sync::{mpsc, oneshot};

/// How a call ended on the client's side.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientAnswer {
    Ok(serde_json::Value),
    Err(String),
    /// The connection closed before it answered.
    Disconnected,
}

pub struct ClientConnection {
    pub id: String,
    /// One complete client frame per value, written by the adapter that owns the socket.
    /// Taken on `close`, so the socket's writer task can finish once the socket ends even
    /// while queued requests still hold this connection.
    outbound: Mutex<Option<mpsc::Sender<serde_json::Value>>>,
    pending: Mutex<HashMap<String, oneshot::Sender<ClientAnswer>>>,
    closed: AtomicBool,
}

impl ClientConnection {
    pub fn new(outbound: mpsc::Sender<serde_json::Value>) -> Arc<Self> {
        Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            outbound: Mutex::new(Some(outbound)),
            pending: Mutex::new(HashMap::new()),
            closed: AtomicBool::new(false),
        })
    }

    pub fn is_open(&self) -> bool {
        !self.closed.load(Ordering::SeqCst)
    }

    /// Wait for an answer to `call_id`; `None` once the connection has closed.
    pub fn register(&self, call_id: &str) -> Option<oneshot::Receiver<ClientAnswer>> {
        let mut pending = self.pending.lock().ok()?;
        // Checked under the lock `close` drains, so a call can't slip in after the drain.
        if !self.is_open() {
            return None;
        }
        let (tx, rx) = oneshot::channel();
        pending.insert(call_id.to_string(), tx);
        Some(rx)
    }

    /// Deliver an answer; `false` when the call is not pending on this connection.
    pub fn answer(&self, call_id: &str, answer: ClientAnswer) -> bool {
        let waiter = self.pending.lock().ok().and_then(|mut p| p.remove(call_id));
        match waiter {
            Some(tx) => tx.send(answer).is_ok(),
            None => false,
        }
    }

    /// Stop waiting for `call_id` and tell the client.
    pub fn cancel(&self, call_id: &str) {
        let removed = self.pending.lock().ok().and_then(|mut p| p.remove(call_id));
        if removed.is_some() {
            self.send(serde_json::json!({ "client_tool_cancelled": { "call_id": call_id } }));
        }
    }

    /// Write one frame to the socket without waiting; `false` if it could not be queued.
    pub fn send(&self, frame: serde_json::Value) -> bool {
        let Ok(outbound) = self.outbound.lock() else {
            return false;
        };
        match outbound.as_ref() {
            Some(tx) => tx.try_send(frame).is_ok(),
            None => false,
        }
    }

    /// The socket ended: every waiting call is answered `Disconnected`, and nothing more is
    /// written.
    pub fn close(&self) {
        let drained: Vec<_> = match self.pending.lock() {
            Ok(mut pending) => {
                self.closed.store(true, Ordering::SeqCst);
                pending.drain().map(|(_, tx)| tx).collect()
            }
            Err(_) => {
                self.closed.store(true, Ordering::SeqCst);
                vec![]
            }
        };
        for tx in drained {
            let _ = tx.send(ClientAnswer::Disconnected);
        }
        if let Ok(mut outbound) = self.outbound.lock() {
            outbound.take();
        }
    }
}

impl std::fmt::Debug for ClientConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientConnection")
            .field("id", &self.id)
            .field("open", &self.is_open())
            .finish()
    }
}

#[cfg(test)]
impl ClientConnection {
    /// A connection whose frames land in the returned receiver.
    pub fn for_tests() -> (Arc<Self>, mpsc::Receiver<serde_json::Value>) {
        let (tx, rx) = mpsc::channel(64);
        (Self::new(tx), rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_answer_resolves_the_waiting_call() {
        let (conn, _rx) = ClientConnection::for_tests();
        let waiter = conn.register("c1").unwrap();
        assert!(conn.answer("c1", ClientAnswer::Ok(serde_json::json!(1))));
        assert_eq!(waiter.await.unwrap(), ClientAnswer::Ok(serde_json::json!(1)));
        // Answered once; a second answer is not pending.
        assert!(!conn.answer("c1", ClientAnswer::Ok(serde_json::json!(2))));
    }

    #[test]
    fn an_unknown_call_is_not_pending() {
        let (conn, _rx) = ClientConnection::for_tests();
        assert!(!conn.answer("nope", ClientAnswer::Err("x".into())));
    }

    #[tokio::test]
    async fn closing_disconnects_every_waiter_and_refuses_new_calls() {
        let (conn, _rx) = ClientConnection::for_tests();
        let a = conn.register("a").unwrap();
        let b = conn.register("b").unwrap();
        conn.close();
        assert_eq!(a.await.unwrap(), ClientAnswer::Disconnected);
        assert_eq!(b.await.unwrap(), ClientAnswer::Disconnected);
        assert!(conn.register("c").is_none());
        assert!(!conn.is_open());
        assert!(!conn.send(serde_json::json!({})));
    }

    #[tokio::test]
    async fn cancelling_tells_the_client() {
        let (conn, mut rx) = ClientConnection::for_tests();
        let _waiter = conn.register("c1").unwrap();
        conn.cancel("c1");
        let frame = rx.recv().await.unwrap();
        assert_eq!(frame["client_tool_cancelled"]["call_id"], "c1");
        assert!(!conn.answer("c1", ClientAnswer::Ok(serde_json::json!(null))));
    }
}
