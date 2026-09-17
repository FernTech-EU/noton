# Feature choices for Noton

These choices are for this planning session. “Recommend excluding” means outside the
product being proposed, not a commitment to implement it in an unspecified later phase.
Owner responses have been requested; no response has been assumed to be approval.

Microsoft describes OneNote's movable containers, images, tables, attachments, recording,
tags/checklists, search and shared notebooks in [Introducing OneNote](https://support.microsoft.com/en-us/onenote/onenote-help-and-learning/introducing-onenote).
Capabilities differ between clients; [Microsoft's web/desktop comparison](https://learn.microsoft.com/en-us/office365/servicedescriptions/office-online-service-description/onenote-online)
helps avoid treating the product family as one uniform feature set.

| Capability | Recommendation | Consequence for Noton |
|---|---|---|
| Two-level folder/note organisation | Required by owner | Flat folder list, scoped note list; no recursive tree |
| Free-positioned rich-text blocks | Include | Genuine SceneView editors; keyboard movement and resizing |
| Editable linear reading presentation | Include | Same blocks and history, stored reading order; full navigation without spatial interaction |
| Headings, basic formatting, lists, links | Include | Round-trip-tested Djot subset; commands and selection state exposed to AT |
| Checklists | Include | Checked state and toggling work for keyboard and screen readers; searchable text |
| Images | Include | Managed internal assets, alt text, keyboard resizing, memory/decode limits |
| Full-text search | Include | Library search, stale-result handling, French accent-aware matching |
| Links between notes | Include | Durable UUID targets, rename-safe links, broken-target state |
| Trash and restore | Include | Persistent recovery with explicit permanent purge |
| Autosave and native backups | Required/recommended respectively | Visible durability state, failure recovery and coherent snapshots |
| HTML/plain-text export | Include | Export in reading order; explain geometry loss; native backup remains lossless |
| Tables | Ask owner; useful if frequently needed | Editor supports table operations, but storage, AT, paste and export all need matrix tests |
| Tags | Ask owner | Decide note-level tags versus inline semantic tags; they are different models |
| File attachments | Ask owner | Immutable managed blobs, safe extraction/opening, metadata and portable export |
| Pen/ink and shapes | Recommend excluding unless central | Pressure/strokes, eraser, geometry, accessible descriptions and non-drawing alternatives |
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

Questions already sent:

1. Which baseline features should be removed, and which optional features should be included?
2. Canvas plus editable linear view, flowing documents only, or two distinct page types?
3. Linux only, or Linux + Windows + macOS? Local-only portable library, or mandatory sync?

Additional decisions to settle with the same scope discussion: whether printing, encryption,
and multiple simultaneous libraries are needed. Recommendation: one active library per
window, no concurrent editing of the same file by different processes, with architecture
identities scoped per library. Do not infer a mobile/web release from Teksilo portability.

A supplied feature choice overrides these recommendations. If sync, ink, or two page types
are selected, revise storage, accessibility and history contracts before approving the plan.
