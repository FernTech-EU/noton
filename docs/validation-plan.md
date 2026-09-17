# Noton validation and acceptance plan

Planning artifact, not a report of tests already run. There is no Noton application to
exercise yet. The core entity proposal has been checked with Qleany; no UI pass is claimed.

## Harnesses

1. Pure domain tests exercise real Qleany use cases/UoWs against the generated store,
   including failure rollback and events emitted only after commit.
2. UI headless tests mount Noton's actual root/session and models with
   `WidgetTree`, `LayoutContext::for_testing` and `MockTextBackend`. Compile/test/lint
   both the real UI and its `mocks` feature; mocks cannot replace real-backend tests.
3. An app-specific MCP headless harness owns that real widget tree on one thread and
   dispatches `teksilo_automation::execute`. `teksilo-automation-mcp --headless` alone
   tests the toolkit's demo, **not Noton**.
4. The debug desktop binary enables Teksilo `automation` and
   `.install_automation_bridge_in_debug()`. Attach explicitly using
   `teksilo-automation-mcp --attach-pid <Noton PID>` to avoid selecting another app.
   Release packaging verifies that it exposes no automation bridge.
5. Real assistive-technology sessions test Orca on Linux, NVDA on Windows and VoiceOver
   on macOS for each platform actually selected. Internal MCP actions do not validate
   OS adapters, speech, native menu interaction or IME integration on their own.

No compatible Teksilo automation MCP tool is connected in the current session. The
planned integration must make this server available to the test runner; generic browser
automation is not a substitute for the native Teksilo tree.

## MCP protocol discipline

Snapshot → find named/identified node → inspect advertised actions → act → settle →
assert semantic state and durable result. Use `snapshot_tree`, `find_node`, `read_node`,
`invoke_action` with explicit `action`, `inject_key`, `type_text`, `type_ime`,
`layout_tree`, `get_overlays`, `pull_announcements`, and screenshots where appropriate.
Use the server's discovered schema rather than guessed JSON parameters.

Use `command: true` for the platform accelerator. Re-find nodes after structural rebuilds
and density changes. Advance simulated time for widget debounce, and inject a shared clock
into Noton's SaveCoordinator for deterministic application deadlines; do not assume the
MCP clock automatically controls background I/O or operating-system clocks. Await an actual
save acknowledgement or condition before asserting on disk. Screenshots are physical pixels;
node bounds/pointer coordinates are logical. Record scale when comparing them.

## Required end-to-end scenarios

