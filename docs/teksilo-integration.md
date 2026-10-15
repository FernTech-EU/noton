# Teksilo dependency integration

Noton builds from published crates.io releases. No sibling repositories, Git
checkouts, or `[patch.crates-io]` source overrides are needed.

The current baseline is Teksilo and teksilo-scene **0.18.0**, text-document
**1.13.0**, and the transitive text-typeset **2.0.0** dependency. `Cargo.lock`
pins the resolved registry versions and checksums. `text-typeset` supplies the
text layout used by Teksilo; it is not a direct Noton dependency.

## Build and verification

```sh
cargo build --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p noton-teksilo-ui --features mocks --locked
cargo clippy -p noton-teksilo-ui --features mocks --all-targets --locked -- -D warnings
# Restore the production binary after mock checks.
cargo build -p noton-teksilo-ui --locked
python3 tools/check_locales.py
python3 tools/mcp_smoke.py target/debug/noton
python3 tools/mcp_stories.py target/debug/noton target/user-story-results
python3 tools/mcp_table_accessibility.py target/debug/noton
```

CI uses the same registry dependencies and lockfile. The former source bootstrap
script and Git revision manifest have been removed. Existing development
checkouts must remove any old `.cargo/config.toml` source overrides before building
against these releases.

For native automation, install a client matching the framework and use fresh
output directories:

```sh
cargo install teksilo-automation-mcp --version 0.18.0 --locked --root target/automation
PATH="$PWD/target/automation/bin:$PATH" python3 tools/mcp_ergonomics.py target/debug/noton /tmp/noton-ergonomics-new
PATH="$PWD/target/automation/bin:$PATH" python3 tools/mcp_visual_audit.py target/debug/noton /tmp/noton-visual-audit-new
PATH="$PWD/target/automation/bin:$PATH" python3 tools/check_native_scheduling.py target/debug/noton /tmp/noton-native-scheduling-new
```

Native scripts require a desktop session and drive only the processes they launch.
The ergonomics suite verifies initial keyboard focus, direct writing, navigation
and match selection through the native event loop. Teksilo 0.18.0 headless MCP
does not drain mount callbacks, so it cannot validate initial-focus behavior.
A passing table probe does not certify native Orca, NVDA or VoiceOver behavior.

## Verification results

See [the October update and project review](review-2026-10-15.md) for results and
open findings, including the registry verification follow-up. The
[September user-story and visual audit](ui-audit.md) is historical coverage.

## Image drops and frame controls

`cargo test -p noton-teksilo-ui --test image_drop --locked` exercises the native
external-drag entry points on the production widget tree. Coverage includes
Wayland-style advertised formats followed by file payloads, multiple independent
frames, zoom/pan coordinates, raw image data, empty-library creation, persistence,
undo, invalid data and selected-frame Delete. It does not simulate dragging from
a separate file-manager process through the operating system.

For native pointer/keyboard checks, generate a disposable fixture and run:

```sh
cargo run -p noton-teksilo-ui --example image_drop_fixture --locked -- /tmp/image-drop-fixture.noton
PATH="$PWD/target/automation/bin:$PATH" python3 tools/mcp_image_frames.py target/debug/noton /tmp/image-drop-fixture.noton /tmp/image-frame-audit
```

The native script verifies header dragging, resize handles, persisted geometry,
Delete and undo, immediate sketch deletion, and text deletion versus frame deletion.
It copies the fixture before making changes. Teksilo 0.18 header drags produce
Programmatic/Record transactions; Noton records these alongside User/Record
transforms. App-driven geometry synchronization uses Ignore and remains excluded.

Verification for this change: 178 workspace tests and 26 mock tests passed, with
three pre-existing ignored doctests. Strict real/mock Clippy, formatting and locale
parity passed. Smoke, all 17 existing stories, all six native ergonomics scenarios,
and the new native frame-control script passed. Native selected and resized image
captures were visually inspected. Evidence is in `target/audit-image-drop/`.
