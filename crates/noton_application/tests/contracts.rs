use common::entities::BlockKind;
use noton_application::{Kind, LibraryService};
use noton_format::{Geometry, LibraryFile, Sample, Stroke};
use uuid::Uuid;

#[test]
fn undo_creation_keeps_later_text_and_geometry_undo_never_rewinds_text() {
    let s = LibraryService::new("Test".into()).unwrap();
    let org = s.new_stack();
    let page = s.new_stack();
    let f = s.create_folder("Folder".into(), org).unwrap();
    let n = s.create_note(f, "Note".into(), org).unwrap();
    let b = s
        .create_block(n, BlockKind::RichText, "Text".into(), page)
        .unwrap();
    s.flush_text(b, "Latest writing\n".into()).unwrap();
    s.undo(page).unwrap();
    assert!(s.block(b).unwrap().trashed);
    s.redo(page).unwrap();
    assert_eq!(s.block(b).unwrap().content_djot, "Latest writing\n");
    let original_x = s.block(b).unwrap().x;
    s.geometry(
        b,
        Geometry {
            x: 700.,
            ..Default::default()
        },
        page,
    )
    .unwrap();
    s.flush_text(b, "Even newer\n".into()).unwrap();
    s.undo(page).unwrap();
    assert_eq!(s.block(b).unwrap().content_djot, "Even newer\n");
    assert_eq!(s.block(b).unwrap().x, original_x);
}
#[test]
fn portable_file_roundtrip_lock_and_backup() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("test.noton");
    let s = LibraryService::new("Carnet".into()).unwrap();
    let stack = s.new_stack();
    let f = s.create_folder("Études".into(), stack).unwrap();
    let n = s.create_note(f, "Table".into(), stack).unwrap();
    let b = s
        .create_block(n, BlockKind::Drawing, "Croquis".into(), stack)
        .unwrap();
    s.add_stroke(
        b,
        Stroke {
            uid: Uuid::new_v4(),
            trashed: false,
            version: 1,
            width: 2.,
            rgba: 0xff0000ff,
            pressure: true,
            samples: vec![Sample {
                x: 2.,
                y: 4.,
                pressure: 0.7,
            }],
        },
        stack,
    )
    .unwrap();
    let snapshot = s.snapshot(vec![]).unwrap();
    let mut file = LibraryFile::open(&path, true).unwrap();
    file.save(&snapshot).unwrap();
    assert!(LibraryFile::open(&path, false).is_err());
    assert_eq!(file.load().unwrap(), snapshot);
    let copy = d.path().join("backup.noton");
    file.backup(&copy).unwrap();
    assert_eq!(
        LibraryFile::open(&copy, false).unwrap().load().unwrap(),
        snapshot
    );
    let loaded = LibraryService::load(&snapshot).unwrap();
    assert_eq!(loaded.snapshot(vec![]).unwrap(), snapshot);
}
#[test]
fn move_note_is_atomic_and_undo_restores_order() {
    let s = LibraryService::new("Test".into()).unwrap();
    let stack = s.new_stack();
    let a = s.create_folder("A".into(), stack).unwrap();
    let b = s.create_folder("B".into(), stack).unwrap();
    let n = s.create_note(a, "N".into(), stack).unwrap();
    s.move_note(n, b, stack).unwrap();
    assert!(s.notes(a).unwrap().is_empty());
    assert_eq!(s.notes(b).unwrap()[0].id, n);
    s.undo(stack).unwrap();
    assert_eq!(s.notes(a).unwrap()[0].id, n);
    assert!(s.notes(b).unwrap().is_empty());
    assert!(s.move_note(n, 999, stack).is_err());
    assert_eq!(s.notes(a).unwrap()[0].id, n);
}
#[test]
fn invalid_geometry_and_ink_do_not_enter_store() {
    let s = LibraryService::new("Test".into()).unwrap();
    let stack = s.new_stack();
    let f = s.create_folder("F".into(), stack).unwrap();
    let n = s.create_note(f, "N".into(), stack).unwrap();
    let b = s
        .create_block(n, BlockKind::Drawing, "D".into(), stack)
        .unwrap();
    assert!(
        s.geometry(
            b,
            Geometry {
                x: f64::NAN,
                ..Default::default()
            },
            stack
        )
        .is_err()
    );
    assert!(s.block(b).unwrap().x.is_finite());
    assert!(
        s.add_stroke(
            b,
            Stroke {
                uid: Uuid::new_v4(),
                trashed: false,
                version: 1,
                width: 2.,
                rgba: 0,
                pressure: false,
                samples: vec![Sample {
                    x: 0.,
                    y: 0.,
                    pressure: 2.
                }]
            },
            stack
        )
        .is_err()
    );
    assert!(s.block(b).unwrap().strokes.is_empty());
}
fn overlaps(a: &common::entities::Block, b: &common::entities::Block) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}
#[test]
fn new_blocks_land_in_free_space() {
    let s = LibraryService::new("Test".into()).unwrap();
    let stack = s.new_stack();
    let f = s.create_folder("F".into(), stack).unwrap();
    let n = s.create_note(f, "N".into(), stack).unwrap();
    for i in 0..6 {
        let kind = if i % 2 == 0 {
            BlockKind::RichText
        } else {
            BlockKind::Drawing
        };
        s.create_block(n, kind, format!("B{i}"), stack).unwrap();
    }
    let blocks = s.blocks(n).unwrap();
    for (i, a) in blocks.iter().enumerate() {
        for b in &blocks[i + 1..] {
            assert!(!overlaps(a, b), "{} overlaps {}", a.label, b.label);
        }
    }
    assert_eq!((blocks[0].x, blocks[0].y), (32., 28.));
    // The second block sits beside the first, 24 px away, on the same row.
    assert_eq!(blocks[1].x, blocks[0].x + blocks[0].width + 24.);
    assert_eq!(blocks[1].y, blocks[0].y);
}
#[test]
fn duplicate_copies_content_and_strokes_in_one_undo_step() {
    let s = LibraryService::new("Test".into()).unwrap();
    let stack = s.new_stack();
    let f = s.create_folder("F".into(), stack).unwrap();
    let n = s.create_note(f, "N".into(), stack).unwrap();
    let b = s
        .create_block(n, BlockKind::Drawing, "Croquis".into(), stack)
        .unwrap();
    s.describe_sketch(b, "Accueil mène à Note".into(), stack)
        .unwrap();
    s.add_stroke(
        b,
        Stroke {
            uid: Uuid::new_v4(),
            trashed: false,
            version: 1,
            width: 2.5,
            rgba: 0x202830ff,
            pressure: true,
            samples: vec![Sample {
                x: 10.,
                y: 10.,
                pressure: 1.,
            }],
        },
        stack,
    )
    .unwrap();
    let copy = s.duplicate_block(b, stack).unwrap();
    let (a, c) = (s.block(b).unwrap(), s.block(copy).unwrap());
    assert_eq!(
        (c.label.as_str(), c.description.as_str()),
        ("Croquis", "Accueil mène à Note")
    );
    assert_eq!(c.strokes.len(), 1);
    assert_ne!(
        s.ink_stroke(c.strokes[0]).unwrap().uid,
        s.ink_stroke(a.strokes[0]).unwrap().uid
    );
    assert!(!overlaps(&a, &c));
    assert_eq!(s.note(n).unwrap().blocks, vec![b, copy]);
    s.undo(stack).unwrap();
    assert!(s.block(copy).unwrap().trashed);
    assert!(!s.block(b).unwrap().trashed);
    s.snapshot(vec![]).unwrap();
}
#[test]
fn arrange_and_alt_text_are_single_undo_steps() {
    let s = LibraryService::new("Test".into()).unwrap();
    let stack = s.new_stack();
    let f = s.create_folder("F".into(), stack).unwrap();
    let n = s.create_note(f, "N".into(), stack).unwrap();
    let a = s
        .create_block(n, BlockKind::RichText, "A".into(), stack)
        .unwrap();
    let b = s
        .create_block(n, BlockKind::RichText, "B".into(), stack)
        .unwrap();
    assert!(s.block(b).unwrap().z > s.block(a).unwrap().z);
    s.arrange_block(a, true, stack).unwrap();
    assert!(s.block(a).unwrap().z > s.block(b).unwrap().z);
    s.arrange_block(a, false, stack).unwrap();
    assert!(s.block(a).unwrap().z < s.block(b).unwrap().z);
    s.undo(stack).unwrap();
    assert!(s.block(a).unwrap().z > s.block(b).unwrap().z);
    assert!(
        s.set_alt_text(a, "Photo".into(), stack).is_err(),
        "text blocks have no alt text"
    );
}
#[test]
fn folders_and_notes_reorder_in_one_undo_step() {
    let s = LibraryService::new("Test".into()).unwrap();
    let stack = s.new_stack();
    let a = s.create_folder("A".into(), stack).unwrap();
    let b = s.create_folder("B".into(), stack).unwrap();
    let n1 = s.create_note(a, "1".into(), stack).unwrap();
    let n2 = s.create_note(a, "2".into(), stack).unwrap();
    s.reorder_folder(b, 0, stack).unwrap();
    assert_eq!(
        s.folders()
            .unwrap()
            .iter()
            .map(|f| f.id)
            .collect::<Vec<_>>(),
        vec![b, a]
    );
    s.reorder_note(a, n2, 0, stack).unwrap();
    assert_eq!(s.folder(a).unwrap().notes, vec![n2, n1]);
    s.undo(stack).unwrap();
    assert_eq!(s.folder(a).unwrap().notes, vec![n1, n2]);
    s.undo(stack).unwrap();
    assert_eq!(s.folders().unwrap()[0].id, a);
    s.snapshot(vec![]).unwrap();
}
#[test]
fn undone_creations_leave_the_lists_the_trash_and_the_file() {
    let s = LibraryService::new("Test".into()).unwrap();
    let org = s.new_stack();
    let page = s.new_stack();
    let f = s.create_folder("F".into(), org).unwrap();
    let n = s.create_note(f, "N".into(), org).unwrap();
    let b = s
        .create_block(n, BlockKind::RichText, "B".into(), page)
        .unwrap();
    s.undo(page).unwrap();
    assert!(s.is_discarded(Kind::Block, b));
    assert!(!s.is_discarded(Kind::Note, n), "ids repeat across kinds");
    assert!(s.blocks(n).unwrap().is_empty());
    assert!(
        s.snapshot(vec![]).unwrap().folders[0].notes[0]
            .blocks
            .is_empty()
    );
    s.redo(page).unwrap();
    assert_eq!(s.blocks(n).unwrap()[0].id, b);
    let m = s.create_note(f, "Empty".into(), org).unwrap();
    s.undo(org).unwrap();
    assert_eq!(s.notes(f).unwrap().len(), 1, "not in the Trash either");
    assert_eq!(s.snapshot(vec![]).unwrap().folders[0].notes.len(), 1);
    s.redo(org).unwrap();
    assert_eq!(s.notes(f).unwrap()[1].id, m);
}
#[test]
fn an_undone_creation_that_gained_content_goes_to_the_trash() {
    let s = LibraryService::new("Test".into()).unwrap();
    let org = s.new_stack();
    let page = s.new_stack();
    let f = s.create_folder("F".into(), org).unwrap();
    let n = s.create_note(f, "N".into(), org).unwrap();
    let b = s
        .create_block(n, BlockKind::RichText, "B".into(), page)
        .unwrap();
    // Typing is text history, not a step of the page stack.
    s.flush_text(b, "Written since\n".into()).unwrap();
    s.undo(page).unwrap();
    assert!(!s.is_discarded(Kind::Block, b));
    assert!(s.blocks(n).unwrap()[0].trashed, "restorable from the Trash");
    s.redo(page).unwrap();
    // The note holds a block (from another stack): undoing its creation
    // trashes it with its content.
    s.undo(org).unwrap();
    assert!(s.notes(f).unwrap()[0].trashed);
    let saved = s.snapshot(vec![]).unwrap();
    assert!(saved.folders[0].notes[0].trashed);
    assert_eq!(saved.folders[0].notes[0].blocks.len(), 1);
}
#[test]
fn only_images_in_the_library_keep_their_asset_in_the_file() {
    let s = LibraryService::new("Test".into()).unwrap();
    let stack = s.new_stack();
    let f = s.create_folder("F".into(), stack).unwrap();
    let n = s.create_note(f, "N".into(), stack).unwrap();
    let asset = noton_format::Asset::new("a.png".into(), "image/png".into(), vec![1, 2, 3]);
    let b = s.insert_image(n, &asset, 100., 50., stack).unwrap();
    assert_eq!(s.image_asset(b).unwrap(), asset.uid);
    assert_eq!(s.snapshot(vec![asset.clone()]).unwrap().assets.len(), 1);
    s.undo(stack).unwrap();
    assert!(s.snapshot(vec![asset.clone()]).unwrap().assets.is_empty());
    s.redo(stack).unwrap();
    s.trash_block(b, true, stack).unwrap();
    assert_eq!(
        s.snapshot(vec![asset]).unwrap().assets.len(),
        1,
        "a trashed image can still be restored"
    );
}
#[test]
fn commands_that_change_nothing_add_no_undo_step() {
    let s = LibraryService::new("Test".into()).unwrap();
    let stack = s.new_stack();
    let f = s.create_folder("F".into(), stack).unwrap();
    let n = s.create_note(f, "N".into(), stack).unwrap();
    let a = s
        .create_block(n, BlockKind::RichText, "A".into(), stack)
        .unwrap();
    let b = s
        .create_block(n, BlockKind::RichText, "B".into(), stack)
        .unwrap();
    let head = s.head(stack);
    s.arrange_block(b, true, stack).unwrap();
    s.arrange_block(a, false, stack).unwrap();
    s.reorder_block(n, a, 0, stack).unwrap();
    assert_eq!(s.head(stack), head);
    assert!(s.rename_note(n, "  ".into(), stack).is_err());
    assert!(s.rename_folder(f, String::new(), stack).is_err());
}
#[test]
fn alt_text_and_decorative_are_one_undo_step() {
    let s = LibraryService::new("Test".into()).unwrap();
    let stack = s.new_stack();
    let f = s.create_folder("F".into(), stack).unwrap();
    let n = s.create_note(f, "N".into(), stack).unwrap();
    let asset = noton_format::Asset::new("a.png".into(), "image/png".into(), vec![1, 2, 3]);
    let b = s.insert_image(n, &asset, 100., 50., stack).unwrap();
    s.describe_image(b, "Rule".into(), true, stack).unwrap();
    let e = s.block(b).unwrap();
    assert_eq!((e.alt_text.as_str(), e.decorative), ("Rule", true));
    s.undo(stack).unwrap();
    let e = s.block(b).unwrap();
    assert_eq!((e.alt_text.as_str(), e.decorative), ("a.png", false));
}
