//! [`SyncGroup`] — batch async CA ops + collective wait.
//!
//! Mirrors libca `ca_sg_*` (`syncgrp.cpp`):
//!
//! ```text
//! CA_SYNC_GID gid;
//! ca_sg_create(&gid);
//! ca_sg_array_get(gid, ...);        // schedule a get
//! ca_sg_array_put(gid, ...);        // schedule a put
//! ca_sg_block(gid, 5.0);             // wait for all
//! ca_sg_delete(gid);
//! ```
//!
//! In Rust the same pattern can be written with `tokio::try_join!`,
//! but applications porting from libca want the explicit
//! "schedule N ops, then await as a unit" surface. SyncGroup gives
//! it: every `get` / `put` returns a typed future that the group
//! collects; `block(timeout)` waits for them all (`try_join_all`)
//! and returns the get results in submit order plus any put errors.
//!
//! The group is single-use — drop it after `block`. For long-lived
//! batches reset by creating a new group, exactly like libca's
//! `ca_sg_reset()` recipe (delete + recreate).

use std::time::Duration;

use epics_base_rs::error::{CaError, CaResult};
use epics_base_rs::types::{DbFieldType, EpicsValue};

use super::CaChannel;

/// One pending operation. Internally a boxed dyn-Future so we can
/// store heterogeneous op shapes (get, put, get-with-metadata)
/// alongside each other.
type GetFuture = std::pin::Pin<
    Box<dyn std::future::Future<Output = CaResult<(DbFieldType, EpicsValue)>> + Send>,
>;
type PutFuture = std::pin::Pin<Box<dyn std::future::Future<Output = CaResult<()>> + Send>>;

/// Single-use op group. Mirrors libca `CA_SYNC_GID`.
#[derive(Default)]
pub struct SyncGroup {
    gets: Vec<GetFuture>,
    puts: Vec<PutFuture>,
}

/// Outcome of [`SyncGroup::block`]: every scheduled get's result
/// in submission order, plus the count of completed puts and any
/// put errors. pvxs/libca surface this as separate accessors; we
/// hand back a struct so the caller can grab whichever slice they
/// need.
#[derive(Debug)]
pub struct SyncGroupResults {
    /// One entry per `get` call, in submission order. `Err` slots
    /// surface the per-op error (timeout, disconnect, ...).
    pub gets: Vec<CaResult<(DbFieldType, EpicsValue)>>,
    /// One entry per `put` call, in submission order.
    pub puts: Vec<CaResult<()>>,
}

impl SyncGroup {
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedule a get. The future runs when [`Self::block`] is
    /// awaited; until then it is a deferred handle.
    pub fn get(&mut self, ch: &CaChannel) {
        let ch = ch.clone();
        self.gets.push(Box::pin(async move { ch.get().await }));
    }

    /// Schedule a put. Same deferred semantics as [`Self::get`].
    pub fn put(&mut self, ch: &CaChannel, value: EpicsValue) {
        let ch = ch.clone();
        self.puts
            .push(Box::pin(async move { ch.put(&value).await }));
    }

    /// Wait until every scheduled op completes or `timeout` elapses.
    /// Mirrors libca `ca_sg_block(gid, timeout)`. Returns the
    /// per-op results in submission order; the outer `Result`
    /// reports timeout only.
    pub async fn block(self, timeout: Duration) -> CaResult<SyncGroupResults> {
        let SyncGroup { gets, puts } = self;

        let get_join = futures_util::future::join_all(gets);
        let put_join = futures_util::future::join_all(puts);
        let combined = async {
            let g = get_join.await;
            let p = put_join.await;
            (g, p)
        };

        let (gets_res, puts_res) = tokio::time::timeout(timeout, combined)
            .await
            .map_err(|_| CaError::Timeout)?;
        Ok(SyncGroupResults {
            gets: gets_res,
            puts: puts_res,
        })
    }

    /// Number of currently scheduled operations.
    pub fn len(&self) -> usize {
        self.gets.len() + self.puts.len()
    }

    /// True if no ops have been scheduled yet.
    pub fn is_empty(&self) -> bool {
        self.gets.is_empty() && self.puts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_group_blocks_immediately() {
        let g = SyncGroup::new();
        assert!(g.is_empty());
        assert_eq!(g.len(), 0);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let res = rt.block_on(async { g.block(Duration::from_millis(50)).await });
        let r = res.expect("empty group should never time out");
        assert_eq!(r.gets.len(), 0);
        assert_eq!(r.puts.len(), 0);
    }
}