| ID | Scenario | Pass criterion |
|---|---|---|
| N01 | Create two folders and two notes each; change folder | Correct owner's notes only; IDs/selection survive reorder |
| N02 | Rename, reorder and move a note; undo/redo | One atomic operation per action; original order/ownership restored |
| N03 | Empty library, empty folder, last-note deletion | Reachable Create action and correct focus; no panic or invalid selection |
| E01 | Type French accented text, emoji and multiline text in two blocks | Exact committed text after save/reopen; offsets remain character-correct |
| E02 | IME composition, focus/view switch and close | No duplicate/preedit text, no lost committed text, deterministic commit/cancel |
| E03 | Bold-only edit, link edit, checklist toggle, undo after save | All mutations mark dirty and survive reopen with semantic fidelity |
| E04 | Paste formatted HTML, image and unsupported markup | Supported structure retained, unsafe/external content controlled, no silent corruption |
| S01 | Drag, keyboard move, AT move, resize, cancel | Same domain result; one history step per gesture; cancel makes no committed edit |
| S02 | Pan/zoom with a caret and selection | Correct hit testing, visible caret, preserved focus and selection |
| S03 | Far-offscreen block reached through navigator | Item is revealed, focus is valid and AT tree includes focused node |
| S04 | Canvas ↔ Reading view at an active caret | Same text/block identity/history; no duplicate AT content or modified geometry |
| U01 | Text typing → drag → text undo | Explicit scoped result; labels match the active/latched scope |
| U02 | Open Edit menu or toolbar; open Settings modal | Original scope survives menu; modal cannot undo unrelated content |
| U03 | Undo text with empty text history | No fallthrough to folder/note deletion or movement |
| U04 | Undo a note/block creation after descendant edits | Recoverable content retained; redo restores latest content, not initial empty content |
| U05 | Trash/restore while note has text and scene histories | Ownership, content, asset refs, focus and eligible histories are coherent |
| U06 | Purge with dependent history | Affected histories invalidated atomically; no command targets a deleted ID |
| P01 | Edit again while a deliberately slow save is running | Old acknowledgement cannot show Saved; queued new save covers new revision |
| P02 | Continuous typing beyond debounce | Save requests meet maximum interval target without splitting composition |
| P03 | Disk-full, denied write, failed codec or failed commit | No false success; old file valid, buffers retained; retry/save-copy succeeds |
| P04 | Close/switch/export while saving | Waits for own barrier sequence, not any completion; cancel remains safe |
| P05 | Kill at transaction fault points, then reopen | All acknowledged revisions recover; uncommitted transaction is absent, not partial |
| P06 | Two processes open same library; symlink aliases | One writer; read-only/conflict route; canonical-path lock cannot be bypassed |
| P07 | Migration/newer file/corrupt relationships/missing blob | Backup before migration; fail without destructive rewrite; actionable diagnosis |
| P08 | Move portable library to a new directory/machine | No absolute asset paths or numeric session IDs break content/links |
| A01 | Entire folder/note/edit/settings/backup workflow keyboard-only | Every function reachable; no trap, obscured focus or drag-only operation |
| A02 | Advertised AT actions on menus, lists, blocks and checklists | Action actually changes intended state and reports correct result |
| A03 | Screen-reader reading/caret/selection at zoom and scale | Correct text/ranges/order, no skipped or duplicate blocks |
| A04 | Light/dark/high contrast, 200% text, reduced motion, narrow window | Readable controls, reflow, visible focus and usable navigation |
| L01 | Switch EN ↔ FR with focus, menu and error visible | All chrome/AT translations update; user content and focus preserved |
| Q01 | Search accents, emoji, tables if selected, trash, stale results | Correct scope and range; result opens exact note/block |
| X01 | Native export/import and HTML/text export | Native semantic identity preserved; linear exports use explicit reading order |

Optional features selected by the owner add equally concrete scenarios before development:
for example a table requires insert/delete rows, keyboard cell traversal, AT reading order,
undo, search offsets, paste and save/export round-trip—not just a working Insert button.

## Lower-level properties and failure injection

- Round-trip fixtures for every approved formatting construct and combinations of them;
  preserve semantics, UUIDs, ordering, asset digests, language and links.
- For each command, execute→undo restores owned fields; redo restores the executed state;
  unrelated fields edited in another history scope remain unchanged.
- Multi-entity failures publish no partial event sequence and leave no orphaned relationships.
- Scene replay never records itself. Rejected/abandoned gestures do not clear redo or mark
  the library dirty after restoration. No RefCell borrow is held through event fan-out.
- Fuzz/mutate file inputs and impose limits on decompressed image dimensions, blob lengths,
  note/block counts, markup nesting and resource references. Unknown formats fail explicitly.
- Asset reachability includes trash and retained history; interrupted ingest leaves no live
  broken reference. Backup restoration verifies the entire content graph.

## Performance acceptance targets

Proposed targets for a documented reference desktop and release build; not measurements:
10,000 notes of 2 KB average text, 100 folders, 100 MB of managed media, and active pages
with 20/200/1,000 blocks as separate workloads. Record hardware, font backend, GPU, OS,
window size and scale. Active editing of one long block is a separate stress case.

Target p95 input-to-frame below 50 ms; common page switch below 150 ms once loaded;
search below 200 ms after indexing; idle CPU below 1%; no UI-thread disk I/O. Record
cold open, memory, index build, save latency and binary/package size. Target empty-library
resident memory below 150 MiB and the defined corpus below 350 MiB, excluding unusual
GPU-driver accounting; report both RSS and attributable CPU/GPU allocations.

A missed target triggers measured correction before release, not an invented benchmark
result or an unexplained raised limit. Do not equate dormant scene widgets with unloaded
editors. Test native packages under realistic font/IME/accessibility conditions.

## Completion evidence

The final implementation report contains exact commands/results, failing-case regressions,
MCP transcripts/assertions, selected screenshots, real screen-reader results, corruption and
restart evidence, performance measurements, packaged platforms, and the pinned dependency
revisions. No `todo!`, `unimplemented!`, reachable placeholder command, disabled promised
feature or skipped required gate is accepted as completion.
