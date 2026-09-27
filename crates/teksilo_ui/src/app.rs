use crate::{
    icons::Icon,
    session_vm::{SessionVm, UndoInfo},
    widgets::Rebuild,
};
use ::common::entities::BlockKind;
use teksilo::core::MenuItemId;
use teksilo::tokens::VAlignment;
use teksilo::widgets::primitives::icon_widget::IconMode;
use teksilo::{prelude::*, widgets::*};

/// The Edit menu's id; the menu is rebuilt when the undo targets change so
/// every entry names what it undoes.
const EDIT_MENU: u64 = 0x004e_6f74_6f6e_0001;

fn edit_items(info: &UndoInfo) -> impl FnOnce(MenuItems) -> MenuItems {
    let named = |action: &Option<String>, undo: bool| match (action, undo) {
        (Some(a), true) => tr!(undo_named(action = a.clone())).resolve_now(),
        (Some(a), false) => tr!(redo_named(action = a.clone())).resolve_now(),
        (None, true) => tr!(undo_nothing()).resolve_now(),
        (None, false) => tr!(redo_nothing()).resolve_now(),
    };
    let (undo, redo, can_undo, can_redo) = if info.typing_in.is_some() {
        (
            tr!(undo_typing()).resolve_now(),
            tr!(redo_typing()).resolve_now(),
            true,
            true,
        )
    } else if info.library_focused {
        (
            named(&info.library_undo, true),
            named(&info.library_redo, false),
            info.library_undo.is_some(),
            info.library_redo.is_some(),
        )
    } else {
        (
            named(&info.page_undo, true),
            named(&info.page_redo, false),
            info.page_undo.is_some(),
            info.page_redo.is_some(),
        )
    };
    let scoped = |action: &Option<String>, page: bool, undo: bool| {
        let a = action.clone().unwrap_or_default();
        lit!(match (page, undo) {
            (true, true) => tr!(page_undo_named(action = a)).resolve_now(),
            (true, false) => tr!(page_redo_named(action = a)).resolve_now(),
            (false, true) => tr!(library_undo_named(action = a)).resolve_now(),
            (false, false) => tr!(library_redo_named(action = a)).resolve_now(),
        })
    };
    let info = info.clone();
    move |m: MenuItems| {
        m.item(
            MenuEntry::new(lit!(undo))
                .intent("text.undo")
                .shortcut("text.undo")
                .enabled(can_undo),
        )
        .item(
            MenuEntry::new(lit!(redo))
                .intent("text.redo")
                .shortcut("text.redo")
                .enabled(can_redo),
        )
        .separator()
        .item(
            MenuEntry::new(scoped(&info.page_undo, true, true))
                .intent("page.undo")
                .enabled(info.page_undo.is_some()),
        )
        .item(
            MenuEntry::new(scoped(&info.page_redo, true, false))
                .intent("page.redo")
                .enabled(info.page_redo.is_some()),
        )
        .separator()
        .item(
            MenuEntry::new(scoped(&info.library_undo, false, true))
                .intent("library.undo")
                .enabled(info.library_undo.is_some()),
        )
        .item(
            MenuEntry::new(scoped(&info.library_redo, false, false))
                .intent("library.redo")
                .enabled(info.library_redo.is_some()),
        )
    }
}

