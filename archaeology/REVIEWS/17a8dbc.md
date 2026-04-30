---
short_sha: 17a8dbc
status: not-applicable
files_changed: []
---
The C bug was `dbDbLink.c::dbDbGetValue` skipping `dbChannelRunPreChain` / `dbChannelRunPostChain` (channel filter execution) on the array/general read path — only the scalar fast-path checked filters — so DB links to channels with `{arr: {s:5, e:10}}`, deadband, timestamp, etc. silently bypassed user-configured filters. In `epics-base-rs` the channel-filter subsystem (`db/filters/`, `dbChannel`, pre/post chain machinery) is not ported. DB-link reads go through `links.rs::read_link_value` → `PvDatabase::get_pv` → record `get_field` with no filter pipeline. There are no `Channel::filters()` to consult, no `dbChannelRunPreChain` / `dbChannelRunPostChain` Rust equivalents, and no INP/OUT filter-spec parsing on the link side. Filtering at the Rust layer is done at the source (e.g., per-record DEAD/MDEL fields) rather than at the link-read surface. Structurally absent: no filter-bypass path because there is no filter chain to bypass.
