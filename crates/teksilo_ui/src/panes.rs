//! The Folders and Notes panes: lists that carry information, inline rename,
//! context menus, and the trash as a place rather than a footer button.
use crate::{
    icons::Icon,
    session_vm::{Row, SessionVm},
    status::offer_undo,
    theme::PANEL_PADDING,
    widgets::{AutoFocus, caption, icon_button},
};
use ::common::types::EntityId;
use teksilo::data::KeyedSelectionModel;
use teksilo::tokens::VAlignment;
use teksilo::{prelude::*, widgets::*};

fn key_down(event: &WidgetEvent) -> Option<(Key, Modifiers)> {
    match event {
        WidgetEvent::KeyDown { key, modifiers, .. } => Some((*key, *modifiers)),
        _ => None,
    }
}

fn pane_header(
    title: impl Into<teksilo::i18n::LocalizedString>,
    trailing: impl Widget + 'static,
) -> Padding {
    Padding::new(0., 2., 6., 8.).child(
        HStack::new()
            .spacing(8.)
            .alignment(VAlignment::Center)
            .child(TextWidget::new(title).style(TextStyleRole::BodyBold))
            .child(Spacer::new())
            .child(trailing),
    )
}

/// The inline rename field that replaces a folder row's label.
fn folder_rename_field(vm: &SessionVm, row: &Row) -> Box<dyn Widget> {
    let id = row.id;
    let text = vm.folder_rename_text(id, &row.title);
    let (submit, blur, cancel) = (vm.clone(), vm.clone(), vm.clone());
    let (t1, t2) = (text.clone(), text.clone());
    let input = TextInput::new(text)
        .label(tr!(rename()))
        .on_submit_fn(move |_| submit.report(submit.rename_folder(id, t1.get())))
        .on_blur_fn(move |_| {
            if blur.0.renaming_folder.get() == Some(id) {
                blur.report(blur.rename_folder(id, t2.get()));
            }
        });
    let focus = AutoFocus::new(input).once(vm.0.folder_rename_focused.clone());
    Box::new(focus.on_key_preview(move |event, _| {
        if matches!(key_down(event), Some((Key::Escape, _))) {
            cancel.0.renaming_folder.set(None);
            *cancel.0.folder_rename.borrow_mut() = None;
            cancel.report(cancel.refresh());
            EventResponse::Handled
        } else {
            EventResponse::Ignored
        }
    }))
}

fn folder_menu(vm: &SessionVm, id: EntityId) -> MenuList {
    let trashed = vm.0.service.folder(id).is_ok_and(|f| f.trashed);
    let (rename, trash) = (vm.clone(), vm.clone());
    MenuList::new()
        .item(
            MenuItem::new(tr!(rename()))
                .icon(Icon::Pen.widget())
                .shortcut_label("F2")
                .enabled(!trashed)
                .on_activate_fn(move |_| rename.report(rename.start_folder_rename(id))),
        )
        .separator()
        .item(
            if trashed {
                MenuItem::new(tr!(restore_folder())).icon(Icon::Restore.widget())
            } else {
                MenuItem::new(tr!(move_to_trash()))
                    .icon(Icon::Trash.widget())
                    .text_role(TextRole::Error)
                    .shortcut_label("Del")
            }
            .on_activate_fn(move |ctx| {
                trash.report(trash.trash_folder(id));
                offer_undo(&trash, ctx);
            }),
        )
}

