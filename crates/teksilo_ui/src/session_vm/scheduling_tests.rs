use super::*;
use std::any::Any;
use std::sync::mpsc;
use teksilo::core::{AppEventPoster, event_source::SubscriptionId};

struct Poster(mpsc::Sender<Box<dyn Any + Send>>);
impl AppEventPoster for Poster {
    fn post_subscription_event(&self, _: SubscriptionId, _: Box<dyn Any + Send>) {}
    fn post_external(&self, payload: Box<dyn Any + Send>) {
        self.0.send(payload).unwrap();
    }
}
fn attach(vm: &SessionVm) -> mpsc::Receiver<Box<dyn Any + Send>> {
    let (tx, rx) = mpsc::channel();
    vm.0.wake.attach(Some(Arc::new(Poster(tx))));
    rx
}
fn text_note(vm: &SessionVm) -> EntityId {
    vm.new_folder().unwrap();
    vm.new_note().unwrap();
    vm.insert(BlockKind::RichText).unwrap();
    vm.0.active_block.get().unwrap()
}
fn completion(vm: &SessionVm, rx: &mpsc::Receiver<Box<dyn Any + Send>>) {
    let timeout = Instant::now() + Duration::from_secs(5);
    while vm.0.inflight.get().is_some() {
        let event = rx
            .recv_timeout(timeout.saturating_duration_since(Instant::now()))
            .unwrap();
        assert!(vm.handle_coordinator_event(event.as_ref()));
    }
}

#[test]
fn last_edit_schedules_save_and_worker_wakes_idle_ui() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("idle.noton");
    let vm = SessionVm::open(path.clone()).unwrap();
    let rx = attach(&vm);
    let id = text_note(&vm);
    vm.document(id)
        .unwrap()
        .set_plain_text("Saved without another key")
        .unwrap();
    // Deliver actual queued notifications, as the native event router does.
    let event = rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(vm.handle_coordinator_event(event.as_ref()));
    let due = vm.0.deadline.get().expect("last edit must arm autosave");
    vm.run_scheduled_at(due - Duration::from_millis(1)).unwrap();
    assert!(
        vm.0.inflight.get().is_none(),
        "do not save before the deadline"
    );
    vm.run_scheduled_at(due).unwrap();
    assert!(vm.0.inflight.get().is_some());
    // No MCP observation or polling tick delivers this acknowledgement.
    completion(&vm, &rx);
    assert_eq!(vm.0.save_state.get(), SaveState::Saved);
    assert!(
        vm.0.deadline.get().is_none(),
        "saved library must stop scheduling"
    );
    drop(vm);
    let reopened = SessionVm::open(path).unwrap();
    let snapshot = reopened.0.service.snapshot(vec![]).unwrap();
    assert!(
        snapshot
            .folders
            .iter()
            .flat_map(|f| &f.notes)
            .flat_map(|n| &n.blocks)
            .any(
                |b| matches!(&b.content, noton_format::Content::RichText { djot, .. }
                if djot.contains("Saved without another key"))
            )
    );
}

#[test]
fn continuous_edits_keep_the_five_second_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let vm = SessionVm::open(dir.path().join("continuous.noton")).unwrap();
    text_note(&vm);
    vm.run_scheduled().unwrap();
    let first = Instant::now();
    vm.0.first_edit.set(Some(first));
    vm.0.last_edit.set(first + Duration::from_millis(4800));
    vm.update_deadline();
    assert_eq!(vm.0.deadline.get(), Some(first + Duration::from_secs(5)));
}

#[test]
fn existing_earlier_tree_deadline_is_preserved_and_own_work_rearms() {
    let vm = SessionVm::memory().unwrap();
    vm.run_scheduled().unwrap();
    let now = Instant::now();
    let shared = Rc::new(Cell::new(Some(now + Duration::from_millis(10))));
    *vm.0.tree_wake.borrow_mut() = Some(shared.clone());
    vm.0.deadline.set(Some(now + Duration::from_secs(1)));
    vm.arm_deadline();
    assert_eq!(shared.get(), Some(now + Duration::from_millis(10)));
    shared.set(None); // The other widget's wake was consumed.
    vm.run_scheduled_at(now + Duration::from_millis(10))
        .unwrap();
    assert_eq!(shared.get(), Some(now + Duration::from_secs(1)));
}

#[test]
fn save_failure_does_not_arm_automatic_retries() {
    let dir = tempfile::tempdir().unwrap();
    let vm = SessionVm::open(dir.path().join("failure.noton")).unwrap();
    text_note(&vm);
    vm.mark_save_failed();
    vm.run_scheduled().unwrap();
    assert!(vm.0.deadline.get().is_none());
    assert_eq!(vm.0.save_state.get(), SaveState::Failed);
}

