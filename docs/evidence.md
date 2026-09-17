# Evidence and verification record

Reviewed on 17 September 2026 against local files, not assumed published API parity.

| Input | Observed revision/status |
|---|---|
| Noton base | `c183ecf` (initial README-only repository) |
| Planning worktree | `/tmp/noton-architecture`, branch `codex/noton-architecture` |
| Teksilo | `e7c75c478d05dd9ae3c02f11ed315d2ae9667dc4`; pre-existing modifications in skill/app-guide files, left untouched |
| Skribisto | `85d1eb2539c99676223b5785976468e3f30ff3bb` |
| Qleany | Installed CLI reports `1.12.0`; new manifest schema `6` |
| Requested Skribisto CLAUDE.md | Missing in this checkout, including hidden-file search; `CLA.md` is a contributor agreement, not development instructions |
| Requested app guide | `/home/cyril/Devel/teksilo_app_guide.md`, 937 lines, read; byte-identical to the local teksilo-app skill's bundled guide |

## Applied local skills

- [/home/cyril/.claude/skills/teksilo-app/SKILL.md](/home/cyril/.claude/skills/teksilo-app/SKILL.md): version-matched APIs, model ownership, thread affinity, automation/testing conventions.
- [/home/cyril/.claude/skills/qleany/SKILL.md](/home/cyril/.claude/skills/qleany/SKILL.md): model exploration, generated/owned boundaries, commit-before-generation discipline.

Other available Claude skills were inventoried. Audio notification/creative-pack skills
were unrelated; widget API extraction and teksu macro authoring are implementation tools,
not necessary to produce a planning proposal. No application code was generated.

## Local evidence anchors

| Finding | Evidence |
|---|---|
| Rust Qleany uses an ephemeral HashMap store; custom save/load required | `qleany docs rust --md`, sections HashMap Store Backend and Ephemeral Database Pattern |
| Teksilo singles, list models, event bridge and mocks are generated | `qleany docs manifest --md`, “What rust_teksilo generates”; `qleany docs rust --md`, “The Teksilo UI crate” |
| Rust use cases execute synchronously; events follow commit | `qleany docs flow --md`, Rust execution sequence |
| Strong ownership controls undoability rules | `qleany docs undo --md`, Undo-Redo Rules |
| Regeneration overwrites, never merges | `qleany docs regen --md` |
| Qleany commands require Send | `/home/cyril/Devel/skribisto/crates/common/src/undo_redo.rs:82` |
| Scene is Rc/RefCell and owning records are separate from cloneable notifications | `/home/cyril/Devel/teksilo/docs/teksilo-scene.md:1033`, `:2696`; source `crates/teksilo-scene/src/journal.rs` |
| Scene abandoned transaction does not undo itself | `/home/cyril/Devel/teksilo/docs/teksilo-scene.md:1221` |
| Lightweight SceneListAdapter is not a rich-editor adapter | `/home/cyril/Devel/teksilo/docs/teksilo-scene.md:2765` and its delegate return type |
| Heavyweight retention does not avoid materialization | `/home/cyril/Devel/teksilo/docs/teksilo-scene.md:1950` |
| Payload replacement rebuilds editors | `/home/cyril/Devel/teksilo/docs/teksilo-scene.md:2726` |
| Scene traversal and offscreen controls exist | `/home/cyril/Devel/teksilo/docs/teksilo-scene-a11y.md:39`, `:302` |
| Interactive lightweight overlay AT ordering gap is documented | `/home/cyril/Devel/teksilo/docs/teksilo-scene-a11y.md:876` |
| Transform preview differs from commit; heavyweight rotation refused | `/home/cyril/Devel/teksilo/docs/scene/transform_session.md` |
| Skribisto names/latches/suspends undo domains | `/home/cyril/Devel/skribisto/crates/teksilo_ui/src/edit/undo_group_vm.rs:5` and `edit/domains.rs` |
| Prose flush is untracked and self-refresh can clear history | `/home/cyril/Devel/skribisto/crates/teksilo_ui/src/tabs.rs:1556` |
| Save serialization and revision coverage prevent old completion from losing data | `/home/cyril/Devel/skribisto/crates/teksilo_ui/src/save/save_queue.rs:5` |
| Existing model uses owner relationships and rewires on rebuild | `/home/cyril/Devel/skribisto/crates/teksilo_ui/src/models/binder_list_model.rs:79` |
| Shared model-driven hamburger/native menu pattern | `/home/cyril/Devel/skribisto/crates/teksilo_ui/src/shell/windows.rs:724` |
| Text content_revision excludes format-only notifications | `/home/cyril/Devel/text-document/crates/public_api/src/inner.rs:192`; `document.rs:1837` |
| Editor on_change / document_version / IME semantics differ | `/home/cyril/Devel/teksilo/docs/widgets/rich_text.md:482` |
| Toolkit MCP headless demo is not an app harness | `/home/cyril/Devel/teksilo/docs/automation-mcp.md`; app guide automation section |
| Framework internal a11y assessment is not live conformance evidence | `/home/cyril/Devel/teksilo/docs/accessibility-internal-audit.md:6` |

