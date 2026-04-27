//! Static address-list backend — same content as `EPICS_CA_ADDR_LIST`,
//! but routed through the discovery framework so the merge logic in
//! the search engine is uniform.

use std::net::SocketAddr;

use super::Backend;

/// Returns a fixed list of addresses on every `discover()` call.
pub struct StaticBackend {
    addrs: Vec<SocketAddr>,
}

impl StaticBackend {
    pub fn new(addrs: Vec<SocketAddr>) -> Self {
        Self { addrs }
    }
}

#[async_trait::async_trait]
impl Backend for StaticBackend {
    async fn discover(&self) -> Vec<SocketAddr> {
        self.addrs.clone()
    }
}
