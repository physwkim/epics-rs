//! iocsh commands for pvalink — `pvxr`, `pvxrefdiff`, `dbpvxr`.
//!
//! Mirrors pvxs `ioc/pvalink.cpp` (`dbpvxr`, `pvxrefdiff`,
//! `testqsrvWaitForLinkConnected`). Pre-warms link entries so the
//! synchronous record-link resolver can read cached monitor values
//! without `block_on(GET)`.

use epics_base_rs::server::iocsh::registry::{
    ArgDesc, ArgType, ArgValue, CommandContext, CommandDef, CommandOutcome,
};

use super::integration::PvaLinkResolver;

/// `pvxr <pv_name>` — pre-open a link in INP+monitor mode so the
/// resolver returns cached values for that PV without a blocking GET
/// on first access. Mirrors pvxs `pvalinkOpen` (pvalink_channel.cpp).
pub fn db_pvxr_command(resolver: PvaLinkResolver) -> CommandDef {
    CommandDef::new(
        "pvxr",
        vec![ArgDesc {
            name: "pv_name",
            arg_type: ArgType::String,
            optional: false,
        }],
        "pvxr <pv_name>",
        move |args: &[ArgValue], ctx: &CommandContext| {
            let name = match args.first() {
                Some(ArgValue::String(s)) => s.clone(),
                _ => return Err("pvxr: missing pv_name".into()),
            };
            let resolver = resolver.clone();
            let handle = ctx.runtime_handle().clone();
            let result = std::thread::spawn(move || {
                handle.block_on(async move { resolver.open(&name).await })
            })
            .join();
            match result {
                Ok(Ok(_link)) => {
                    ctx.println("pvxr: opened (monitor active)");
                    Ok(CommandOutcome::Continue)
                }
                Ok(Err(e)) => Err(format!("pvxr: open failed: {e}")),
                Err(_) => Err("pvxr: panic in runtime thread".into()),
            }
        },
    )
}

/// `pvxrefdiff` — print "links touched since last call" delta.
/// Mirrors pvxs `pvxrefdiff` (iochooks.cpp:270). Uses interior counter
/// state on the [`PvaLinkResolver`] — the first call shows the
/// running total, subsequent calls show deltas vs. the previous call.
pub fn pvxrefdiff_command(resolver: PvaLinkResolver) -> CommandDef {
    let last = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    CommandDef::new(
        "pvxrefdiff",
        vec![],
        "pvxrefdiff",
        move |_args: &[ArgValue], ctx: &CommandContext| {
            let now = resolver.read_count();
            let prev = last.swap(now, std::sync::atomic::Ordering::Relaxed);
            let delta = now.wrapping_sub(prev);
            ctx.println(&format!(
                "pvxrefdiff: {delta} read(s) since last call (total {now}, {} cached link(s))",
                resolver.link_count()
            ));
            Ok(CommandOutcome::Continue)
        },
    )
}

/// `dbpvxr <recordName>` — print pvalink debug info for the named
/// record. Mirrors pvxs `dbpvxr` (pvalink.cpp:185). Currently shows
/// resolver-level stats only; per-record link state would require
/// record-instance access we don't yet thread through here.
pub fn dbpvxr_command(resolver: PvaLinkResolver) -> CommandDef {
    CommandDef::new(
        "dbpvxr",
        vec![ArgDesc {
            name: "record",
            arg_type: ArgType::String,
            optional: true,
        }],
        "dbpvxr [<recordName>]",
        move |args: &[ArgValue], ctx: &CommandContext| {
            let target = match args.first() {
                Some(ArgValue::String(s)) if !s.is_empty() => Some(s.clone()),
                _ => None,
            };
            ctx.println(&format!(
                "dbpvxr: {} cached link(s), {} total reads",
                resolver.link_count(),
                resolver.read_count()
            ));
            if let Some(rec) = target {
                ctx.println(&format!(
                    "  (per-record dump for '{rec}' not yet implemented; see pvxs `dbpvxr` for the full output)"
                ));
            }
            Ok(CommandOutcome::Continue)
        },
    )
}

/// Convenience: build the full pvalink iocsh command set bound to
/// `resolver`. Drop the result into [`epics_base_rs::server::ioc_app::IocRunConfig::shell_commands`].
pub fn register_pvalink_commands(resolver: PvaLinkResolver) -> Vec<CommandDef> {
    vec![
        db_pvxr_command(resolver.clone()),
        pvxrefdiff_command(resolver.clone()),
        dbpvxr_command(resolver),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_resolver() -> PvaLinkResolver {
        PvaLinkResolver::new(tokio::runtime::Handle::current())
    }

    #[tokio::test]
    async fn register_pvalink_commands_returns_three() {
        let r = dummy_resolver();
        let cmds = register_pvalink_commands(r);
        assert_eq!(cmds.len(), 3);
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"pvxr"));
        assert!(names.contains(&"pvxrefdiff"));
        assert!(names.contains(&"dbpvxr"));
    }
}