Where prose conflicts with source, source wins. One concrete example: the beginning of
`teksilo-scene.md` says the crate does not depend on teksilo-widgets, but the current
`crates/teksilo-scene/Cargo.toml` explicitly does. Do not rely on the old dependency claim
for a minimal-binary estimate. Also avoid old title-bar examples with removed `add_child`
or `*_id` builders; implementation must extract exact APIs and compile against its pin.

Skribisto's app code is a pattern reference, not a wholesale template to copy: its current
manifest does not itself reproduce all the handwritten Teksilo frontend. Qleany 1.12's
Teksilo generation is the correct starting point for Noton.

## External primary sources

- [Microsoft: Introducing OneNote](https://support.microsoft.com/en-us/onenote/onenote-help-and-learning/introducing-onenote) — movable containers, images, tables, recording, tagging/checklists, search and shared notebooks.
- [Microsoft: OneNote web service description](https://learn.microsoft.com/en-us/office365/servicedescriptions/office-online-service-description/onenote-online) — feature differences between clients.
- [Microsoft: Take and format notes](https://support.microsoft.com/en-us/onenote/take-and-format-notes) — note editing/navigation entry points.
- [SQLite: Atomic commit](https://www.sqlite.org/atomiccommit.html) — transactional storage and filesystem assumptions.
- [SQLite: synchronous](https://www.sqlite.org/pragma.html#pragma_synchronous) — durability modes, including EXTRA for rollback journals.
- [SQLite: Backup API](https://www.sqlite.org/backup.html) — consistent snapshots of a live database.

## Checks actually performed

- Created the requested separate Git worktree and branch; original worktree left unchanged.
- Read the local sources and authoritative installed Qleany documentation listed above.
- Created `docs/qleany-core-proposal.yaml` as an entity-design artifact, not a complete
  implementation manifest. It deliberately contains no feature scaffolds; custom operation
  contracts are described in the architecture and must be encoded after scope selection.
- `qleany check -m docs/qleany-core-proposal.yaml`: **Manifest is valid**.
- `qleany list entities -m docs/qleany-core-proposal.yaml`: EntityBase, Root, Library,
  Folder, Note, Block, Asset.
- No Noton Rust compilation, UI tests, MCP runs or screen-reader tests were possible:
  this is a planning session over a README-only repository. The validation plan specifies
  the evidence the implementation must produce.

## External reviewer status

A read-only Claude Opus review was attempted using the installed CLI and only Read/Glob/Grep
tools. The sandboxed attempt failed with `EAI_AGAIN` (API unreachable). The network retry
was rejected by automatic approval review because repository excerpts would be sent to
Anthropic without sufficiently explicit data-transfer approval. A separate approval question
was sent to the owner. No Opus review result exists and none is credited to this plan.
Local review continued and identified the formatting-revision and creation-undo hazards.
