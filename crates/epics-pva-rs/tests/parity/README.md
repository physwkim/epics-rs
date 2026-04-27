# PVA parity test harness

End-to-end and byte-level parity tests against the upstream EPICS C++
reference implementation [`pvxs`](https://github.com/epics-base/pvxs).

This directory backs Phase 0 of the spvirit-removal plan
(`/Users/stevek/.claude/plans/crystalline-roaming-quilt.md`).

## Layout

```
tests/parity/
├── README.md                 # this file
├── fixtures/
│   └── golden_wire/          # captured byte-exact PVA messages
│       (drop *.bin files here, see "Capturing fixtures" below)
└── (cross-implementation interop runners — added incrementally)
```

## Goals

1. **Byte-exact wire format** — every command we emit/decode is verified
   against a fixture captured from `pvxs` 1.x.
2. **4-way interop matrix** — confirm that all combinations of clients and
   servers (ours / pvxs) talk to each other:
   - our pvget-rs ↔ pvxs `softIocPVX`
   - pvxs `pvget`  ↔ our `PvaServer`
   - our pvget-rs ↔ our `PvaServer`
   - pvxs `pvget`  ↔ pvxs `softIocPVX` (control)
3. **NormativeType conformance** — `structure_id`, field order, and
   alarm/timeStamp payloads must match pvxs byte-for-byte for NTScalar,
   NTScalarArray, NTEnum, NTTable, NTNDArray.
4. **BitSet semantics** — first event has all bits set; subsequent events
   carry only the changed-field bitset; nested structure changes use the
   correct depth-first field index.

## Current coverage

| Area | Status |
|---|---|
| Size encoding parity | ✅ `proto_spvirit_parity::size_matches_spvirit` |
| String encoding parity | ✅ `proto_spvirit_parity::string_matches_spvirit` |
| Header parity | ✅ `proto_spvirit_parity::header_matches_spvirit_application` |
| Status OK parity | ✅ `proto_spvirit_parity::status_ok_matches_spvirit` |
| IPv4 wire conversion | ✅ `proto_spvirit_parity::ip_to_bytes_matches_spvirit` |
| FieldDesc structure encoding | ✅ `proto_spvirit_parity::field_desc_matches_spvirit_structure_desc` |
| `pvRequest` builder | ✅ `proto_spvirit_parity::pv_request_matches_spvirit` |
| SEARCH command | ✅ `proto_spvirit_parity::codec_search_matches_spvirit` |
| CREATE_CHANNEL | ✅ `proto_spvirit_parity::codec_create_channel_matches_spvirit` |
| GET/PUT/MONITOR/GET_FIELD/DESTROY | ✅ `proto_spvirit_parity::codec_op_requests_match_spvirit` |
| CONNECTION_VALIDATED | ✅ `proto_spvirit_parity::codec_connection_validated_matches_spvirit` |
| BitSet decode of all-bits-set | ✅ `proto_spvirit_parity::bitset_decodes_spvirit_first_event_payload` |
| GET response decode | ⏳ Phase 3 |
| MONITOR delta apply | ⏳ Phase 3 |
| NTScalar conformance | ⏳ Phase 5 |
| NTScalarArray conformance | ⏳ Phase 5 |
| NTEnum conformance | ⏳ Phase 5 |
| NTTable conformance | ⏳ Phase 5 |
| NTNDArray conformance | ⏳ Phase 5 |
| 4-way interop matrix | ⏳ Phase 4 (needs server) |

## Capturing fixtures

Once a `pvxs` build is available locally:

```bash
# Build pvxs
cd ~/codes/pvxs && make -j

# Run softIocPVX and capture wire bytes
sudo tcpdump -i lo -w fixtures/golden_wire/get_double.pcap \
    'tcp port 5075 or udp port 5076'

# In another shell:
~/codes/pvxs/bundle/usr/local/lib/perl/PVXS/softIocPVX -d test.db &
~/codes/pvxs/bundle/usr/local/bin/pvget MY:DOUBLE
```

Then convert `.pcap` → raw bytes (one file per direction per command):

```bash
# Strip TCP/UDP framing, save reassembled application payloads
tshark -r get_double.pcap -T fields -e tcp.payload -e udp.payload \
    | xxd -r -p > fixtures/golden_wire/get_double.bin
```

Test code under `tests/parity/golden_wire.rs` (added in Phase 3) loads
these `.bin` files and asserts byte-exact decode + re-encode.

## Running

```bash
cargo test -p epics-pva-rs --test proto_spvirit_parity   # Phase 1+2
cargo test -p epics-pva-rs --test 'parity_*'             # all parity tests
```

Once `spvirit-codec` is removed in Phase 6, the cross-check tests in
`tests/proto_spvirit_parity.rs` will be deleted; the golden-wire fixtures
remain as the long-term parity ground truth.