pub fn menu() -> MenuModel {
    MenuModel::new()
        .menu(tr!(file()), |m| {
            m.item(MenuEntry::new(tr!(open_library())).intent("file.open"))
                .item(MenuEntry::new(tr!(new_library())).intent("file.new"))
                .separator()
                .item(MenuEntry::new(tr!(export_text())).intent("file.export_text"))
                .item(MenuEntry::new(tr!(export_html())).intent("file.export_html"))
                .separator()
                .item(
                    MenuEntry::new(tr!(save()))
                        .intent("file.save")
                        .shortcut("file.save"),
                )
                .item(MenuEntry::new(tr!(backup())).intent("file.backup"))
                .separator()
                .item(
                    MenuEntry::new(tr!(close()))
                        .intent("file.close")
                        .shortcut("file.close"),
                )
        })
        .menu_with_id(
            MenuItemId::from_raw(EDIT_MENU),
            tr!(edit()),
            edit_items(&UndoInfo::default()),
        )
        .menu(tr!(insert()), |m| {
            m.item(
                MenuEntry::new(tr!(insert_text()))
                    .intent("block.text")
                    .shortcut("block.text"),
            )
            .item(
                MenuEntry::new(tr!(insert_sketch()))
                    .intent("block.sketch")
                    .shortcut("block.sketch"),
            )
            .item(
                MenuEntry::new(tr!(insert_image()))
                    .intent("block.image")
                    .shortcut("block.image"),
            )
            .separator()
            .item(
                MenuEntry::new(tr!(new_note()))
                    .intent("note.new")
                    .shortcut("note.new"),
            )
            .item(
                MenuEntry::new(tr!(new_folder()))
                    .intent("folder.new")
                    .shortcut("folder.new"),
            )
        })
        .menu(tr!(view()), |m| {
            m.item(
                MenuEntry::new(tr!(canvas()))
                    .intent("view.canvas")
                    .shortcut("view.canvas"),
            )
            .item(
                MenuEntry::new(tr!(reading()))
                    .intent("view.reading")
                    .shortcut("view.reading"),
            )
            .separator()
            .item(
                MenuEntry::new(tr!(zoom_in()))
                    .intent("zoom.in")
                    .shortcut("zoom.in"),
            )
            .item(
                MenuEntry::new(tr!(zoom_out()))
                    .intent("zoom.out")
                    .shortcut("zoom.out"),
            )
            .item(
                MenuEntry::new(tr!(zoom_reset()))
                    .intent("zoom.reset")
                    .shortcut("zoom.reset"),
            )
            .item(
                MenuEntry::new(tr!(zoom_fit()))
                    .intent("zoom.fit")
                    .shortcut("zoom.fit"),
            )
            .separator()
            .item(MenuEntry::new(tr!(show_trash())).intent("view.trash"))
            .separator()
            .item(
                MenuEntry::new(tr!(settings_menu()))
                    .intent("view.settings")
                    .shortcut("view.settings"),
            )
        })
}

/// The title bar's contents: app mark, library name and the search field.
pub fn title_parts(vm: &SessionVm, tree: &mut teksilo::core::WidgetTree) -> (WidgetId, WidgetId) {
    let search = SearchField::new(vm.0.query.clone())
        .placeholder(tr!(search_library()))
        .label(tr!(search_library()));
    let search = tree.add(FixedSize::new().width(340.).child(search));
    vm.0.search_field.set(Some(search));
    let mark = tree.add(
        HStack::new()
            .spacing(8.)
            .alignment(VAlignment::Center)
            .child(
                IconWidget::from_svg(include_str!("../resources/mark.svg"))
                    .icon_size(18.)
                    .mode(IconMode::FullColor),
            )
            .child(TextWidget::new(lit!("Noton")).style(TextStyleRole::BodyBold))
            .child(TextWidget::new(lit!("/")).color(TextRole::Disabled))
            .child(
                TextWidget::new(lit!(vm.library_name()))
                    .color(TextRole::Secondary)
                    .single_line(),
            ),
    );
    (mark, search)
}

pub struct AppRoot {
    vm: SessionVm,
    menu: MenuModel,
    root: Option<WidgetId>,
    tick: Option<teksilo::core::frame_tick_scheduler::FrameTickSubscription>,
    folder_selection: teksilo::data::KeyedSelectionModel<u64>,
    note_selection: teksilo::data::KeyedSelectionModel<u64>,
    split: SplitterModel,
}
impl std::fmt::Debug for AppRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppRoot").finish_non_exhaustive()
    }
}
impl AppRoot {
    pub fn new(vm: SessionVm, menu: MenuModel) -> Self {
        Self {
            vm,
            menu,
            root: None,
            tick: None,
            folder_selection: teksilo::data::KeyedSelectionModel::new(
                teksilo::data::SelectionMode::Single,
            ),
            note_selection: teksilo::data::KeyedSelectionModel::new(
                teksilo::data::SelectionMode::Single,
            ),
            split: SplitterModel::from_panes(
                vec![
                    PaneDescriptor::new()
                        .size(208.)
                        .min_size(150.)
                        .stretch(0.)
                        .collapsible(true),
                    PaneDescriptor::new()
                        .size(272.)
                        .min_size(180.)
                        .stretch(0.)
                        .collapsible(true),
                    PaneDescriptor::new().min_size(320.).stretch(1.),
                ],
                Orientation::Horizontal,
            ),
        }
    }
}

