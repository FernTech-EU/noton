//! The only startup substitution for the mock application: isolated, in-memory fixtures.
use crate::session_vm::SessionVm;
use std::path::PathBuf;
#[cfg(not(feature = "mocks"))]
pub fn open(path: PathBuf) -> anyhow::Result<SessionVm> {
    SessionVm::open(path)
}
#[cfg(feature = "mocks")]
pub fn open(_path: PathBuf) -> anyhow::Result<SessionVm> {
    let vm = SessionVm::memory()?;
    let folder =
        vm.0.service
            .create_folder("Example".into(), vm.0.organization)?;
    let note =
        vm.0.service
            .create_note(folder, "Welcome to Noton".into(), vm.0.organization)?;
    vm.select_note(Some(note))?;
    let block = vm.0.service.create_block(
        note,
        common::entities::BlockKind::RichText,
        "Notes".into(),
        vm.page_stack()?,
    )?;
    vm.0.service
        .flush_text(block, "# Noton\n\nAn isolated example library.\n".into())?;
    vm.refresh()?;
    Ok(vm)
}