pub fn folders_pane(vm: &SessionVm, selection: KeyedSelectionModel<u64>) -> impl Widget + 'static {
    let v = vm.clone();
    let delegate_vm = vm.clone();
    let drop_vm = vm.clone();
    let reorder: crate::models::navigation::Reorder =
        std::rc::Rc::new(move |key, target, position| {
            let after = position == teksilo::data::DropPosition::After;
            drop_vm.drop_folder(key, target, after).unwrap_or_else(|e| {
                drop_vm.report(Err(e));
                false
            })
        });
    let list = ListView::from_source_keyed(
        crate::models::navigation::NavigationSource(vm.0.folders.clone(), Some(reorder)),
        selection,
        move |_, row: &Row, selected| {
            let vm = delegate_vm.clone();
            let id = row.id;
            if vm.0.renaming_folder.get() == Some(id) {
                return Box::new(
                    StandardListItem::new(lit!(row.title.clone()))
                        .leading_slot(Icon::Folder.widget().color(TextRole::Accent))
                        .label_slot(folder_rename_field(&vm, row))
                        .selected(selected),
                );
            }
            let mut item = StandardListItem::new(lit!(row.title.clone()))
                .leading_slot(Icon::Folder.widget().color(if selected {
                    TextRole::Accent
                } else {
                    TextRole::Secondary
                }))
                .trailing_slot(caption(lit!(row.meta.clone())))
                .selected(selected);
            if selected {
                item = item.label_style(TextStyleRole::BodyBold);
            }
            if row.trashed {
                item = item
                    .subtitle(lit!(row.subtitle.clone()))
                    .subtitle_leading_slot(Icon::Trash.sized(12.).color(TextRole::Secondary));
            }
            Box::new(item.context_menu(move |_, _| Some(Box::new(folder_menu(&vm, id)))))
        },
    )
    .activate_on(ActivateOn::SingleClick)
    .reorderable(true)
    .on_activate(move |index, _| {
        if let Some(id) = v.0.folders.with_item(index, |r| r.id) {
            v.report(v.select_folder(id));
        }
    })
    .type_ahead_label(|r| r.title.clone());
    let keys = vm.clone();
    let list = list.on_key(move |event, ctx| {
        let Some(id) = keys.0.folder.get() else {
            return EventResponse::Ignored;
        };
        match key_down(event) {
            Some((Key::F2, _)) if keys.0.renaming_folder.get().is_none() => {
                keys.report(keys.start_folder_rename(id));
                EventResponse::Handled
            }
            Some((Key::Delete, _)) => {
                keys.report(keys.trash_folder(id));
                offer_undo(&keys, ctx);
                EventResponse::Handled
            }
            _ => EventResponse::Ignored,
        }
    });
    let new_folder = icon_button(Icon::Plus, tr!(new_folder()))
        .size(IconButtonSize::Compact)
        .on_activate_fn(|ctx| ctx.send_intent(Intent::new("folder.new")));
    let count = Signal::new(String::new());
    let c = count.clone();
    let t = vm.clone();
    vm.keep(
        vm.0.folders
            .observe_changes(move |_| c.set(t.trash_count().to_string())),
    );
    count.set(vm.trash_count().to_string());
    let (trash_vm, key_vm, click_vm) = (vm.clone(), vm.clone(), vm.clone());
    let trash = StandardListItem::new(tr!(trash()))
        .leading_slot(Icon::Trash.widget().color(TextRole::Secondary))
        .trailing_slot(
            TextWidget::new(lit!(""))
                .text(count)
                .style(TextStyleRole::Small)
                .color(TextRole::Secondary),
        )
        .selected(vm.0.trash.clone())
        .tooltip(tr!(show_trash()))
        .on_tap(move |_, _| trash_vm.report(toggle_trash(&trash_vm)))
        .on_key(move |event, _| match key_down(event) {
            Some((Key::Enter | Key::Space, _)) => {
                key_vm.report(toggle_trash(&key_vm));
                EventResponse::Handled
            }
            _ => EventResponse::Ignored,
        })
        .focusable(true)
        .access_role(accesskit::Role::Button)
        .access_label(tr!(trash()))
        .access_action(accesskit::Action::Click, move |_| {
            click_vm.report(toggle_trash(&click_vm))
        });
    Padding::uniform(PANEL_PADDING).child(
        VStack::new()
            .spacing(2.)
            .child(pane_header(tr!(folders()), new_folder))
            .child(Expand::new().child(list))
            .child(Padding::symmetric(6., 4.).child(Divider::horizontal()))
            .child(trash),
    )
}

fn toggle_trash(vm: &SessionVm) -> anyhow::Result<()> {
    vm.0.trash.set(!vm.0.trash.get());
    vm.refresh()?;
    vm.rebuild_page();
    Ok(())
}

pub fn note_menu(vm: &SessionVm, id: EntityId) -> MenuList {
    let trashed = vm.0.service.note(id).is_ok_and(|n| n.trashed);
    let (rename, trash, moves) = (vm.clone(), vm.clone(), vm.clone());
    let folders = vm.other_folders(id);
    MenuList::new()
        .item(
            MenuItem::new(tr!(rename_note()))
                .icon(Icon::Pen.widget())
                .shortcut_label("F2")
                .enabled(!trashed)
                .on_activate_fn(move |_| {
                    if rename.0.note.get() != Some(id) {
                        rename.report(rename.select_note(Some(id)));
                    }
                    rename.0.renaming_note.set(true);
                }),
        )
        .item(
            MenuItem::submenu(tr!(move_to_folder()), move || {
                let mut list = MenuList::new();
                for (folder, title) in folders.clone() {
                    let vm = moves.clone();
                    list = list.item(
                        MenuItem::new(lit!(title))
                            .icon(Icon::Folder.widget())
                            .on_activate_fn(move |ctx| {
                                vm.report(vm.move_note_to(id, folder));
                                ctx.dismiss_all_overlays();
                            }),
                    );
                }
                Box::new(list)
            })
            .icon(Icon::Folder.widget())
            .enabled(!trashed && !vm.other_folders(id).is_empty()),
        )
        .separator()
        .item(
            if trashed {
                MenuItem::new(tr!(restore_note())).icon(Icon::Restore.widget())
            } else {
                MenuItem::new(tr!(move_to_trash()))
                    .icon(Icon::Trash.widget())
                    .text_role(TextRole::Error)
                    .shortcut_label("Del")
            }
            .on_activate_fn(move |ctx| {
                trash.report(trash.trash_note(id));
                offer_undo(&trash, ctx);
            }),
        )
}

