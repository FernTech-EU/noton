# Choosing Noton's page model

Updated after the owner's selections: **tables and best-effort ink included**, Linux,
Windows and macOS required. **Owner decision: A with movable sketch blocks for ink.** This comparison
is based on current Teksilo documentation and source; its referenced tests were inspected,
not executed during this planning session.

## A. Free canvas with an editable linear view — selected

A note is a collection of movable text/table, image and drawing blocks. Click an empty
area to start a text block; drag its handle to reposition it, resize its width to reflow
text. Tables live inside rich-text blocks. Drawings retain vector strokes.

Example: put meeting notes on the left, a table of decisions on the right, and a sketch
below. They stay where placed. Growing the notes can overlap another block; the app must
make that understandable rather than quietly repositioning the rest of the canvas.

“Linear view” means presenting those **same blocks** vertically, in a stored reading
order. Editing the meeting notes there edits the same TextDocument, not an exported copy.
Text adapts to available width; wide tables get a labelled horizontal scroller. Drawings
remain drawings with accessible titles/descriptions and editing commands. Switching back
restores the original canvas coordinates. View changes create no history entry.

This is not two note formats. It is one format with spatial coordinates plus reading order,
rendered by a SceneView and a normal flowing layout. Select/caret restoration refers to
block UUID and document character offset; it does not reuse widget IDs across rebuilt views.

**What Teksilo already provides:**

- `SceneModel`, `SceneView`, shared payloads and per-view widget delegates.
- Actual `RichTextEditor` children with tables, selection, caret, clipboard and IME logic.
- `SceneCard::height_for_width`: stored width, content-driven height; horizontal resizing
  previews live reflow. Vertical resize intentionally has no effect for these cards.
- Canvas pan/zoom, selection, transform sessions, reversible scene transactions and
  explicit focus/accessibility controls.
- Regression tests for a real embedded rich editor revealing its caret through the scene.
- `PathItem` vector rendering/hit testing; pointer and synthetic pen input primitives.

**What Noton must implement:** the block/document adapter, UI composition, reading order,
the linear view, drawing tools, persistence, history routing and the complete test matrix.
These are application work; Teksilo does not supply a complete OneNote editor.

**Limits:** scene culling parks but does not demand-load all heavyweight editors; keep the
scene scoped to the active page and measure its size envelope. Heavyweight rotation is
not supported by the transform controller—do not offer Rotate on text/table blocks.
Canvas AT still needs validation; a linear view is not permission to leave it inaccessible.

### The important ink choice inside A

The accepted ink contract is self-contained, movable drawing blocks. They can be positioned
anywhere and edited in place in drawing mode. Their strokes use local drawing coordinates,
so moving/resizing the block carries the sketch consistently into the linear presentation.

Drawing freely *across* several text blocks is a different feature. A circle around a word
will not follow that word when its paragraph reflows; x/y coordinates do not encode a text
anchor. Do not silently imply otherwise. Cross-block annotation would require an explicit
anchor model and a way to present the spatial relationship in the linear view. It is outside the accepted scope: the owner explicitly chose movable sketch blocks.

## B. Flowing document with embedded sketches — lowest integration risk

A note behaves like a conventional document: paragraphs, tables, images and sketch objects
run top to bottom. Adding a paragraph pushes the table and sketch down. Content naturally
reflows as the window or text size changes. There is no free placement of text blocks.

Use a main `TextDocument`/`RichTextEditor`. A sketch is a vector drawing asset represented
by an inline preview; activating it opens a Teksilo Scene drawing editor. The vector source
remains authoritative; a preview is a derivative, not the only saved copy. Teksilo's image
activation and custom asset resolution hooks are candidate integration seams, not a
ready-made embedded drawing object. Noton supplies the identity mapping and editor.

This makes the strongest use of Teksilo's existing rich-text/flowing-document surface and
Skribisto's practical experience. It simplifies reading order, text selection, navigation
and export. Scene is used for the sketch editor, satisfying the technology requirement,
but the primary note is not a OneNote-style spatial page.

Sketch mutations need an app-level history entry coordinated with the containing document:
undoing sketch insertion must not orphan later sketch edits, and redo must retain the latest
recoverable vector data. This is manageable but not automatically provided by image previews.

Choose B if “light” principally means a conventional writing tool with occasional diagrams,
and free placement is not important. It is the best fit for **minimum integration risk**,
not the best match to the original OneNote-like canvas idea.

## C. Separate Canvas and Document note types — not recommended

At note creation choose Canvas (A's spatial page) or Document (B's flowing document).
Each type gets its own editing rules. Switching type becomes a conversion operation:
canvas to document needs reading order and a policy for overlapping/annotated items;
document to canvas needs deterministic segmentation and placement. It is not a harmless
view toggle.

Teksilo can render each separately, but does not supply cross-format conversion or a
unified lifetime/history model. Noton would maintain two sets of clipboard, export,
accessibility, search-result navigation, migration and undo behaviours. This creates more
product surface than either A or B, without a demonstrated need for two document kinds.

