# chunk_04 — 1 APPLIES candidate / 29 N/A

## 68cc69b — APPLIES (UNVERIFIED) — client: propagate exception during early op. setup
**pva-rs target**: client_native/channel.rs, context.rs
**Fix**: Wrap setup in try-catch (Result), capture exceptions during Channel::build(), propagate to operation callbacks/futures.

29 others: N/A (version bumps, NTTable helpers, threadOnce refactor, IOC test prepare, IOC group source, owned_ptr update, error message tweaks, port logging, doc, NTTable feature, IOC trigger warning, $PVXS_ENABLE_IPV6 env var, IOC display.precision, log colorize, version_information consolidate, gcc 12 -Wnoexcept, protoTCP init, long string detection, DBEntry::info, doc tree formatting, misc, namespace reservations, copyable, StructTop optimization, SockAttach refactor).
