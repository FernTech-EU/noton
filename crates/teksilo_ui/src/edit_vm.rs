//! Focus-scoped history. Menu focus retains the last text surface; an empty
//! text history never falls through into structural undo.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use teksilo::{
    core::text_surface::{TextSurface, TextSurfaces},
    prelude::*,
};
#[derive(Clone, Default)]
pub struct EditVm {
    surface: Rc<RefCell<Option<Rc<dyn TextSurface>>>>,
    registry: Rc<RefCell<Option<TextSurfaces>>>,
    organization: Rc<Cell<bool>>,
    suspended: Rc<Cell<usize>>,
}
impl std::fmt::Debug for EditVm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditVm").finish_non_exhaustive()
    }
}
#[derive(Debug)]
pub struct UndoSuspend(Rc<Cell<usize>>);
impl Drop for UndoSuspend {
    fn drop(&mut self) {
        self.0.set(self.0.get().saturating_sub(1));
    }
}
impl EditVm {
    pub fn suspend(&self) -> UndoSuspend {
        self.suspended.set(self.suspended.get() + 1);
        UndoSuspend(self.suspended.clone())
    }
    pub fn wire(&self, ctx: &mut BuildContext) {
        let registry = ctx.text_surfaces();
        *self.registry.borrow_mut() = Some(registry.clone());
        let target = self.surface.clone();
        ctx.effect(&registry.focus_signal(), move |_| {
            if let Some(surface) = registry.focused() {
                *target.borrow_mut() = Some(surface);
            }
        });
    }
    pub fn claim_text(&self, surface: Rc<dyn TextSurface>) {
        *self.surface.borrow_mut() = Some(surface);
    }
    /// Whether a text surface has keyboard focus right now.
    pub fn text_focused(&self) -> bool {
        self.registry
            .borrow()
            .as_ref()
            .is_some_and(|r| r.focused().is_some())
    }
    /// Which scope Ctrl+Z reaches now: (a text block, the folder and note lists).
    /// Neither means the page structure.
    pub fn target(&self) -> (bool, bool) {
        let focused = self.registry.borrow().as_ref().and_then(|r| r.focused());
        let typing = focused.is_some() || self.surface.borrow().is_some();
        (typing, !typing && self.organization.get())
    }
    pub fn claim_structure(&self, organization: bool) {
        self.surface.borrow_mut().take();
        self.organization.set(organization);
    }
    pub fn invoke(&self, vm: &crate::session_vm::SessionVm, redo: bool) -> anyhow::Result<()> {
        if self.suspended.get() != 0 {
            return Ok(());
        }
        let focused = self.registry.borrow().as_ref().and_then(|r| r.focused());
        let surface = focused.or_else(|| self.surface.borrow().clone());
        if let Some(surface) = surface {
            if !surface.history_frozen() {
                if redo { surface.redo() } else { surface.undo() }
            }
            Ok(())
        } else {
            vm.undo(redo, self.organization.get())
        }
    }
}
