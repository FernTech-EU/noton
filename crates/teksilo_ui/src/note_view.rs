//! The note area: header (title, view switch, Insert, note menu), the
//! contextual toolbar row, banners, and the canvas or reading view.
use crate::{
    icons::Icon,
    page::PageView,
    session_vm::{SaveState, SessionVm},
    theme,
    widgets::{AutoFocus, Rebuild, caption, icon_button},
};
use teksilo::core::styles::TextInputVariant;
use teksilo::tokens::{HAlignment, VAlignment};
use teksilo::{prelude::*, widgets::*};

fn start_rename(vm: &SessionVm) {
    if vm.0.note.get().is_some() && !vm.0.note_trashed.get() {
        vm.0.renaming_note.set(true);
    }
}

fn finish_rename(vm: &SessionVm) {
    if vm.0.renaming_note.get() {
        vm.0.renaming_note.set(false);
        vm.report(vm.rename_note());
    }
}

/// The note title: Literata text that turns into a field to rename it.
fn title(vm: &SessionVm) -> Rebuild {
    let v = vm.clone();
    Rebuild::new(move || {
        let vm = v.clone();
        if vm.0.renaming_note.get() {
            let (submit, blur, cancel) = (vm.clone(), vm.clone(), vm.clone());
            let input = TextInput::new(vm.0.title.clone())
                .variant(TextInputVariant::Underline)
                .label(tr!(title()))
                .placeholder(tr!(title()))
                .on_submit_fn(move |_| finish_rename(&submit))
                .on_blur_fn(move |_| finish_rename(&blur));
            return Box::new(AutoFocus::new(input).on_key_preview(move |event, _| {
                if matches!(
                    event,
                    WidgetEvent::KeyDown {
                        key: Key::Escape,
                        ..
                    }
                ) {
                    let vm = &cancel;
                    if let Some(note) = vm.0.note.get().and_then(|id| vm.0.service.note(id).ok()) {
                        vm.0.title.set(note.title);
                    }
                    vm.0.renaming_note.set(false);
                    EventResponse::Handled
                } else {
                    EventResponse::Ignored
                }
            }));
        }
        let (tap, key) = (vm.clone(), vm.clone());
        Box::new(
            TextWidget::new(lit!(""))
                .text(vm.0.title.clone())
                .style(theme::note_title_style())
                .overflow(TextOverflow::Ellipsis(EllipsisMode::Trailing))
                .single_line()
                .on_tap(move |_, _| start_rename(&tap))
                .on_key(move |event, _| match event {
                    WidgetEvent::KeyDown {
                        key: Key::Enter | Key::F2,
                        ..
                    } => {
                        start_rename(&key);
                        EventResponse::Handled
                    }
                    _ => EventResponse::Ignored,
                })
                .focusable(true)
                .cursor(CursorIcon::Text)
                .access_role(accesskit::Role::Button)
                .access_label(tr!(rename_note()))
                .access_value(vm.0.title.clone()),
        )
    })
    .on(&vm.0.renaming_note)
}

fn view_switch(vm: &SessionVm, ctx: &mut BuildContext) -> SegmentedControl {
    let index = Signal::new(usize::from(vm.0.reading.get()));
    let i = index.clone();
    ctx.effect(&vm.0.reading, move |reading| {
        i.set_if_changed(usize::from(*reading));
    });
    let canvas = Segment::new(tr!(canvas())).icon(|| Icon::Canvas.widget());
    let reading = Segment::new(tr!(reading())).icon(|| Icon::Reading.widget());
    let reading_id = reading.segment_id();
    SegmentedControl::indexed(index)
        .segment(canvas)
        .segment(reading)
        .display(SegmentDisplay::IconText)
        .label(tr!(view_switch()))
        .on_change(move |id, ctx| {
            ctx.send_intent(Intent::new(if id == reading_id {
                "view.reading"
            } else {
                "view.canvas"
            }))
        })
}

fn insert_menu(vm: &SessionVm) -> PopoverButton {
    let editable =
        vm.0.note
            .zip(&vm.0.note_trashed)
            .map(|(note, trashed)| note.is_some() && !trashed);
    let item = |label, icon: Icon, intent: &'static str, shortcut: &'static str| {
        MenuItem::new(label)
            .icon(icon.widget())
            .shortcut_label(shortcut)
            .on_activate_fn(move |ctx| ctx.send_intent(Intent::new(intent)))
    };
    PopoverButton::new(
        Button::new(tr!(insert_button()))
            .variant(ButtonVariant::Filled)
            .icon(Icon::Plus.widget(), IconLocation::Leading)
            .enabled(editable),
    )
    .show_disclosure_caret(true)
    .bare()
    .content(
        MenuList::new()
            .item(item(
                tr!(insert_text()),
                Icon::Text,
                "block.text",
                "Ctrl Alt T",
            ))
            .item(item(
                tr!(insert_sketch()),
                Icon::Pen,
                "block.sketch",
                "Ctrl Alt D",
            ))
            .item(item(
                tr!(insert_image()),
                Icon::Image,
                "block.image",
                "Ctrl Alt I",
            )),
    )
}

/// The note's ⋮ menu, rebuilt with the note so its items match it.
fn note_actions(vm: &SessionVm) -> Rebuild {
    let v = vm.clone();
    Rebuild::new(move || {
        let Some(note) = v.0.note.get() else {
            return Box::new(Spacer::new());
        };
        let (text, html) = (
            MenuItem::new(tr!(export_text()))
                .icon(Icon::Export.widget())
                .on_activate_fn(|ctx| ctx.send_intent(Intent::new("file.export_text"))),
            MenuItem::new(tr!(export_html()))
                .icon(Icon::Export.widget())
                .on_activate_fn(|ctx| ctx.send_intent(Intent::new("file.export_html"))),
        );
        let menu = crate::panes::note_menu(&v, note)
            .separator()
            .item(text)
            .item(html);
        Box::new(
            PopoverIconButton::new(icon_button(Icon::More, tr!(note_actions())))
                .show_disclosure_caret(false)
                .bare()
                .content(menu),
        )
    })
    .on(&vm.0.note)
    .on(&vm.0.note_trashed)
    .on(&vm.0.folders_version)
}

