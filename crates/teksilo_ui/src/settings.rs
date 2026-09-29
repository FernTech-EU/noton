use crate::theme::Appearance;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use teksilo::core::{
    modal::{ModalCloseBehavior, ModalPresentation, ModalRequest},
    widget::WidgetPlacement,
};
use teksilo::tokens::VAlignment;
use teksilo::{prelude::*, widgets::*};

/// Match Skribisto: one in-tree, instant-apply modal, closed by Escape, X or Done.
pub fn present(
    ctx: &mut EventContext,
    edit: crate::edit_vm::EditVm,
    vm: crate::session_vm::SessionVm,
) {
    ctx.present_modal(
        ModalRequest::deferred(move |tree| tree.add(SettingsView::new(edit.suspend(), vm)))
            .presentation(ModalPresentation::InTree)
            .title(tr!(settings()))
            .close_behavior(ModalCloseBehavior::EscapeKey),
    );
}

/// How a note opens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefaultView {
    #[default]
    Canvas,
    Reading,
}

/// Unknown fields are ignored, not refused: a file written by a newer Noton
/// must not stop an older one from starting.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// `None` follows the system language.
    pub locale: Option<String>,
    /// Superseded by `appearance`; still read so older files keep a dark choice.
    pub dark: bool,
    pub appearance: Option<Appearance>,
    /// 1.0 = 100 %, within 0.8–2.0.
    pub text_scale: f32,
    /// `None` follows the system.
    pub reduce_motion: Option<bool>,
    pub default_view: DefaultView,
    /// `None` writes backups next to the library.
    pub backup_dir: Option<std::path::PathBuf>,
    /// Backups kept per library; older ones are deleted. 0 keeps them all.
    pub backup_keep: u32,
    /// The narrow-window offer to read a note was dismissed for good.
    pub narrow_hint_dismissed: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            locale: None,
            dark: false,
            appearance: None,
            text_scale: 1.,
            reduce_motion: None,
            default_view: DefaultView::Canvas,
            backup_dir: None,
            backup_keep: 10,
            narrow_hint_dismissed: false,
        }
    }
}
impl Preferences {
    pub fn appearance(&self) -> Appearance {
        self.appearance.unwrap_or(if self.dark {
            Appearance::Dark
        } else {
            Appearance::System
        })
    }
    pub fn theme(&self) -> Theme {
        crate::theme::for_preferences(self.appearance(), self.reduce_motion)
    }
}
fn path() -> anyhow::Result<std::path::PathBuf> {
    Ok(teksilo::settings::AppPaths::new("eu", "FernTech", "Noton")
        .ok_or_else(|| anyhow::anyhow!("No configuration directory"))?
        .config_dir()
        .join("preferences.json"))
}
/// Read the preferences. An interface language Noton does not ship falls back
/// to the system's; only an unreadable file is an error.
pub fn load() -> anyhow::Result<Preferences> {
    let path = path()?;
    if !path.exists() {
        return Ok(Preferences::default());
    }
    parse(&std::fs::read(&path)?).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))
}
fn parse(json: &[u8]) -> serde_json::Result<Preferences> {
    let mut value: Preferences = serde_json::from_slice(json)?;
    if value
        .locale
        .as_deref()
        .is_some_and(|v| v != "en-US" && v != "fr-FR")
    {
        log::warn!("Ignoring unsupported interface locale {:?}", value.locale);
        value.locale = None;
    }
    value.text_scale = value.text_scale.clamp(0.8, 2.);
    Ok(value)
}
/// [`load`], or the defaults when the file cannot be read: preferences never
/// keep Noton from starting.
pub fn load_or_default() -> Preferences {
    load().unwrap_or_else(|e| {
        log::warn!("Using default preferences: {e:#}");
        Preferences::default()
    })
}
/// Change the saved preferences and the view model's copy. An unreadable
/// file is replaced by the preferences in use, changed.
pub fn persist(
    vm: &crate::session_vm::SessionVm,
    f: impl FnOnce(&mut Preferences),
) -> anyhow::Result<Preferences> {
    let mut p = load().unwrap_or_else(|_| vm.0.prefs.borrow().clone());
    f(&mut p);
    save(&p)?;
    *vm.0.prefs.borrow_mut() = p.clone();
    Ok(p)
}
pub fn save(value: &Preferences) -> anyhow::Result<()> {
    let path = path()?;
    let dir = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid settings directory"))?;
    std::fs::create_dir_all(dir)?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    use std::io::Write;
    temp.write_all(&serde_json::to_vec_pretty(value)?)?;
    temp.as_file().sync_all()?;
    temp.persist(path)?;
    Ok(())
}
/// Rebuilds on `signal` and, after each change, runs `apply` with an event
/// context: the only way a radio-tile choice can call `set_theme`, since
/// `RadioTileGroup` (unlike `SegmentedControl`) has no change callback.
type Apply<T> = Rc<dyn Fn(&T, &mut EventContext)>;
struct ApplyOnChange<T: Clone + 'static> {
    signal: Signal<T>,
    apply: Apply<T>,
    seen: Option<T>,
}
impl<T: Clone + PartialEq + std::fmt::Debug + 'static> std::fmt::Debug for ApplyOnChange<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApplyOnChange")
            .field("seen", &self.seen)
            .finish()
    }
}
impl<T: Clone + PartialEq + std::fmt::Debug + 'static> Widget for ApplyOnChange<T> {
    fn build(&mut self, ctx: &mut BuildContext) -> Vec<WidgetId> {
        self.signal.bind_to(
            ctx.self_id(),
            ctx.binding_registry(),
            teksilo::core::binding::BindingLevel::Rebuild,
        );
        let value = self.signal.get();
        if self.seen.as_ref().is_some_and(|seen| *seen != value) {
            let apply = self.apply.clone();
            let v = value.clone();
            ctx.run_after_mount(move |ec| apply(&v, ec));
        }
        self.seen = Some(value);
        vec![]
    }
    fn layout_response(&self, _: SizeProposal, _: &LayoutContext) -> LayoutResponse {
        Size::new(0., 0.).into()
    }
}
fn on_change<T: Clone + PartialEq + std::fmt::Debug + 'static>(
    signal: &Signal<T>,
    apply: impl Fn(&T, &mut EventContext) + 'static,
) -> ApplyOnChange<T> {
    ApplyOnChange {
        signal: signal.clone(),
        apply: Rc::new(apply),
        seen: None,
    }
}

