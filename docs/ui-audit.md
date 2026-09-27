# Noton user-story and visual audit — 18 September 2026

**Noton is not feature-complete or release-ready. The earlier smoke test was not
coverage of all user stories, and the earlier screenshot should not have been
accepted.** This report maps every one of the 43 acceptance scenarios to actual
evidence or a gap. A passing check of part of a story is not a pass for the whole story.

## What was run

Against the real backend in `/home/cyril/Devel/noton`, on Linux:

- 16 production-tree MCP scenarios in `tools/mcp_stories.py`, with SQLite assertions:
  all passed. [Machine-readable results](audit/2026-09-18/user-stories.json).
- Native MCP through `teksilo-automation-mcp`, attached only to test processes the
  runner created. Desktop 1280×850 and narrow 600×700; 19 screenshots, semantic trees
  and compressed transcripts retained under [audit evidence](audit/2026-09-18/).
  Both runs and their restart checks passed.
- Settings modal: initial focus, twelve Tab steps constrained to the dialog,
  document undo isolation, English→French, light→dark, Escape, Done and header Close.
- 144 Rust workspace tests passed; three generated doctests remain ignored by their
  existing annotations. Real and mock configurations passed strict Clippy; mock
  tests, formatting and EN/FR key parity passed.
- The dedicated headless table-accessibility probe still fails on a **fresh reopen**:
  a first snapshot lacks cell bounds. Native reopen captures expose all nine cell
  bounds in both projections. This discrepancy remains a failed headless acceptance
  check, not proof of a native screen-reader failure or a full accessibility pass.

The real binary is explicitly rebuilt after mock checks: Cargo's integration-test
build can leave a mock `target/debug/noton`. CI now does that before running both
MCP suites, avoiding a test against fabricated data.

These are not Windows/macOS execution results. Orca, NVDA, VoiceOver, physical pen
hardware, high contrast and 200% text have not been validated.

## Defects corrected during the audit

1. Card controls were placed in SceneCard's unbounded header; they and the editor
   extended past the card. Controls now wrap inside the bounded body, outside the
   draggable title. Editors fill the available width.
2. Adding a sketch crashed native rendering: `Canvas::save/restore` saves transforms,
   not clipping. Ink now pairs `set_clip` with `clear_clip`. Headless MCP now renders
   and validates render stacks as well as laying out the tree, so this failure is
   observable in the ordinary smoke test.
3. The sketch drawing surface extended below its card. It now uses the height left
   after controls; reading view gives the sketch an explicit height.
4. Settings was an independent, nonmodal window. It now follows Skribisto's in-tree
   presentation, instant-apply behavior, focus containment, Escape/X/Done dismissal,
   named modal accessibility node and RAII suspension of document history.
5. Empty text history could fall through into undoing block creation. Noton now
   latches its rich-text handle on editor focus, in addition to the general text
   surface registry. The no-fallthrough regression passes through MCP.
6. Formatting-only undo could change the live document without updating persistence.
   Noton now subscribes to semantic document events, including formatting and history,
   tracks changes independently of the input callback, and flushes them through the
   same IME-aware save barrier. A Rust regression verifies both the formatted snapshot
   and the subsequent undo; the MCP test verifies the saved SQLite payload.
7. Search did not match `cafe` to `Café`. It now normalizes accents and case and excludes
   trashed blocks from ordinary search; descriptions are included.
8. Pen release coordinates were omitted, and the eraser tested only sample points.
   The final sample is retained; whole-stroke erasure tests segments, including sparse
   crossing traces. Escape discards provisional ink. Additional pointer presses cannot
   replace an active stroke.
9. Unclear blank fields and identical Trash controls made navigation ambiguous.
   Fields have placeholders/current values, trash and restore actions name their target,
   and folder/note activation supports a single click and selection changes.
10. The headless host did not poll completed saves or settle externally changed signals.
    It now runs the same coordinator before observations, then settles and paints.
    Tests wait for actual save acknowledgement and inspect persisted content.

## Reviewed screenshots

[Settings modal, English/light](audit/2026-09-18/1280x850/08-settings-modal.png) ·
[Settings modal, French/dark](audit/2026-09-18/1280x850/10-settings-dark.png) ·
[Text canvas](audit/2026-09-18/1280x850/02-text-canvas.png) ·
[Ink canvas](audit/2026-09-18/1280x850/04-ink-canvas.png) ·
[Reading view](audit/2026-09-18/1280x850/05-reading.png) ·
[Table after reopen](audit/2026-09-18/1280x850/14-table-reopened-reading.png) ·
[Narrow reading view](audit/2026-09-18/600x700/03-reading.png) ·
[Narrow note list](audit/2026-09-18/600x700/04-notes.png).

Visual findings still open: the toolbar is crowded and does not show active formatting;
new canvas blocks are offset over existing blocks rather than placed in an unobstructed
area; Settings choices do not indicate their current selection. At narrow widths the
fixed-size spatial canvas clips cards at the viewport edge; reading view reflows, but
this does not establish that all offscreen content is conveniently reachable.
These are not accepted as polished final UX.

## Coverage of every acceptance scenario

“Partial” means only the stated subcase passed. “Unverified” means no end-to-end
result is claimed. “Missing” is a product implementation gap, not a skipped test.

