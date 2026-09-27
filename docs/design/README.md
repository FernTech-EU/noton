# Carnet design boards

These files are the Claude Design canvas "Noton UI redesign", exported on
27 September 2026 from the brief in [`../claude-design-prompt.md`](../claude-design-prompt.md).
Each `.dc.html` file is one artboard; `canvas.json` holds their layout and the
widget and keyboard notes that sit under each screen. Direction A, **Carnet**,
is the one implemented.

| Board | What it specifies | Where it lives in the code |
| --- | --- | --- |
| `Handoff.dc.html` | Light and dark colour tokens, shape, shadow, spacing and type | `crates/teksilo_ui/src/theme.rs` |
| `DesignSystem.dc.html` | Components and their states | `theme.rs`, `widgets.rs`, `resources/icons/` |
| `Main.dc.html` | Screen 1: title bar, panes, canvas, contextual toolbar, status bar | `lib.rs`, `app.rs`, `panes.rs`, `note_view.rs`, `page/`, `status.rs` |
| `S2-Reading.dc.html` | Screen 2: reading view | `page/mod.rs` (`reading`) |
| `S3-Sketch.dc.html` | Screen 3: drawing mode | `page/ink.rs`, `page/mod.rs` (`sketch`) |
| `S4-BlockMenu.dc.html` | Screen 4: block menu, Position and size | `page/mod.rs` (`block_menu`, `canvas_keys`), `dialogs.rs` |
| `S5-Lists.dc.html` | Screen 5: folder and note lists | `panes.rs`, `models/navigation.rs` |
| `S6-Status.dc.html` | Screen 6: save state, Save failed banner, Undo snackbar | `status.rs`, `note_view.rs` |
| `S7-Settings.dc.html` | Screen 7: settings | `settings.rs` |
| `S8a-…`, `S8b-….dc.html` | Screen 8: narrow window | `app.rs` (`narrow_bar`), `note_view.rs` |
| `DirB-Atelier.dc.html` | Direction B, not chosen | — |

## Where the build differs from the boards

- `border_strong` is `#877F72` in both themes, not the handoff's `#8F877A` /
  `#7A7266`: those fall just under 3:1 on the selected-row fill. The contrast
  test in `theme.rs` checks every text and boundary pair in both themes.
- Note rows show the block count where the boards show a time: the library
  file does not store modification dates yet.
- A sketch's paper stays light in the dark theme, so saved ink colours keep
  their meaning.
- Appearance *System* is read when Noton starts and when it is chosen;
  Teksilo's own OS-following mode swaps in its built-in themes, so it is not
  used.
- Reduce motion *On* removes Noton's animation durations; *System* and *Off*
  both leave Teksilo following the OS setting.
- The drawing tools are icon-only (labels are tooltips and accessible names)
  so the bar fits a 600 px window.

## Not built yet

- From the boards: the size chip beside a selected card, the checklist button
  (the text engine has no checklist list style), moving selected strokes, and
  fading the other blocks while drawing.
- The P2 screens: block navigator and minimap, click-to-write on empty canvas,
  image drop, search snippets with highlighted matches, the trash view with
  permanent delete, the command palette, and the macOS and 200 % variants
  as checked screens.
