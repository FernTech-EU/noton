//! Small modal forms opened from block menus: Position and size, alt text,
//! and renaming or describing a sketch. Enter applies, Esc closes.
use crate::{session_vm::SessionVm, widgets::AutoFocus};
use ::common::{entities::BlockKind, types::EntityId};
use std::rc::Rc;
use teksilo::core::modal::{ModalCloseBehavior, ModalPresentation, ModalRequest};
use teksilo::i18n::LocalizedString;
use teksilo::widgets::dialog::{DialogContent, ModalContainer};
use teksilo::{prelude::*, widgets::*};

type Apply = Rc<dyn Fn()>;

/// Present a form with Cancel and Apply. `body` receives the apply action so
/// its fields can submit on Enter.
fn present(
    ctx: &mut EventContext,
    title: LocalizedString,
    help: Option<LocalizedString>,
    body: impl FnOnce(Apply) -> Box<dyn Widget> + 'static,
    apply: impl Fn() + 'static,
) {
    let name = title.clone().resolve_now();
    let apply: Apply = Rc::new(apply);
    ctx.present_modal(
        ModalRequest::deferred(move |tree| {
            let submit = apply.clone();
            let footer = HStack::new()
                .spacing(8.)
                .child(Spacer::new())
                .child(
                    Button::new(tr!(close()))
                        .variant(ButtonVariant::Plain)
                        .on_activate_fn(|ctx| ctx.dismiss_modal()),
                )
                .child(
                    Button::new(tr!(apply()))
                        .variant(ButtonVariant::Filled)
                        .on_activate_fn(move |ctx| {
                            submit();
                            ctx.dismiss_modal();
                        }),
                );
            let mut content = DialogContent::new()
                .title(title)
                .body(body(apply))
                .footer(footer);
            if let Some(help) = help {
                content = content.supporting_text(help);
            }
            tree.add(ModalContainer::new(content).min_width(360.))
        })
        .presentation(ModalPresentation::InTree)
        .title(name)
        .close_behavior(ModalCloseBehavior::EscapeKey),
    );
}

/// A field that applies the form and closes it on Enter.
fn field(text: &Signal<String>, label: LocalizedString, apply: &Apply) -> TextInput {
    let apply = apply.clone();
    TextInput::new(text.clone())
        .label(label.clone())
        .placeholder(label)
        .on_submit_fn(move |ctx| {
            apply();
            ctx.dismiss_modal();
        })
}

fn number(value: f64) -> Signal<String> {
    Signal::new(format!("{}", value.round() as i64))
}

/// X, Y, width and (for images and sketches) height, in scene pixels.
pub fn position_and_size(vm: &SessionVm, id: EntityId, ctx: &mut EventContext) {
    let Ok(b) = vm.0.service.block(id) else {
        return;
    };
    let text = b.kind == BlockKind::RichText;
    let (x, y, w, h) = (number(b.x), number(b.y), number(b.width), number(b.height));
    let keep = Signal::new(!text);
    let (fx, fy, fw, fh, fk) = (x.clone(), y.clone(), w.clone(), h.clone(), keep.clone());
    let v = vm.clone();
    let parse =
        |s: &Signal<String>, fallback: f64| s.get().trim().parse::<f64>().unwrap_or(fallback);
    let apply = move || {
        let width = parse(&w, b.width).max(80.);
        let height = if text {
            None
        } else if keep.get() && (width - b.width).abs() > 0.5 && parse(&h, b.height) == b.height {
            Some(b.height * width / b.width)
        } else {
            Some(parse(&h, b.height))
        };
        v.report(v.place(id, parse(&x, b.x), parse(&y, b.y), width, height));
    };
    let digits = |c: char| c.is_ascii_digit() || c == '-' || c == '.';
    present(
        ctx,
        if text {
            tr!(position_width())
        } else {
            tr!(position_size())
        },
        Some(tr!(position_help())),
        move |apply| {
            let row = |a: TextInput, b: Box<dyn Widget>| {
                HStack::new()
                    .spacing(12.)
                    .child(Expand::horizontal().child(a))
                    .child(Expand::horizontal().child(b))
            };
            let input = |s: &Signal<String>, label| {
                field(s, label, &apply).char_filter(digits).suffix("px")
            };
            let height: Box<dyn Widget> = if text {
                Box::new(
                    VStack::new()
                        .spacing(4.)
                        .child(crate::widgets::caption(tr!(height())))
                        .child(TextWidget::new(tr!(follows_content())).color(TextRole::Secondary)),
                )
            } else {
                Box::new(input(&fh, tr!(height())))
            };
            let mut body = VStack::new()
                .spacing(12.)
                .child(row(
                    input(&fx, tr!(x())),
                    Box::new(AutoFocus::new(input(&fy, tr!(y())))),
                ))
                .child(row(input(&fw, tr!(width())), height));
            if !text {
                body = body.child(Checkbox::new(fk).label(tr!(keep_aspect())));
            }
            Box::new(body)
        },
        apply,
    );
}

/// The image's alt text, or a sketch's title and description.
pub fn describe(vm: &SessionVm, id: EntityId, ctx: &mut EventContext) {
    let Ok(b) = vm.0.service.block(id) else {
        return;
    };
    let v = vm.clone();
    if b.kind == BlockKind::Image {
        let alt = Signal::new(b.alt_text);
        let a = alt.clone();
        present(
            ctx,
            tr!(alt_text()),
            Some(tr!(alt_help())),
            move |apply| Box::new(AutoFocus::new(field(&a, tr!(alt_text()), &apply))),
            move || v.report(v.describe(id, alt.get())),
        );
    } else {
        let title = Signal::new(b.label);
        let description = Signal::new(b.description);
        let (t, d) = (title.clone(), description.clone());
        present(
            ctx,
            tr!(rename_sketch()),
            Some(tr!(describe_help())),
            move |apply| {
                Box::new(
                    VStack::new()
                        .spacing(12.)
                        .child(AutoFocus::new(field(&t, tr!(sketch_title()), &apply)))
                        .child(field(&d, tr!(describe_sketch()), &apply)),
                )
            },
            move || {
                v.report(v.rename_block(id, title.get()));
                v.report(v.describe(id, description.get()));
            },
        );
    }
}
