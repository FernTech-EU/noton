use teksilo::prelude::*;
use teksilo_ui::{automation::build, session_vm::SessionVm};

#[test]
fn double_click_places_text_in_scene_coordinates_and_preserves_camera() {
    for (width, zoom, pan, edge) in [
        (1280., 1., (-8., -4.), false),
        (1280., 1., (-8., -4.), true),
        (1280., 1.5, (-60., -30.), false),
        (600., 0.75, (25., 40.), false),
    ] {
        let vm = SessionVm::memory().unwrap();
        vm.new_folder().unwrap();
        vm.new_note().unwrap();
        vm.insert(common::entities::BlockKind::RichText).unwrap();
        let mut app = build(vm.clone(), "en-US", (width as u32, 850)).unwrap();
        let mut ops = teksilo::automation::RecordingWindowOps::new();
        app.tree.run_mount_actions(&mut ops);
        let camera = vm.0.camera.borrow().clone().unwrap();
        camera.zoom.set(zoom);
        camera.pan_x.set(pan.0);
        camera.pan_y.set(pan.1);
        app.tree.layout(SizeProposal::exact(width, 850.));
        let snapshot = app.tree.accessibility_tree_snapshot();
        let canvas = snapshot
            .nodes
            .iter()
            .find(|(_, n)| n.role() == accesskit::Role::Pane && n.label() == Some("Canvas"))
            .unwrap()
            .1
            .bounds()
            .unwrap();
        let offset = if edge { canvas.width() - 100. } else { 150. };
        let (x, y) = (canvas.x0 + offset, canvas.y0 + 300.);
        let op = serde_json::from_value(
            serde_json::json!({"InjectPointer": {"action": "double_click", "x": x, "y": y}}),
        )
        .unwrap();
        let reply = teksilo::automation::execute(&mut app.tree, &mut ops, &op, &Default::default());
        assert!(
            matches!(reply, teksilo::automation::AutomationReply::Ok { .. }),
            "{reply:?}"
        );
        for _ in 0..3 {
            app.tree.layout(SizeProposal::exact(width, 850.));
            app.tree.run_mount_actions(&mut ops);
        }
        let blocks = vm.0.service.blocks(vm.0.note.get().unwrap()).unwrap();
        assert_eq!(blocks.len(), 2, "{}", vm.0.error.get());
        assert!((blocks[1].x - ((offset as f32 - pan.0) / zoom) as f64).abs() < 0.1);
        assert!((blocks[1].y - ((300. - pan.1) / zoom) as f64).abs() < 0.1);
        let current = vm.0.camera.borrow().clone().unwrap();
        assert_eq!(
            (current.zoom.get(), current.pan_x.get(), current.pan_y.get()),
            (zoom, pan.0, pan.1)
        );
        assert!(vm.0.focus_in_place.get().is_none());
    }
}