#[test]
fn canvas_fit_waits_for_viewport_then_stops() {
    let vm = SessionVm::memory().unwrap();
    text_note(&vm);
    vm.0.fit_pending.set(vm.0.note.get());
    let viewport = Signal::new(Size::ZERO);
    vm.attach_camera(Camera {
        zoom: Signal::new(1.),
        pan_x: Signal::new(0.),
        pan_y: Signal::new(0.),
        viewport: viewport.clone(),
    });
    vm.run_scheduled().unwrap();
    vm.run_scheduled_at(vm.0.fit_at.get().unwrap()).unwrap();
    assert!(
        vm.0.deadline.get().is_none(),
        "zero viewport must not busy-wait"
    );
    assert!(vm.0.fit_waiting.get());
    viewport.set(Size::new(400., 300.));
    vm.run_scheduled().unwrap();
    vm.run_scheduled_at(vm.0.fit_at.get().unwrap()).unwrap();
    assert!(!vm.0.fit_waiting.get());
    assert!(vm.0.deadline.get().is_none());
}

#[test]
fn unchanged_production_tree_has_no_recurring_frame_deadline() {
    let vm = SessionVm::memory().unwrap();
    let mut app = crate::automation::build(vm.clone(), "en-US", (1280, 850)).unwrap();
    for _ in 0..10 {
        app.tree.request_frame();
        app.tree.layout(SizeProposal::exact(1280., 850.));
        app.tree.render();
    }
    // Consume the final requested frame. A root subscription would rearm on render.
    app.tree.layout(SizeProposal::exact(1280., 850.));
    app.tree.render();
    assert!(
        app.tree.frame_tick_deadline().is_none(),
        "root must not rearm frames"
    );
    assert!(vm.0.deadline.get().is_none());
}
fn focused_editor(vm: &SessionVm) -> (teksilo::app::HeadlessApp, EntityId) {
    use teksilo::automation::{self, AutomationOp, RecordingWindowOps, SettleSpec};
    let id = text_note(vm);
    vm.0.reading.set(true);
    let mut app = crate::automation::build(vm.clone(), "en-US", (1280, 850)).unwrap();
    let update = app.tree.sync_accessibility();
    let node = update
        .nodes
        .iter()
        .find(|(_, n)| n.role() == teksilo::core::accesskit::Role::MultilineTextInput)
        .expect("rich editor")
        .0;
    let reply = automation::execute(
        &mut app.tree,
        &mut RecordingWindowOps::new(),
        &AutomationOp::FocusNode { node: node.0 },
        &SettleSpec::default(),
    );
    assert!(
        matches!(reply, automation::AutomationReply::Ok { .. }),
        "{reply:?}"
    );
    (app, id)
}

#[test]
fn format_changes_wake_deferred_flush_and_toolbar_without_input() {
    let vm = SessionVm::memory().unwrap();
    let (_app, id) = focused_editor(&vm);
    let doc = vm.document(id).unwrap();
    doc.set_plain_text("format me").unwrap();
    vm.run_scheduled().unwrap();
    let handle = vm.0.handles.borrow().get(&id).unwrap().clone();
    handle.select_range(0, 9);
    let rx = attach(&vm);
    while let Ok(event) = rx.try_recv() {
        vm.handle_coordinator_event(event.as_ref());
    }
    handle.toggle_bold();
    let event = rx
        .recv_timeout(Duration::from_secs(1))
        .expect("format change wakes the UI");
    vm.handle_coordinator_event(event.as_ref());
    assert!(vm.0.service.block(id).unwrap().content_djot.contains('*'));
    assert_eq!(
        vm.0.editing.get(),
        Some(id),
        "editor focus must select the toolbar target"
    );
    assert!(vm.0.format.bold.get(), "handle bold: {}", handle.is_bold());
    handle.undo();
    let event = rx.recv_timeout(Duration::from_secs(1)).unwrap();
    vm.handle_coordinator_event(event.as_ref());
    assert!(!vm.0.service.block(id).unwrap().content_djot.contains('*'));
}

#[test]
fn ime_preview_is_not_flushed_and_commit_wakes_flush() {
    let vm = SessionVm::memory().unwrap();
    let (mut app, id) = focused_editor(&vm);
    vm.run_scheduled().unwrap();
    let before = vm.0.service.block(id).unwrap().content_djot;
    let rx = attach(&vm);
    app.tree.dispatch_event(WidgetEvent::ImeComposition {
        text: "ni".into(),
        cursor: None,
    });
    app.tree.layout(SizeProposal::exact(1280., 850.));
    while let Ok(event) = rx.try_recv() {
        vm.handle_coordinator_event(event.as_ref());
    }
    assert!(vm.0.handles.borrow().get(&id).unwrap().is_composing());
    assert_eq!(vm.0.service.block(id).unwrap().content_djot, before);
    app.tree
        .dispatch_event(WidgetEvent::ImeCommit { text: "你".into() });
    // The editor owns the keystroke batch deadline; advance its clock, not the coordinator.
    app.tree.advance_time(Duration::from_millis(250));
    app.tree.layout(SizeProposal::exact(1280., 850.));
    let event = rx
        .recv_timeout(Duration::from_secs(1))
        .expect("commit must wake the UI");
    vm.handle_coordinator_event(event.as_ref());
    assert!(!vm.0.handles.borrow().get(&id).unwrap().is_composing());
    assert!(vm.0.service.block(id).unwrap().content_djot.contains('你'));
    assert!(!vm.0.service.block(id).unwrap().content_djot.contains("ni"));
}
