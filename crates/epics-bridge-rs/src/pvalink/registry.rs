//! Process-wide registry of open [`PvaLink`]s, keyed by PV name + direction.
//!
//! Used by record handlers so multiple records pointing at the same PV
//! share a single underlying client connection.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use super::config::{LinkDirection, PvaLinkConfig};
use super::link::{PvaLink, PvaLinkResult};

/// Cached PvaLink. Returns the same `Arc<PvaLink>` for repeated `(pv, direction)` pairs.
#[derive(Default)]
pub struct PvaLinkRegistry {
    map: RwLock<HashMap<(String, LinkDirection), Arc<PvaLink>>>,
}

impl PvaLinkRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get an existing link or open a new one.
    pub async fn get_or_open(&self, config: PvaLinkConfig) -> PvaLinkResult<Arc<PvaLink>> {
        let key = (config.pv_name.clone(), config.direction);
        if let Some(existing) = self.map.read().get(&key).cloned() {
            return Ok(existing);
        }
        let link = Arc::new(PvaLink::open(config).await?);
        let mut guard = self.map.write();
        // Double-checked: another task may have raced.
        if let Some(existing) = guard.get(&key).cloned() {
            return Ok(existing);
        }
        guard.insert(key, link.clone());
        Ok(link)
    }

    pub fn close_all(&self) {
        self.map.write().clear();
    }

    pub fn len(&self) -> usize {
        self.map.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn close_all_empties_registry() {
        let reg = PvaLinkRegistry::new();
        // Don't actually open links (would require a running PVA server);
        // just exercise the empty-state APIs.
        assert!(reg.is_empty());
        reg.close_all();
        assert_eq!(reg.len(), 0);
    }
}
