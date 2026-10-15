pub mod format_bar;
pub mod ink;
use crate::{
    icons::Icon,
    session_vm::SessionVm,
    status::offer_undo,
    theme,
    widgets::{caption, icon_button},
};
use ::common::{
    entities::{Block, BlockKind},
    types::EntityId,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use teksilo::core::styles::{CardVariant, TextInputVariant};
use teksilo::tokens::{HAlignment, VAlignment};
use teksilo::widgets::rich_text::RichTextEditor;
use teksilo::{core::binding::BindingLevel, prelude::*, widgets::*};
use teksilo_scene::{
    CardMode, ChangeSource, HistoryMode, ItemChange, ItemFlags, ItemId, SceneCard, SceneEdit,
    SceneModel, SceneSelection, SceneSelectionMode, SceneView, TransformConfig, TransformHandleSet,
    TxnOutcome,
};

/// The canvas on screen, so geometry commands can move a card in place
/// instead of rebuilding the page (which would drop selection and focus).
#[derive(Clone)]
pub struct LiveScene {
    pub model: SceneModel,
    pub items: HashMap<EntityId, ItemId>,
    pub selection: SceneSelection,
}
impl LiveScene {
    /// Put the card of `block` where the store says it is.
    pub fn sync(&self, block: &Block) {
        if let Some(item) = self.items.get(&block.id).copied() {
            let _t = self
                .model
                .transaction(ChangeSource::Programmatic, HistoryMode::Ignore);
            self.model
                .set_local_pos(item, Point::new(block.x as f32, block.y as f32));
            self.model.set_local_bounds(
                item,
                Rect::new(0., 0., block.width as f32, block.height as f32),
            );
            self.model.set_z(item, block.z as f32);
        }
    }
}

#[derive(Debug)]
pub struct PageView {
    vm: SessionVm,
    root: Option<WidgetId>,
}
impl PageView {
    pub fn new(vm: SessionVm) -> Self {
        Self { vm, root: None }
    }
}

/// Attach the blank-canvas gesture to the scene itself, inside its scroll bars.
#[derive(Debug)]
struct WritingCanvas {
    vm: SessionVm,
    scroll: teksilo_scene::SceneScrollView,
    preserve_pan: Option<(f32, f32)>,
    handlers: Option<teksilo::core::widget_builder::HandlerSet>,
}
impl Widget for WritingCanvas {
    fn build(&mut self, ctx: &mut BuildContext) -> Vec<WidgetId> {
        let children = self.scroll.build(ctx);
        if let Some(scene) = children.first().copied() {
            let vm = self.vm.clone();
            ctx.run_after_mount(move |ec| {
                if vm
                    .0
                    .focus_frame
                    .take()
                    .is_some_and(|id| vm.0.selected.get() == Some(id))
                {
                    ec.request_focus(scene);
                }
            });
        }
        if let (Some(scene), Some(handlers)) = (children.first(), self.handlers.take()) {
            ctx.apply_handlers(*scene, handlers);
        }
        children
    }
    fn layout_response(&self, p: SizeProposal, ctx: &LayoutContext) -> LayoutResponse {
        self.scroll.layout_response(p, ctx)
    }
    fn place_children(
        &self,
        bounds: Rect,
        p: SizeProposal,
        children: &mut [teksilo::core::widget::WidgetPlacement],
        ctx: &LayoutContext,
    ) {
        if self.vm.0.focus_block.get().is_none() && self.vm.0.focus_in_place.take().is_some() {
            // Focus can reveal the whole editor before its first paint. Restore
            // the click's camera before laying out that newly focused frame.
            if let (Some((x, y)), Some(camera)) =
                (self.preserve_pan, self.vm.0.camera.borrow().as_ref())
            {
                camera.pan_x.set(x);
                camera.pan_y.set(y);
            }
        }
        self.scroll.place_children(bounds, p, children, ctx);
    }
    fn children(&self) -> Vec<WidgetId> {
        self.scroll.children()
    }
}

fn kind_icon(kind: &BlockKind) -> Icon {
    match kind {
        BlockKind::RichText => Icon::Text,
        BlockKind::Drawing => Icon::Pen,
        BlockKind::Image => Icon::Image,
    }
}

/// The card is the chrome: the editor draws only its text.
struct Frameless;
impl teksilo::core::styles::RichTextEditorStyle for Frameless {
    fn make_body(
        &self,
        cfg: &teksilo::core::styles::RichTextEditorStyleConfig,
        ctx: &mut BuildContext,
    ) -> WidgetId {
        ctx.add(Padding::symmetric(2., 4.).child(cfg.viewport))
    }
}

fn text_editor(
    vm: &SessionVm,
    id: EntityId,
    mode: Signal<CardMode>,
) -> anyhow::Result<Box<dyn Widget>> {
    let doc = vm.document(id)?;
    let pending = Rc::downgrade(&vm.0);
    let editor = RichTextEditor::editor(doc)
        .style(Frameless)
        .typography_defaults(teksilo::text::EditorTypographyDefaults {
            font_family: Some(theme::CONTENT_FAMILY.into()),
            line_height: 1.45,
            paragraph_spacing_after: 4.,
            ..Default::default()
        })
        .min_lines(2)
        .on_change(move || {
            if let Some(v) = pending.upgrade() {
                v.pending.borrow_mut().insert(id);
                v.wake.notify();
            }
        });
    let handle = editor.handle();
    let text_surface = Rc::new(handle.clone());
    let weak = Rc::downgrade(&vm.0);
    let card_mode = mode.clone();
    let focus = editor.focused_signal().observe(move |active| {
        if *active {
            // Typing makes the card Editing, so its Escape hands the keyboard
            // back to the card and arrows move the block.
            card_mode.set_if_changed(CardMode::Editing);
            if let Some(inner) = weak.upgrade() {
                inner.edit.claim_text(text_surface.clone());
                inner.active_text.set(Some(id));
                inner.active_block.set(Some(id));
                if inner.drawing.get().is_none() {
                    inner.editing.set_if_changed(Some(id));
                }
                inner.format_dirty.set(true);
                inner.wake.notify();
            }
        }
    });
    // These fire inside the editor's own mutation, where reading its format
    // would re-borrow it: mark the toolbar stale and let the tick sync it.
    let stale = |signal: Signal<u64>| {
        let weak = Rc::downgrade(&vm.0);
        signal.observe(move |_| {
            if let Some(inner) = weak.upgrade() {
                if inner.editing.get() == Some(id) {
                    inner.format_dirty.set(true);
                    inner.wake.notify();
                }
            }
        })
    };
    let weak = Rc::downgrade(&vm.0);
    let caret = handle.cursor_position_signal().observe(move |_| {
        if let Some(inner) = weak.upgrade() {
            if inner.editing.get() == Some(id) {
                inner.format_dirty.set(true);
                inner.wake.notify();
            }
        }
    });
    let formats = stale(handle.format_version());
    // Focus leaving the card (for the toolbar itself) must not close the
    // toolbar; Escape does, in `canvas_keys`.
    vm.0.format_observers
        .borrow_mut()
        .insert(id, vec![focus, caret, formats]);
    if let Some(old) = vm.0.handles.borrow().get(&id) {
        let (a, p) = old.selection();
        handle.select_range(a, p);
    }
    vm.0.handles.borrow_mut().insert(id, handle.clone());
    Ok(Box::new(FocusEditor {
        vm: vm.clone(),
        id,
        content: Some(Box::new(editor)),
        mode,
        handle: Some(handle),
        root: None,
    }))
}

#[derive(Debug)]
struct FocusEditor {
    vm: SessionVm,
    id: EntityId,
    content: Option<Box<dyn Widget>>,
    mode: Signal<CardMode>,
    handle: Option<teksilo::widgets::rich_text::EditorHandle>,
    root: Option<WidgetId>,
}
impl Widget for FocusEditor {
    fn build(&mut self, ctx: &mut BuildContext) -> Vec<WidgetId> {
        if let Some(content) = self.content.take() {
            let root = ctx.add_boxed(content);
            self.root = Some(root);
            let (vm, id, handle) = (self.vm.clone(), self.id, self.handle.clone());
            let mode = self.mode.clone();
            ctx.run_after_mount(move |ec| {
                if let Some((target, range)) = vm.0.focus_block.get() {
                    if target == id {
                        vm.0.focus_block.set(None);
                        let in_place = vm.0.focus_in_place.get() == Some(id);
                        if let Some(handle) = handle {
                            mode.set(CardMode::Editing);
                            handle.focus(ec);
                            if let Some((start, end)) = range {
                                handle.select_range(start, end);
                                if !handle.reveal_range(ec, start, end) {
                                    handle.reveal_widget(ec);
                                }
                            } else if !in_place {
                                handle.reveal_widget(ec);
                            }
                        } else {
                            ec.ensure_widget_visible(root);
                        }
                    }
                }
            });
        }
        self.root.into_iter().collect()
    }
    fn layout_response(&self, p: SizeProposal, c: &LayoutContext) -> LayoutResponse {
        self.root
            .and_then(|id| c.child_size(id, p))
            .unwrap_or_else(|| p.resolve(0., 0.))
            .into()
    }
    fn children(&self) -> Vec<WidgetId> {
        self.root.into_iter().collect()
    }
}
fn start_writing_button(vm: &SessionVm) -> impl Widget + 'static {
    let v = vm.clone();
    Button::new(tr!(start_writing()))
        .variant(ButtonVariant::Filled)
        .on_activate_fn(move |_| v.report(v.start_writing()))
}

