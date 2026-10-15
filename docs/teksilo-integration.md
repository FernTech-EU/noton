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
