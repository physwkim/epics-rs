# pvxs Archaeology — Master Index

Total KEEP: 220 / 839 candidates  
Severity: HIGH=18, MEDIUM=81, LOW=121  
Verdict: applies=90, partial=10, uncertain=77, eliminated=43  
Actionable (non-eliminated): 177  

## Actionable Items

| # | SHA | Date | Sev | Verdict | Subject |
|---|-----|------|-----|---------|--------|
| 1 | 3b8540f52002 | 2023-07-19 | HIGH | applies | client: try to slow down reconnect loop |
| 2 | 4d12da87205e | 2023-05-11 | HIGH | applies | client: don't attempt to reconnect NS during shutdown |
| 3 | 5d3a21f03010 | 2021-04-17 | HIGH | applies | client Channel search bypass |
| 4 | 7d490dc69ece | 2020-02-21 | HIGH | applies | client info() error delivery |
| 5 | 8363c7fe9a5f | 2021-04-17 | HIGH | applies | client add TCP search |
| 6 | 86fa8c8cf6bf | 2020-10-19 | HIGH | applies | fix usage/example of Subscription::pop() |
| 7 | 92f728f5c9c4 | 2022-06-16 | HIGH | applies | Add hold-off timer when reconnecting to a specific server |
| 8 | a064677e3625 | 2021-01-26 | HIGH | applies | detect UDP RX buffer overflows |
| 9 | a3ffbd2a9b77 | 2020-03-02 | HIGH | applies | client fix Channel reconnect |
| 10 | acfba6469ed3 | 2020-02-22 | HIGH | applies | start client beacon rx |
| 11 | b17f8207676d | 2022-12-27 | HIGH | applies | sharedpv: avoid deadlock on error path |
| 12 | cce797263d1d | 2022-05-17 | HIGH | applies | fix handling of pva_ctrl_msg::SetEndian |
| 13 | cf91bc3033e2 | 2019-12-19 | HIGH | applies | fix array decode |
| 14 | e9ce80880d92 | 2021-01-12 | HIGH | applies | remote file:line from decode errors |
| 15 | f7b3821e10b4 | 2021-07-02 | HIGH | applies | client: consistent Channel disconnect handling |
| 16 | af3c870b7a16 | 2023-02-19 | HIGH | uncertain | Value::copyIn() add Array -> Array w/ implied alloc+convert |
| 17 | 0356eee74037 | 2021-01-12 | MEDIUM | applies | decode "null" string |
| 18 | 0de17036f4a6 | 2022-09-22 | MEDIUM | applies | add Context::close() |
| 19 | 0eea8fd1c7e0 | 2022-10-11 | MEDIUM | applies | fix CMD_MESSAGE handling |
| 20 | 280919b3ec08 | 2020-08-08 | MEDIUM | applies | server: adjust handling of invalid SID |
| 21 | 289f508af6fe | 2025-10-13 | MEDIUM | applies | server: plug channel leak |
| 22 | 2f4484889186 | 2025-01-31 | MEDIUM | applies | server: handle monitor created without initial ACK |
| 23 | 4af3028930c8 | 2025-09-20 | MEDIUM | applies | OperationBase::chan is nullptr until Channel is created, check before  |
| 24 | 5019744fa79c | 2020-02-21 | MEDIUM | applies | server GET_FIELD fix onLastDisconnect |
| 25 | 5301785233a6 | 2019-10-22 | MEDIUM | applies | drop sockaddr_storage |
| 26 | 64cf5c2334f8 | 2020-03-09 | MEDIUM | applies | drop SockAddr from public API |
| 27 | 6861f03c6075 | 2021-01-13 | MEDIUM | applies | increase TCP timeout to 40 seconds |
| 28 | 70735383350b | 2025-07-13 | MEDIUM | applies | fix remote error handling during PUT with autoExec=false |
| 29 | 772cc5297cf8 | 2020-02-18 | MEDIUM | applies | server fix spurious Beacon truncated |
| 30 | 7de1f7d32f63 | 2019-12-14 | MEDIUM | applies | server decode credentials |
| 31 | 84ef355a4a1a | 2021-01-26 | MEDIUM | applies | client: try not to fragment search packets |
| 32 | 882a7720fb92 | 2020-03-02 | MEDIUM | applies | more beacon wrong thread |
| 33 | 8d58409481ef | 2023-10-16 | MEDIUM | applies | server: check tx buffer limit to throttle |
| 34 | 8db40be29c81 | 2025-10-02 | MEDIUM | applies | client: log error for context with no search destinations |
| 35 | 91fed88cdd7f | 2025-02-20 | MEDIUM | applies | "Beacon tx error" show destination |
| 36 | 9b77c061b07c | 2024-04-22 | MEDIUM | applies | Timeout exception should say "Timeout" |
| 37 | 9d128b2f8aa6 | 2021-01-05 | MEDIUM | applies | more onInit() error handling |
| 38 | a6e7e9488d09 | 2020-07-28 | MEDIUM | applies | parse IPs with aToIPAddr() |
| 39 | adcac746efff | 2020-02-18 | MEDIUM | applies | server avoid verbose Beacon tx errors |
| 40 | b2b264ee9b13 | 2021-01-07 | MEDIUM | applies | client: fix monitor INIT error handling |
| 41 | b33ea5df3113 | 2020-03-07 | MEDIUM | applies | simplify beacon clean timer |
| 42 | b38b33db034e | 2021-01-26 | MEDIUM | applies | raise search reply processing limit |
| 43 | bab82affb84d | 2019-11-07 | MEDIUM | applies | redo packet build/parse |
| 44 | ca662bf6cc3c | 2019-12-14 | MEDIUM | applies | fixup data decode |
| 45 | cc5071cd22c4 | 2020-02-06 | MEDIUM | applies | fix server beacon tx |
| 46 | d7c19c0c5843 | 2020-02-25 | MEDIUM | applies | Value parse string -> scalar |
| 47 | da004bc54bb3 | 2021-01-14 | MEDIUM | applies | configurable timeout (with $EPICS_PVA_CONN_TMO) |
| 48 | e077e9663ccb | 2023-06-12 | MEDIUM | applies | missing 'throw' |
| 49 | f2e49a88445f | 2025-02-17 | MEDIUM | applies | pvalink: control parse warnings with logging |
| 50 | ff1d6510cbf8 | 2025-01-19 | MEDIUM | applies | reduce Search tx log spam |
| 51 | ff3c0e4da4c0 | 2020-03-09 | MEDIUM | applies | drop use of std::regex in pvRequest parsing |
| 52 | 027e590fbaaa | 2020-05-19 | MEDIUM | partial | improve type change error messages |
| 53 | 36dc71a158b8 | 2020-03-28 | MEDIUM | partial | MSVC missing includes |
| 54 | 4bd884719ef6 | 2024-03-29 | MEDIUM | partial | Workaround TCP_NODELAY error on winsock |
| 55 | 522434c1dd4f | 2023-07-03 | MEDIUM | partial | server: op->error() dispatch |
| 56 | 1663c0b775fb | 2020-02-25 | MEDIUM | uncertain | fix server ExecOp::error() |
| 57 | 1aa0f1a61100 | 2021-01-26 | MEDIUM | uncertain | incorrect deferred read |
| 58 | 274133bcfc63 | 2023-02-11 | MEDIUM | uncertain | ~fix magic union autoselect |
| 59 | 37f5391864e9 | 2020-12-24 | MEDIUM | uncertain | Value::as(T&) return false on transform error |
| 60 | 3a264e0d1299 | 2020-06-29 | MEDIUM | uncertain | Fix missing pointer dereference |
| 61 | 40bafaee0778 | 2021-02-12 | MEDIUM | uncertain | drop unused |
| 62 | 49c9d8205368 | 2025-02-09 | MEDIUM | uncertain | drop unused IfaceMap |
| 63 | 4a86694605b4 | 2021-04-18 | MEDIUM | uncertain | missing includes |
| 64 | 6020e28284a3 | 2023-03-24 | MEDIUM | uncertain | qsrv: drop qsrv executable |
| 65 | 60d68940fb83 | 2023-07-16 | MEDIUM | uncertain | missing header |
| 66 | 6828ea06c814 | 2025-10-01 | MEDIUM | uncertain | Revert "GetAddrInfo wrapper always numeric" |
| 67 | 6dba1d91f63f | 2026-01-28 | MEDIUM | uncertain | fix minor |
| 68 | 7e031a20ff8b | 2024-11-22 | MEDIUM | uncertain | ioc: fix dbLoadGroups command. |
| 69 | 87c5aabc2f72 | 2020-02-29 | MEDIUM | uncertain | server close connection on stop() |
| 70 | 9f9f03805568 | 2020-06-25 | MEDIUM | uncertain | allow Null Member |
| 71 | a6b3eb58bd42 | 2023-11-10 | MEDIUM | uncertain | add missing check on invalid Union[] selector |
| 72 | b47482e38a30 | 2021-02-20 | MEDIUM | uncertain | fix usage of recvmsg() |
| 73 | ba0974e1a54a | 2019-12-23 | MEDIUM | uncertain | drop unimplemented Value iteration |
| 74 | c2e5fdca551a | 2024-02-22 | MEDIUM | uncertain | client: avoid FD leak on failed connect() |
| 75 | c66c0fd1003e | 2022-01-29 | MEDIUM | uncertain | fix printf() spec |
| 76 | d10eefac0e4b | 2020-03-09 | MEDIUM | uncertain | drop unused FieldDesc::hash |
| 77 | d52272e148a4 | 2020-03-23 | MEDIUM | uncertain | fix EvInBuf::refill() |
| 78 | e36db5527c82 | 2020-07-24 | MEDIUM | uncertain | Server fail hard on invalid EPICS_PVAS_INTF_ADDR_LIST |
| 79 | e9ecf7e8dd13 | 2023-09-05 | MEDIUM | uncertain | missing copyright boilerplate |
| 80 | f260fa2774f6 | 2023-06-17 | MEDIUM | uncertain | fix shared_array output limit off by one |
| 81 | f44ff9754cc4 | 2019-10-21 | MEDIUM | uncertain | diagnose osx bind error |
| 82 | f4576d4c332c | 2020-07-21 | MEDIUM | uncertain | Include input string when reporting parsing error (as NoConvert) |
| 83 | fe6974025ab0 | 2022-03-25 | MEDIUM | uncertain | Add missing <limits> header |
| 84 | 021bcb4a0622 | 2025-07-13 | LOW | applies | server: fix Dead op cleanup |
| 85 | 07713faff4a6 | 2023-04-03 | LOW | applies | fix: schedule initial search use separate event from the generic work  |
| 86 | 0d5a3f62e1fc | 2022-10-12 | LOW | applies | client: fix locking of monitor members during pop() |
| 87 | 17464a117acc | 2023-11-07 | LOW | applies | disallow "null" size by default |
| 88 | 1f91eb9e5d3e | 2020-03-01 | LOW | applies | client fix sendDestroyRequest() |
| 89 | 21d9cb6b1ce6 | 2021-03-16 | LOW | applies | fix monitor queue locking |
| 90 | 2247c20bee44 | 2023-05-15 | LOW | applies | quiet MONITOR exceeds window size |
| 91 | 2ab08c115be4 | 2021-09-21 | LOW | applies | sharedpv: fix race w/ subscribers list |
| 92 | 344a96207f93 | 2022-02-20 | LOW | applies | Fix pvxsl: serv->getSource needs order argument |
| 93 | 3dd4dd6a669a | 2021-01-02 | LOW | applies | client: different onInit() for monitor |
| 94 | 48ca7b34c74e | 2020-04-16 | LOW | applies | fix VERSION_INT() order |
| 95 | 4ee7ce210841 | 2021-08-28 | LOW | applies | ignore beacons with proto!='tcp' |
| 96 | 4fac0672872b | 2020-08-08 | LOW | applies | client: monitor avoid extra wakeups |
| 97 | 525c711ee56c | 2026-02-11 | LOW | applies | server: reduce log spam from beacon tx |
| 98 | 57f9468c86ff | 2025-11-06 | LOW | applies | udp: clarify orig/reply addressing, fix mcast handling |
| 99 | 5897fe273e43 | 2023-04-14 | LOW | applies | fix intermittent of testsock |
| 100 | 5ddc2beb475f | 2020-01-26 | LOW | applies | server monitor throttle using send queue size |
| 101 | 722759416b49 | 2022-09-19 | LOW | applies | server: change monitor watermark meaning |
| 102 | 78273124f04d | 2020-02-22 | LOW | applies | more server beacon |
| 103 | 78f54455e673 | 2023-01-26 | LOW | applies | Value fix delta output format |
| 104 | 82adcb938d57 | 2020-01-29 | LOW | applies | server monitor pvRequest |
| 105 | 839fc01bfd1e | 2025-05-05 | LOW | applies | fix Source::Search::source() IPv6 representation |
| 106 | 8c55bf7de7c6 | 2020-08-06 | LOW | applies | SharedPV monitor discard empty updates |
| 107 | 92d519702f1a | 2024-10-01 | LOW | applies | client: search retry step reset on channel reconnection (fixes epics-b |
| 108 | 94b60d0ac0e2 | 2020-03-10 | LOW | applies | client monitor cleanup and logging |
| 109 | 94f0065a4d4d | 2021-06-05 | LOW | applies | fix beaconSenders locking |
| 110 | 9fefa95df1f7 | 2020-02-25 | LOW | applies | fix client PUT |
| 111 | a2b424cba204 | 2025-02-20 | LOW | applies | increase max UDP packet batch size |
| 112 | a36dd2a9cca7 | 2023-05-10 | LOW | applies | fix monitor pipeline and finish() |
| 113 | b0eecb949f38 | 2020-04-10 | LOW | applies | fixup client operation object lifetime |
| 114 | b8d204e35c88 | 2020-12-18 | LOW | applies | proto bug: client search requests incorrectly set Server direction |
| 115 | c32d1ae0e24b | 2020-04-17 | LOW | applies | fix pipeline w/ queueSize=1 |
| 116 | c373da671b51 | 2023-04-05 | LOW | applies | server: fix default monitor queueSize to 4 |
| 117 | cc5d38293042 | 2022-11-21 | LOW | applies | client: monitor yield "complete" updates |
| 118 | cfde0e26d85a | 2022-10-13 | LOW | applies | avoid assert for mostly absurdly long PV names |
| 119 | d15430fb17b4 | 2020-12-30 | LOW | applies | fix poke race |
| 120 | dd2f076b4aa6 | 2023-04-17 | LOW | applies | client: only advance search bucket during normal search |
| 121 | eb11d9e1bc8b | 2020-05-01 | LOW | applies | Fix registering functions with EPICS |
| 122 | f063bd26f58c | 2019-11-19 | LOW | applies | tcp search |
| 123 | fa25bf2aecac | 2021-04-16 | LOW | applies | server: fix TCP search reply |
| 124 | 1ed51c597c4f | 2022-10-16 | LOW | partial | avoid redundant atomic load |
| 125 | 6a46e44da971 | 2020-07-31 | LOW | partial | fix SharedPV onLastDisconnect when not open() |
| 126 | 785e180f9bce | 2020-12-29 | LOW | partial | evhelper: ensure work dtor before notify |
| 127 | 8ed998a89605 | 2021-09-21 | LOW | partial | sharedpv: fix race w/ current |
| 128 | aea4a4f80451 | 2019-12-15 | LOW | partial | use c++11 atomic |
| 129 | af973bea668d | 2020-02-21 | LOW | partial | harmonize signal handling |
| 130 | 01745aad727e | 2021-04-25 | LOW | uncertain | fix shared_array::back() and ::swap() |
| 131 | 056fb2c27b40 | 2020-03-07 | LOW | uncertain | fix iteration of sub-struct |
| 132 | 05f2b1864e03 | 2024-12-12 | LOW | uncertain | fix: use prepare cleanup hooks when epics-base>=7.0.8.0 (#67) |
| 133 | 0ac8092f13a7 | 2019-10-22 | LOW | uncertain | Revert "oops" |
| 134 | 0f90531615b9 | 2021-06-02 | LOW | uncertain | fix EPICS_PVAS_INTF_ADDR_LIST |
| 135 | 1220dc7d3e3c | 2020-01-19 | LOW | uncertain | fix Value::as(T&) and add Value::as(fn&&) |
| 136 | 30b040465a0d | 2023-02-11 | LOW | uncertain | fix Value::unmark() w/ parents=true |
| 137 | 383f332d2018 | 2021-05-06 | LOW | uncertain | src/conn.cpp: add limits header to fix 'numeric_limits' is not a membe |
| 138 | 38c15e655fc7 | 2020-07-16 | LOW | uncertain | fix/test allocArray |
| 139 | 3e12931f685e | 2022-08-12 | LOW | uncertain | fix tree format |
| 140 | 46ee1a6917c1 | 2024-07-08 | LOW | uncertain | ioc: ACF fix write permit when groups are present |
| 141 | 4d3683d75e77 | 2020-03-11 | LOW | uncertain | fix RPCBuilder |
| 142 | 51bd6a3d6c12 | 2023-06-12 | LOW | uncertain | ioc: fix LocalFieldLog "fast path" |
| 143 | 5210b7041d60 | 2020-07-21 | LOW | uncertain | fix TypeDef amend |
| 144 | 55d1b7292a5f | 2025-04-29 | LOW | uncertain | fix: Fixing how the hostname is identified to consider IPv6 |
| 145 | 5f8006fbf370 | 2023-05-16 | LOW | uncertain | fix MCastMembership::operator< |
| 146 | 69ed03e50886 | 2021-06-02 | LOW | uncertain | client: fix bcast addr id |
| 147 | 6d9a77d03bbd | 2023-01-26 | LOW | uncertain | SigInt fix disarm |
| 148 | 6fdd4989bd9e | 2023-09-15 | LOW | uncertain | Fix size/type typo |
| 149 | 7d16ab3a6279 | 2023-03-21 | LOW | uncertain | fix unsigned handling |
| 150 | 7e6a08def72e | 2020-04-17 | LOW | uncertain | fix Delta print of Union |
| 151 | 816838bcd590 | 2019-12-11 | LOW | uncertain | fix hex dump |
| 152 | 8333ce30ec99 | 2022-09-25 | LOW | uncertain | re-define user bufferevent limits in terms of OS buffer size |
| 153 | 90131d0a85e8 | 2022-06-14 | LOW | uncertain | ifaddrs::ifa_addr can be NULL |
| 154 | 939391590e90 | 2023-05-11 | LOW | uncertain | client: clear nameServers during close() |
| 155 | 9996abef3159 | 2020-03-19 | LOW | uncertain | fix Value::isMarked parents=true |
| 156 | a4e974def908 | 2023-05-10 | LOW | uncertain | client: fix batch pop() of exception |
| 157 | abeb78a9cdf8 | 2023-03-20 | LOW | uncertain | fix TypeDef(const Value& val) for Union/UnionA/StructA |
| 158 | b0c36f365e79 | 2022-09-27 | LOW | uncertain | bevRead fix low water mark and optimize |
| 159 | b3778581156b | 2020-03-23 | LOW | uncertain | fix shared_array operator<< |
| 160 | b54b9fb78d4c | 2020-01-29 | LOW | uncertain | server fix bind() when 5075 in use |
| 161 | b8be9bd05833 | 2020-06-25 | LOW | uncertain | fix Value iteration |
| 162 | b9170a98857d | 2023-02-11 | LOW | uncertain | fix Value::nmembers |
| 163 | b9b22adb15db | 2019-12-18 | LOW | uncertain | fix version_str() |
| 164 | c2f1f13bb3b8 | 2022-01-28 | LOW | uncertain | server: fix beacons TX on Linux |
| 165 | c7b4650ba132 | 2022-12-14 | LOW | uncertain | fix TypeStore maintenance |
| 166 | c87041590840 | 2025-01-30 | LOW | uncertain | fix formatting of uint8 and int8 fields |
| 167 | cacc9d088d6e | 2020-01-31 | LOW | uncertain | fix de-serialize of sub-sub-struct |
| 168 | d65abb28ea8f | 2020-04-17 | LOW | uncertain | shared_array fix print of char[] |
| 169 | dc4c4ae87032 | 2020-03-04 | LOW | uncertain | fix *Builder visibility |
| 170 | dfbed0c85074 | 2021-07-14 | LOW | uncertain | server ExecOp timer |
| 171 | e09f901e72d7 | 2023-09-26 | LOW | uncertain | client: fix _reExecPut() allowed for .get() |
| 172 | e0a8572c2d2b | 2023-05-10 | LOW | uncertain | server: fix stats(reset=true) |
| 173 | e51954529a75 | 2023-06-27 | LOW | uncertain | ioc: avoid *NULL on exit when partially initialized |
| 174 | e93909cf7e63 | 2023-02-11 | LOW | uncertain | fix shared_array::convertTo() |
| 175 | ed5b15d38e39 | 2020-02-07 | LOW | uncertain | fix log |
| 176 | ed5bcc8a4fb1 | 2020-04-17 | LOW | uncertain | fix handling of segmented messages |
| 177 | f2777e319b34 | 2020-07-16 | LOW | uncertain | fix shared_array::convertTo() |

## Eliminated (not ported — memory/RAII)

- 1627eb9e07ca [HIGH] log_exc_printf() print message before maybe abort()
- c7ba7d21b61a [HIGH] add client monitor
- 0b0dfde5c97b [MEDIUM] ioc: group put w/o effect is an error.
- 20c4ff0c26f5 [MEDIUM] test for Context leak
- 479f0f1f4dac [MEDIUM] fix spelling in public headers
- 6fd01c7bec2e [MEDIUM] improve error handling
- 9ccd7b50ab4f [MEDIUM] ioc: fix block=true to DBF_ENUM
- 9ceab63d0282 [MEDIUM] Introspect can error
- adab53e5c5fd [MEDIUM] client: error on empty PV name
- bcea4f032aa3 [MEDIUM] server missing channel onClose
- c127e5ae1f65 [MEDIUM] do beacon clean on UDP worker
- cfda7e2260ac [MEDIUM] client: trap error in close()
- d852758b7b83 [MEDIUM] client: ensure worker is joined on close()
- dd3706aa0f9d [MEDIUM] Avoid client Context leak
- f20d958c4682 [MEDIUM] client: avoid assert() with invalid forceServer
- f948a4fbb0ee [MEDIUM] client: log invalid monitor queueSize
- 06f1a8c2db7b [LOW] fix Timer ownership
- 187def97f990 [LOW] fix UDPCollector sharing
- 20acafa963ef [LOW] fix Connection ownership
- 25e7285c11b6 [LOW] SigInt use worker thread
- 2b16c1c087f3 [LOW] fix Value move ctor
- 3dcf2f59fe1a [LOW] fix logger_level_set()
- 4141775c716e [LOW] client: add non-intrusive free-list for subscription queue
- 597330c949f9 [LOW] ioc: fix PUT to scalar mapping
- 59c7fde958dd [LOW] ioc: fix group put over-process
- 691a5825ae15 [LOW] ioc: fix DBE_ARCHIVE handling w/ singlesource
- 6b7862393228 [LOW] sharedpv possible locking issue with concurrent open()/close() vs. post()
- 7ae659678fa3 [LOW] fix: do not re-search for other channels when doing initial channel search
- 8246a6480674 [LOW] client: monitor connect autoExec()
- 85285546b2a5 [LOW] fix Subscription::shared_from_this()
- 92fb0a4afa50 [LOW] client: fix delta sync of Compound
- 964da05ef27e [LOW] Revert "accommodate gcc vs. msvc handling of empty __VA_ARGS__"
- 9aa37558794f [LOW] fix MPMCFIFO emplace()
- 9af841cccde5 [LOW] server/client search logging
- a556e7e29043 [LOW] fix array of scalar xcode
- a7d761d5077e [LOW] fix spelling
- bebd7a91aaab [LOW] SockAddr fallback to sync. dns lookup
- c2a4224a2176 [LOW] server monitor
- d0b62d695f2d [LOW] udp_collector: fix listener filtering
- e52ae674ca70 [LOW] client: bypass search throttling during Channel creation
- e8649ecdd2e8 [LOW] add StaticSource::close()
- ed6fa0bd1a11 [LOW] fix encoding of (Sub)Struct w/ valid set
- f88733d7c641 [LOW] shared_array: fix assembly from void*
