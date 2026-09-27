# Claude Design prompt: Noton UI redesign

## Attach these files before pasting

Current prototype (from `docs/audit/2026-09-18/`):

- `1280x850/02-text-canvas.png`, `1280x850/04-ink-canvas.png`, `1280x850/05-reading.png`,
  `1280x850/07-table-canvas.png`, `1280x850/08-settings-modal.png`,
  `1280x850/11-main-fr-dark.png`, `600x700/03-reading.png`, `600x700/04-notes.png`

Teksilo widget anatomy (from `../teksilo/docs/widgets/img/`):

- `toolbar`, `button`, `icon_button`, `split_button`, `segmented_control`, `standard_item`,
  `list_view`, `search_field`, `text_input`, `card`, `rich_text`, `menu_list`, `menu_item`,
  `tooltip`, `splitter`, `status_bar`, `banner`, `snackbar`, `dialog`, `message_box`,
  `radio_tile_group`, `toggle`, `combo_box`, `slider`, `swatch`, `color_edit`, `badge`,
  `breadcrumb`, `group_header`, `form_layout`, `theme_switcher`, `language_switcher`,
  `text_scale_control`, `drop_zone` (all `.png`)

Reference: `../teksilo/docs/widgets-overview.md`

---

## Prompt (paste everything below this line)

Redesign the UI of **Noton**, a local-first desktop notebook. The attached screenshots show
the current prototype, a first draft whose job was to prove the backend. It works, but it
looks like Windows 95: every command is a grey text button and every button is always on
screen. I want a calm, modern design that I can build **with an existing native Rust GUI
toolkit (Teksilo)**. It is not a web app, so read the constraints before you start drawing.

### 1. What Noton is

- **Two levels only: Folder → Note.** A *library* is one file, opened one per window. It is
  not a third level. There are no subfolders, notebooks, sections or tags.
- **A note is a free canvas of movable blocks**, like OneNote:
  - **Text blocks**: rich text with headings, bold/italic/underline/strikethrough, bullet and
    numbered lists, links, alignment, and **tables inside the text**. Checklists are planned
    too.
  - **Image blocks**, with alt text or an explicit "decorative" flag.
  - **Sketch blocks**: a self-contained drawing surface with vector ink strokes that stay
    inside the block. Each sketch has a title and a text description, which is its screen
    reader equivalent. Mouse drawing works everywhere; pen pressure where the hardware
    supports it.
  - A block is moved by dragging its **header**. Once selected, it gets a **selection frame
    with resize handles**. Text blocks only resize in width: their height follows the
    content. Text blocks never rotate.
- **Two views of the same note:**
  - **Canvas** is the spatial layout.
  - **Reading view** shows the same blocks stacked vertically in a stored *reading order*,
    and they remain fully editable there. The reading order is independent of x/y position
    and the user can reorder it.
  - Switching views is not an edit, and nothing moves on the canvas.
- **Autosave** is always on. The user sees the state (*Saved / Saving… / Save failed*).
  "Save now" is a menu command, not a main button.
- **Trash and restore** for folders, notes and blocks, plus an explicit "Delete permanently".
- **Search** across the library, accent- and case-insensitive (`cafe` finds `Café`).
- **Export** to HTML or plain text in reading order. **Native backup** of the library file.
- **English and French**, **light and dark**, on **Linux, Windows and macOS**. The app draws
  a custom title bar with a hamburger menu on Linux and Windows. On macOS the same menu goes
  into the native menu bar and the window has native traffic lights.
- **Audience:** people who want a lightweight notebook for meeting notes, research,
  planning and quick sketches. It is not a pro design tool.

### 2. What's wrong with the prototype (see screenshots)

1. **Nothing is prioritised.** Fourteen formatting buttons sit in two rows, always visible,
   even with no text focused. Block, view and note commands all share the same look.
2. **No state is visible.** Bold doesn't look pressed when it's on. The current view
   (Canvas or Reading) isn't marked. Settings choices don't show which one is selected.
3. **Every block card is buried under controls**: *Move left / Move right / Move up / Move
   down / Make wider / Make narrower / Trash block*, then *Draw / Erase stroke / Select* on
   sketches. The content ends up smaller than the controls.
4. New blocks land on top of existing ones instead of in free space.
5. The folder and note lists only show titles. Rename fields and *Trash folder / Trash note*
   sit at the bottom of the lists. The footer mixes *Back / Save now / Show trash /
   Settings*. The hamburger floats alone in an empty title strip.
6. There are no icons, no empty states, no type hierarchy, and no identity. Dark mode is the
   same grey rectangles, inverted.
7. At 600 px wide, the canvas clips cards at the edge of the window.
8. Missing UI: a **block navigator** to reach off-screen blocks, sketch naming, checklist
   and link editing, and a trash view with permanent delete.

