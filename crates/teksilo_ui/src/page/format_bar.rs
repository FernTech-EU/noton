//! The formatting toolbar. It shows only while a text block is being edited,
//! its toggles reflect the formatting at the caret, and what does not fit
//! overflows into the toolbar's own ⌄ menu.
use crate::{icons::Icon, session_vm::SessionVm};
use teksilo::widgets::rich_text::EditorHandle;
use teksilo::{prelude::*, widgets::*};
use text_document::Alignment;

/// Run `f` on the editor being edited, then refresh the pressed states.
fn with_editor(vm: &SessionVm, f: impl Fn(&EditorHandle)) {
    if let Some(h) = vm.editing_handle() {
        f(&h);
        vm.0.pending.borrow_mut().extend(vm.0.editing.get());
        vm.sync_format();
    }
}

fn toggle(
    vm: &SessionVm,
    label: teksilo::i18n::LocalizedString,
    icon: Icon,
    state: &Signal<bool>,
    apply: fn(&EditorHandle, bool),
) -> ToolbarAction {
    let (v, s) = (vm.clone(), state.clone());
    ToolbarAction::new(label.clone(), move || icon.widget())
        .tooltip(label)
        .toggle(state.clone())
        .on_activate(move |_| with_editor(&v, |h| apply(h, s.get())))
}

fn action(
    vm: &SessionVm,
    label: teksilo::i18n::LocalizedString,
    icon: Icon,
    apply: fn(&EditorHandle),
) -> ToolbarAction {
    let v = vm.clone();
    ToolbarAction::new(label.clone(), move || icon.widget())
        .tooltip(label)
        .on_activate(move |_| with_editor(&v, apply))
}

fn menu_item(
    vm: &SessionVm,
    label: teksilo::i18n::LocalizedString,
    apply: fn(&EditorHandle),
) -> MenuItem {
    let v = vm.clone();
    MenuItem::new(label).on_activate_fn(move |_| with_editor(&v, apply))
}

fn paragraph_style(vm: &SessionVm) -> ComboBox<u8> {
    let v = vm.clone();
    ComboBox::from_items(0u8..=3, vm.0.format.heading.clone(), |level| match level {
        1 => tr!(heading_1()),
        2 => tr!(heading_2()),
        3 => tr!(heading_3()),
        _ => tr!(paragraph()),
    })
    .label(tr!(paragraph_style()))
    .on_select(move |level, _| {
        let level = *level;
        with_editor(&v, |h| h.set_heading_level(level));
    })
}

/// The Link popover: the address of the link at the caret, Apply and Remove.
fn link(vm: &SessionVm) -> PopoverIconButton {
    let address = Signal::new(String::new());
    let (open, apply, remove, submit) = (vm.clone(), vm.clone(), vm.clone(), vm.clone());
    let (a1, a2, a3) = (address.clone(), address.clone(), address.clone());
    let set = move |vm: &SessionVm, href: String| {
        with_editor(vm, |h| {
            if href.trim().is_empty() {
                h.clear_link()
            } else {
                h.set_link(href.trim())
            }
        })
    };
    let (set1, set2) = (set.clone(), set);
    PopoverIconButton::new(
        IconButton::new(Icon::Link.widget())
            .tooltip(tr!(link()))
            .toggle(vm.0.format.link.clone()),
    )
    .has_popup_kind(accesskit::HasPopup::Dialog)
    .show_disclosure_caret(false)
    .on_open(move || {
        a1.set(
            open.editing_handle()
                .and_then(|h| h.link_at_caret())
                .map(|l| l.href)
                .unwrap_or_default(),
        )
    })
    .content(
        Padding::uniform(12.).child(
            VStack::new()
                .spacing(10.)
                .child(
                    FixedSize::new().width(280.).child(
                        TextInput::new(address)
                            .label(tr!(link_address()))
                            .placeholder(lit!("https://"))
                            .on_submit_fn(move |ctx| {
                                set1(&submit, a2.get());
                                ctx.dismiss_self_overlay_chain();
                            }),
                    ),
                )
                .child(
                    HStack::new()
                        .spacing(8.)
                        .child(Spacer::new())
                        .child(
                            Button::new(tr!(remove_link()))
                                .variant(ButtonVariant::Plain)
                                .on_activate_fn(move |ctx| {
                                    with_editor(&remove, |h| h.clear_link());
                                    ctx.dismiss_self_overlay_chain();
                                }),
                        )
                        .child(
                            Button::new(tr!(apply()))
                                .variant(ButtonVariant::Filled)
                                .on_activate_fn(move |ctx| {
                                    set2(&apply, a3.get());
                                    ctx.dismiss_self_overlay_chain();
                                }),
                        ),
                ),
        ),
    )
}

