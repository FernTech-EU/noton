//! Small building blocks shared by the panes, the note view and the canvas.
use crate::icons::Icon;
use std::{cell::Cell, rc::Rc};
use teksilo::i18n::LocalizedString;
use teksilo::{prelude::*, widgets::*};

/// An icon-only button. Its tooltip is also its accessible name.
pub fn icon_button(icon: Icon, label: LocalizedString) -> IconButton {
    IconButton::new(icon.widget()).tooltip(label)
}

/// A plain surface: a fill and nothing else (no border, radius or padding).
pub fn fill(color: impl Into<ColorProp>) -> Panel {
    Panel::new()
        .background(color)
        .border_width(0.)
        .corner_radius(0.)
        .padding(0.)
}

/// A keyboard chip: `Ctrl Z`, `Esc`.
pub fn kbd(keys: &str) -> Panel {
    Panel::new()
        .padding(0.)
        .background(SurfaceRole::Sunken)
        .border_color(BorderRole::Default)
        .border_width(1.)
        .corner_radius(4.)
        .child(
            Padding::symmetric(5., 1.).child(
                TextWidget::new(lit!(keys))
                    .style(TextStyleRole::Mono)
                    .color(TextRole::Secondary),
            ),
        )
}

/// Secondary text in the small role.
pub fn caption(text: impl Into<LocalizedString>) -> TextWidget {
    TextWidget::new(text)
        .style(TextStyleRole::Small)
        .color(TextRole::Secondary)
}

/// Focuses the editable field of a text input as soon as it is mounted, for
/// inline rename and the fields a popover opens on.
#[derive(Debug)]
pub struct AutoFocus {
    input: Option<TextInput>,
    field: Rc<Cell<Option<WidgetId>>>,
    once: Option<Rc<Cell<bool>>>,
    root: Option<WidgetId>,
}
impl AutoFocus {
    pub fn new(input: TextInput) -> Self {
        Self {
            field: input.field_id(),
            input: Some(input),
            once: None,
            root: None,
        }
    }
    /// Focus only if `done` is still false, then set it: a field rebuilt by a
    /// list refresh must not take the focus back from wherever it went.
    pub fn once(mut self, done: Rc<Cell<bool>>) -> Self {
        self.once = Some(done);
        self
    }
}
impl Widget for AutoFocus {
    fn build(&mut self, ctx: &mut BuildContext) -> Vec<WidgetId> {
        if let Some(input) = self.input.take() {
            self.root = Some(ctx.add(input));
            if self.once.as_ref().is_some_and(|done| done.replace(true)) {
                return self.root.into_iter().collect();
            }
            let field = self.field.clone();
            ctx.run_after_mount(move |ec| {
                if let Some(id) = field.get() {
                    ec.request_focus(id);
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

/// A subtree rebuilt whenever one of its signals changes, for chrome whose
/// structure (not just its values) follows state: a popover menu that depends
/// on the selected note, a banner whose text changes.
/// Binds one signal to the widget being built.
type Dependency = Box<dyn Fn(&mut BuildContext)>;
pub struct Rebuild {
    deps: Vec<Dependency>,
    make: Box<dyn Fn() -> Box<dyn Widget>>,
    root: Option<WidgetId>,
}
impl std::fmt::Debug for Rebuild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rebuild").finish_non_exhaustive()
    }
}
impl Rebuild {
    pub fn new(make: impl Fn() -> Box<dyn Widget> + 'static) -> Self {
        Self {
            deps: vec![],
            make: Box::new(make),
            root: None,
        }
    }
    pub fn on<T: Clone + 'static>(mut self, signal: &Signal<T>) -> Self {
        let signal = signal.clone();
        self.deps.push(Box::new(move |ctx| {
            signal.bind_to(
                ctx.self_id(),
                ctx.binding_registry(),
                teksilo::core::binding::BindingLevel::Rebuild,
            )
        }));
        self
    }
}
impl Widget for Rebuild {
    fn build(&mut self, ctx: &mut BuildContext) -> Vec<WidgetId> {
        for dep in &self.deps {
            dep(ctx);
        }
        let root = ctx.add_boxed((self.make)());
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