### 3. Design direction

- **Content first.** The note is the hero, and the chrome steps back. A light app should
  feel light.
- **Give it an identity.** It should not look like the neutral IntelliJ-style default theme
  in the widget images, and not like the prototype. Think of a quiet paper notebook with one
  confident accent colour.
- **Progressive disclosure:**
  - The formatting toolbar is contextual to the focused text block, with active states.
  - Block commands live in the card header's `⋮` menu and on the keyboard, not as permanent
    buttons.
  - Drawing tools appear only while a sketch is in drawing mode.
- Use icons with text labels where there is room. Icon-only buttons always have a tooltip
  and an accessible name. Toolbars overflow into a `⌄` menu instead of wrapping onto a
  second row.
- **Before building the full set**, show **two distinct visual directions** applied to
  Screen 1 (light), side by side and each with a short name. Then build everything in the
  one you recommend. I'll redirect you if I disagree.

### 4. Hard constraints: it must be buildable in Teksilo

**Widgets.** Every element must map to an existing Teksilo widget. The *appearance* is fully
themable through tokens and per-widget style traits, so restyle freely. The *structure and
behaviour* are fixed. The attached widget images show anatomy in the neutral default theme:
**use them for structure, not for style.** Available widgets relevant to Noton:

- **Shell:**
  - `TitleBar` (custom title bar: drag region, window controls) and `MenuBar`/`MenuModel`
    (the hamburger menu).
  - `Splitter` (resizable, collapsible panes) and `StatusBar`.
  - `Breadcrumb` (collapses the middle items when narrow) and `Toolbar`.
  - `CommandPalette` (type-to-run any command).
- **Lists:** `ListView` with `StandardListItem` rows. A row has a leading slot (icon), a
  label, an optional subtitle line with its own leading and trailing slots, and a trailing
  slot for a badge or meta text. Rows support selection, type-ahead, drag reorder and
  context menus.
- **Toolbar details:** actions are icon, label or icon plus label. Toggles have a pressed
  state. Separators and flexible space are available, and excess items overflow
  automatically into a `⌄` menu.
- **Buttons:**
  - `Button` in 7 variants: Filled, Tinted, Outlined, Plain, Ghost, Link, Destructive.
  - `IconButton` in 5 sizes: Compact, Default, Toolbar, Large, Hero.
  - `SplitButton`, `PopoverButton` and `PopoverIconButton`.
- **Choices:**
  - `SegmentedControl`, `Toggle` (Switch, Pill, Square or Inset) and `Checkbox`.
  - `RadioButton` and `RadioTileGroup` (selectable cards: icon, title, description).
  - `ComboBox` and `Slider`.
  - `ColorSwatch` and `ColorEdit`, for ink colour.
- **Text:**
  - `TextInput` in 4 variants: Outlined, Filled, Underline, Bare.
  - `SearchField` (magnifier and clear button).
  - `RichTextEditor`, the in-block editor. It supports headings, lists, blockquote, tables,
    links, alignment and images.
- **Overlays:**
  - `MenuList` and `MenuItem`: icon or check or radio items, right-aligned shortcuts,
    submenus and separators.
  - `Popover` (anchored, with an optional caret) and `TooltipWidget` (plain, or rich with a
    shortcut chip).
  - `Dialog` (modal) and `MessageBox` (confirmations with platform button order).
  - `Toast`/`Snackbar` (with an action such as *Undo*) and `Banner` (persistent inline
    info, warning or error).
- **Containers:** `Panel` (Plain, Sunken, Raised, Highlighted), `Card` (Plain, Elevated,
  Outlined, Filled), `GroupHeader` (a section label with a rule), `FormLayout` and `Badge`.
- **Canvas** (`teksilo-scene`):
  - `SceneView` with pan and zoom.
  - `SceneCard`: the block chrome, with a header that works as the drag handle and a
    trailing `⋮` slot.
  - Card states: idle, selected (a transform frame drawn *outside* the card, with handles)
    and editing.
  - `SceneMinimap`, a thumbnail of the canvas with the viewport rectangle.
- **Settings presets:** `ThemeSwitcher`, `LanguageSwitcher`, `TextScaleControl` (80–200 %)
  and `ShortcutSettings`.
- **Files:** `DropZone`/`DropTarget` for dropping image files onto the canvas.

If you need something that isn't on this list, label it **CUSTOM** in the annotations and
keep it simple.

**Visual effects.** Use solid fills, 1 px borders, corner radii, drop shadows (4 elevation
levels), subtle opacity fades, slides and height/width reveals. Motion must have a
reduced-motion fallback. **Avoid** web-only effects: backdrop-filter glass, gradient text,
blend modes, parallax, and anything that depends on CSS. Gradients are fine only as a minor
accent.