| ID | Status | Evidence and remaining scope |
|---|---|---|
| N01 | Partial | Two folders, two notes each, owner filtering and selection pass; reorder not implemented/tested. |
| N02 | Partial | Rename, move and organization undo pass; note reorder missing; redo of that flow not exercised. |
| N03 | Partial | Empty startup and last-note trash exercised; exhaustive focus restoration not established. |
| E01 | Partial | Accents, emoji, keyboard newline and projection switching persist; two simultaneous text editors not covered. |
| E02 | Partial | Synthetic preedit→committed French/Japanese text passes; composition during close/view change unverified. |
| E03 | Partial / missing | Bold-only save and undo pass. Checklist and link editing UI missing. |
| E04 | Unverified | Rich clipboard paste, image paste and unsupported markup not exercised. |
| S01 | Partial | Accessible move/resize buttons and field-specific undo pass; pointer drag, AT transform handles and cancel not covered. |
| S02 | Unverified | Pan/zoom with caret and selection not exercised. |
| S03 | Missing | No block navigator for distant offscreen blocks. |
| S04 | Partial | Shared text across both projections and native images checked; active caret/selection continuity not established. |
| U01 | Partial | Text formatting history and separate geometry undo pass; mixed drag/text routing remains unverified. |
| U02 | Covered subcases | Menu preserves text undo target; live Settings focus loop and Ctrl+Z isolation pass. No settings text-field scenario exists. |
| U03 | Passed | Empty editor Ctrl+Z leaves block creation intact, verified on disk. |
| U04 | Domain only | Existing domain contract preserves descendant text across creation undo; no MCP story for this. |
| U05 | Partial | Note trash/restore preserves text and active visibility; asset references and all history combinations unverified. |
| U06 | Missing | Permanent purge and dependent-history invalidation are not implemented. |
| P01 | Unverified | No slow-writer fault injection for stale acknowledgements. |
| P02 | Partial | Idle autosave without Save passes; continuous typing maximum interval not tested. |
| P03 | Unverified | Disk full, failed commit, denied write and recovery routes not exercised. |
| P04 | Partial | Save acknowledgement, native backup, native close/reopen exercised; export and close cancellation during save unverified. |
| P05 | Unverified | No kill-at-transaction-fault-point test. |
| P06 | Partial | A second process through a symlink is refused while the first writer remains open on Linux; GUI conflict recovery not covered. Windows CI tests same-path locking, without requiring symlink privilege. |
| P07 | Unverified | No end-to-end migration, future-format, corrupt relationship or missing-blob matrix. |
| P08 | Unverified | No cross-machine/relocation scenario. Native backup alone is not this test. |
| A01 | Partial | Modal keyboard containment and narrow navigation pass; the entire workflow is not keyboard-certified. |
| A02 | Partial / missing | Many advertised click actions exercised; all advertised actions not audited and checklists missing. |
| A03 | Unverified | No native assistive-technology session. |
| A04 | Partial | Desktop/narrow, light/dark, card containment screenshots checked; high contrast, reduced motion and 200% text unverified. |
| L01 | Partial | Native EN→FR updates chrome while retaining user text; error/menu-open translation cases not covered. |
| Q01 | Partial | Accent-insensitive full-text search passes; emoji/table/trash/stale-result matrix and exact result ranges not covered. |
| X01 | Partial | Native backup contents checked; HTML/text export and import through native dialogs not automated. |
| T01 | Partial | Table insertion, cell edit, row/column insertion/removal and saved dimensions pass; table-specific undo/redo not covered. |
| T02 | Partial / failing harness | Native table/cell bounds present in both projections, including reopen; fresh headless reopen probe fails. Keyboard/AT at 200% unverified. |
| T03 | Partial | Native save/reopen table text and geometry checked; pasted tables, after-table search and exports unverified. |
| I01 | Partial | Synthetic pen pressure, local coordinates and final sample checked; full mouse/no-pressure matrix unverified. |
| I02 | Partial / missing | Escape cancellation passes. Explicit Pan tool missing; mode-switch/lost-capture cases not fully exercised. |
| I03 | Partial | Add/erase/undo/redo checked in SQLite and native ink reopen viewed; multi-stroke ordering not tested end-to-end. |
| I04 | Partial / missing | Controls/descriptions exist and movement/resizing/trash routes are present; sketch naming UI missing, complete keyboard story unverified. |
| I05 | Unverified | Transformed-viewport drawing/cumulative drift not tested. |
| I06 | Hardware unverified | Synthetic input is not a physical-tablet result; no hardware was available. |
| I07 | Partial | Surface containment and balanced clipping verified; boundary-crossing plus resize/source-sample invariance not fully tested. |
| I08 | Unverified | Surrounding text reflow/reorder with sketch geometry invariance not exercised. |

Other approved baseline gaps include durable links between notes, image alt-text editing,
a complete image import/decoding limit audit, and backup retention/recovery UX.

## Reproduction

From the repository root, with the configured sibling dependency overrides:

```sh
cargo build -p noton-teksilo-ui --locked
python3 tools/mcp_smoke.py target/debug/noton
python3 tools/mcp_stories.py target/debug/noton /tmp/noton-user-stories
python3 tools/mcp_visual_audit.py target/debug/noton /tmp/noton-visual-audit-new
python3 tools/mcp_table_accessibility.py target/debug/noton
```

The visual runner needs desktop/socket access and a **fresh output directory**. It
creates disposable libraries, explicitly attaches to its own PIDs, saves screenshots
and transcripts, then closes its own windows through the app's normal close action.
The last command is the separately reported failing headless table check; it is not
silently skipped or counted as passed. Upstream repositories were not modified.
