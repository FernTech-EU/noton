use teksilo::{
    automation::{self, AutomationOp, RecordingWindowOps, SettleSpec},
    prelude::*,
};
use teksilo_ui::{automation::build, session_vm::SessionVm};
#[test]
fn formatting_undo_is_durable() {
    let vm = SessionVm::memory().unwrap();
    let mut app = build(vm.clone(), "en-US", (1280, 850)).unwrap();
    vm.new_folder().unwrap();
    vm.new_note().unwrap();
    vm.insert(common::entities::BlockKind::RichText).unwrap();
    app.tree.layout(SizeProposal::exact(1280., 850.));
    let id = vm.0.active_block.get().unwrap();
    let doc = vm.document(id).unwrap();
    let handle = vm.0.handles.borrow().get(&id).unwrap().clone();
    doc.set_plain_text("Texte accentué").unwrap();
    handle.select_range(0, 14);
    handle.toggle_bold();
    let mut ops = RecordingWindowOps::new();
    let _ = automation::execute(
        &mut app.tree,
        &mut ops,
        &AutomationOp::Settle,
        &SettleSpec::default(),
    );
    vm.flush().unwrap();
    assert_eq!(
        vm.0.service.block(id).unwrap().content_djot,
        "*Texte accentué*"
    );
    handle.undo();
    let _ = automation::execute(
        &mut app.tree,
        &mut ops,
        &AutomationOp::Settle,
        &SettleSpec::default(),
    );

    vm.flush().unwrap();
    assert!(!vm.0.service.block(id).unwrap().content_djot.contains('*'));
}
