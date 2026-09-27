# Teksilo dependency integration

Noton's source and development executable live in `/home/cyril/Devel/noton`.

## Local framework development

Teksilo's new rich-text accessibility walk uses `TableVisualInfo`,
`DocumentFlow::table_visual_info` and `DocumentFlow::block_geometry` from
`text-typeset`. Overriding Teksilo alone is insufficient: Cargo does not inherit a
dependency's `.cargo/config.toml`. Noton's workspace must override the typesetter too.

From the Noton root:

```sh
python3 tools/bootstrap.py --teksilo ../teksilo --text-document ../text-document --text-typeset ../text-typeset
cargo build --locked
python3 tools/mcp_smoke.py target/debug/noton
python3 tools/mcp_table_accessibility.py target/debug/noton
```

The bootstrap command refuses to overwrite a different existing
`.cargo/config.toml`; preserve that file before selecting different sources.
Explicit local paths use the working copies, including their uncommitted changes.
No upstream files are modified by this integration.

## Reproducible builds

`python3 tools/bootstrap.py` checks out the committed revisions in
`tools/dependencies.json` under `.dependencies/`. All three source dependencies
are pinned, and `Cargo.lock` resolves `text-typeset` through the same source override
in both local development and CI.

At this update, the new table accessibility implementation is still uncommitted
in the sibling Teksilo and text-typeset repositories. The committed pins therefore
do **not** contain that fix. The separate table accessibility probe intentionally
fails against those older implementations; it is an acceptance check for the new
upstream work, not part of the pinned baseline CI until that work has a committed
revision to pin. The ordinary MCP smoke test runs in CI.

Even a passing table probe does not certify native Orca, NVDA or VoiceOver behavior.
It verifies structure and cell text in Noton's actual canvas and reading projections,
including save/reopen. Native assistive-technology validation remains necessary.

## Verification results

See [the current user-story and visual audit](ui-audit.md) for the latest tests,
screenshots and remaining failures. The initial API update passed 142 workspace
tests and strict Clippy. The subsequent audit added regressions (144 tests) and
16 MCP scenario checks, plus native screenshots and Settings-modal checks.

The table probe now gets through canvas and reading-view checks but fails on the
first headless snapshot after reopening. Native reopen screenshots and MCP trees
show all nine cell bounds in both views. This discrepancy remains open and does
not substitute for native assistive-technology testing.

The framework working copies were read, never modified. Their uncommitted changes
are not part of Noton's committed dependency pins.