fn image(vm: &SessionVm, b: &Block) -> anyhow::Result<Box<dyn Widget>> {
    let image = vm.image(vm.0.service.image_asset(b.id)?)?;
    // `from_raw`, not `new`: `new` names the texture after the icon's address,
    // which two images can share, and then both show the first one's pixels.
    let picture = ImageWidget::from_raw(image.pixels().to_vec(), image.width(), image.height())
        .resizable(true);
    // A decorative image is hidden from screen readers and has no alt text.
    let (picture, caption, badge) = if b.decorative {
        (picture.a11y_hidden(), String::new(), tr!(decorative()))
    } else {
        (
            picture.alt(b.alt_text.clone()),
            b.alt_text.clone(),
            tr!(alt_badge()),
        )
    };
    let caption_style = teksilo::tokens::TextStyle {
        family: theme::CONTENT_FAMILY.into(),
        size: 13.,
        weight: teksilo::tokens::FontWeight::REGULAR,
        line_height: 19. / 13.,
        letter_spacing: 0.,
    };
    Ok(Box::new(
        VStack::new()
            .spacing(6.)
            .child(Expand::new().child(picture))
            .child(
                HStack::new()
                    .spacing(8.)
                    .alignment(VAlignment::Center)
                    .child(
                        Expand::horizontal().child(
                            TextWidget::new(lit!(caption))
                                .style(caption_style)
                                .color(TextRole::Secondary)
                                .overflow(TextOverflow::Ellipsis(EllipsisMode::Trailing))
                                .single_line()
                                .a11y_hidden(),
                        ),
                    )
                    .child(
                        Badge::new(badge)
                            .tooltip(tr!(alt_text()))
                            .text_role(TextRole::Secondary),
                    ),
            ),
    ))
}

