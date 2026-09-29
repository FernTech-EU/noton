use common::entities::BlockKind;
use std::rc::Rc;
use teksilo::prelude::*;
use teksilo_ui::{automation::build, session_vm::SessionVm};

fn note_with_blocks(vm: &SessionVm, count: usize) -> Vec<u64> {
    vm.new_folder().unwrap();
    vm.new_note().unwrap();
    for _ in 0..count {
        vm.insert(BlockKind::RichText).unwrap();
    }
    vm.0.service.note(vm.0.note.get().unwrap()).unwrap().blocks
}

fn live_order(vm: &SessionVm) -> Vec<u64> {
    vm.0.service
        .blocks(vm.0.note.get().unwrap())
        .unwrap()
        .into_iter()
        .filter(|b| !b.trashed)
        .map(|b| b.id)
        .collect()
}

#[test]
fn moving_earlier_passes_over_a_block_in_the_trash() {
    let vm = SessionVm::memory().unwrap();
    let ids = note_with_blocks(&vm, 3);
    vm.trash_block(ids[1]).unwrap();
    vm.reorder(ids[2], -1).unwrap();
    assert_eq!(live_order(&vm), vec![ids[2], ids[0]]);
    vm.reorder(ids[2], 1).unwrap();
    assert_eq!(live_order(&vm), vec![ids[0], ids[2]]);
}

#[test]
fn after_typing_ctrl_z_takes_back_a_block_move() {
    let vm = SessionVm::memory().unwrap();
    let mut app = build(vm.clone(), "en-US", (1280, 850)).unwrap();
    let id = note_with_blocks(&vm, 1)[0];
    app.tree.layout(SizeProposal::exact(1280., 850.));
    let handle = vm.0.handles.borrow().get(&id).unwrap().clone();
    // What focusing the editor does.
    vm.0.edit.claim_text(Rc::new(handle));
    assert!(vm.0.edit.target().0, "typing: Ctrl+Z undoes the text");
    let x = vm.0.service.block(id).unwrap().x;
    vm.nudge(id, 24., 0.).unwrap();
    assert!(!vm.0.edit.target().0, "the move is now the thing to undo");
    vm.0.edit.invoke(&vm, false).unwrap();
    assert_eq!(vm.0.service.block(id).unwrap().x, x);
}

#[test]
fn trashing_a_note_offers_to_undo_that_trash() {
    let vm = SessionVm::memory().unwrap();
    note_with_blocks(&vm, 0);
    let note = vm.0.note.get().unwrap();
    vm.trash_note(note).unwrap();
    let (stack, seq, _) = vm.0.undo_toast.get().expect("an undo offer");
    // Restoring offers nothing.
    vm.trash_note(note).unwrap();
    assert!(vm.0.undo_toast.get().is_none());
    vm.trash_note(note).unwrap();
    let (stack2, seq2, _) = vm.0.undo_toast.get().unwrap();
    assert!(seq2 > seq && stack2 == stack);
    vm.undo_offered(stack2, seq2).unwrap();
    assert!(!vm.0.service.note(note).unwrap().trashed);
}

#[test]
fn search_follows_edited_text() {
    let vm = SessionVm::memory().unwrap();
    let id = note_with_blocks(&vm, 1)[0];
    let note = vm.0.note.get().unwrap();
    let found = |query: &str| {
        vm.0.query.set(query.to_owned());
        vm.refresh().unwrap();
        (0..vm.0.notes.len()).any(|i| vm.0.notes.with_item(i, |r| r.id) == Some(note))
    };
    vm.0.service
        .flush_text(id, "Café au lait\n".into())
        .unwrap();
    assert!(found("cafe"));
    assert!(!found("the"));
    vm.0.service.flush_text(id, "Thé vert\n".into()).unwrap();
    assert!(found("the"), "a changed block is normalized again");
    assert!(!found("cafe"));
}
