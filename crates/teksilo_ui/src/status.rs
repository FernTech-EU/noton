//! The status bar: save state, what Ctrl+Z will undo, and the canvas zoom.
use crate::{
    icons::Icon,
    session_vm::{SaveState, SessionVm},
    widgets::{Rebuild, icon_button, kbd},
};
use teksilo::core::OverlayPlacement;
use teksilo::tokens::VAlignment;
use teksilo::widgets::toast::{Toast, ToastAction};
use teksilo::{prelude::*, widgets::*};

fn separator() -> impl Widget + 'static {
    FixedSize::new()
        .width(1.)
        .height(14.)
        .child(crate::widgets::fill(BorderRole::Default))
}

/// Present the snackbar that takes back a trash, if one was just done.
pub fn offer_undo(vm: &SessionVm, ctx: &mut EventContext) {
    let Some((stack, seq, message)) = vm.0.undo_toast.get() else {
        return;
    };
    // Offered once: a later restore must not present this trash again.
    vm.0.undo_toast.set(None);
    let v = vm.clone();
    Toast::info(lit!(message.clone()))
        .leading(Icon::Trash.widget())
        .action(ToastAction::primary(tr!(undo()), move |_| {
            v.report(v.undo_offered(stack, seq))
        }))
        .auto_dismiss_after(std::time::Duration::from_secs(8))
        .announcement(lit!(message))
        .present(ctx);
}

fn save_state(vm: &SessionVm) -> impl Widget + 'static {
    let v = vm.clone();
    let icon = Rebuild::new(move || {
        let (icon, role) = match v.0.save_state.get() {
            SaveState::Saved => (Icon::Saved, TextRole::Success),
            SaveState::Unsaved => (Icon::Unsaved, TextRole::Secondary),
            SaveState::Saving => (Icon::Saving, TextRole::Secondary),
            SaveState::Failed => (Icon::Alert, TextRole::Error),
        };
        Box::new(icon.sized(14.).color(role))
    })
    .on(&vm.0.save_state);
    let text =
        vm.0.save_state
            .zip(&vm.0.saved_at)
            .zip(&vm.0.status)
            .map(|((state, at), status)| {
                if *state == SaveState::Saved && !at.is_empty() {
                    tr!(saved_at(time = at.clone())).resolve_now()
                } else {
                    status.clone()
                }
            });
    let failed = vm.0.save_state.map(|s| *s == SaveState::Failed);
    HStack::new()
        .spacing(6.)
        .alignment(VAlignment::Center)
        .child(icon)
        .child(
            TextWidget::new(lit!(""))
                .text(text.clone())
                .style(TextStyleRole::Small)
                .color(failed.map(|f| {
                    if *f {
                        TextRole::Error
                    } else {
                        TextRole::Secondary
                    }
                }))
                .single_line(),
        )
        .access_role(accesskit::Role::Status)
        .access_live(accesskit::Live::Polite)
        .access_label(text)
}

fn undo_hint(vm: &SessionVm) -> impl Widget + 'static {
    let text =
        vm.0.undo_info.map(
            |info| match (&info.typing_in, &info.page_undo, &info.library_undo) {
                (Some(block), _, _) if block.is_empty() => tr!(undo_hint_field()).resolve_now(),
                (Some(block), _, _) => tr!(undo_hint_typing(block = block.clone())).resolve_now(),
                (None, _, Some(action)) if info.library_focused => {
                    tr!(undo_hint(action = action.clone())).resolve_now()
                }
                (None, Some(action), _) => tr!(undo_hint(action = action.clone())).resolve_now(),
                _ => tr!(undo_hint_none()).resolve_now(),
            },
        );
    let chip = if cfg!(target_os = "macos") {
        "⌘Z"
    } else {
        "Ctrl Z"
    };
    HStack::new()
        .spacing(6.)
        .alignment(VAlignment::Center)
        .child(kbd(chip))
        .child(
            TextWidget::new(lit!(""))
                .text(text)
                .style(TextStyleRole::Small)
                .color(TextRole::Secondary)
                .overflow(TextOverflow::Ellipsis(EllipsisMode::Trailing))
                .single_line(),
        )
}

fn zoom_controls(vm: &SessionVm) -> impl Widget + 'static {
    let percent =
        vm.0.zoom
            .map(|z| format!("{} %", (z * 100.).round() as i32));
    let (out, reset, zin, fit) = (vm.clone(), vm.clone(), vm.clone(), vm.clone());
    let levels = MenuList::new().items([0.5f32, 0.75, 1., 1.5, 2.].map(|z| {
        let v = reset.clone();
        MenuItem::new(lit!(format!("{} %", (z * 100.) as i32)))
            .on_activate_fn(move |_| v.zoom_to(z))
    }));
    HStack::new()
        .spacing(2.)
        .alignment(VAlignment::Center)
        .child(
            icon_button(Icon::Minus, tr!(zoom_out()))
                .size(IconButtonSize::Compact)
                .on_activate_fn(move |_| out.zoom_step(false)),
        )
        .child(
            PopoverButton::new(
                Button::new(tr!(zoom_level()))
                    .label(percent)
                    .variant(ButtonVariant::Plain)
                    .text_style(TextStyleRole::Mono)
                    .tooltip(tr!(zoom_level())),
            )
            .placement(OverlayPlacement::Above)
            .bare()
            .content(levels),
        )
        .child(
            icon_button(Icon::Plus, tr!(zoom_in()))
                .size(IconButtonSize::Compact)
                .on_activate_fn(move |_| zin.zoom_step(true)),
        )
        .child(
            Button::new(tr!(zoom_fit()))
                .variant(ButtonVariant::Plain)
                .icon(Icon::Fit.sized(14.), IconLocation::Leading)
                .tooltip(tr!(zoom_fit_help()))
                .on_activate_fn(move |_| fit.report(fit.zoom_to_fit())),
        )
}

pub fn status_bar(vm: &SessionVm) -> StatusBar {
    let canvas =
        vm.0.reading
            .zip(&vm.0.note)
            .map(|(reading, note)| !reading && note.is_some());
    StatusBar::new()
        .background(SurfaceRole::Main)
        .border_color(BorderRole::Divider)
        .border_width(1.)
        .child(
            Padding::symmetric(3., 8.).child(
                HStack::new()
                    .spacing(12.)
                    .alignment(VAlignment::Center)
                    .child(save_state(vm))
                    .child(separator())
                    .child(Expand::horizontal().child(undo_hint(vm)))
                    .child(separator().visible_when(canvas.clone()))
                    .child(zoom_controls(vm).visible_when(canvas)),
            ),
        )
}
