//! iocsh commands for the CA server — currently just `casr`.
//!
//! Mirrors RSRV's `casr` (`caservertask.c:906`): a one-line summary
//! of the live CA server state. Intended to be cheap so operators
//! can poll it from a shell without disturbing the data path.
//!
//! Wiring:
//!
//! ```ignore
//! use std::sync::Arc;
//! use epics_ca_rs::server::iocsh;
//!
//! let server = Arc::new(CaServer::from_parts(...));
//! let mut cfg = IocRunConfig::default();
//! cfg.shell_commands.push(iocsh::casr_command(server.stats()));
//! ```

use std::sync::Arc;

use epics_base_rs::server::iocsh::registry::{
    ArgDesc, ArgType, ArgValue, CommandContext, CommandDef, CommandOutcome,
};

use super::ca_server::ServerStats;

/// `casr [<level>]` — print CA server runtime statistics. With no
/// argument prints summary counters; level 1+ adds the active-client
/// counter; level 2+ adds the uptime breakdown. RSRV format hints
/// from `caservertask.c:906`.
pub fn casr_command(stats: Arc<ServerStats>) -> CommandDef {
    CommandDef::new(
        "casr",
        vec![ArgDesc {
            name: "level",
            arg_type: ArgType::Int,
            optional: true,
        }],
        "casr [<level>]",
        move |args: &[ArgValue], ctx: &CommandContext| {
            let level = match args.first() {
                Some(ArgValue::Int(n)) => *n,
                _ => 0,
            };
            use std::sync::atomic::Ordering::Relaxed;
            let connects = stats.connects_total.load(Relaxed);
            let disconnects = stats.disconnects_total.load(Relaxed);
            let active = stats.active_clients();
            let up = stats.uptime();
            ctx.println(&format!(
                "Channel Access Server: {active} active client(s), {connects} connect(s) total, {disconnects} disconnect(s) total"
            ));
            if level >= 1 {
                ctx.println(&format!("    uptime: {:.1}s", up.as_secs_f64()));
            }
            if level >= 2 {
                let secs = up.as_secs();
                let h = secs / 3600;
                let m = (secs % 3600) / 60;
                let s = secs % 60;
                ctx.println(&format!("    uptime breakdown: {h}h {m}m {s}s"));
            }
            Ok(CommandOutcome::Continue)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn casr_command_returns_named_command() {
        let stats = Arc::new(ServerStats::default());
        let cmd = casr_command(stats);
        assert_eq!(cmd.name, "casr");
    }
}