/// The drawing surface, and under it what a screen reader hears instead.
fn sketch(vm: &SessionVm, b: &Block) -> anyhow::Result<Box<dyn Widget>> {
    let surface = ink::InkSurface::new(vm.clone(), b.id)?;
    if vm.0.drawing.get() == Some(b.id) {
        let description = Signal::new(b.description.clone());
        let (save, text, id) = (vm.clone(), description.clone(), b.id);
        return Ok(Box::new(
            VStack::new()
                .spacing(8.)
                .child(Expand::new().child(surface))
                .child(ink::escape_hint())
                .child(
                    TextInput::new(description)
                        .label(tr!(describe_sketch()))
                        .placeholder(tr!(describe_help()))
                        .on_blur_fn(move |_| save.report(save.describe(id, text.get()))),
                ),
        ));
    }
    let described = !b.description.is_empty();
    Ok(Box::new(
        VStack::new()
            .spacing(6.)
            .child(Expand::new().child(surface))
            .child(
                HStack::new()
                    .spacing(6.)
                    .alignment(VAlignment::Center)
                    .child(if described {
                        Icon::Check.sized(12.).color(TextRole::Success)
                    } else {
                        Icon::Info.sized(12.).color(TextRole::Secondary)
                    })
                    .child(
                        TextWidget::new(if described {
                            tr!(described())
                        } else {
                            tr!(not_described())
                        })
                        .style(TextStyleRole::Tiny)
                        .color(TextRole::Secondary),
                    ),
            ),
    ))
}

fn body(vm: &SessionVm, b: &Block, mode: Signal<CardMode>) -> anyhow::Result<Box<dyn Widget>> {
    if b.kind == BlockKind::RichText {
        return text_editor(vm, b.id, mode);
    }
    let content = match b.kind {
        BlockKind::Drawing => sketch(vm, b)?,
        BlockKind::Image => image(vm, b)?,
        BlockKind::RichText => unreachable!(),
    };
    Ok(Box::new(FocusEditor {
        vm: vm.clone(),
        id: b.id,
        content: Some(content),
        mode,
        handle: None,
        root: None,
    }))
}

/// What a block menu item does.
type BlockCommand = Rc<dyn Fn(&SessionVm, &mut EventContext)>;

/// Open a block dialog after the menu that asked for it has closed: presented
/// from the menu's own handler, the menu's dismissal would take it down too.
fn open_dialog(vm: &SessionVm, id: EntityId, intent: &'static str, ctx: &mut EventContext) {
    vm.0.dialog_block.set(Some(id));
    ctx.send_intent(Intent::new(intent));
}

