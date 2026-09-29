# Coordinator scheduling

Noton does not register a recurring frame-tick subscription. The application
coordinator runs on coalesced notifications or a one-shot deadline:

- Document changes (including formatting and history), editor input, focus and
  caret changes, and backend history events notify the coordinator. Editor state
  is read later, outside the original mutation. IME preedit is not flushed.
- A dirty library schedules autosave at the earlier of 750 ms after the latest
  edit and five seconds after the first unsaved edit. An in-flight save or a
  save failure does not schedule repeated checks.
- The writer queues its acknowledgement before posting a UI wake. Sequence
  numbers still determine which edits the acknowledgement covers.
- Initial canvas fitting observes viewport changes and uses a single deferred
  deadline. A zero-sized viewport waits for a new viewport notification.
- Deadlines merge with the tree's existing earlier wake. If another widget's
  earlier wake consumes that slot, the coordinator re-arms its later deadline.

The frame signal observer is passive: it does not request further frames when
there is no work. The MCP host processes only pending notifications and due
deadlines, rather than running an unconditional coordinator tick on observation.

## Regression checks

`cargo test --workspace` includes scheduling regressions for autonomous save
completion, autosave deadlines, failed-save backoff, shared tree deadlines,
canvas fitting, idle frame scheduling, formatting, and IME composition/commit.
The native and mock UI configurations are also checked with strict Clippy.

For the Linux desktop regression, build the real binary (a mock test build can
replace it), then use a fresh output directory:

```sh
cargo build -p noton-teksilo-ui
python3 tools/check_native_scheduling.py target/debug/noton /tmp/noton-scheduling-check
```

The probe launches its own app with isolated preferences and a disposable library.
It moves focus off text inputs, allows prior observation work to finish, then
checks for recurring redraws during a 2.5-second quiet interval. One carried-over
inspection redraw is allowed because trace counters reset at report time. Main-thread
context switches are recorded separately: OS notifications may wake it without
rendering. The unit test independently rejects any recurring root frame deadline.
It then types text and observes SQLite without further UI/MCP calls until autosave
persists the text, checks the Saved status, and closes its own window. Logs and
measurements remain in the supplied directory. This checks one Linux environment,
not cross-platform performance or physical input hardware.
