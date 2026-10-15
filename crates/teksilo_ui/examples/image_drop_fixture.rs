//! Generate an image-drop fixture for native frame interaction automation.
use base64::Engine;
use teksilo::core::drag_payload::{DragPayload, ExternalDropData};
use teksilo_ui::{image_drop, session_vm::SessionVm};
fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("output .noton path");
    let bytes = base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAACAAAAAYCAIAAAAUMWhjAAAAJElEQVR4nGN4MMGBpohh1IJRC0YtGLVg1IJRC0YtGLVgaFgAAAKQEEzcGbP7AAAAAElFTkSuQmCC")?;
    let vm = SessionVm::memory()?;
    let payload = DragPayload::external(ExternalDropData {
        mime: [("image/png".into(), bytes)].into(),
        ..Default::default()
    });
    image_drop::import(&vm, &payload, Some((80., 80.)))?;
    let id = vm.0.selected.get().unwrap();
    vm.resize(id, 280., 200.)?;
    vm.save_copy(std::path::Path::new(&path))
}
