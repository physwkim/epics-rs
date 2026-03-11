# Sequencer Demo

Hand-written Rust implementation of `demo.st`, demonstrating the [seq](../../crates/seq/) runtime API for state-based automation.

Two concurrent state machines coordinate via event flags and PV monitoring:

- **counter_ss** — increments a counter PV from 0 to 10 at 1-second intervals, then exits
- **light_ss** — monitors counter changes via event flag and sets a light PV accordingly

## State Machines

```
counter_ss:                          light_ss:

  ┌──────┐   delay(1s)   ┌────────┐    ┌──────┐
  │ init │──────────────>│counting│    │ idle │<──┐
  └──────┘               └────────┘    └──────┘  │
                           │    │                  │
              counter>=10  │    │ delay(1s)        │ efTestAndClear
                           │    │ counter++        │ → set light
                           ▼    └──────────┘       │
                         ┌──────┐                  │
                         │ done │              ────┘
                         └──────┘
```

## PV Channels

| Channel | PV Name | Monitored | Sync | Description |
|---------|---------|-----------|------|-------------|
| `counter` | `{P}counter` | Yes | `ef_counter` | Incremented by counter_ss |
| `light` | `{P}light` | No | — | Set by light_ss (0 or 1) |

Default prefix: `P=SEQ:`.

## Original SNL Source

The `demo.st` file contains the SNL source that this Rust code implements:

```c
program demo
option +s;

double counter;
assign counter to "{P}counter";
monitor counter;

evflag ef_counter;
sync counter to ef_counter;

ss counter_ss {
    state init {
        when (delay(1.0)) { counter = 0.0; pvPut(counter); } state counting
    }
    state counting {
        when (counter >= 10.0) {} state done
        when (delay(1.0)) { counter += 1.0; pvPut(counter); } state counting
    }
    state done { when (delay(0.1)) {} exit }
}
```

## Build and Run

```bash
# Build
cargo build -p seq-demo

# Run (requires PVs to be served by an IOC)
cargo run -p seq-demo

# With custom prefix
cargo run -p seq-demo -- "P=myprefix:"
```

Before running, create the target PVs (e.g., via softioc-rs):

```bash
softioc-rs --record ai:SEQ:counter --record bo:SEQ:light
```

## License

MIT