## Comparison

| Criterion | A: Canvas + linear projection | B: Flowing document | C: Two note types |
|---|---|---|---|
| OneNote-style text placement | Yes | No | Canvas notes only |
| Tables | Existing rich editor inside each text block | Existing main rich editor | Both paths need validation |
| Ink | Positioned vector drawing blocks | Inline sketch preview + Scene editor | Both representations |
| Screen-reader reading order | Explicit stored block order, plus text/table semantics | Mostly document order | Two contracts |
| Reflow | Text within block; linear presentation reflows | Native main document flow | Type-dependent |
| Teksilo reuse | SceneCard + RichTextEditor + PathItem | RichTextEditor + Scene sketch editor | Both stacks |
| Noton-specific integration | Moderate/high | Lowest | Highest |
| Product recommendation | **Best fit for Noton** | Best if free placement is unwanted | Avoid without a concrete need |

## Best-effort ink is a precise support level

Current source (`teksilo-platform/src/pen.rs`) reports native pen shims on **Windows and
Linux/Wayland**. **macOS and Linux/X11 use NullPenSource**: mouse-emulated tablet events
may be available through the OS/device, but pressure/tool identity cannot be promised.
Read runtime backend capabilities; never infer pen availability solely from the OS name.

Provide working mouse drawing everywhere, pen input where supplied, a uniform-width
fallback when pressure is absent, and explicit Draw/Select/Pan/Erase modes. Use whole-stroke
erasure initially. A stroke is one Qleany command; an erase gesture is one command over
its affected stroke IDs. Escape/lost capture cancels provisional strokes; no partial
stroke leaks into autosave. Existing content remains accessible while drawing is off.

Store versioned vector samples (local x/y, time offsets, optional pressure/tilt), stroke
UUID/order and brush width/colour, together with the drawing's title and description.
`PathItem` does not itself turn pressure samples into a variable-width brush: Noton must
compute a filled stroke outline or another appropriate geometry. Persist samples and
brush version so rendering is deterministic and does not overwrite the original input.
Do not commit every pointer sample as a domain edit. Cap sample counts and validate finite
coordinates before serialization. Painting previews must remain cheap.

Keyboard/AT users can insert a drawing, name/describe it, select strokes via a list,
delete/restore, reposition/resize and change drawing properties. A freehand sketch does
not become understandable to a screen reader merely because its path is in AccessKit;
provide a textual equivalent, and expose/edit that equivalent. OCR, handwriting recognition,
automatic diagram descriptions, advanced palm rejection and tilt-dependent brushes are not
promised by this ink selection.

Best-effort applies to **unverified physical tablet behaviour**, not data integrity,
undo/redo, crash recovery, basic mouse drawing, or accessible controls. Synthetic pen
traces through MCP test pressure/missing-pressure, eraser selection where the current schema
supports it, cancellation and persistence. Lower-level tests cover fields MCP does not
expose. Record actual hardware testing as unavailable rather than fabricate a pass. This
is an explicit accepted limitation; it does not block releases for the absence of hardware.

## Accepted decision

The owner chose **“A” with movable sketch blocks for ink**. One note model stores spatial
geometry and explicit reading order; canvas and linear views share content and identities.
Tables remain editable rich-text content. Each sketch owns its vector strokes, bounds,
title and textual description. Strokes are clipped to that sketch and use its local
coordinates. There is no cross-block ink layer and no implicit anchoring to words.

Switching presentation preserves block identity, caret where applicable and history; it
neither converts document formats nor creates an undo entry. Moving a sketch changes its
canvas placement, not its stroke samples or reading order. Resizing its viewport transforms
its rendering without repeatedly rewriting the source samples. In linear view the sketch
keeps its aspect ratio and offers the same explicit editing commands; text reflow around
it never changes its internal geometry.

Physical tablet support remains best-effort as agreed. This decision settles page/ink
presentation; the session remains planning-only and does not authorise upstream edits.

## Evidence

- `/home/cyril/Devel/teksilo/docs/scene/scene_card.md:317` — height-for-width semantics.
- `/home/cyril/Devel/teksilo/crates/teksilo-scene/tests/scene_card_reveal.rs:137` — real-editor caret reveal case.
- `/home/cyril/Devel/teksilo/docs/widgets/rich_text.md:919` — table insertion and row/column APIs.
- `/home/cyril/Devel/teksilo/docs/scene/transform_session.md` — resizing and heavyweight rotation limits.
- `/home/cyril/Devel/teksilo/crates/teksilo-platform/src/pen.rs:30` — native pen support matrix.
- `/home/cyril/Devel/teksilo/docs/touch-and-pen.md:747` — stylus input, capabilities and test limits.
- `/home/cyril/Devel/teksilo/crates/teksilo-scene/src/items/path.rs` — vector path rendering/hit testing.
- `/home/cyril/Devel/skribisto/.claude/CLAUDE.md` — feature-first MVVM, stable documents,
  model/mock boundary, reactive UI, global menu actions, save and history contracts.
