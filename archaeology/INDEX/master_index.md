# epics-base Archaeology — Master Index
Generated: 2026-04-30 | Total: 365 commits | Applies: 127 | Partial: 44 | Eliminated: 194

## Summary by Category
| Category | Total | Applies | Partial | Eliminated |
|----------|-------|---------|---------|------------|
| bounds | 49 | 18 | 7 | 24 |
| flow-control | 10 | 7 | 2 | 1 |
| leak | 17 | 2 | 0 | 15 |
| lifecycle | 99 | 47 | 17 | 35 |
| network-routing | 31 | 10 | 4 | 17 |
| other | 40 | 1 | 6 | 33 |
| race | 47 | 13 | 3 | 31 |
| timeout | 12 | 3 | 1 | 8 |
| type-system | 42 | 12 | 3 | 27 |
| wire-protocol | 18 | 14 | 1 | 3 |

## Summary by Crate (audit_targets)
| Crate | Applies+Partial count |
|-------|----------------------|
| base-rs | 179 |
| ca-rs | 42 |
| pva-rs | 3 |

## High-Priority Audit Targets (applies, high severity)

- `0a1fb25` | bounds | base-rs/src/server/database/db_ca.rs | dbCaGetLink fails with alarm when reading scalar from empty CA-linked array
- `12cfd41` | bounds | base-rs/src/server/database/db_access.rs | dbPut raises LINK/INVALID alarm when writing empty array into scalar field
- `2340c6e` | bounds | base-rs/src/server/database/record_support.rs | Array records: move bptr assignment from cvt_dbaddr to get_array_info
- `2bcaa54` | bounds | ca-rs/src/client/udp.rs | CA UDP: memcpy with non-null extsize but null pExt pointer — null dereference
- `3176651` | bounds | base-rs/src/server/database/db_access.rs | dbGet: Return error when reading scalar from empty array
- `3627c38` | bounds | base-rs/src/server/database/links.rs | Crash when filter result reduces array to 0 elements in dbDbGetValue
- `39c8d56` | bounds | base-rs/src/server/database/db_access.rs | dbGet crashes on empty array: missing element-count guard before filter
- `552b2d1` | bounds | base-rs/src/server/database/const_link.rs | dbConstAddLink: missing bounds check on dbrType before table lookup
- `60fa2d3` | bounds | base-rs/src/calc/postfix.rs | Null pointer dereference in postfix() on empty operator stack
- `6b5abf7` | bounds | base-rs/src/server/database/db_db_link.rs | dbDbLink: remove early error return that blocked empty array reads
- `87acb98` | bounds | ca-rs/src/client/addr_list.rs | CA hostname length limit overflow when parsing EPICS_CA_ADDR_LIST
- `8c9e42d` | bounds | base-rs/src/server/iocsh/registry.rs | Numeric overflow in epicsStrnRawFromEscaped octal/hex escape parsing
- `446e0d4` | flow-control | base-rs/src/server/database/filters/dbnd.rs | dbnd filter: pass through DBE_ALARM and DBE_PROPERTY events unconditionally
- `39b0301` | leak | base-rs/src/server/database/db_static_lib.rs | Record deletion leaks all link field allocations (dbDeleteRecord)
- `0a6b9e4` | lifecycle | base-rs/src/server/database/scan.rs | scanStop() Before scanStart() Causes Crash or Hang
- `16c3202` | lifecycle | base-rs/src/server/database/waveform_record.rs | waveform: PACT=TRUE Lost, Causes Double-Processing on Async Completion
- `27fe3e4` | lifecycle | base-rs/src/server/database/db_field_log.rs | db_field_log: eliminate dbfl_type_rec, unify live-record reference into dbfl_type_ref
- `29fa062` | lifecycle | base-rs/src/log/errlog.rs | errlog: rewrite with double-buffering to avoid holding lock during print
- `3124d97` | lifecycle | base-rs/src/server/database/db_lex_routines.rs | Fix crash in popFirstTemp() when temp list is empty on bad record name
- `3f382f6` | lifecycle | base-rs/src/server/database/ca_link.rs | Revert: dbCa iocInit wait for local CA links to connect
- `56f05d7` | lifecycle | base-rs/src/server/database/db_access.rs | dbGet: wrong condition for using db_field_log vs. live record data
- `62c11c2` | lifecycle | base-rs/src/server/database/dbDbLink.rs | dbDbLink processTarget: self-link must not set RPRO (infinite reprocess loop)
- `717d69e` | lifecycle | ca-rs/src/client/db_ca.rs | dbCa: iocInit must wait for local CA links to connect before PINI
- `7709239` | lifecycle | base-rs/src/server/database/db_access.rs | Null guard for put_array_info function pointer before calling in dbPut
- `85822f3` | lifecycle | base-rs/src/server/database/db_access.rs | db_field_log: missing abstraction for data-ownership check enables scan-lock races
- `8a0fc03` | lifecycle | base-rs/src/server/database/db_access.rs | dbPutFieldLink: propagate dbChannelOpen() error status correctly
- `a46bd5a` | lifecycle | base-rs/src/server/database/ca_link.rs | dbCa: iocInit wait for local CA links to connect (later reverted)
- `a74789d` | lifecycle | base-rs/src/server/database/filters/decimate.rs | Decimate and Sync Filters Incorrectly Drop DBE_PROPERTY Monitor Events
- `ac6eb5e` | lifecycle | base-rs/src/server/database/callback.rs | callbackRequest: No Guard Against Uninitialized Callback Queue
- `b34aa59` | lifecycle | base-rs/src/server/database/db_lex_routines.rs | Null guard cascade for popFirstTemp() return in DB parser
- `b35064d` | lifecycle | base-rs/src/server/database/db_event.rs | dbEvent: Revert join, Implement Safe Exit Semaphore Shutdown Protocol
- `bac8851` | lifecycle | base-rs/src/server/database/as_ca.rs | Revert asCaStop() thread join to avoid deadlock on shutdown
- `c51c83b` | lifecycle | base-rs/src/server/database/links.rs | Revert stack-allocated field-log fix: heap alloc required for PINI safety
- `ca2ea14` | lifecycle | base-rs/src/server/database/db_event.rs | dbEvent: Worker Thread Must Be Joined on Close
- `e0dfb6c` | lifecycle | base-rs/src/server/database/links.rs | PINI crash: use stack-local field-log to avoid heap UAF in filter chain
- `e860617` | lifecycle | base-rs/src/server/database/dbDbLink.rs | dbDbLink processTarget: add procThread ownership to fix RPRO/PUTF regression
- `f4be9da` | lifecycle | base-rs/src/server/database/callback.rs | Null callback function pointer crash in callbackRequest
- `fab8fd7` | lifecycle | base-rs/src/server/database/db_event.rs | dbEvent: handle multiple db_event_cancel() calls safely
- `51191e6` | network-routing | ca-rs/src/client/udp.rs | Linux IP_MULTICAST_ALL Default Causes Unintended Multicast Reception
- `530eba1` | network-routing | ca-rs/src/server/client.rs | rsrv: use verified client IP address instead of client-supplied hostname
- `772c10d` | network-routing | ca-rs/src/server/rsrv.rs | RSRV_SERVER_PORT Truncated for Port Numbers Above 9999
- `97bf917` | network-routing | ca-rs/src/client/repeater.rs | caRepeater does not join multicast groups — misses multicast CA beacons
- `271f20f` | race | base-rs/src/server/database/event.rs | dbEvent: expand synchronization — fix busy-wait and labor-pending race
- `71e4635` | race | ca-rs/src/client/db_ca.rs | testdbCaWaitForEvent: race between event destroy and CA context flush
- `7a6e11c` | race | ca-rs/src/server/stats.rs | RSRV: guard casStatsFetch and casClientInitiatingCurrentThread against uninitialized state
- `8735a7b` | race | base-rs/src/server/database/db_ca.rs | dbCa: Acquire dbScanLock around db_process() in CA link task
- `89f0f13` | race | base-rs/src/server/database/callback.rs | Callback subsystem uses non-atomic state flag causing data races on init/stop/cleanup
- `9f78899` | race | base-rs/src/server/database/channel.rs | db: acquire record lock before db_create_read_log and dbChannelGetField
- `9f868a1` | race | base-rs/src/server/database/db_event.rs | Concurrent db_cancel_event causes hang via shared flush semaphore
- `a4bc0db` | race | base-rs/src/server/database/db_ca.rs | dbCa: CP link updates must set PUTF/RPRO via dbCaTask, not scanOnce callback
- `a864f16` | race | ca-rs/src/client/link.rs | dbCa Test Sync Race: Missing Refcount and Wrong Lock Release Order
- `dac620a` | race | base-rs/src/server/database/db_link.rs | dbGet infinite recursion when input link points back to same field
- `e9e576f` | race | base-rs/src/server/database/db_ca.rs | Fix dbCaSync() and add testdbCaWaitForUpdateCount()
- `7b6e48f` | timeout | ca-rs/src/bin/casw.rs | casw uses monotonic clock for beacon timestamps — wrong clock domain
- `f1cbe93` | timeout | ca-rs/src/client/search_timer.rs | Revert getMonotonic() → getCurrent() in CA timers and timer queue
- `3091f7c` | type-system | base-rs/src/server/database/int64in_record.rs | int64in: Monitor Delta Comparison Truncated to 32 Bits
- `6c914d1` | type-system | base-rs/src/server/database/db_access.rs | Validate dbrType before indexing conversion table to prevent OOB access
- `b6fffc2` | type-system | base-rs/src/server/database/db_convert.rs | String-to-epicsUInt32 conversion uses ULONG_MAX bound instead of UINT_MAX
- `b833f12` | type-system | base-rs/src/util/stdlib.rs | epicsStrtod: use strtoll/strtoull for hex parsing on 32-bit architectures
- `b94afaa` | type-system | base-rs/src/server/database/db_access.rs | UTAG field widened from epicsInt32 to epicsUInt64
- `c5012d9` | type-system | ca-rs/src/client/com_buf.rs | Make sure epicsInt8 is signed on all architectures
- `f6e8a75` | type-system | base-rs/src/server/database/db_link.rs | DB link reads DBF_MENU field as DBF_ENUM due to wrong type query
- `8cc2039` | wire-protocol | ca-rs/src/client/codec.rs | Fix dbr_size_n macro: COUNT==0 must yield base size, not zero
- `b1d9c57` | wire-protocol | base-rs/src/server/database/db_event.rs | db_field_log::mask overwritten with actual event mask on post