fn shortcut(ctx: &mut BuildContext, intent: &'static str, stroke: KeyStroke) {
    ctx.register_shortcut_global(Shortcut::new(intent).primary(stroke).intent(intent).build());
}

fn register_actions(vm: &SessionVm, ctx: &mut BuildContext) {
    for intent in [
        "file.save",
        "file.backup",
        "folder.new",
        "note.new",
        "block.text",
        "block.sketch",
        "page.undo",
        "page.redo",
        "library.undo",
        "library.redo",
        "text.undo",
        "text.redo",
        "view.canvas",
        "view.reading",
        "view.trash",
        "zoom.in",
        "zoom.out",
        "zoom.reset",
        "zoom.fit",
        "nav.back",
    ] {
        let v = vm.clone();
        ctx.register_action_global(Action::new(intent).on_invoke(move |_, _| {
            let result = match intent {
                "file.save" => v.save(),
                "file.backup" => v.backup(),
                "folder.new" => v.new_folder().and_then(|_| match v.0.folder.get() {
                    Some(id) => v.start_folder_rename(id),
                    None => Ok(()),
                }),
                "note.new" => v.new_note().map(|_| v.0.renaming_note.set(true)),
                "block.text" => v.insert(BlockKind::RichText),
                "block.sketch" => v.insert(BlockKind::Drawing),
                "page.undo" => v.undo(false, false),
                "page.redo" => v.undo(true, false),
                "library.undo" => v.undo(false, true),
                "library.redo" => v.undo(true, true),
                "text.undo" | "text.redo" => v.0.edit.invoke(&v, intent == "text.redo"),
                "view.canvas" => v.flush().map(|_| v.0.reading.set(false)),
                "view.reading" => v.flush().map(|_| {
                    v.0.drawing.set(None);
                    v.0.reading.set(true)
                }),
                "view.trash" => {
                    v.0.trash.set(!v.0.trash.get());
                    v.refresh().map(|_| v.rebuild_page())
                }
                "zoom.in" => {
                    v.zoom_step(true);
                    Ok(())
                }
                "zoom.out" => {
                    v.zoom_step(false);
                    Ok(())
                }
                "zoom.reset" => {
                    v.zoom_to(1.);
                    Ok(())
                }
                "zoom.fit" => v.zoom_to_fit(),
                "nav.back" => {
                    v.0.navigation.set(v.0.navigation.get().saturating_sub(1));
                    Ok(())
                }
                _ => Ok(()),
            };
            v.report(result);
        }));
    }
    {
        let vm = vm.clone();
        ctx.register_action_global(Action::new("block.image").on_invoke(move |_, ctx| {
            let vm = vm.clone();
            let _ = ctx.pick_file(
                FileDialogRequest::pick_file()
                    .title(tr!(image()))
                    .add_filter(tr!(image()).resolve_now(), &["png", "jpg", "jpeg", "webp"]),
                move |result, _| {
                    if let FileDialogResult::File(Some(path)) = result {
                        vm.report(vm.import_image(&path));
                    }
                },
            );
        }));
    }
    for (intent, html) in [("file.export_text", false), ("file.export_html", true)] {
        let vm = vm.clone();
        ctx.register_action_global(Action::new(intent).on_invoke(move |_, ctx| {
            let vm = vm.clone();
            let title = if html {
                tr!(export_html())
            } else {
                tr!(export_text())
            };
            let _ = ctx.save_file(
                FileDialogRequest::save_file()
                    .title(title)
                    .default_file_name(if html { "Note.html" } else { "Note.txt" }),
                move |result, _| {
                    if let FileDialogResult::Saved(Some(path)) = result {
                        vm.report(vm.export_note(&path, html));
                    }
                },
            );
        }));
    }
    {
        let vm = vm.clone();
        ctx.register_action_global(Action::new("file.save_copy").on_invoke(move |_, ctx| {
            let vm = vm.clone();
            let _ = ctx.save_file(
                FileDialogRequest::save_file()
                    .title(tr!(save_copy()))
                    .default_file_name("Notes copy.noton"),
                move |result, _| {
                    if let FileDialogResult::Saved(Some(path)) = result {
                        vm.report(vm.save_copy(&path));
                    }
                },
            );
        }));
    }
    for (intent, new) in [("file.open", false), ("file.new", true)] {
        let vm = vm.clone();
        ctx.register_action_global(Action::new(intent).on_invoke(move |_, ctx| {
            let vm = vm.clone();
            let open = move |result: FileDialogResult, _: &mut EventContext| {
                let path = match result {
                    FileDialogResult::File(Some(p)) | FileDialogResult::Saved(Some(p)) => p,
                    _ => return,
                };
                vm.report((|| {
                    if new {
                        anyhow::ensure!(!path.exists(), "Choose a new library filename");
                    } else {
                        anyhow::ensure!(path.exists(), "Library does not exist");
                    }
                    std::process::Command::new(std::env::current_exe()?)
                        .arg("--library")
                        .arg(path)
                        .spawn()?;
                    Ok(())
                })());
            };
            if new {
                let _ = ctx.save_file(
                    FileDialogRequest::save_file()
                        .title(tr!(new_library()))
                        .default_file_name("Notes.noton"),
                    open,
                );
            } else {
                let _ = ctx.pick_file(
                    FileDialogRequest::pick_file()
                        .title(tr!(open_library()))
                        .add_filter("Noton", &["noton"]),
                    open,
                );
            }
        }));
    }
    ctx.register_action_global(Action::new("file.close").on_invoke(|_, ctx| ctx.close_window()));
    let edit = vm.0.edit.clone();
    let settings_vm = vm.clone();
    ctx.register_action_global(Action::new("view.settings").on_invoke(move |_, ctx| {
        crate::settings::present(ctx, edit.clone(), settings_vm.clone());
    }));
    for (intent, position) in [("block.position", true), ("block.describe", false)] {
        let v = vm.clone();
        ctx.register_action_global(Action::new(intent).on_invoke(move |_, ctx| {
            if let Some(id) = v.0.dialog_block.take() {
                if position {
                    crate::dialogs::position_and_size(&v, id, ctx)
                } else {
                    crate::dialogs::describe(&v, id, ctx)
                }
            }
        }));
    }
    let search = vm.clone();
    ctx.register_action_global(Action::new("search.focus").on_invoke(move |_, ctx| {
        if let Some(id) = search.0.search_field.get() {
            ctx.request_focus_into(id);
        }
    }));
    let primary_alt = Modifiers::COMMAND | Modifiers::ALT;
    shortcut(ctx, "file.close", KeyStroke::command(Key::Q));
    shortcut(ctx, "text.undo", KeyStroke::command(Key::Z));
    shortcut(ctx, "text.redo", KeyStroke::command_shift(Key::Z));
    shortcut(ctx, "file.save", KeyStroke::command(Key::S));
    shortcut(ctx, "note.new", KeyStroke::command(Key::N));
    shortcut(ctx, "folder.new", KeyStroke::command_shift(Key::N));
    shortcut(ctx, "search.focus", KeyStroke::command_shift(Key::F));
    shortcut(ctx, "view.canvas", KeyStroke::command(Key::Character('1')));
    shortcut(ctx, "view.reading", KeyStroke::command(Key::Character('2')));
    shortcut(ctx, "zoom.in", KeyStroke::command(Key::Character('=')));
    shortcut(ctx, "zoom.out", KeyStroke::command(Key::Character('-')));
    shortcut(ctx, "zoom.reset", KeyStroke::command(Key::Character('0')));
    shortcut(ctx, "zoom.fit", KeyStroke::command(Key::Character('9')));
    shortcut(
        ctx,
        "view.settings",
        KeyStroke::command(Key::Character(',')),
    );
    shortcut(ctx, "block.text", KeyStroke::new(Key::T, primary_alt));
    shortcut(ctx, "block.sketch", KeyStroke::new(Key::D, primary_alt));
    shortcut(ctx, "block.image", KeyStroke::new(Key::I, primary_alt));
    shortcut(ctx, "nav.back", KeyStroke::alt(Key::ArrowLeft));
}