/// The block's ⋮ menu, also its context menu and Shift+F10 menu.
pub fn block_menu(vm: &SessionVm, id: EntityId) -> MenuList {
    let Ok(b) = vm.0.service.block(id) else {
        return MenuList::new();
    };
    let item = |label, icon: Icon, shortcut: &str, f: BlockCommand| {
        let v = vm.clone();
        MenuItem::new(label)
            .icon(icon.widget())
            .shortcut_label(shortcut)
            .on_activate_fn(move |ctx| f(&v, ctx))
    };
    let mut menu = MenuList::new();
    if b.trashed {
        return menu.item(item(
            tr!(restore()),
            Icon::Restore,
            "",
            Rc::new(move |v, _| v.report(v.trash_block(id))),
        ));
    }
    menu = match b.kind {
        BlockKind::RichText => menu.item(
            item(
                tr!(edit_text()),
                Icon::Text,
                "Enter",
                Rc::new(move |v, ctx| {
                    if let Some(h) = v.0.handles.borrow().get(&id) {
                        h.focus(ctx);
                    }
                }),
            )
            .text_style(TextStyleRole::BodyBold),
        ),
        BlockKind::Drawing => menu
            .item(
                item(
                    tr!(draw()),
                    Icon::Pen,
                    "Enter",
                    Rc::new(move |v, _| v.enter_drawing(id)),
                )
                .text_style(TextStyleRole::BodyBold),
            )
            .item(item(
                tr!(rename_sketch()),
                Icon::Alt,
                "F2",
                Rc::new(move |v, ctx| open_dialog(v, id, "block.describe", ctx)),
            )),
        BlockKind::Image => menu.item(item(
            tr!(edit_alt()),
            Icon::Alt,
            "F2",
            Rc::new(move |v, ctx| open_dialog(v, id, "block.describe", ctx)),
        )),
    };
    let (position, count) = vm.reading_position(id).unwrap_or((1, 1));
    menu.separator()
        .item(item(
            if b.kind == BlockKind::RichText {
                tr!(position_width())
            } else {
                tr!(position_size())
            },
            Icon::Move,
            "Alt Enter",
            Rc::new(move |v, ctx| open_dialog(v, id, "block.position", ctx)),
        ))
        .item(item(
            tr!(bring_front()),
            Icon::Front,
            "Ctrl Shift ]",
            Rc::new(move |v, _| v.report(v.arrange(id, true))),
        ))
        .item(item(
            tr!(send_back()),
            Icon::Back,
            "Ctrl Shift [",
            Rc::new(move |v, _| v.report(v.arrange(id, false))),
        ))
        .separator()
        .item(
            Padding::new(4., 10., 2., 10.).child(
                TextWidget::new(tr!(reading_order(
                    position = position as i64,
                    count = count as i64
                )))
                .style(TextStyleRole::Tiny)
                .color(TextRole::Secondary),
            ),
        )
        .item(
            item(
                tr!(earlier()),
                Icon::ArrowUp,
                "Alt ↑",
                Rc::new(move |v, _| v.report(v.reorder(id, -1))),
            )
            .enabled(position > 1),
        )
        .item(
            item(
                tr!(later()),
                Icon::ArrowDown,
                "Alt ↓",
                Rc::new(move |v, _| v.report(v.reorder(id, 1))),
            )
            .enabled(position < count),
        )
        .separator()
        .item(item(
            tr!(duplicate()),
            Icon::Copy,
            "Ctrl D",
            Rc::new(move |v, _| v.report(v.duplicate(id))),
        ))
        .separator()
        .item(
            item(
                tr!(move_to_trash()),
                Icon::Trash,
                "Del",
                Rc::new(move |v, ctx| {
                    v.report(v.trash_block(id));
                    offer_undo(v, ctx);
                }),
            )
            .text_role(TextRole::Error),
        )
}

fn menu_button(vm: &SessionVm, id: EntityId) -> PopoverIconButton {
    PopoverIconButton::new(
        icon_button(Icon::More, tr!(block_actions())).size(IconButtonSize::Compact),
    )
    .show_disclosure_caret(false)
    .bare()
    .content(block_menu(vm, id))
}

/// Type icon, label and state badge.
fn card_header(vm: &SessionVm, b: &Block) -> impl Widget + 'static {
    let id = b.id;
    let sketch = b.kind == BlockKind::Drawing;
    let label: Box<dyn Widget> = if sketch && vm.0.drawing.get() == Some(id) {
        let title = Signal::new(b.label.clone());
        let (submit, blur, t1, t2) = (vm.clone(), vm.clone(), title.clone(), title.clone());
        Box::new(
            TextInput::new(title)
                .variant(TextInputVariant::Underline)
                .label(tr!(sketch_title()))
                .on_submit_fn(move |_| submit.report(submit.rename_block(id, t1.get())))
                .on_blur_fn(move |_| blur.report(blur.rename_block(id, t2.get()))),
        )
    } else {
        Box::new(
            TextWidget::new(lit!(b.label.clone()))
                .style(if sketch {
                    TextStyleRole::SmallBold
                } else {
                    TextStyleRole::Small
                })
                .color(if sketch {
                    TextRole::Primary
                } else {
                    TextRole::Secondary
                })
                .overflow(TextOverflow::Ellipsis(EllipsisMode::Trailing))
                .single_line(),
        )
    };
    let editing = vm.0.editing.map(move |e| *e == Some(id));
    let drawing = vm.0.drawing.map(move |d| *d == Some(id));
    Padding::new(5., 2., 5., 10.).child(
        HStack::new()
            .spacing(6.)
            .alignment(VAlignment::Center)
            .child(
                kind_icon(&b.kind)
                    .sized(14.)
                    .color(TextRole::Secondary)
                    .visible_when(editing.map(|e| !e)),
            )
            .child(
                Expand::horizontal()
                    .child(label)
                    .visible_when(editing.map(|e| !e)),
            )
            .child(
                Badge::new(tr!(draw()))
                    .background(SurfaceRole::AccentSubtle)
                    .text_role(TextRole::Accent)
                    .visible_when(drawing),
            ),
    )
}