**Colour tokens.** Deliver the palette keyed to Teksilo's token names, for light and dark.
The main ones:

- **Surfaces:** `surface_main`, `surface_content`, `surface_raised`, `surface_sunken`,
  `surface_hover`, `surface_pressed`, `surface_selected`, `surface_selected_inactive`,
  `surface_disabled`.
- **Text:** `text_primary`, `text_secondary`, `text_disabled`, `text_on_accent`,
  `text_link`, `text_error`, `text_warning`, `text_success`.
- **Accent:** `accent`, `accent_hover`, `accent_pressed`, `accent_subtle_bg`.
- **Borders:** `border`, `border_strong`, `border_focused`, `divider`.
- **Status:** `status_{info,success,warning,error}_{fg,bg}`.
- **Selection and search:** `selection_bg_active`, `selection_bg_inactive`,
  `search_match_bg`, `search_match_current_bg`.
- **Other:** `tooltip_bg`, `tooltip_text`, and the scrollbar thumb and track colours.

**Shape tokens:** `radius_control`, `radius_popup`, `radius_pill`, `border_width`,
`focus_ring_width`, `focus_ring_offset`, `shadow_xs`/`sm`/`md`/`lg` (offset, blur, spread,
colour). For spacing, give me a small scale (control gap, section gap, panel padding).

**Typography.**

- The UI chrome has **exactly six text roles**: `body`, `body_bold`, `small`, `small_bold`,
  `tiny` and `mono`. There are no UI heading styles, so a section header is `body_bold` with
  spacing.
- The defaults are Inter and JetBrains Mono (13 px body). You may propose another font if
  it's open-licensed (OFL) and embeddable.
- **Note content typography is a separate set:** note title, H1–H3, paragraph, list, table
  and caption. Define it as its own editor ramp.

**Icons.** Choose one open-licensed SVG icon set (for example Lucide, Tabler or Phosphor)
with a consistent stroke at 16 and 20 px. Icons are tinted by the theme. List every icon
you use.

**Out of scope. Don't design these, not even as placeholders:** tags, sharing,
collaboration, sync, accounts, AI features, templates, file attachments, audio recording,
OCR, subfolders, page version history, ink drawn across several blocks, and rotating text
blocks.

### 5. Accessibility (a release requirement)

- **Every pointer action needs a keyboard and a menu equivalent.** That covers creating,
  selecting, moving, resizing, reordering the reading order, trashing and restoring. For
  example, arrow keys nudge a selected block, a modifier makes larger steps, and the `⋮`
  menu offers *Position and size…* with numeric fields. Show these shortcuts in the menus.
- **No hover-only affordances.** Anything revealed on hover must also be reachable by
  keyboard or menu.
- **No colour-only information.** Selected, active, error and trashed states each need a
  second cue: shape, icon, weight or text.
- **Focus is always visible and distinct from selection.** This matters most on canvas
  cards, where focus, selection and editing can all apply at once.
- WCAG AA contrast: 4.5:1 for text and 3:1 for UI boundaries and focus rings, in both
  themes. List the ratios for your main text/background pairs.
- The layout must survive **200 % text scale** and **French**, whose strings run about
  20–30 % longer (*Mettre le bloc à la corbeille*, *Ajouter une colonne après*). Controls
  must not overlap or truncate important labels.
- Hit targets are at least 24×24 px; prefer 28–32 px in toolbars.
- **F6 / Shift+F6** cycle the regions: menu, then Folders, then Notes, then the note.
  Escape leaves transient UI.
- **Undo depends on focus:**
  - The focused text block undoes its own text.
  - A selected block undoes page structure: move, resize, insert, trash.
  - A focused folder or note list undoes organisation.

  The UI must make the target obvious. Menu labels say *Undo typing*, *Undo move block* or
  *Undo rename folder*, never a bare *Undo*.

### 6. Screens

Use realistic content, not lorem ipsum, and mix French and English the way a
French-speaking user would: accents, one emoji, a real table. For example, folders
*Travail*, *Perso* and *Recherche*, and notes *Réunion projet — 18 sept.*, *Planning T4*,
*Idées appli* and *Lectures*.

**P1: must have**

1. **Main window, Canvas view, 1280×850, light.**
   - Show the title bar (hamburger, app name, library name, window controls), the Folders
     and Notes panes, the note title, the view switch (Canvas | Reading), and a canvas with:
     - a text block (heading and bullets);
     - a text block containing a 3×3 table of decisions;
     - a sketch block (a small diagram);
     - an image block.
   - Show **one block selected** (with its transform frame and handles) and **one text block
     in edit mode**, with the contextual formatting toolbar showing **Bold active**.
   - Show how the save state is displayed.