/// The narrow-window trail: Back, then Folders › folder › note.
fn narrow_bar(vm: &SessionVm) -> impl Widget + 'static {
    let v = vm.clone();
    let trail = Rebuild::new(move || {
        let vm = v.clone();
        let stage = vm.0.navigation.get();
        let folder =
            vm.0.folder
                .get()
                .and_then(|id| vm.0.service.folder(id).ok())
                .map(|f| f.title);
        let note = vm.0.title.get();
        let go = |stage: usize| {
            let vm = vm.clone();
            move |_: &mut EventContext| vm.0.navigation.set(stage)
        };
        let mut crumbs = Breadcrumb::new().label(tr!(folders()));
        crumbs = crumbs.item(if stage == 0 {
            BreadcrumbItem::current(tr!(folders()))
        } else {
            BreadcrumbItem::new(tr!(folders())).on_activate_fn(go(0))
        });
        if stage >= 1 {
            let label = lit!(folder.unwrap_or_default());
            crumbs = crumbs.item(if stage == 1 {
                BreadcrumbItem::current(label)
            } else {
                BreadcrumbItem::new(label).on_activate_fn(go(1))
            });
        }
        if stage >= 2 {
            crumbs = crumbs.item(BreadcrumbItem::current(lit!(note)));
        }
        Box::new(crumbs)
    })
    .on(&vm.0.navigation)
    .on(&vm.0.folder)
    .on(&vm.0.title);
    let back = vm.clone();
    Padding::symmetric(6., 8.).child(
        HStack::new()
            .spacing(8.)
            .alignment(VAlignment::Center)
            .child(
                Button::new(tr!(back()))
                    .variant(ButtonVariant::Plain)
                    .icon(Icon::ChevronLeft.widget(), IconLocation::Leading)
                    .enabled(vm.0.navigation.map(|s| *s > 0))
                    .on_activate_fn(move |_| {
                        back.0
                            .navigation
                            .set(back.0.navigation.get().saturating_sub(1))
                    }),
            )
            .child(Expand::horizontal().child(trail)),
    )
}