pub fn format_bar(vm: &SessionVm) -> impl Widget + 'static {
    let f = &vm.0.format;
    let (align_vm, table_vm) = (vm.clone(), vm.clone());
    let alignment = ToolbarAction::new(tr!(alignment()), || Icon::AlignLeft.widget())
        .tooltip(tr!(alignment()))
        .menu(move || {
            let v = &align_vm;
            let item = |label, icon: Icon, a: fn(&EditorHandle)| {
                menu_item(v, label, a).icon(icon.widget())
            };
            MenuList::new()
                .item(item(tr!(align_left()), Icon::AlignLeft, |h| {
                    h.set_alignment(Alignment::Left)
                }))
                .item(item(tr!(align_center()), Icon::AlignCenter, |h| {
                    h.set_alignment(Alignment::Center)
                }))
                .item(item(tr!(align_right()), Icon::AlignRight, |h| {
                    h.set_alignment(Alignment::Right)
                }))
                .item(item(tr!(align_justify()), Icon::AlignJustify, |h| {
                    h.set_alignment(Alignment::Justify)
                }))
        });
    let table = ToolbarAction::new(tr!(table_menu()), || Icon::Table.widget())
        .tooltip(tr!(table_menu()))
        .menu(move || {
            let v = &table_vm;
            let in_table = v.0.format.in_table.clone();
            MenuList::new()
                .item(
                    menu_item(v, tr!(table()), |h| h.insert_table(3, 3)).icon(Icon::Table.widget()),
                )
                .separator()
                .item(menu_item(v, tr!(row()), |h| h.insert_row_below()).enabled(in_table.clone()))
                .item(
                    menu_item(v, tr!(column()), |h| h.insert_column_after())
                        .enabled(in_table.clone()),
                )
                .separator()
                .item(
                    menu_item(v, tr!(remove_row()), |h| h.remove_current_row())
                        .enabled(in_table.clone()),
                )
                .item(
                    menu_item(v, tr!(remove_column()), |h| h.remove_current_column())
                        .enabled(in_table.clone()),
                )
                .item(
                    menu_item(v, tr!(remove_table()), |h| h.remove_current_table())
                        .enabled(in_table)
                        .text_role(TextRole::Error),
                )
        });
    let escape = vm.clone();
    Padding::new(6., 12., 6., 16.)
        .child(
            Toolbar::new()
                .label(tr!(text_formatting()))
                .button_size(IconButtonSize::Toolbar)
                .item(ToolbarItem::custom(
                    FixedSize::new().width(140.).child(paragraph_style(vm)),
                ))
                .item(ToolbarItem::separator())
                .action(
                    toggle(vm, tr!(bold()), Icon::Bold, &f.bold, |h, on| h.set_bold(on))
                        .priority(9),
                )
                .action(
                    toggle(vm, tr!(italic()), Icon::Italic, &f.italic, |h, on| {
                        h.set_italic(on)
                    })
                    .priority(8),
                )
                .action(
                    toggle(
                        vm,
                        tr!(underline()),
                        Icon::Underline,
                        &f.underline,
                        |h, on| h.set_underline(on),
                    )
                    .priority(6),
                )
                .action(
                    toggle(vm, tr!(strike()), Icon::Strike, &f.strike, |h, on| {
                        h.set_strikethrough(on)
                    })
                    .priority(4),
                )
                .item(ToolbarItem::separator())
                .action(
                    action(vm, tr!(bullets()), Icon::List, |h| h.insert_list(false)).priority(7),
                )
                .action(
                    action(vm, tr!(numbered()), Icon::ListOrdered, |h| {
                        h.insert_list(true)
                    })
                    .priority(5),
                )
                .item(ToolbarItem::separator())
                .item(ToolbarItem::custom(link(vm)))
                .action(alignment.priority(2))
                .action(table.priority(3))
                .item(ToolbarItem::flexible_space()),
        )
        // Esc on the toolbar finishes editing too.
        .on_key(move |event, _| match event {
            WidgetEvent::KeyDown {
                key: Key::Escape, ..
            } => {
                escape.0.editing.set(None);
                EventResponse::Handled
            }
            _ => EventResponse::Ignored,
        })
}
