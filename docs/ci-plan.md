# Cross-platform CI based on Skribisto

Owner decision: Noton supports **Linux, Windows and macOS**. Reuse Skribisto's CI and
packaging design. This is a planning session with no Cargo application yet: this document
specifies the adaptation; no nonfunctional workflow referencing absent crates is installed.

Source: `/home/cyril/Devel/skribisto/.github/` at
`85d1eb2539c99676223b5785976468e3f30ff3bb`, plus its checked-out scripts and package metadata.
Do not copy Skribisto's project identity, credentials, endpoints or repository destinations.

## Source-to-Noton mapping

| Skribisto source | Noton adaptation |
|---|---|
| `.github/workflows/ci.yml` | Required CI aggregate; Linux real/mocks checks, Windows/macOS native builds and tests, fmt, clippy, docs, locale, lockfile and SPDX checks |
| `.github/actions/install-linux-deps/action.yml` | Reuse lean winit/wgpu packages: build tools, pkg-config, xkbcommon, Wayland, XCB and X11; do not introduce WebKit/GTK for an app that does not use web views |
| `tools/check_locales.py` and checker unit tests | Adapt locale root to Noton; compare EN/FR keys, attributes and interpolation variables |
| `tools/check_spdx_headers.py` | Adapt project identity and agreed license; retain original notices on reused code |
| `.github/workflows/audit.yml` | Dependency security audit with read-only permission |
| `.github/workflows/rust-next.yml` | Stable gate plus advisory beta/nightly canary; same target matrix |
| `.github/workflows/packaging-lint.yml` | Validate Noton desktop entry, AppStream, MIME type/icon consistency and Flatpak exports |
| `.github/workflows/release.yml` | Validate tag then build platform artifacts; aggregate explicit artifact names and checksums; use Noton release destination |
| `.github/workflows/release-macos.yml` | Separate ARM64/x86_64 builds, per-target caches, universal `lipo` assembly, executable-bit and architecture checks, app/DMG packaging |
| `.github/workflows/flatpak.yml` + `package/flatpak/gen-cargo-sources.sh` | Locked dependency vendoring, offline build, bundle validation and exported metadata/icon checks |
| `package/windows/setup.nsi` | Noton installer and `.noton` file association; explicit NSIS installation; separate portable ZIP |
| `scripts/automation_fixture.py` and automation examples | Adapt reusable launch/attach/isolation/protocol mechanics, implement Noton's actual UI scenarios |

Reuse retains the relevant source license notices; Noton's own identity/license metadata
must be supplied consistently before generation/packaging. No claim of signing or store
publication follows from copying a workflow.

## Necessary corrections and deliberate differences

1. **Trigger on Noton's `main`.** Skribisto's current `ci.yml` and packaging guard still
   list `master`; its guide says work is on `dev`. Blind copying could leave Noton pushes
   untested. PRs and `main` pushes must both run.
2. Use `--locked` for Cargo build/test/check/doc jobs, as well as the dedicated
   `cargo metadata --locked` gate. Local sibling `[patch.crates-io]` overlays are development
   only and must not silently rewrite the committed lockfile. Approved unreleased upstream
   dependencies need an explicit reproducible source pin and vendoring path before release.
3. Keep one required aggregate status that fails when any required dependency failed,
   cancelled or unexpectedly skipped. Avoid path-filtering the entire required workflow
   into a permanently pending status; cheap docs/manifest validation can run on every PR.
4. **Test the real and mock UI configurations on all three native runners.** Skribisto
   currently exercises mocks on Linux; Noton adds the corresponding Windows/macOS checks
   because the owner requires all three. Scope `--features mocks` to the UI package.
5. Remove the Skribisto-only `pdf` feature leg unless PDF export becomes selected. Tables
   and ink do not require its heavyweight Typst PDF exporter.
6. Do not inherit blanket `-A deprecated` or weak documentation lint policy without
   evidence. Start warning-clean for Noton-owned code and use targeted documented allowances
   only for unavoidable generated/dependency surfaces.
7. Add Qleany manifest validation, supported-format round trips, ink replay/cancellation,
   real-backend autosave/recovery, and app-specific MCP headless scenarios to the aggregate.
   A passing toolkit demo is not passing Noton UI automation.
8. Keep native OS tests distinct from target compilation. Building Intel macOS from an
   ARM runner proves the binary builds, not that Intel runtime/AT interaction passed.
   Report architecture-level smoke coverage accurately alongside platform support.
9. Do not copy Skribisto website dispatches, in-app update feeds, Flathub fork/PR destinations
   or SignPath project identifiers. Configure only Noton-owned destinations. Publishing is
   outside this planning session.
10. Pin action revisions and packaging tool versions when implementing the workflows;
    record the reviewed source version. Do not infer current hosted-runner capabilities
    from comments—install required packaging tools explicitly and test a cold build.

## Native validation matrix

| Platform | Required build/test | Required GUI/AT checks | Release artifacts inherited from Skribisto design |
|---|---|---|---|
| Linux | Stable Rust, real/mocks, headless MCP, file/recovery tests | Wayland and X11 launch/input/IME; Orca; portal file dialogs; capability-aware pen path | Linux archive + Flatpak |
| Windows | Stable Rust/MSVC, real/mocks, headless MCP, file/lock/recovery tests | Native launch, clipboard/IME, NVDA, title bar/menu, named-pipe automation | x86_64 installer + portable ZIP; static CRT as appropriate |
| macOS | Stable Rust, real/mocks, headless MCP, file/recovery tests; both release architectures built | Native menu, clipboard/IME, VoiceOver, window controls; ARM/Intel smoke evidence recorded separately | Universal ARM64/x86_64 app/DMG |

Native screen-reader checks remain a release checklist with recorded results where a hosted
runner cannot reliably automate them. This is a required test method, not a claim that MCP
substitutes for speech and OS adapters. The sole explicit hardware exception is physical
pen testing: user has accepted best-effort support. Mouse and synthetic-pen tests remain
required on every platform.

## Automation isolation

Use a fresh settings/data/temp library directory for each test. Reuse Skribisto's debug
config-pinning approach to launch a known locale/theme/density; unknown settings must fail
with a useful error. Native runtime/display paths remain intact (notably Linux
`XDG_RUNTIME_DIR`, which carries the display connection). Attach by the launched process ID.
If Noton adopts single-instance IPC, test both normal handoff and an explicit independent
instance mode; two launches that silently hand off cannot test competing file writers.

Upload failing MCP transcripts, semantic/layout snapshots, screenshots when renderable,
application logs and isolated test-library copies without real user content. Collect real
save acknowledgements, not arbitrary sleeps. Unit-test the helper scripts/checkers too.

## Packaging gate

A tag must not publish until all required platform artifacts exist and pass their validation.
Collect by expected artifact name, not a broad glob that also captures unsigned staging
binaries. Produce checksums and verify file associations/icons. Verify debug automation is
absent from shipped binaries. Preserve `chmod +x` after macOS artifact transfer and verify
both universal architectures. Smoke-launch installed/extracted binaries rather than only
checking that package files exist.

Signing follows the owner's available credentials: use Noton-configured signing/notarization
when present; otherwise explicitly identify unsigned/ad-hoc artifacts and test the resulting
installation path. Do not silently report an unsigned package as signed or notarized.