fn card(
    vm: &SessionVm,
    scene: &LiveScene,
    b: Block,
    item: ItemId,
) -> anyhow::Result<Box<dyn Widget>> {
    let id = b.id;
    // A sketch in drawing mode stays Selected: an Editing card would take
    // Escape before the drawing surface could cancel a stroke with it.
    let mode = Signal::new(
        if vm.0.selected.get() == Some(id) || vm.0.drawing.get() == Some(id) {
            CardMode::Selected
        } else {
            CardMode::Idle
        },
    );
    let content = body(vm, &b, mode.clone())?;
    let content: Box<dyn Widget> = if b.kind == BlockKind::RichText {
        Box::new(Expand::horizontal().child(content))
    } else {
        Box::new(Expand::new().child(content))
    };
    let activate = vm.clone();
    let kind = b.kind.clone();
    let card = SceneCard::new(scene.model.clone(), item)
        .selection(scene.selection.clone())
        .mode(mode.clone())
        .label(b.label.clone())
        .header(card_header(vm, &b))
        .header_trailing(menu_button(vm, id))
        .surface(|content| {
            Box::new(
                Card::new()
                    .variant(CardVariant::Outlined)
                    .background(SurfaceRole::Raised)
                    .corner_radius(8.)
                    .padding(0.)
                    .content(content),
            )
        })
        .body(Padding::new(8., 12., 10., 12.).child(content))
        .on_activate(move |_| match kind {
            BlockKind::Drawing => {
                mode.set(CardMode::Selected);
                activate.enter_drawing(id);
            }
            // An image has nothing to edit: activation keeps it selected.
            BlockKind::Image => mode.set(CardMode::Selected),
            BlockKind::RichText => {}
        });
    let menu = vm.clone();
    let card: Box<dyn Widget> = if b.kind == BlockKind::RichText {
        Box::new(card.height_for_width())
    } else {
        Box::new(card)
    };
    Ok(Box::new(card.context_menu(move |_, _| {
        Some(Box::new(block_menu(&menu, id)))
    })))
}

/// The dot grid under the canvas, 24 scene pixels apart.
fn dot_grid(canvas: &mut Canvas, ctx: &PaintContext, region: Rect) {
    let color = if ctx.theme.is_dark() {
        Color::from_hex("#3A352E")
    } else {
        Color::from_hex("#DDD5C6")
    };
    let step = 24.;
    let mut y = (region.y / step).floor() * step;
    while y <= region.y + region.height {
        let mut x = (region.x / step).floor() * step;
        while x <= region.x + region.width {
            canvas.fill_circle(Point::new(x, y), 1., color);
            x += step;
        }
        y += step;
    }
}

fn key_down(event: &WidgetEvent) -> Option<(Key, Modifiers)> {
    match event {
        WidgetEvent::KeyDown { key, modifiers, .. } => Some((*key, *modifiers)),
        _ => None,
    }
}

/// Keys on the selected block: arrows nudge, Shift a grid step, Ctrl+Shift
/// resizes, Alt+↑/↓ reorders, and the menu's shortcuts.
fn canvas_keys(vm: &SessionVm, event: &WidgetEvent, ctx: &mut EventContext) -> EventResponse {
    let Some((key, m)) = key_down(event) else {
        return EventResponse::Ignored;
    };
    // Runs before the card's own Escape (which hands it the keyboard): the
    // text block is no longer being edited, so its toolbar goes.
    if key == Key::Escape && vm.0.editing.get().is_some() {
        vm.0.editing.set(None);
        return EventResponse::Ignored;
    }
    if vm.0.edit.text_focused() || vm.0.drawing.get().is_some() {
        return EventResponse::Ignored;
    }
    let Some(id) = vm.0.selected.get() else {
        return EventResponse::Ignored;
    };
    let step = if m.shift() { 24. } else { 1. };
    let result = match (key, m.command() && m.shift(), m.alt(), m.command()) {
        (Key::ArrowLeft, true, _, _) => vm.resize(id, -24., 0.),
        (Key::ArrowRight, true, _, _) => vm.resize(id, 24., 0.),
        (Key::ArrowUp, true, _, _) => vm.resize(id, 0., -24.),
        (Key::ArrowDown, true, _, _) => vm.resize(id, 0., 24.),
        (Key::ArrowUp, _, true, _) => vm.reorder(id, -1),
        (Key::ArrowDown, _, true, _) => vm.reorder(id, 1),
        (Key::ArrowLeft, false, false, false) => vm.nudge(id, -step, 0.),
        (Key::ArrowRight, false, false, false) => vm.nudge(id, step, 0.),
        (Key::ArrowUp, false, false, false) => vm.nudge(id, 0., -step),
        (Key::ArrowDown, false, false, false) => vm.nudge(id, 0., step),
        (Key::Character(']' | '}'), true, _, _) => vm.arrange(id, true),
        (Key::Character('[' | '{'), true, _, _) => vm.arrange(id, false),
        (Key::D, false, false, true) => vm.duplicate(id),
        (Key::Enter, _, true, _) => {
            crate::dialogs::position_and_size(vm, id, ctx);
            Ok(())
        }
        (Key::F2, ..) => {
            if vm
                .0
                .service
                .block(id)
                .is_ok_and(|b| b.kind != BlockKind::RichText)
            {
                crate::dialogs::describe(vm, id, ctx);
            }
            Ok(())
        }
        (Key::Delete | Key::Backspace, false, false, false) => {
            let result = vm.trash_block(id);
            offer_undo(vm, ctx);
            result
        }
        _ => return EventResponse::Ignored,
    };
    vm.report(result);
    EventResponse::Handled
}