pub fn notes_pane(vm: &SessionVm, selection: KeyedSelectionModel<u64>) -> impl Widget + 'static {
    let v = vm.clone();
    let delegate_vm = vm.clone();
    let drop_vm = vm.clone();
    let reorder: crate::models::navigation::Reorder =
        std::rc::Rc::new(move |key, target, position| {
            let after = position == teksilo::data::DropPosition::After;
            drop_vm.drop_note(key, target, after).unwrap_or_else(|e| {
                drop_vm.report(Err(e));
                false
            })
        });
    let list = ListView::from_source_keyed(
        crate::models::navigation::NavigationSource(vm.0.notes.clone(), Some(reorder)),
        selection,
        move |_, row: &Row, selected| {
            let vm = delegate_vm.clone();
            let id = row.id;
            let mut item = StandardListItem::new(lit!(row.title.clone()))
                .trailing_slot(caption(lit!(row.meta.clone())))
                .label_overflow(TextOverflow::Ellipsis(EllipsisMode::Trailing))
                .selected(selected);
            if selected {
                item = item.label_style(TextStyleRole::BodyBold);
            }
            if !row.subtitle.is_empty() {
                item = item
                    .subtitle(lit!(row.subtitle.clone()))
                    .subtitle_overflow(TextOverflow::Ellipsis(EllipsisMode::Trailing));
            }
            if row.trashed {
                item =
                    item.subtitle_leading_slot(Icon::Trash.sized(12.).color(TextRole::Secondary));
            } else if row.sketch {
                item = item.subtitle_leading_slot(Icon::Pen.sized(12.).color(TextRole::Secondary));
            }
            Box::new(item.context_menu(move |_, _| Some(Box::new(note_menu(&vm, id)))))
        },
    )
    .activate_on(ActivateOn::SingleClick)
    .reorderable(true)
    .on_activate(move |index, _| {
        if let Some(id) = v.0.notes.with_item(index, |r| r.id) {
            v.report(v.select_note(Some(id)));
        }
    })
    .type_ahead_label(|r| r.title.clone());
    let keys = vm.clone();
    let list = list.on_key(move |event, ctx| {
        let Some(id) = keys.0.note.get() else {
            return EventResponse::Ignored;
        };
        match key_down(event) {
            Some((Key::F2, _)) => {
                keys.0.renaming_note.set(true);
                EventResponse::Handled
            }
            Some((Key::Delete, _)) => {
                keys.report(keys.trash_note(id));
                offer_undo(&keys, ctx);
                EventResponse::Handled
            }
            _ => EventResponse::Ignored,
        }
    });
    // Folder name and count, or "Search results" while searching.
    let heading = Signal::new(String::new());
    let count = Signal::new(String::new());
    let update = {
        let (vm, heading, count) = (vm.clone(), heading.clone(), count.clone());
        move || {
            let searching = !vm.0.query.get().trim().is_empty();
            heading.set(if searching {
                tr!(search_results()).resolve_now()
            } else {
                vm.0.folder
                    .get()
                    .and_then(|id| vm.0.service.folder(id).ok())
                    .map(|f| f.title)
                    .unwrap_or_default()
            });
            count.set(tr!(note_count(count = vm.0.notes.len() as i64)).resolve_now());
        }
    };
    update();
    let empty = Signal::new(vm.0.notes.is_empty());
    let (u, e, notes) = (update.clone(), empty.clone(), vm.0.notes.clone());
    vm.keep(vm.0.notes.observe_changes(move |_| {
        u();
        e.set(notes.is_empty());
    }));
    let new_note = Button::new(tr!(new_note()))
        .variant(ButtonVariant::Ghost)
        .icon(Icon::Plus.widget(), IconLocation::Leading)
        .text_role(TextRole::Accent)
        .enabled(vm.0.folder.map(|id| id.is_some()))
        .on_activate_fn(|ctx| ctx.send_intent(Intent::new("note.new")));
    Padding::uniform(PANEL_PADDING).child(
        VStack::new()
            .spacing(2.)
            .child(
                Padding::new(0., 2., 6., 10.).child(
                    HStack::new()
                        .spacing(8.)
                        .alignment(VAlignment::Center)
                        .child(
                            TextWidget::new(lit!(""))
                                .text(heading)
                                .style(TextStyleRole::BodyBold)
                                .single_line(),
                        )
                        .child(
                            TextWidget::new(lit!(""))
                                .text(count)
                                .style(TextStyleRole::Small)
                                .color(TextRole::Secondary)
                                .single_line(),
                        )
                        .child(Spacer::new())
                        .child(new_note),
                ),
            )
            .child(
                Padding::symmetric(8., 10.)
                    .child(caption(tr!(empty_folder())))
                    .visible_when(empty),
            )
            .child(Expand::new().child(list)),
    )
}
