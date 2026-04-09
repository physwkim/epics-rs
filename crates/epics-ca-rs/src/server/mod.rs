//! CA server components — TCP handler, UDP search, beacon, monitor.

pub mod beacon;
pub mod ca_server;
pub mod ioc_app;
pub mod monitor;
pub mod tcp;
pub mod udp;

pub use ca_server::{CaServer, CaServerBuilder};
pub use tcp::ServerConnectionEvent;

use epics_base_rs::error::CaResult;
use epics_base_rs::server::ioc_app::IocRunConfig;

/// Run an IOC with the Channel Access protocol.
///
/// This is the standard protocol runner for [`IocApplication::run`].
/// It creates a [`CaServer`] from the provided configuration and
/// starts the CA server with an interactive iocsh shell.
///
/// # Example
///
/// ```rust,ignore
/// IocApplication::new()
///     .startup_script("st.cmd")
///     .run(epics_ca_rs::server::run_ca_ioc)
///     .await
/// ```
pub async fn run_ca_ioc(config: IocRunConfig) -> CaResult<()> {
    let server = CaServer::from_parts(
        config.db,
        config.port,
        config.acf,
        config.autosave_config,
        config.autosave_manager,
    );
    server
        .run_with_shell(move |shell| {
            for cmd in config.shell_commands {
                shell.register(cmd);
            }
        })
        .await
}