fn canvas(
    vm: &SessionVm,
    ctx: &mut BuildContext,
    note: EntityId,
) -> anyhow::Result<Box<dyn Widget>> {
    let blocks: Vec<_> =
        vm.0.service
            .blocks(note)?
            .into_iter()
            .filter(|b| !b.trashed)
            .collect();
    let model = SceneModel::new();
    let mut items = HashMap::new();
    for b in &blocks {
        let item = model.add_widget_item(
            b.id,
            Rect::new(b.x as f32, b.y as f32, b.width as f32, b.height as f32),
        );
        model.set_z(item, b.z as f32);
        // Moved by the header or the frame; resized by the frame's handles.
        let flags = model.flags(item).unwrap_or_default()
            | ItemFlags::IS_DRAGGABLE
            | ItemFlags::IS_RESIZABLE;
        model.set_flags(item, flags);
        items.insert(b.id, item);
    }
    let selection = SceneSelection::new(SceneSelectionMode::Single);
    if let Some(item) = vm.0.selected.get().and_then(|id| items.get(&id)) {
        selection.select_one(*item);
    }
    let scene = LiveScene {
        model: model.clone(),
        items: items.clone(),
        selection: selection.clone(),
    };
    *vm.0.scene.borrow_mut() = Some(scene.clone());
    let map = Rc::new(RefCell::new(
        items
            .iter()
            .map(|(b, i)| (*i, *b))
            .collect::<HashMap<_, _>>(),
    ));
    let weak = model.downgrade();
    let v = vm.clone();
    let m = map.clone();
    model.set_edit_sink(move |record| {
        // Teksilo 0.18 SceneCard header drags emit Programmatic/Record,
        // while transform handles emit User/Record. Both are committed frame
        // edits. Our own geometry synchronization always uses Ignore.
        if record.source == ChangeSource::Remote
            || record.history == HistoryMode::Ignore
            || record.ephemeral
        {
            return;
        }
        let Some(model) = weak.upgrade() else { return };
        let affected: std::collections::HashSet<_> = record
            .edits
            .iter()
            .filter_map(|e| match e {
                SceneEdit::Change(
                    ItemChange::LocalPosChanged { id, .. }
                    | ItemChange::LocalBoundsChanged { id, .. },
                ) => Some(*id),
                _ => None,
            })
            .collect();
        let result = (|| -> anyhow::Result<()> {
            let mut values = vec![];
            for item in &affected {
                if let Some(id) = m.borrow().get(item).copied() {
                    let b = v.0.service.block(id)?;
                    if let Some(r) = model.scene_rect(*item) {
                        values.push((
                            id,
                            noton_format::Geometry {
                                x: r.x as f64,
                                y: r.y as f64,
                                width: r.width as f64,
                                height: if b.kind == BlockKind::RichText {
                                    b.height
                                } else {
                                    r.height as f64
                                },
                                z: b.z,
                            },
                        ));
                    }
                }
            }
            if record.outcome == TxnOutcome::Committed && !values.is_empty() {
                v.0.service.geometries(&values, v.page_stack()?)?;
                v.claim_page();
                v.changed();
            }
            Ok(())
        })();
        if result.is_err() || record.outcome != TxnOutcome::Committed {
            let _t = model.transaction(ChangeSource::Programmatic, HistoryMode::Ignore);
            for item in affected {
                if let Some(id) = m.borrow().get(&item).copied() {
                    if let Ok(b) = v.0.service.block(id) {
                        model.set_local_pos(item, Point::new(b.x as f32, b.y as f32));
                        model.set_local_bounds(
                            item,
                            Rect::new(0., 0., b.width as f32, b.height as f32),
                        );
                    }
                }
            }
        }
        v.report(result);
    });
    // Selection mirrors into the view model; leaving a text block ends editing.
    let v = vm.clone();
    let m = map.clone();
    ctx.effect(&selection.selection_signal(), move |ids| {
        let selected = ids.iter().next().and_then(|i| m.borrow().get(i).copied());
        v.0.selected.set_if_changed(selected);
        if v.0.editing.get().is_some() && selected != v.0.editing.get() {
            v.0.editing.set(None);
        }
        // Selecting another block than the one last typed in makes the page,
        // not that text, what Ctrl+Z undoes.
        if selected.is_some() && selected != v.0.active_text.get() {
            v.claim_page();
        }
    });
    let v = vm.clone();
    let view = SceneView::with_model(model.clone())
        .selection_model(selection)
        .transform_controller(
            TransformConfig::new()
                .enabled(vm.0.editing.map(|e| e.is_none()))
                .handles(TransformHandleSet::MOVE | TransformHandleSet::ALL_RESIZE)
                .padding(4.)
                .handle_px(8.)
                .min_size(80., 40.),
        )
        .a11y_label(tr!(canvas()))
        .background(dot_grid)
        .delegate_typed::<EntityId>(move |id, item| {
            match v.0.service.block(*id).and_then(|b| {
                let live =
                    v.0.scene
                        .borrow()
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("No canvas"))?;
                card(&v, &live, b, item)
            }) {
                Ok(card) => card,
                Err(e) => Box::new(TextWidget::new(lit!(e.to_string()))),
            }
        });
    let camera = crate::session_vm::Camera {
        zoom: view.zoom_signal(),
        pan_x: view.pan_x_signal(),
        pan_y: view.pan_y_signal(),
        viewport: view.viewport_size_signal(),
    };
    vm.attach_camera(camera.clone());
    if let Some((id, _)) = vm.0.focus_block.get() {
        if vm.0.focus_in_place.get() != Some(id) {
            if let Ok(b) = vm.0.service.block(id) {
                camera.pan_x.set(24. - b.x as f32 * camera.zoom.get());
                camera.pan_y.set(24. - b.y as f32 * camera.zoom.get());
                vm.0.fit_pending.set(None);
            }
        }
    }

    let v = vm.clone();
    ctx.effect(&camera.zoom, move |z| {
        v.0.zoom.set_if_changed(*z);
    });
    let keys = vm.clone();
    let transform = view.view_transform_signal();
    let write = vm.clone();
    let handlers =
        teksilo::core::widget_builder::HandlerSet::new().on_double_tap(move |_event, ctx| {
            let Some(position) = ctx.tree_pointer_position() else {
                return;
            };
            if model.item_at_in_view(position, transform.get()).is_none()
                && !write.0.note_trashed.get()
            {
                if let Some(inverse) = transform.get().inverse() {
                    let point = inverse.apply_point(position);
                    write.report(write.write_at(point.x as f64, point.y as f64));
                }
            }
        });
    let view = WritingCanvas {
        vm: vm.clone(),
        scroll: view.with_scroll_bars(),
        preserve_pan: vm
            .0
            .focus_in_place
            .get()
            .map(|_| (camera.pan_x.get(), camera.pan_y.get())),
        handlers: Some(handlers),
    };
    let empty = blocks.is_empty();
    let view = view.on_key_preview(move |event, ctx| canvas_keys(&keys, event, ctx));
    if empty {
        return Ok(Box::new(
            ZStack::new().child(view).child(
                Padding::uniform(32.).child(
                    VStack::new()
                        .alignment(HAlignment::Center)
                        .child(caption(tr!(empty_note())))
                        .child(start_writing_button(vm)),
                ),
            ),
        ));
    }
    Ok(Box::new(view))
}