## All Entries (applies + partial only)

| short_sha | date | category | severity | rust_verdict | title |
|-----------|------|----------|----------|--------------|-------|
| 0a1fb25 | 2020-06-29 | bounds | high | applies | dbCaGetLink fails with alarm when reading scalar from empty CA-linked array |
| 12cfd41 | 2020-07-06 | bounds | high | applies | dbPut raises LINK/INVALID alarm when writing empty array into scalar field |
| 2340c6e | 2021-02-25 | bounds | high | applies | Array records: move bptr assignment from cvt_dbaddr to get_array_info |
| 2bcaa54 | 2020-02-12 | bounds | high | applies | CA UDP: memcpy with non-null extsize but null pExt pointer — null dereference |
| 3176651 | 2020-06-09 | bounds | high | applies | dbGet: Return error when reading scalar from empty array |
| 3627c38 | 2020-02-12 | bounds | high | applies | Crash when filter result reduces array to 0 elements in dbDbGetValue |
| 39c8d56 | 2020-02-13 | bounds | high | applies | dbGet crashes on empty array: missing element-count guard before filter |
| 552b2d1 | 2021-02-19 | bounds | high | applies | dbConstAddLink: missing bounds check on dbrType before table lookup |
| 60fa2d3 | 2023-07-18 | bounds | high | applies | Null pointer dereference in postfix() on empty operator stack |
| 6b5abf7 | 2020-06-01 | bounds | high | applies | dbDbLink: remove early error return that blocked empty array reads |
| 87acb98 | 2022-08-20 | bounds | high | applies | CA hostname length limit overflow when parsing EPICS_CA_ADDR_LIST |
| 8c9e42d | 2020-07-28 | bounds | high | applies | Numeric overflow in epicsStrnRawFromEscaped octal/hex escape parsing |
| beec00b | 2024-03-14 | bounds | high | partial | Compress Record N-to-M Array Compression Bounds Error with Partial Buffer |
| 446e0d4 | 2021-10-03 | flow-control | high | applies | dbnd filter: pass through DBE_ALARM and DBE_PROPERTY events unconditionally |
| 6ffc9e1 | 2019-09-17 | flow-control | high | partial | logClient flush discards messages already in OS send queue |
| 39b0301 | 2024-06-18 | leak | high | applies | Record deletion leaks all link field allocations (dbDeleteRecord) |
| 0a6b9e4 | 2024-06-14 | lifecycle | high | applies | scanStop() Before scanStart() Causes Crash or Hang |
| 16c3202 | 2021-07-21 | lifecycle | high | applies | waveform: PACT=TRUE Lost, Causes Double-Processing on Async Completion |
| 27fe3e4 | 2020-03-30 | lifecycle | high | applies | db_field_log: eliminate dbfl_type_rec, unify live-record reference into dbfl_type_ref |
| 29fa062 | 2021-02-19 | lifecycle | high | applies | errlog: rewrite with double-buffering to avoid holding lock during print |
| 3124d97 | 2020-06-10 | lifecycle | high | applies | Fix crash in popFirstTemp() when temp list is empty on bad record name |
| 3f382f6 | 2025-10-17 | lifecycle | high | applies | Revert: dbCa iocInit wait for local CA links to connect |
| 56f05d7 | 2021-01-14 | lifecycle | high | applies | dbGet: wrong condition for using db_field_log vs. live record data |
| 62c11c2 | 2019-02-02 | lifecycle | high | applies | dbDbLink processTarget: self-link must not set RPRO (infinite reprocess loop) |
| 717d69e | 2025-09-20 | lifecycle | high | applies | dbCa: iocInit must wait for local CA links to connect before PINI |
| 7709239 | 2020-07-17 | lifecycle | high | applies | Null guard for put_array_info function pointer before calling in dbPut |
| 85822f3 | 2020-04-01 | lifecycle | high | applies | db_field_log: missing abstraction for data-ownership check enables scan-lock races |
| 8a0fc03 | 2021-11-03 | lifecycle | high | applies | dbPutFieldLink: propagate dbChannelOpen() error status correctly |
| a46bd5a | 2025-09-20 | lifecycle | high | applies | dbCa: iocInit wait for local CA links to connect (later reverted) |
| a74789d | 2023-05-03 | lifecycle | high | applies | Decimate and Sync Filters Incorrectly Drop DBE_PROPERTY Monitor Events |
| ac6eb5e | 2021-06-20 | lifecycle | high | applies | callbackRequest: No Guard Against Uninitialized Callback Queue |
| b34aa59 | 2020-06-10 | lifecycle | high | applies | Null guard cascade for popFirstTemp() return in DB parser |
| b35064d | 2019-06-23 | lifecycle | high | applies | dbEvent: Revert join, Implement Safe Exit Semaphore Shutdown Protocol |
| bac8851 | 2020-03-23 | lifecycle | high | applies | Revert asCaStop() thread join to avoid deadlock on shutdown |
| c51c83b | 2020-02-25 | lifecycle | high | applies | Revert stack-allocated field-log fix: heap alloc required for PINI safety |
| ca2ea14 | 2021-04-02 | lifecycle | high | applies | dbEvent: Worker Thread Must Be Joined on Close |
| e0dfb6c | 2020-02-13 | lifecycle | high | applies | PINI crash: use stack-local field-log to avoid heap UAF in filter chain |
| e860617 | 2019-01-27 | lifecycle | high | applies | dbDbLink processTarget: add procThread ownership to fix RPRO/PUTF regression |
| f4be9da | 2023-11-03 | lifecycle | high | applies | Null callback function pointer crash in callbackRequest |
| fab8fd7 | 2023-09-14 | lifecycle | high | applies | dbEvent: handle multiple db_event_cancel() calls safely |
| 2ff44cb | 2022-07-30 | lifecycle | high | partial | callback.c: join callback threads on callbackStop() |
| 49fddaa | 2022-11-15 | lifecycle | high | partial | errlogRemoveListeners: self-removal during callback causes use-after-free |
| 8a30200 | 2022-06-15 | lifecycle | high | partial | ts filter: replace cantProceed with non-fatal error handling |
| bded79f | 2022-07-30 | lifecycle | high | partial | dbScan: join periodic and once-scan threads on scanStop() |
| f430389 | 2022-07-30 | lifecycle | high | partial | iocShutdown: always stop worker threads, not only in isolated mode |
| 51191e6 | 2021-08-04 | network-routing | high | applies | Linux IP_MULTICAST_ALL Default Causes Unintended Multicast Reception |
| 530eba1 | 2018-06-16 | network-routing | high | applies | rsrv: use verified client IP address instead of client-supplied hostname |
| 772c10d | 2024-06-14 | network-routing | high | applies | RSRV_SERVER_PORT Truncated for Port Numbers Above 9999 |
| 97bf917 | 2020-02-12 | network-routing | high | applies | caRepeater does not join multicast groups — misses multicast CA beacons |
| 410921b | 2021-01-07 | network-routing | high | partial | Network interface enumeration: replace SIOCGIFCONF with getifaddrs |
| 271f20f | 2025-08-27 | race | high | applies | dbEvent: expand synchronization — fix busy-wait and labor-pending race |
| 71e4635 | 2025-10-17 | race | high | applies | testdbCaWaitForEvent: race between event destroy and CA context flush |
| 7a6e11c | 2025-02-06 | race | high | applies | RSRV: guard casStatsFetch and casClientInitiatingCurrentThread against uninitialized state |
| 8735a7b | 2025-06-16 | race | high | applies | dbCa: Acquire dbScanLock around db_process() in CA link task |
| 89f0f13 | 2017-11-08 | race | high | applies | Callback subsystem uses non-atomic state flag causing data races on init/stop/cleanup |
| 9f78899 | 2023-02-23 | race | high | applies | db: acquire record lock before db_create_read_log and dbChannelGetField |
| 9f868a1 | 2023-10-23 | race | high | applies | Concurrent db_cancel_event causes hang via shared flush semaphore |
| a4bc0db | 2024-12-27 | race | high | applies | dbCa: CP link updates must set PUTF/RPRO via dbCaTask, not scanOnce callback |
| a864f16 | 2024-06-11 | race | high | applies | dbCa Test Sync Race: Missing Refcount and Wrong Lock Release Order |
| dac620a | 2024-11-29 | race | high | applies | dbGet infinite recursion when input link points back to same field |
| e9e576f | 2021-11-02 | race | high | applies | Fix dbCaSync() and add testdbCaWaitForUpdateCount() |
| e1c1bb8 | 2023-01-22 | race | high | partial | dbEvent: correct eventsRemaining count — skip canceled events |
| 7b6e48f | 2020-02-11 | timeout | high | applies | casw uses monotonic clock for beacon timestamps — wrong clock domain |
| f1cbe93 | 2020-04-23 | timeout | high | applies | Revert getMonotonic() → getCurrent() in CA timers and timer queue |
| 3091f7c | 2021-07-29 | type-system | high | applies | int64in: Monitor Delta Comparison Truncated to 32 Bits |
| 6c914d1 | 2020-06-01 | type-system | high | applies | Validate dbrType before indexing conversion table to prevent OOB access |
| b6fffc2 | 2024-08-12 | type-system | high | applies | String-to-epicsUInt32 conversion uses ULONG_MAX bound instead of UINT_MAX |
| b833f12 | 2025-04-04 | type-system | high | applies | epicsStrtod: use strtoll/strtoull for hex parsing on 32-bit architectures |
| b94afaa | 2020-12-02 | type-system | high | applies | UTAG field widened from epicsInt32 to epicsUInt64 |
| c5012d9 | 2021-12-17 | type-system | high | applies | Make sure epicsInt8 is signed on all architectures |
| f6e8a75 | 2021-08-12 | type-system | high | applies | DB link reads DBF_MENU field as DBF_ENUM due to wrong type query |
| 8cc2039 | 2020-06-05 | wire-protocol | high | applies | Fix dbr_size_n macro: COUNT==0 must yield base size, not zero |
| b1d9c57 | 2021-10-03 | wire-protocol | high | applies | db_field_log::mask overwritten with actual event mask on post |
| 4a0f488 | 2021-02-25 | bounds | medium | applies | histogramRecord wdog callback uses bptr instead of VAL field for db_post_events |
| 5d808b7 | 2020-05-07 | bounds | medium | applies | Introduce distinct error code for zero-element array reads |
| 6e7a715 | 2022-08-16 | bounds | medium | applies | Getting .DTYP from rectype with no devSup returns empty string instead of crash |
| 979dde8 | 2024-06-20 | bounds | medium | applies | get_enum_strs uses pointer arithmetic that trips _FORTIFY_SOURCES=3 |
| e5b4829 | 2024-05-19 | bounds | medium | applies | lsi/lso SIZV Uncapped at 32767: Signed dbAddr::field_size Overflow |
| 11a4bed | 2022-05-11 | bounds | medium | partial | compressRecord: compress_scalar average computation is incorrect |
| 84f4771 | 2022-05-11 | bounds | medium | partial | compressRecord: compress_array rejects valid partial input when PBUF=YES |
| baa4cb5 | 2025-09-30 | bounds | medium | partial | callbackSetQueueSize: reject non-positive queue size before iocInit |
| ec650e8 | 2022-07-26 | bounds | medium | partial | dbPutConvertJSON: empty JSON string not handled, passed to yajl causing parse error |
| 0a3427c | 2019-08-28 | flow-control | medium | applies | logClient: Don't Discard Unsent Buffer on Disconnect |
| 17a8dbc | 2020-02-12 | flow-control | medium | applies | Filters not applied when reading via DB link (dbDbGetValue) |
| 4df48c9 | 2022-06-27 | flow-control | medium | applies | dbEvent queue accumulates duplicate reference-type events instead of compacting them |
| 556de06 | 2026-02-06 | flow-control | medium | applies | epicsThreadGetCPUs overreports CPUs when affinity mask is restricted |
| 8ac2c87 | 2025-01-07 | flow-control | medium | applies | compressRecord: post monitor event when reset via RES field |
| b1f4459 | 2020-02-11 | flow-control | medium | applies | DB links stored DBADDR instead of dbChannel, bypassing filter metadata |
| b6626e4 | 2023-01-22 | flow-control | medium | partial | dbEvent: detect possible queue stall when eventsRemaining is set |
| 4e4e55c | 2024-06-19 | leak | medium | applies | dbDeleteRecordLinks only freed plink->text, skipping full link contents cleanup |
| 08b741e | 2021-04-19 | lifecycle | medium | applies | CA Repeater: Fallback to In-Process Thread When exec Fails |
| 0f75e0a | 2019-03-13 | lifecycle | medium | applies | dbDbLink processTarget: replace assert() with errlogPrintf for procThread mismatches |
| 13d6ca5 | 2025-02-05 | lifecycle | medium | applies | initHookRegister: make idempotent and use mallocMustSucceed |
| 1d85bc7 | 2021-03-10 | lifecycle | medium | applies | longout special() sets link-changed flag before OUT link is updated |
| 23d9176 | 2018-10-26 | lifecycle | medium | applies | aai/waveform record cleanup: nord initialization and waveform returns readValue status |
| 3fb10b6 | 2018-12-29 | lifecycle | medium | applies | dbNotify must set PUTF on the first-record call only |
| 4737901 | 2020-02-13 | lifecycle | medium | applies | devAiSoft read_ai returns error status on device read failure |
| 51c5b8f | 2023-03-09 | lifecycle | medium | applies | subArray process: missing NORD db_post_events when element count changes |
| 5b37663 | 2020-08-06 | lifecycle | medium | applies | aToIPAddr crashes on NULL input string |
| 5d1f572 | 2023-03-08 | lifecycle | medium | applies | Remove NORD db_post_events from aai and waveform device support layers |
| 62c3b0a | 2019-08-27 | lifecycle | medium | applies | iocLog: errlog Listener Registered on Wrong Object (All Clients) |
| 64011ba | 2023-03-09 | lifecycle | medium | applies | Remove duplicate NORD db_post_events from subArray device support |
| 6c573b4 | 2021-03-10 | lifecycle | medium | applies | longout with OOPT=On Change skips output write on first process |
| 8c08c57 | 2023-03-08 | lifecycle | medium | applies | errSymbolAdd fails if called before errSymBld (init ordering bug) |
| 8e7d3e9 | 2021-06-30 | lifecycle | medium | applies | initHookName: Shutdown States Missing from Name Table |
| 8fdaa13 | 2021-02-22 | lifecycle | medium | applies | errlog: restore errlogFlush() call in eltc() |
| aff7463 | 2023-03-08 | lifecycle | medium | applies | aai and aao process: add NORD db_post_events when element count changes |
| d0cf47c | 2024-11-19 | lifecycle | medium | applies | AMSG alarm message not propagated through MSS links |
| dabcf89 | 2021-10-03 | lifecycle | medium | applies | mbboDirect: fix init priority — B0-B1F bits override VAL when VAL is UDF |
| f1e83b2 | 2017-02-18 | lifecycle | medium | applies | Timestamp updated after outlinks: downstream TSEL reads stale timestamp |
| f57acd2 | 2021-11-05 | lifecycle | medium | applies | Add testdbCaWaitForConnect() for CA link connection synchronization |
| 1c566e2 | 2021-02-27 | lifecycle | medium | partial | aai record: allow device support to defer init_record to pass 1 |
| 280aa0b | 2025-10-08 | lifecycle | medium | partial | Initialize errSymTable before database errors can occur in dbReadCOM |
| 5d5e552 | 2019-11-14 | lifecycle | medium | partial | Add de-init hook announcements to iocShutdown sequence |
| 6dba2ec | 2020-02-13 | lifecycle | medium | partial | caRepeater inherits parent stdin/out/err — causes problems when spawned by caget |
| 7448a8b | 2022-11-14 | lifecycle | medium | partial | errlog worker exits loop before draining buffer at shutdown |
| 832abbd | 2022-12-20 | lifecycle | medium | partial | subRecord: propagate error from bad INP links instead of silently succeeding |
| 9df98c1 | 2019-08-28 | lifecycle | medium | partial | logClient pending messages not flushed immediately after reconnect |
| e11f880 | 2022-10-18 | lifecycle | medium | partial | ts Filter Uses Stale db_field_log API — dtor Field Moved Out of Union |
| eeb198d | 2020-03-30 | lifecycle | medium | partial | arrRecord: Move pfield assignment from cvt_dbaddr to get_array_info |
| 19146a5 | 2020-06-19 | network-routing | medium | applies | WIN32: Disable SO_REUSEADDR for Windows sockets |
| 5064931 | 2020-02-05 | network-routing | medium | applies | Datagram fanout socket: must set both SO_REUSEPORT and SO_REUSEADDR on Linux |
| 65ef6e9 | 2020-01-12 | network-routing | medium | applies | POSIX datagram fanout: SO_REUSEADDR insufficient on BSD — need SO_REUSEPORT |
| 951b6ac | 2020-08-03 | network-routing | medium | applies | Cygwin missing TCP_NODELAY declaration causes CA build failure |
| c23012d | 2018-01-30 | network-routing | medium | applies | CA server (rsrv) suppresses repeated beacon UDP send error messages |
| cae597d | 2018-11-14 | network-routing | medium | applies | CA client suppresses repeated UDP send error messages per destination |
| 932e9f3 | 2019-06-04 | network-routing | medium | partial | asLib: soft-fail DNS lookup, store "unresolved:<host>" instead of aborting |
| c9b6709 | 2019-09-18 | network-routing | medium | partial | logClient zero-byte send to detect broken TCP connections |
| 4c20518 | 2024-02-26 | other | medium | applies | recGblRecordError Skips Error Symbol Lookup for Negative Status Codes |
| 3dbc9ea | 2023-02-01 | other | medium | partial | iocsh argument splitter: EOF sentinel (-1) misread as valid char |
| d47fa4c | 2022-08-08 | other | medium | partial | aSub record: dbGetLink called on constant input links causing error |
| 5aca4c6 | 2023-09-13 | race | medium | applies | dbEvent: clear callBackInProgress before signaling pflush_sem |
| 5ba8080 | 2022-05-13 | race | medium | applies | Waveform NORD posted before timestamp update causes undefined timestamp on first CA monitor update |
| 059d32a | 2023-05-25 | race | medium | partial | dbChannel Type Probe Struct Has Uninitialized Members |
| 333446e | 2025-06-16 | race | medium | partial | dbDbLink: Assert lockset ownership before dbPutLink |
| 4966baf | 2024-05-19 | type-system | medium | applies | SIZV Field Uncapped at 32767: Signed field_size Overflow |
| 5485ada | 2022-04-15 | type-system | medium | applies | Make epicsNAN and epicsINF truly const on all platforms |
| b36e526 | 2020-08-21 | type-system | medium | applies | Const link string init fails for DBF_CHAR waveform fields |
| e88a186 | 2023-11-24 | type-system | medium | applies | Signed bit field UB in struct link::flags |
| b460c26 | 2022-11-01 | type-system | medium | partial | Menu field conversion returns error for out-of-range enum index instead of numeric string |
| 235f8ed | 2020-04-20 | wire-protocol | medium | applies | db_field_log missing DBE_* mask prevents filter from distinguishing DBE_PROPERTY |
| 3b3261c | 2020-05-22 | wire-protocol | medium | applies | Revert S_db_emptyArray — empty array must return S_db_badField for compatibility |
| 82ec539 | 2021-08-08 | wire-protocol | medium | applies | dbPut: long-string (nRequest>1) skips get_array_info, corrupts write path |
| 88bfd6f | 2025-11-05 | wire-protocol | medium | applies | dbConvert: allow hex/octal string-to-integer conversion in dbPut/dbGet |
| 9e7cd24 | 2024-09-02 | wire-protocol | medium | applies | DBE_PROPERTY events missing for mbbi/mbbo when val != changed string index |
| a42197f | 2020-06-08 | wire-protocol | medium | applies | CA client: Allow writing zero-element arrays via caput |
| b7cc33c | 2024-09-02 | wire-protocol | medium | applies | DBE_PROPERTY event posted after DBE_VALUE instead of before |
| cd0e6a4 | 2021-02-05 | wire-protocol | medium | applies | caProto.h uses IPPORT_USERRESERVED without including its definition |
| d763541 | 2025-10-08 | wire-protocol | medium | applies | CA client: expose server protocol minor version via ca_host_minor_protocol() |
| f2fe9d1 | 2023-11-02 | wire-protocol | medium | applies | bi "Raw Soft Channel" did not apply MASK to RVAL |
| faac1df | 2024-08-30 | wire-protocol | medium | applies | Spurious DBE_PROPERTY events posted even when property field value unchanged |
| 275c4c7 | 2020-05-07 | bounds | low | applies | Wrong pointer deref in empty-array guard in dbGet |
| 4ab9808 | 2020-03-30 | bounds | low | partial | arr filter: wrapArrayIndices early-return clarifies empty-slice path |
| 8488c9e | 2023-09-03 | bounds | low | partial | initHookName() Missing Compile-Time Array Length Consistency Check |
| 8483ff9 | 2024-11-14 | lifecycle | low | applies | NAMSG not cleared after alarm promoted to AMSG, leaving stale message |
| bc7ee94 | 2019-01-03 | lifecycle | low | applies | Remove spurious warning when PUTF is set on target with PACT false |
| 372e937 | 2021-01-14 | lifecycle | low | partial | dbGet: duplicated dbfl_type_val/ref dispatch replaced with dbfl_pfield macro |
| 550e902 | 2023-01-19 | lifecycle | low | partial | iocLogPrefix warns on identical re-set instead of accepting silently |
| acd1aef | 2025-10-08 | lifecycle | low | partial | Silent CP/CPP Modifier Discard on Output Links |
| 73cdea5 | 2019-05-08 | network-routing | low | partial | rsrv/asLib: rename asUseIP→asCheckClientIP, ignore client hostname when set |
| 144f975 | 2024-06-13 | other | low | partial | iocsh: propagate error codes from db/libcom commands via iocshSetError |
| 3b484f5 | 2023-03-06 | other | low | partial | dbConstLink: treat empty string same as unset link |
| 5c77c84 | 2025-07-31 | other | low | partial | Test Harness Cannot Detect NaN Equality; DBR Type IDs Not Human-Readable |
| a352865 | 2023-10-27 | other | low | partial | Print ANSI-colored error prefix to stderr in udpiiu and tools |
| e4a81bb | 2022-01-04 | timeout | low | applies | Document zero and NaN timeout semantics for CA and epicsEvent APIs |
| 1d056c6 | 2022-12-06 | timeout | low | partial | CA Command-Line Tools Ignore EPICS_CLI_TIMEOUT Environment Variable |
| 457387e | 2024-08-12 | type-system | low | applies | dbf_type_to_text macro signed comparison warning with unsigned type argument |
| 27918cb | 2021-02-04 | type-system | low | partial | dbPutString: insufficient error message for DBF_MENU/DEVICE invalid choice |
| 2c1c352 | 2021-02-05 | type-system | low | partial | DBF_MENU/DEVICE: missing "did you mean" suggestion on parse error |
| 8c99340 | 2019-05-09 | wire-protocol | low | applies | CA: clarify count=0 means variable-size array subscription |
| d1491e0 | 2020-07-17 | wire-protocol | low | partial | dbpf switches from whitespace-delimited to JSON array format for array puts |
