# Feature choices for Noton

These choices are for this planning session. “Recommend excluding” means outside the
product being proposed, not a commitment to implement it in an unspecified later phase.
Owner response: add **tables and ink (best-effort physical tablet support)**; support
**Linux, Windows and macOS**, using Skribisto's CI. The baseline remains the proposal with
these additions; other optional features are not selected. Page layout is **A: canvas + editable linear view, with movable ink sketch blocks**.
No external Opus/Sonnet review is requested; work stays in this ChatGPT session.

Microsoft describes OneNote's movable containers, images, tables, attachments, recording,
tags/checklists, search and shared notebooks in [Introducing OneNote](https://support.microsoft.com/en-us/onenote/onenote-help-and-learning/introducing-onenote).
Capabilities differ between clients; [Microsoft's web/desktop comparison](https://learn.microsoft.com/en-us/office365/servicedescriptions/office-online-service-description/onenote-online)
helps avoid treating the product family as one uniform feature set.

| Capability | Recommendation | Consequence for Noton |
|---|---|---|
| Two-level folder/note organisation | Required by owner | Flat folder list, scoped note list; no recursive tree |
| Free-positioned rich-text blocks | **Selected: A** | Genuine SceneView editors; keyboard movement and resizing |
| Editable linear reading presentation | **Selected: A** | Same blocks and history, stored reading order; full navigation without spatial interaction |
| Headings, basic formatting, lists, links | Include | Round-trip-tested Djot subset; commands and selection state exposed to AT |
| Checklists | Include | Checked state and toggling work for keyboard and screen readers; searchable text |
| Images | Include | Managed internal assets, alt text, keyboard resizing, memory/decode limits |
| Full-text search | Include | Library search, stale-result handling, French accent-aware matching |
| Links between notes | Include | Durable UUID targets, rename-safe links, broken-target state |
| Trash and restore | Include | Persistent recovery with explicit permanent purge |
| Autosave and native backups | Required/recommended respectively | Visible durability state, failure recovery and coherent snapshots |
| HTML/plain-text export | Include | Export in reading order; explain geometry loss; native backup remains lossless |
| Tables | **Selected** | Editor supports table operations, but storage, AT, paste and export all need matrix tests |
| Tags | Ask owner | Decide note-level tags versus inline semantic tags; they are different models |
| File attachments | Ask owner | Immutable managed blobs, safe extraction/opening, metadata and portable export |
| Pen/ink | **Selected, hardware best-effort** | Mouse and synthetic-pen validation required; vector strokes, whole-stroke eraser, undo, persistence and accessible descriptions; shapes/recognition not implied |
| Audio recording | Recommend excluding | Devices, permissions, codecs, playback controls and transcript access |
| OCR / handwriting recognition | Recommend excluding | Recognition engine, language packs, confidence and correcting recognised text |
| PDF annotation / file printouts | Recommend excluding | PDF rendering, page anchors, accessible annotation order, export fidelity |
| Templates | Ask owner | Templates are document data; instantiate with fresh UUIDs and remapped links |
| Persistent page history | Ask owner | Snapshot retention, restore-as-new-version, assets and storage budgets; distinct from undo |
| Synchronisation / sharing | Recommend excluding from local-first product | Conflict model, remote identities, authentication, deletion/attachment sync; affects the core architecture |
| OneNote import | Ask owner only if migration is needed | Specify exact accepted source/export format and fidelity; never promise generic .one compatibility |
| Password-protected/encrypted libraries | Ask if required | Key management, recovery and backup format; not achieved by hiding folders in the UI |
| Printing / PDF export | Ask owner | Explicit reading/spatial pagination, font embedding, selectable text and accessible output requirements |
| Web clipping, meeting integrations, AI features | Recommend excluding | External integration/privacy surfaces with little value to the proposed lightweight core |

Confirmed decisions:

1. Tables and best-effort ink are included. The unselected extras are outside the current
   proposed scope, rather than an implied future-feature backlog.
2. Linux, Windows and macOS are required; see [CI adaptation](ci-plan.md).
3. Read Skribisto's actual guide at `/home/cyril/Devel/skribisto/.claude/CLAUDE.md`.
4. Do not use Opus/Sonnet or the Claude CLI. The earlier external-review request is withdrawn.

The page-model choice is settled: **A with movable sketch blocks for ink**. See
[the decision and alternatives](page-options.md). Canvas and linear presentations use the
same content model. B (flowing-only documents) and C (separate note types) are not selected.
Cross-block handwritten annotations are outside scope.

Local-first portable storage remains a recommendation; no sync feature was selected.
One active library per window remains the proposed starting point. Selecting ink does not
by itself select cross-block handwritten annotations, OCR, or handwriting recognition.
