//! External image drops use the same image blocks and asset storage as Insert.
use crate::session_vm::{Camera, SessionVm};
use std::path::Path;
use teksilo::{
    core::{DropFeedback, drag_payload::DragPayload},
    prelude::*,
};

const IMAGE_TYPES: [(&str, &str); 3] = [
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/webp", "webp"),
];

fn image_path(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "webp"
        )
    })
}

pub fn accepts(payload: &DragPayload) -> bool {
    payload.is_external()
        && (payload.files().iter().any(|p| image_path(p))
            || IMAGE_TYPES.iter().any(|(mime, _)| payload.has_mime(mime))
            // Wayland advertises formats before it transfers file paths/bytes.
            || payload.formats().iter().any(|f| {
                f == "text/uri-list" || IMAGE_TYPES.iter().any(|(mime, _)| f == mime)
            }))
}

pub fn import(
    vm: &SessionVm,
    payload: &DragPayload,
    position: Option<(f64, f64)>,
) -> anyhow::Result<usize> {
    anyhow::ensure!(payload.is_external(), "Not an external image drop");
    let mut imported = 0;
    let mut errors = Vec::new();
    let mut add = |name: String, bytes: anyhow::Result<Vec<u8>>| {
        let point = position.map(|(x, y)| (x + imported as f64 * 24., y + imported as f64 * 24.));
        match bytes.and_then(|bytes| vm.import_image_bytes(name.clone(), bytes, point)) {
            Ok(_) => imported += 1,
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    };
    for path in payload.files().iter().filter(|p| image_path(p)) {
        let bytes = (|| {
            anyhow::ensure!(
                std::fs::metadata(path)?.len() <= 100 * 1024 * 1024,
                "Image exceeds 100 MiB"
            );
            Ok(std::fs::read(path)?)
        })();
        add(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            bytes,
        );
    }
    // A file drag may also advertise image bytes: prefer files, never duplicate
    // their alternate representations. Multiple MIME encodings are one image.
    if payload.files().is_empty() {
        for (mime, extension) in IMAGE_TYPES {
            if let Some(bytes) = payload.get_mime(mime) {
                add(format!("Image.{extension}"), Ok(bytes.to_vec()));
                break;
            }
        }
    }
    if imported > 0 {
        if vm.0.reading.get() {
            vm.0.reading.set(false);
            if let Some(id) = vm.0.selected.get() {
                vm.0.focus_block.set(Some((id, None)));
            }
            vm.rebuild_page();
        }
        vm.0.navigation.set(2);
    }
    if !errors.is_empty() {
        let error = anyhow::anyhow!(errors.join("\n"));
        if imported == 0 {
            return Err(error);
        }
        vm.report(Err(error));
    }
    anyhow::ensure!(
        imported > 0,
        "Drop a PNG, JPEG or WebP image file or image data"
    );
    Ok(imported)
}

pub fn target(
    widget: impl Widget + 'static,
    vm: &SessionVm,
    camera: Option<Camera>,
) -> impl Widget + 'static {
    let hover = vm.clone();
    let drop = vm.clone();
    widget
        .on_drag_hover(move |payload, _, _| {
            if !hover.0.trash.get() && !hover.0.note_trashed.get() && accepts(payload) {
                DropFeedback::Accept
            } else {
                DropFeedback::NoFeedback
            }
        })
        .on_drop(move |payload, point, _| {
            let position = camera.as_ref().map(|c| {
                let zoom = c.zoom.get().max(0.01);
                (
                    ((point.x - c.pan_x.get()) / zoom) as f64,
                    ((point.y - c.pan_y.get()) / zoom) as f64,
                )
            });
            match import(&drop, &payload, position) {
                Ok(_) => true,
                Err(e) => {
                    drop.report(Err(e));
                    false
                }
            }
        })
}