/// Blocks stacked in reading order, still editable, reordered with the ↑ ↓
/// buttons, the menu or Alt+↑/↓.
fn reading(vm: &SessionVm, note: EntityId) -> anyhow::Result<Box<dyn Widget>> {
    *vm.0.scene.borrow_mut() = None;
    let blocks: Vec<_> =
        vm.0.service
            .blocks(note)?
            .into_iter()
            .filter(|b| !b.trashed || vm.0.trash.get())
            .collect();
    let count = blocks.iter().filter(|b| !b.trashed).count();
    let mut list = VStack::new().spacing(16.);
    if blocks.is_empty() {
        list = list
            .child(caption(tr!(empty_note())))
            .child(start_writing_button(vm));
    }
    // Reading order counts only the blocks a reader sees; one in the trash
    // (shown while the trash is) has no position.
    let mut position = 0;
    for b in blocks {
        let id = b.id;
        let live = !b.trashed;
        position += usize::from(live);
        let (up, down) = (vm.clone(), vm.clone());
        let mut gutter = VStack::new().spacing(4.).alignment(HAlignment::Center);
        if live {
            gutter = gutter.child(Badge::new(lit!(position.to_string())).tooltip(tr!(
                block_position(position = position as i64, count = count as i64)
            )));
        }
        let gutter = gutter
            .child(
                icon_button(Icon::ArrowUp, tr!(earlier()))
                    .size(IconButtonSize::Compact)
                    .enabled(live && position > 1)
                    .on_activate_fn(move |_| up.report(up.reorder(id, -1))),
            )
            .child(
                icon_button(Icon::ArrowDown, tr!(later()))
                    .size(IconButtonSize::Compact)
                    .enabled(live && position < count)
                    .on_activate_fn(move |_| down.report(down.reorder(id, 1))),
            );
        let mut header = HStack::new()
            .spacing(6.)
            .alignment(VAlignment::Center)
            .child(kind_icon(&b.kind).sized(14.).color(TextRole::Secondary))
            .child(
                Expand::horizontal().child(
                    TextWidget::new(lit!(b.label.clone()))
                        .style(TextStyleRole::Small)
                        .color(TextRole::Secondary)
                        .single_line(),
                ),
            );
        if b.trashed {
            header = header.child(Badge::new(tr!(in_trash())).text_role(TextRole::Secondary));
        }
        header = header.child(menu_button(vm, id));
        let content = body(vm, &b, Signal::new(CardMode::Idle))?;
        let content: Box<dyn Widget> = if b.kind == BlockKind::RichText {
            content
        } else {
            Box::new(
                FixedSize::new()
                    .height((b.height as f32).min(420.))
                    .child(content),
            )
        };
        let keys = vm.clone();
        let menu = vm.clone();
        list = list.child(
            HStack::new()
                .spacing(10.)
                .alignment(VAlignment::Top)
                .child(gutter)
                .child(
                    Expand::horizontal().child(
                        Panel::new()
                            .background(SurfaceRole::Raised)
                            .border_color(BorderRole::Default)
                            .border_width(1.)
                            .corner_radius(8.)
                            .padding(0.)
                            .child(
                                Padding::new(8., 12., 12., 12.).child(
                                    VStack::new()
                                        .spacing(6.)
                                        .child(header)
                                        .child(Expand::horizontal().child(content)),
                                ),
                            ),
                    ),
                )
                .on_key_preview(move |event, ctx| match key_down(event) {
                    Some((Key::ArrowUp, m)) if m.alt() => {
                        keys.report(keys.reorder(id, -1));
                        let _ = ctx;
                        EventResponse::Handled
                    }
                    Some((Key::ArrowDown, m)) if m.alt() => {
                        keys.report(keys.reorder(id, 1));
                        EventResponse::Handled
                    }
                    _ => EventResponse::Ignored,
                })
                .context_menu(move |_, _| Some(Box::new(block_menu(&menu, id)))),
        );
    }
    Ok(Box::new(ScrollArea::new().child(
        Padding::new(20., 24., 32., 16.).child(MaxSize::width(760.).child(list)),
    )))
}