fn header(vm: &SessionVm, ctx: &mut BuildContext) -> impl Widget + 'static {
    Padding::new(10., 12., 10., 24.).child(
        HStack::new()
            .spacing(12.)
            .alignment(VAlignment::Center)
            .child(Expand::horizontal().child(title(vm)))
            .child(view_switch(vm, ctx))
            .child(insert_menu(vm))
            .child(note_actions(vm)),
    )
}

/// The save-failed banner (persistent, no dismiss) and the error banner.
fn banners(vm: &SessionVm) -> VStack {
    let failed = vm.0.save_state.map(|s| *s == SaveState::Failed);
    let discard = vm.clone();
    let save_failed = Banner::error(tr!(save_failed_title()))
        .description(tr!(save_failed_detail()))
        .action(
            HStack::new()
                .spacing(8.)
                .child(
                    Button::new(tr!(retry()))
                        .variant(ButtonVariant::Outlined)
                        .on_activate_fn(|ctx| ctx.send_intent(Intent::new("file.save"))),
                )
                .child(
                    Button::new(tr!(save_copy()))
                        .variant(ButtonVariant::Plain)
                        .on_activate_fn(|ctx| ctx.send_intent(Intent::new("file.save_copy"))),
                )
                // Without it a library whose disk went away could not be closed.
                .child(
                    Button::new(tr!(close_without_saving()))
                        .variant(ButtonVariant::Plain)
                        .text_role(TextRole::Error)
                        .on_activate_fn(move |ctx| {
                            discard.close_without_saving();
                            ctx.close_window();
                        }),
                ),
        );
    let v = vm.clone();
    let error = Rebuild::new(move || {
        let message = v.0.error.get();
        if message.is_empty() {
            return Box::new(Spacer::new());
        }
        let dismiss = v.clone();
        Box::new(
            Banner::error(tr!(error_title()))
                .description(lit!(message))
                .on_dismiss(move |_| dismiss.0.error.set(String::new())),
        )
    })
    .on(&vm.0.error);
    // Narrow windows: the canvas is zoomed to fit; offer Reading view for
    // this note without changing the view setting.
    let (read, never) = (vm.clone(), vm.clone());
    let narrow_hint = Banner::info(tr!(narrow_hint()))
        .description(tr!(narrow_hint_detail()))
        .action(
            HStack::new()
                .spacing(8.)
                .child(
                    Button::new(tr!(read_this_note()))
                        .variant(ButtonVariant::Outlined)
                        .icon(Icon::Reading.widget(), IconLocation::Leading)
                        .on_activate_fn(|ctx| ctx.send_intent(Intent::new("view.reading"))),
                )
                .child(
                    Button::new(tr!(dont_show_again()))
                        .variant(ButtonVariant::Plain)
                        .on_activate_fn(move |_| {
                            never.0.narrow_hint.set(false);
                            never.report(
                                crate::settings::persist(&never, |p| {
                                    p.narrow_hint_dismissed = true
                                })
                                .map(drop),
                            );
                        }),
                ),
        )
        .on_dismiss(move |_| read.0.narrow_hint.set(false));
    let show_hint =
        vm.0.narrow
            .zip(&vm.0.narrow_hint)
            .zip(&vm.0.reading)
            .map(|((narrow, hint), reading)| *narrow && *hint && !reading);
    VStack::new()
        .spacing(8.)
        .child(
            Padding::symmetric(4., 12.)
                .child(narrow_hint)
                .visible_when(show_hint),
        )
        .child(
            Padding::symmetric(4., 16.)
                .child(save_failed)
                .visible_when(failed),
        )
        .child(
            Padding::symmetric(4., 16.)
                .child(error)
                .visible_when(vm.0.error.map(|e| !e.is_empty())),
        )
}

/// Header, contextual toolbars, banners and the page; an invitation when no
/// note is open.
pub fn note_view(vm: &SessionVm, ctx: &mut BuildContext) -> impl Widget + 'static {
    let has_note = vm.0.note.map(|n| n.is_some());
    let editing =
        vm.0.editing
            .zip(&vm.0.drawing)
            .zip(&vm.0.reading)
            .map(|((e, d), _)| e.is_some() && d.is_none());
    let drawing = vm.0.drawing.map(|d| d.is_some());
    let empty = Padding::uniform(32.).child(
        VStack::new()
            .spacing(8.)
            .alignment(HAlignment::Center)
            .child(Icon::Note.sized(32.).color(TextRole::Secondary))
            .child(caption(tr!(empty()))),
    );
    crate::widgets::fill(SurfaceRole::Content).child(
        ZStack::new()
            .child(
                VStack::new()
                    .child(header(vm, ctx))
                    .child(
                        crate::widgets::fill(SurfaceRole::Raised)
                            .child(crate::page::format_bar::format_bar(vm))
                            .visible_when(editing),
                    )
                    .child(
                        crate::widgets::fill(SurfaceRole::Raised)
                            .child(crate::page::ink::drawing_bar(vm))
                            .visible_when(drawing),
                    )
                    .child(Divider::horizontal())
                    .child(banners(vm))
                    .child(Expand::new().child(PageView::new(vm.clone())))
                    .visible_when(has_note.clone()),
            )
            .child(empty.visible_when(has_note.map(|h| !h))),
    )
}