2. **The same note in Reading view.** Same blocks, stacked in reading order and still
   editable, with an affordance to reorder them. Wide tables scroll horizontally, with a
   label.
3. **Sketch block in drawing mode.**
   - Tools: Draw, Erase (whole stroke), Select and Pan, plus a small set of brush widths
     and colours.
   - The sketch title and a *Describe this sketch* field (the screen reader equivalent).
   - A hint that Escape cancels the current stroke.
   - How the user leaves drawing mode.
4. **The block `⋮` menu:**
   - *Position and size…*, *Bring to front*, *Send to back*;
   - *Read earlier* and *Read later* (reading order);
   - *Duplicate*;
   - *Edit alt text…* (image) and *Rename / describe sketch…* (sketch);
   - *Move to trash*.

   Include keyboard shortcuts, plus the *Position and size* popover (X, Y, width).
5. **Folder and note lists in detail.**
   - Note rows show the title, a snippet or modified date, and meta.
   - Show the selected row, keyboard focus, inline rename, the context menu (*Rename*, *Move
     to folder…*, *Move to trash*), the drag-reorder drop indicator, the new folder and new
     note affordances, and the counts.
6. **Status and feedback:**
   - The *Saving…* and *Saved* states.
   - *Save failed* as a persistent `Banner` with *Retry* and *Save a copy…*.
   - A *Note moved to trash · Undo* toast.
7. **Settings (modal dialog, changes apply instantly):**
   - Language: System, English, Français.
   - Appearance: System, Light, Dark.
   - Text size (80–200 %).
   - Reduce motion: System, On, Off.
   - Default note view: Canvas or Reading.
   - Backup location and retention.
   - *Keyboard shortcuts…*.

   The current choice must be unmistakable. Include Done and Close.
8. **Narrow window, 600×700.**
   - Navigation stacks into one column: Folders → Notes → Note, with Back or a breadcrumb.
   - Show the note list step and the note step.
   - Propose how the canvas behaves at this width: zoom to fit, pan, or an offer to switch
     to Reading view. Never switch the user's view setting behind their back.

**P2: after P1**

9. **Block navigator.**
   - A list of every block in reading order (type icon and first line). Clicking an entry
     reveals and focuses that block on the canvas, and entries can be reordered by drag or
     `Alt+↑/↓`.
   - Also the minimap and zoom controls (zoom %, Fit, 100 %).
   - You decide where it lives: a collapsible side panel or a popover.
10. **Inserting content:**
    - Click on empty canvas to start a text block.
    - An Insert control for Text, Table, Sketch and Image.
    - New blocks are placed in free space.
    - Drop an image file onto the canvas.
11. **Search:**
    - Results show the note title, the folder, and a snippet with the match highlighted.
    - A no-results state.
12. **Trash view:**
    - Trashed folders, notes and blocks, each with its origin and date.
    - *Restore*, and *Delete permanently* with a confirmation `MessageBox`.
    - *Empty trash*.
13. **App menu open** (hamburger):
    - File: *Open library…*, *New library…*, *Export as text…*, *Export as HTML…*, *Save
      now*, *Back up now*, *Close*.
    - Edit: the named undo and redo.
    - Insert and View.

    Also the `CommandPalette` (Ctrl+K).
14. **Empty states:** a new empty library, an empty folder, and an empty note that invites
    "click anywhere to write".
15. **Variants:**
    - Screens 1, 3 and 7 in **dark**.
    - Screens 1 and 7 in **French**.
    - Screen 1 at **200 % text scale**.
    - Screen 1 as a **macOS** window (traffic lights, no hamburger).

### 7. Deliverables

1. **A design system page:**
   - the light and dark palettes by token name, with hex values and contrast ratios;
   - the shape, shadow and spacing tokens;
   - the UI type roles and the content type ramp;
   - the icon set and the list of icons;
   - a component sheet showing every component you use in its states: default, hover,
     pressed, focused, selected, disabled, and error where relevant. Include the canvas card
     states: idle, hover, selected, editing, and focused-while-selected.
2. **The screens** above at their real pixel sizes.
3. **Annotations** on each screen naming the Teksilo widget and variant behind every
   non-obvious element. For example: "`Toolbar`, icon + label, overflow",
   "`SegmentedControl`", "`StandardListItem` with subtitle", "`Button`: Ghost", "CUSTOM:
   …". Add one or two lines of keyboard and screen reader notes per screen.
4. **A short changes list** comparing the design to the prototype.
5. **A theme handoff table** in plain form that I can paste into a Rust `Theme` override,
   for example `colors.accent = #1F6FEB` and `shape.radius_control = 6`.

Don't write Rust. Keep the HTML and CSS of your mockups within the visual-effects limits
above, so that what you show is what I can build.