impl Widget for AppRoot {
    fn build(&mut self, ctx: &mut BuildContext) -> Vec<WidgetId> {
        let vm = self.vm.clone();
        vm.0.edit.wire(ctx);
        let scale = vm.0.prefs.borrow().text_scale;
        if (scale - 1.).abs() > 0.001 {
            ctx.run_after_mount(move |ec| ec.set_text_scale(scale));
        }
        self.tick = Some(ctx.subscribe_frame_tick_throttled(std::time::Duration::from_millis(100)));
        let v = vm.clone();
        ctx.effect(&ctx.frame_tick(), move |_| v.report(v.tick()));
        let v = vm.clone();
        ctx.effect(&vm.0.query, move |_| v.report(v.refresh()));
        // Named undo: rebuild the Edit menu when what it would undo changes.
        let (menu, locale) = (self.menu.clone(), ctx.locale_signal());
        let rebuild = move |info: &UndoInfo| {
            let id = MenuItemId::from_raw(EDIT_MENU);
            if menu.remove(id) {
                menu.insert_menu_at(1, id, tr!(edit()), edit_items(info));
            }
        };
        let r = rebuild.clone();
        ctx.effect(&vm.0.undo_info, move |info| r(info));
        let info = vm.0.undo_info.clone();
        ctx.effect(&locale, move |_| rebuild(&info.get()));
        // Row meta ("3 blocks") and the page's labels are resolved text.
        let v = vm.clone();
        ctx.effect(&ctx.locale_signal(), move |_| {
            v.report(v.refresh());
            v.rebuild_page();
        });
        register_actions(&vm, ctx);
        // Selection follows the view model both ways.
        for (selection, current) in [
            (self.folder_selection.clone(), vm.0.folder.clone()),
            (self.note_selection.clone(), vm.0.note.clone()),
        ] {
            if let Some(id) = current.get() {
                selection.select(id)
            }
            ctx.effect(&current, move |id| match id {
                Some(id) => selection.select(*id),
                None => selection.clear(),
            });
        }
        let v = vm.clone();
        ctx.effect(&self.folder_selection.selection_signal(), move |keys| {
            if let Some(id) = keys.iter().next().copied() {
                if v.0.folder.get() != Some(id) {
                    v.report(v.select_folder(id));
                }
            }
        });
        let v = vm.clone();
        ctx.effect(&self.note_selection.selection_signal(), move |keys| {
            if let Some(id) = keys.iter().next().copied() {
                if v.0.note.get() != Some(id) {
                    v.report(v.select_note(Some(id)));
                }
            }
        });
        // Focus in a list makes the lists the undo target and ends text editing.
        let lists_focus = Signal::new(false);
        let v = vm.clone();
        ctx.effect(&lists_focus, move |focused| {
            if *focused {
                v.0.edit.claim_structure(true);
                v.0.editing.set(None);
            }
        });
        let folders = crate::panes::folders_pane(&vm, self.folder_selection.clone())
            .focus_within(lists_focus.clone());
        let notes =
            crate::panes::notes_pane(&vm, self.note_selection.clone()).focus_within(lists_focus);
        let note = crate::note_view::note_view(&vm, ctx);
        let regions = [
            ctx.add(crate::widgets::fill(SurfaceRole::Main).child(folders)),
            ctx.add(crate::widgets::fill(SurfaceRole::AltRow).child(notes)),
            ctx.add(note),
        ];
        // F6 / Shift+F6 cycle Folders → Notes → note → status bar.
        let status = ctx.add(crate::status::status_bar(&vm));
        let cycle = std::cell::Cell::new(0usize);
        let order = [regions[0], regions[1], regions[2], status];
        ctx.apply_self_handlers(
            teksilo::core::widget_builder::HandlerSet::new().on_key_preview(move |event, ec| {
                if let WidgetEvent::KeyDown {
                    key: Key::F6,
                    modifiers,
                    ..
                } = event
                {
                    let n = order.len();
                    let next = if modifiers.shift() {
                        (cycle.get() + n - 1) % n
                    } else {
                        (cycle.get() + 1) % n
                    };
                    cycle.set(next);
                    ec.request_focus_into(order[next]);
                    EventResponse::Handled
                } else {
                    EventResponse::Ignored
                }
            }),
        );
        // Narrow windows stack the panes: Folders → Notes → Note.
        if let Some(window) = ctx.window() {
            let size = window.size().clone();
            let navigation = vm.0.navigation.clone();
            let (split, narrow) = (self.split.clone(), vm.0.narrow.clone());
            let update = move |width: u32, stage: usize| {
                let is_narrow = width < 760;
                if narrow.get() != is_narrow {
                    narrow.set(is_narrow);
                }
                for i in 0..3 {
                    split.set_pane_visible(i, !is_narrow || i == stage);
                }
            };
            update(size.get().0, navigation.get());
            let apply = std::rc::Rc::new(update);
            let a = apply.clone();
            let nav = navigation.clone();
            ctx.effect(&size, move |size| a(size.0, nav.get()));
            ctx.effect(&navigation, move |stage| apply(size.get().0, *stage));
        }
        let content = Splitter::new(self.split.clone())
            .panes(regions)
            .pane_label(0, tr!(folders()))
            .pane_label(1, tr!(notes()))
            .pane_label(2, tr!(note_title()));
        let narrow = vm.0.narrow.clone();
        let root = ctx.add(
            crate::widgets::fill(SurfaceRole::Main).child(
                VStack::new()
                    .child(narrow_bar(&vm).visible_when(narrow.clone()))
                    .child(Divider::horizontal().visible_when(narrow))
                    .child(Expand::new().child(content))
                    .child(status),
            ),
        );
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

/// A plain window header for the headless automation host and platforms
/// without a custom title bar: menu, mark and search.
pub fn plain_header(
    vm: &SessionVm,
    menu: MenuModel,
    tree: &mut teksilo::core::WidgetTree,
) -> WidgetId {
    let (mark, search) = title_parts(vm, tree);
    let menubar = MenuBar::from_model(menu).collapse_policy(CollapsePolicy::Always);
    tree.add(
        Padding::symmetric(4., 6.).child(
            HStack::new()
                .spacing(8.)
                .alignment(VAlignment::Center)
                .child(menubar)
                .child(mark)
                .child(Spacer::new())
                .child(search)
                .child(Spacer::new()),
        ),
    )
}
