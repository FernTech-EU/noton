use base64::Engine;
use common::entities::BlockKind;
use teksilo::{
    core::drag_payload::{DragPayload, ExternalDropData},
    prelude::*,
};
use teksilo_ui::{automation::build, image_drop, session_vm::SessionVm};

fn png() -> Vec<u8> {
    base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAACAAAAAYCAIAAAAUMWhjAAAAJElEQVR4nGN4MMGBpohh1IJRC0YtGLVg1IJRC0YtGLVgaFgAAAKQEEzcGbP7AAAAAElFTkSuQmCC").unwrap()
}
fn raw_image() -> ExternalDropData {
    ExternalDropData {
        mime: [("image/png".into(), png())].into(),
        ..Default::default()
    }
}

#[test]
fn external_file_drop_uses_canvas_coordinates_and_creates_independent_frames() {
    let vm = SessionVm::memory().unwrap();
    vm.new_folder().unwrap();
    vm.new_note().unwrap();
    let mut app = build(vm.clone(), "en-US", (1280, 850)).unwrap();
    let camera = vm.0.camera.borrow().clone().unwrap();
    camera.zoom.set(1.5);
    camera.pan_x.set(-60.);
    camera.pan_y.set(-30.);
    app.tree.layout(SizeProposal::exact(1280., 850.));
    let snapshot = app.tree.accessibility_tree_snapshot();
    let canvas = snapshot
        .nodes
        .iter()
        .find(|(_, n)| n.role() == accesskit::Role::Pane && n.label() == Some("Canvas"))
        .unwrap()
        .1
        .bounds()
        .unwrap();
    let position = Point::new(canvas.x0 as f32 + 220., canvas.y0 as f32 + 180.);
    let expected = (
        (220. - camera.pan_x.get()) / camera.zoom.get(),
        (180. - camera.pan_y.get()) / camera.zoom.get(),
    );
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.PNG");
    let second = dir.path().join("second.png");
    std::fs::write(&first, png()).unwrap();
    std::fs::write(&second, png()).unwrap();
    let data = ExternalDropData {
        files: vec![first, second],
        ..Default::default()
    };
    let mut ops = teksilo::automation::RecordingWindowOps::new();
    // The native backend supplies formats at enter and paths only at drop.
    app.tree.begin_external_drag(
        position,
        ExternalDropData {
            formats: vec!["text/uri-list".into()],
            ..Default::default()
        },
        &mut ops,
    );
    app.tree.end_external_drag(position, data, &mut ops);
    let blocks = vm.0.service.blocks(vm.0.note.get().unwrap()).unwrap();
    assert_eq!(blocks.len(), 2, "{}", vm.0.error.get());
    for (i, block) in blocks.iter().enumerate() {
        assert_eq!(block.kind, BlockKind::Image);
        assert!(
            (block.x - expected.0 as f64 - i as f64 * 24.).abs() < 0.1,
            "x={} expected={:?}",
            block.x,
            expected
        );
        assert!((block.y - expected.1 as f64 - i as f64 * 24.).abs() < 0.1);
    }
    assert_eq!(vm.0.selected.get(), Some(blocks[1].id));
    vm.nudge(blocks[1].id, 24., 0.).unwrap();
    vm.resize(blocks[1].id, 24., 24.).unwrap();
    assert_eq!(vm.0.service.block(blocks[0].id).unwrap().x, blocks[0].x);
    assert!(vm.0.service.block(blocks[1].id).unwrap().width > blocks[1].width);
}

#[test]
fn raw_image_drop_creates_a_note_and_is_saved_with_undoable_geometry() {
    let vm = SessionVm::memory().unwrap();
    let mut app = build(vm.clone(), "en-US", (1280, 850)).unwrap();
    let position = Point::new(800., 350.);
    let mut ops = teksilo::automation::RecordingWindowOps::new();
    app.tree
        .begin_external_drag(position, raw_image(), &mut ops);
    app.tree.end_external_drag(position, raw_image(), &mut ops);
    let id = vm.0.selected.get().unwrap_or_else(|| {
        panic!(
            "a dropped image is selected; note={:?}, error={}",
            vm.0.note.get(),
            vm.0.error.get()
        )
    });
    assert_eq!(
        vm.0.service.blocks(vm.0.note.get().unwrap()).unwrap().len(),
        1
    );
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("images.noton");
    vm.save_copy(&file).unwrap();
    let reopened = SessionVm::open(file).unwrap();
    let restored = reopened
        .0
        .service
        .blocks(reopened.0.note.get().unwrap())
        .unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].kind, BlockKind::Image);
    assert!(
        reopened
            .image(reopened.0.service.image_asset(restored[0].id).unwrap())
            .is_ok()
    );
    vm.0.edit.invoke(&vm, false).unwrap();
    assert!(
        !vm.0
            .service
            .blocks(vm.0.note.get().unwrap())
            .unwrap()
            .iter()
            .any(|b| b.id == id)
    );
    vm.0.edit.invoke(&vm, true).unwrap();
    assert_eq!(vm.0.service.block(id).unwrap().kind, BlockKind::Image);
}

#[test]
fn rejected_drops_do_not_create_notes_or_modify_trash() {
    let vm = SessionVm::memory().unwrap();
    let bad = DragPayload::external(ExternalDropData {
        mime: [("image/png".into(), vec![1, 2, 3])].into(),
        ..Default::default()
    });
    assert!(image_drop::import(&vm, &bad, None).is_err());
    assert!(vm.0.note.get().is_none());
    assert!(!image_drop::accepts(&DragPayload::typed(
        "internal reorder"
    )));
    assert!(!image_drop::accepts(&DragPayload::external(
        ExternalDropData {
            text: Some("hello".into()),
            ..Default::default()
        }
    )));
    vm.0.trash.set(true);
    assert!(image_drop::import(&vm, &DragPayload::external(raw_image()), None).is_err());
    assert!(vm.0.note.get().is_none());
}

#[test]
fn delete_selected_frame_and_undo_do_not_touch_other_frames() {
    let vm = SessionVm::memory().unwrap();
    vm.new_folder().unwrap();
    vm.new_note().unwrap();
    vm.insert(BlockKind::Drawing).unwrap();
    let sketch = vm.0.selected.get().unwrap();
    let mut app = build(vm.clone(), "en-US", (1280, 850)).unwrap();
    let mut ops = teksilo::automation::RecordingWindowOps::new();
    app.tree.run_mount_actions(&mut ops);
    let payload = DragPayload::external(raw_image());
    image_drop::import(&vm, &payload, Some((400., 100.))).unwrap();
    let image = vm.0.selected.get().unwrap();
    app.tree.layout(SizeProposal::exact(1280., 850.));
    app.tree.run_mount_actions(&mut ops);
    let key: teksilo::automation::AutomationOp =
        serde_json::from_value(serde_json::json!({"InjectKey": {"key": "Delete"}})).unwrap();
    let reply = teksilo::automation::execute(&mut app.tree, &mut ops, &key, &Default::default());
    assert!(
        matches!(reply, teksilo::automation::AutomationReply::Ok { .. }),
        "{reply:?}"
    );
    assert!(vm.0.service.block(image).unwrap().trashed);
    assert!(!vm.0.service.block(sketch).unwrap().trashed);
    vm.0.edit.invoke(&vm, false).unwrap();
    assert!(!vm.0.service.block(image).unwrap().trashed);
}
