---
short_sha: e0dfb6c
status: not-applicable
files_changed: []
---
Same structural absence as c51c83b: there is no `db_db_get_value` function and no `FieldLog`/`db_field_log` type in base-rs. The C bug — heap-allocated `db_field_log` reused/freed by re-entrant PINI callbacks — depends on the dbDbLink filter-chain architecture (`dbChannelRunPreChain` / `RunPostChain` operating on a heap log pointer that the chain itself can swap). base-rs `links.rs::read_link_value` is a direct lookup with no chain, no log, and no PINI re-entrancy hazard. Nothing to fix.
