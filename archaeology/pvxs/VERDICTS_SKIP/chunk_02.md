# SKIP Chunk 02 Verdicts

## ea507d46 — N/A — make OperationBase::channelName constant
**Reason**: C++ API refactor without security or functional implications.

## 45e31f46 — N/A — evutil_getaddrinfo() expects result pointer to be pre-zeroed
**Reason**: libevent-specific crash fix on win64 static builds; pva-rs uses tokio::net.

## 65d2f943 — N/A — IfMapDaemon SockAttach
**Reason**: libevent event handling; pva-rs uses async/tokio.

## 12b7ecac — N/A — doc
**Reason**: Documentation only.

## 04047e794 — N/A — preserve UDP port in classification logic
**Reason**: UDP multicast forwarding refinement; pva-rs does not relay multicast.

## 07c06f2e — N/A — handle ORIGIN_TAG 0.0.0.0
**Reason**: UDP forwarding edge case; pva-rs lacks forwarding.

## 2509525b — N/A — minor
**Reason**: Whitespace cleanup.

## da6003d8 — N/A — 1.4.0a1
**Reason**: Version tag.

## 01c11e16 — N/A — add SockAddr::map6to4()
**Reason**: IPv6-to-IPv4 mapping utility; not in pva-rs scope.

## d069f488 — N/A — ioc: record._options.process accept numeric values
**Reason**: EPICS IOC record parsing; pva-rs has no record system.

## fdef7502 — N/A — remote log DBE parsing
**Reason**: EPICS IOC remote logging; not in pva-rs.

## a372d936 — N/A — add server to client remote logging
**Reason**: EPICS IOC infrastructure absent in pva-rs.

## b8a4001c — N/A — GetAddrInfo wrapper always numeric
**Reason**: libevent config; pva-rs uses tokio::net.

## 190eb875 — N/A — update local mcast hack logic to sendmsg()
**Reason**: UDP multicast forwarding; pva-rs does not forward.

## a464e9a6 — N/A — redesign IfaceMap
**Reason**: libevent worker redesign; pva-rs uses async/tokio.

## 25f5f1dc — N/A — IfaceMap add look up index by address, and loopback by index
**Reason**: libevent interface tracking; pva-rs uses async.

## 80c63888 — N/A — add sendtox
**Reason**: libevent sendmsg() + IP_PKTINFO wrapper; pva-rs async model.

## e30640a6 — N/A — always "bind" to iface bcast when not any
**Reason**: libevent server bind; pva-rs uses tokio::net TcpListener.

## 66788f51 — N/A — minor
**Reason**: Code cleanup.

## edcc21bc — N/A — quiet clang warning
**Reason**: Compiler warning; pva-rs Rust compiler analogous.

## 847c5480 — N/A — pvalink: AMSG when disconnected
**Reason**: EPICS IOC pvalink; pva-rs has no pvalink integration.

## 2fef15f3 — N/A — pvalink: also copy AMSG with MS/MSI
**Reason**: EPICS IOC pvalink; not in pva-rs.

## f42bc976 — N/A — Fixed pvaGetLink for Union types; Added tests
**Reason**: EPICS IOC pvalink Union handling; pva-rs supports Union independently.

## 542e0fe9 — N/A — Changed Float32 and Float64 to Float32A and Float64A in NTNDArray TypeDef
**Reason**: NTNDArray typedef refactor; architectural, not bug fix.

## c3e91f60 — N/A — client: defer notification of connect() failure
**Reason**: libevent bevEvent() callback deferral; pva-rs uses tokio channels.

## 73c25448 — N/A — oops...
**Reason**: Follow-up to IOC group sourcing fix; not in pva-rs.

## fb4a3b9d — N/A — ioc: improve group processing warning messages
**Reason**: EPICS IOC warning formatting; not in pva-rs.

## 8502f91b — N/A — iocsh dbLoadGroup() not immediate
**Reason**: EPICS IOC initialization; not in pva-rs.

## a3a685ba — N/A — server: correctly adjudicate collision bind() of specific port
**Reason**: libevent SO_REUSEADDR + listen() retry logic; pva-rs uses tokio::net.

## 330097b7 — N/A — cache_sync() copy Any/Union
**Reason**: Delta mutation pattern; pva-rs lacks cache_sync() infrastructure.