impl Widget for PageView {
    fn build(&mut self, ctx: &mut BuildContext) -> Vec<WidgetId> {
        self.vm
            .0
            .revision
            .bind_to(ctx.self_id(), ctx.binding_registry(), BindingLevel::Rebuild);
        self.vm
            .0
            .reading
            .bind_to(ctx.self_id(), ctx.binding_registry(), BindingLevel::Rebuild);
        let result = (|| -> anyhow::Result<Box<dyn Widget>> {
            let Some(note) = self.vm.0.note.get() else {
                *self.vm.0.scene.borrow_mut() = None;
                return Ok(Box::new(Spacer::new()));
            };
            if self.vm.0.reading.get() {
                reading(&self.vm, note)
            } else {
                canvas(&self.vm, ctx, note)
            }
        })();
        let widget = match result {
            Ok(w) => w,
            Err(e) => {
                self.vm.report(Err(e));
                Box::new(TextWidget::new(tr!(error())))
            }
        };
        // Decoded images are kept only while this page shows them.
        let shown = self
            .vm
            .0
            .note
            .get()
            .and_then(|note| self.vm.0.service.blocks(note).ok())
            .unwrap_or_default()
            .iter()
            .filter(|b| b.kind == BlockKind::Image)
            .filter_map(|b| self.vm.0.service.image_asset(b.id).ok())
            .collect();
        self.vm.keep_images(&shown);
        let camera = if self.vm.0.reading.get() {
            None
        } else {
            self.vm.0.camera.borrow().clone()
        };
        let root = ctx.add(crate::image_drop::target(widget, &self.vm, camera));
        self.root = Some(root);
        vec![root]
    }
    fn layout_response(&self, p: SizeProposal, c: &LayoutContext) -> LayoutResponse {
        self.root
            .and_then(|id| c.child_size(id, p))
            .unwrap_or_else(|| p.resolve(0., 0.))
            .into()
    }
    fn children(&self) -> Vec<WidgetId> {
        self.root.into_iter().collect()
    }
}
