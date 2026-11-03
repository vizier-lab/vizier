//! Testing one Fleet role on its own.
//!
//! # Writing a role test
//!
//! A role is built from a [`Bus`](vizier_schema::Bus), a config and the two storage traits, so a
//! test needs no other role running. [`Harness`] provides all of that in memory and on a
//! temporary directory, and a [`ScriptedPeer`] plays the role's neighbours.
//!
//! 1. Build the harness: `let h = Harness::new().await?;` It holds an in-process bus, an
//!    embedded SQLite database, a local blob store and a throwaway workspace.
//! 2. Build the role under test from it: `let worker = h.build_worker();`. The role learns only
//!    the bus and the storage handles, never which backend is behind them.
//! 3. Play the neighbours: `let mut peer = h.peer();`.
//!    - `peer.on("agent.*.req", |subject, request| ...)` answers requests the role sends.
//!    - `peer.listen("session.>")` subscribes **before** you act; after the action,
//!      `listener.expect(timeout).await` asserts on what the role published.
//!    - `peer.publish(..)` / `peer.request(..)` drive the role from outside.
//! 4. Subjects and what each role may use are in `vizier_schema::contracts` and in the docs page
//!    `docs/src/fleet/bus-contract.md`.
//! 5. Start the runtime paused (`#[tokio::test(start_paused = true)]`) so timeouts and lease TTLs
//!    cost nothing.
//!
//! `tests/roles.rs` is a worked example. A role that needs a capability the backend lacks is
//! refused by [`Harness::require_for`], with the missing capability named.

use std::sync::Arc;

use tempfile::TempDir;
use vizier_core::config::VizierConfig;
use vizier_core::storage::blob::{BlobKey, BlobStore};
use vizier_core::storage::document::BlobDocumentStore;
use vizier_core::storage::sqlite::SqlStorage;
use vizier_core::storage::VizierStorage;
use vizier_schema::testing::ScriptedPeer;
use vizier_schema::{Bus, BusError, Capabilities, InProcess};

use crate::{Channels, Control, GatewayGrpc, GatewayHttp, Worker};

type Error = Box<dyn std::error::Error + Send + Sync>;

/// Everything a role needs, in memory or on a temporary directory.
pub struct Harness {
    pub bus: Bus,
    pub config: Arc<VizierConfig>,
    pub storage: VizierStorage,
    pub blobs: Arc<dyn BlobStore>,
    /// Removed when the harness drops.
    _workspace: TempDir,
}

impl Harness {
    /// An in-process bus advertising everything.
    pub async fn new() -> Result<Self, Error> {
        Self::with_bus(InProcess::new().bus()).await
    }

    /// The same harness on a bus you chose, for example one that lacks a capability.
    pub async fn with_bus(bus: Bus) -> Result<Self, Error> {
        let workspace = tempfile::tempdir()?;
        let config = Arc::new(VizierConfig::with_workspace(workspace.path().to_path_buf()));
        let sql = vizier_core::storage::open_store(&config)?;
        let blobs = vizier_core::storage::blob::open(&config).await?;
        let documents = Arc::new(BlobDocumentStore::new(blobs.clone(), BlobKey::new("agents")?));
        let storage = VizierStorage::new(SqlStorage::new(sql, documents));
        Ok(Self { bus, config, storage, blobs, _workspace: workspace })
    }

    /// A scripted peer on this harness's bus.
    pub fn peer(&self) -> ScriptedPeer {
        ScriptedPeer::new(&self.bus)
    }

    /// Refuse a role whose `needs` the bus does not meet, naming what is missing.
    pub fn require_for(&self, needs: &Capabilities) -> Result<(), BusError> {
        self.bus.require(needs)
    }

    pub fn build_worker(&self) -> Worker {
        Worker::new(self.bus.clone(), self.config.clone(), self.storage.clone(), self.blobs.clone())
    }

    pub fn build_control(&self) -> Control {
        Control::new(self.bus.clone(), self.config.clone(), self.storage.clone(), self.blobs.clone())
    }

    pub fn build_channels(&self) -> Channels {
        Channels::new(self.bus.clone(), self.config.clone(), self.storage.clone(), self.blobs.clone())
    }

    pub fn build_gateway_http(&self) -> GatewayHttp {
        GatewayHttp::new(self.bus.clone(), self.config.clone(), self.storage.clone(), self.blobs.clone())
    }

    pub fn build_gateway_grpc(&self) -> GatewayGrpc {
        GatewayGrpc::new(self.bus.clone(), self.config.clone(), self.storage.clone(), self.blobs.clone())
    }
}