/// Load, change and save the preferences; the view model keeps its copy.
fn update(
    vm: &crate::session_vm::SessionVm,
    error: &Signal<String>,
    f: impl FnOnce(&mut Preferences),
) -> Option<Preferences> {
    match persist(vm, f) {
        Ok(p) => Some(p),
        Err(e) => {
            error.set(e.to_string());
            None
        }
    }
}

/// The locale the system asks for, among the ones Noton ships.
fn system_locale() -> String {
    let config = teksilo::i18n::I18nConfig::new()
        .source_locale("en-US".parse().expect("valid locale"))
        .supported_locales([
            "en-US".parse().expect("valid locale"),
            "fr-FR".parse().expect("valid locale"),
        ]);
    teksilo::i18n::I18nManager::resolve_initial_locale(&config).to_string()
}

fn label(text: teksilo::i18n::LocalizedString) -> TextWidget {
    TextWidget::new(text).style(TextStyleRole::BodyBold)
}

#[derive(Debug)]
pub struct SettingsView {
    root: Option<WidgetId>,
    initial_focus: Option<WidgetId>,
    vm: crate::session_vm::SessionVm,
    _undo: crate::edit_vm::UndoSuspend,
}
impl SettingsView {
    pub fn new(undo: crate::edit_vm::UndoSuspend, vm: crate::session_vm::SessionVm) -> Self {
        Self {
            root: None,
            initial_focus: None,
            vm,
            _undo: undo,
        }
    }
}
impl Widget for SettingsView {
    fn build(&mut self, ctx: &mut BuildContext) -> Vec<WidgetId> {
        let vm = self.vm.clone();
        let prefs = load_or_default();
        let error = Signal::new(String::new());
        // Language: System, English, Français.
        let language_index = match prefs.locale.as_deref() {
            Some("en-US") => 1,
            Some("fr-FR") => 2,
            _ => 0,
        };
        let language_selected = Signal::new(language_index);
        let languages =
            [tr!(language_system()), lit!("English"), lit!("Français")].map(Segment::new);
        let ids = languages.each_ref().map(|s| s.segment_id());
        let (v, e) = (vm.clone(), error.clone());
        let language = SegmentedControl::indexed(language_selected)
            .label(tr!(language()))
            .segments(languages)
            .on_change(move |id, c| {
                let locale = match ids.iter().position(|s| *s == id) {
                    Some(1) => Some("en-US".to_owned()),
                    Some(2) => Some("fr-FR".to_owned()),
                    _ => None,
                };
                if update(&v, &e, |p| p.locale = locale.clone()).is_some() {
                    c.set_locale(locale.unwrap_or_else(system_locale));
                }
            });
        let language = ctx.add(language);
        self.initial_focus = Some(language);
        // Appearance: System, Light, Dark, as tiles.
        let appearance = Signal::new(match prefs.appearance() {
            Appearance::System => 0,
            Appearance::Light => 1,
            Appearance::Dark => 2,
        });
        let (v, e) = (vm.clone(), error.clone());
        let appearance_apply = on_change(&appearance, move |index, c| {
            let choice = [Appearance::System, Appearance::Light, Appearance::Dark][(*index).min(2)];
            if let Some(p) = update(&v, &e, |p| p.appearance = Some(choice)) {
                c.set_theme(p.theme());
            }
        });
        let tile = |value: usize, icon: crate::icons::Icon, title, description| {
            RadioTile::new()
                .selection(value, appearance.clone())
                .icon(icon.sized(20.))
                .title(title)
                .description(description)
                .show_indicator(false)
        };
        let tiles = RadioTileGroup::new(appearance.clone())
            .label(tr!(appearance()))
            .layout(TileLayout::Grid {
                min_tile_width: 150.,
            })
            .tile(tile(
                0,
                crate::icons::Icon::Monitor,
                tr!(system()),
                tr!(system_help()),
            ))
            .tile(tile(
                1,
                crate::icons::Icon::Sun,
                tr!(light()),
                tr!(light_help()),
            ))
            .tile(tile(
                2,
                crate::icons::Icon::Moon,
                tr!(dark()),
                tr!(dark_help()),
            ));
        // Text size, applied by the control itself; persisted here.
        let scale = Signal::new(prefs.text_scale);
        let (v, e) = (vm.clone(), error.clone());
        ctx.effect(&scale, move |value| {
            let value = *value;
            if (v.0.prefs.borrow().text_scale - value).abs() > 0.001 {
                update(&v, &e, |p| p.text_scale = value);
            }
        });
        // Reduce motion: System, On, Off.
        let motion = Signal::new(match prefs.reduce_motion {
            None => 0,
            Some(true) => 1,
            Some(false) => 2,
        });
        let motions = [tr!(system()), tr!(on()), tr!(off())].map(Segment::new);
        let ids = motions.each_ref().map(|s| s.segment_id());
        let (v, e) = (vm.clone(), error.clone());
        let motion_control = SegmentedControl::indexed(motion)
            .label(tr!(reduce_motion()))
            .segments(motions)
            .on_change(move |id, c| {
                let value = match ids.iter().position(|s| *s == id) {
                    Some(1) => Some(true),
                    Some(2) => Some(false),
                    _ => None,
                };
                if let Some(p) = update(&v, &e, |p| p.reduce_motion = value) {
                    c.set_theme(p.theme());
                }
            });
        // Default note view.
        let view = Signal::new(usize::from(prefs.default_view == DefaultView::Reading));
        let (v, e) = (vm.clone(), error.clone());
        ctx.effect(&view, move |index| {
            let value = if *index == 1 {
                DefaultView::Reading
            } else {
                DefaultView::Canvas
            };
            if v.0.prefs.borrow().default_view != value {
                update(&v, &e, |p| p.default_view = value);
            }
        });
        let view_control = SegmentedControl::indexed(view)
            .label(tr!(default_view()))
            .segment(Segment::new(tr!(canvas())).icon(|| crate::icons::Icon::Canvas.widget()))
            .segment(Segment::new(tr!(reading())).icon(|| crate::icons::Icon::Reading.widget()));
        // Backups: where, and how many to keep.
        let location = Signal::new(
            prefs
                .backup_dir
                .as_ref()
                .map(|d| d.display().to_string())
                .unwrap_or_else(|| tr!(next_to_library()).resolve_now()),
        );
        let (v, e, l) = (vm.clone(), error.clone(), location.clone());
        let change = Button::new(tr!(change()))
            .variant(ButtonVariant::Outlined)
            .on_activate_fn(move |c| {
                let (v, e, l) = (v.clone(), e.clone(), l.clone());
                let _ = c.pick_file(
                    FileDialogRequest::pick_folder().title(tr!(backup_location())),
                    move |result, _| {
                        if let FileDialogResult::Folder(Some(dir)) = result {
                            l.set(dir.display().to_string());
                            update(&v, &e, |p| p.backup_dir = Some(dir));
                        }
                    },
                );
            });
        const KEEP: [u32; 4] = [5, 10, 20, 0];
        let keep = Signal::new(Some(prefs.backup_keep));
        let (v, e) = (vm.clone(), error.clone());
        let keep_control = ComboBox::from_items(KEEP, keep, |n| {
            if *n == 0 {
                tr!(keep_all())
            } else {
                tr!(keep_n(count = *n as i64))
            }
        })
        .label(tr!(keep_backups()))
        .on_select(move |n, _| {
            let n = *n;
            update(&v, &e, |p| p.backup_keep = n);
        });
        let backups = VStack::new()
            .spacing(8.)
            .child(
                HStack::new()
                    .spacing(8.)
                    .child(
                        Expand::horizontal().child(
                            TextInput::new(location)
                                .read_only(true)
                                .label(tr!(backup_location())),
                        ),
                    )
                    .child(change),
            )
            .child(
                HStack::new()
                    .spacing(8.)
                    .alignment(VAlignment::Center)
                    .child(TextWidget::new(tr!(keep_backups())).color(TextRole::Secondary))
                    .child(FixedSize::new().width(160.).child(keep_control)),
            );
        // Keyboard shortcuts open in place of the form.
        let shortcuts_open = Signal::new(false);
        let open = shortcuts_open.clone();
        let shortcuts_button = Button::new(tr!(shortcuts()))
            .variant(ButtonVariant::Outlined)
            .icon(crate::icons::Icon::Keyboard.widget(), IconLocation::Leading)
            .on_activate_fn(move |_| open.set(true));
        let form = FormLayout::new()
            .label_gap(16.)
            .row_spacing(18.)
            .line(label(tr!(language())), language)
            .line(label(tr!(appearance())), tiles)
            .line(
                label(tr!(text_size())),
                TextScaleControl::new(scale).access_label(tr!(text_size())),
            )
            .line(label(tr!(reduce_motion())), motion_control)
            .line(label(tr!(default_view())), view_control)
            .line(label(tr!(backups())), backups)
            .full_width(shortcuts_button);
        let close = shortcuts_open.clone();
        let shortcuts = VStack::new()
            .spacing(12.)
            .child(
                Button::new(tr!(back()))
                    .variant(ButtonVariant::Plain)
                    .icon(
                        crate::icons::Icon::ChevronLeft.widget(),
                        IconLocation::Leading,
                    )
                    .on_activate_fn(move |_| close.set(false)),
            )
            .child(Expand::new().child(ShortcutSettings::new()));
        let header = HStack::new()
            .spacing(8.)
            .alignment(VAlignment::Center)
            .child(TextWidget::new(tr!(settings())).style(TextStyleRole::BodyBold))
            .child(Spacer::new())
            .child(
                crate::widgets::icon_button(crate::icons::Icon::Close, tr!(close()))
                    .on_activate_fn(|ctx| ctx.dismiss_modal()),
            );
        let footer = HStack::new()
            .spacing(8.)
            .alignment(VAlignment::Center)
            .child(crate::widgets::caption(tr!(settings_help())))
            .child(TextWidget::new(lit!("")).text(error).color(TextRole::Error))
            .child(Spacer::new())
            .child(
                Button::new(tr!(done()))
                    .variant(ButtonVariant::Filled)
                    .on_activate_fn(|ctx| ctx.dismiss_modal()),
            );
        let form_open = shortcuts_open.map(|o| !o);
        let root = ctx.add(
            Panel::new()
                .variant(teksilo::core::styles::PanelVariant::Raised)
                .corner_radius(10.)
                .child(
                    Padding::uniform(20.).child(
                        VStack::new()
                            .spacing(12.)
                            .child(Expand::horizontal().child(header))
                            .child(Divider::new())
                            .child(
                                Expand::new().child(
                                    ZStack::new()
                                        .child(
                                            ScrollArea::new()
                                                .child(Padding::new(4., 12., 4., 4.).child(form))
                                                .visible_when(form_open),
                                        )
                                        .child(shortcuts.visible_when(shortcuts_open)),
                                ),
                            )
                            .child(Divider::new())
                            .child(Expand::horizontal().child(footer))
                            .child(appearance_apply),
                    ),
                ),
        );
        self.root = Some(root);
        vec![root]
    }
    fn layout_response(&self, p: SizeProposal, _c: &LayoutContext) -> LayoutResponse {
        p.resolve(640., 600.).into()
    }
    fn place_children(
        &self,
        bounds: Rect,
        _p: SizeProposal,
        children: &mut [WidgetPlacement],
        _c: &LayoutContext,
    ) {
        let width = (bounds.width - 32.).clamp(320., 640.);
        let height = (bounds.height - 32.).clamp(320., 600.);
        for child in children {
            child.origin = Point::new(
                bounds.x + (bounds.width - width) / 2.,
                bounds.y + (bounds.height - height) / 2.,
            );
            child.size = Size::new(width, height);
        }
    }
    fn initial_focus_hint(&self) -> Option<WidgetId> {
        self.initial_focus
    }
    fn accessibility(&self, builder: &mut AccessNodeBuilder) {
        builder.set_role(accesskit::Role::Dialog);
        builder.set_name(tr!(settings()).resolve_now());
        builder.set_modal();
    }
    fn accessible_title_hint(&self) -> Option<String> {
        Some(tr!(settings()).resolve_now())
    }
    fn children(&self) -> Vec<WidgetId> {
        self.root.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_or_foreign_preferences_file_still_loads() {
        let p = parse(br#"{"locale":"de-DE","dark":true,"added_in_2027":{"x":1}}"#).unwrap();
        assert_eq!(p.locale, None, "an unshipped language follows the system");
        assert_eq!(p.appearance(), Appearance::Dark);
        assert!(parse(b"{ not json").is_err());
    }
}
