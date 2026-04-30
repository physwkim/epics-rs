---
short_sha: ca2ea14
status: not-applicable
files_changed: []
---
Same structural absence as b35064d / b35064d's predecessor: there is no `db_event.rs`, no `EventUser` worker-thread struct, and no `db_close_events` shutdown path in base-rs. Monitor delivery is per-subscriber push into bounded mpsc channels; `DbSubscription::Drop` closes the rx side and the `Subscriber` slot (record_instance.rs:1395-1450) becomes dead but harmless. There is no separately-spawned event task whose `JoinHandle` could be dropped mid-execution while another thread frees its shared state. Nothing to fix.
