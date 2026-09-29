//! Application use cases. All multi-record mutations commit once and publish afterwards.
use anyhow::{Context, Result, ensure};
use common::{
    database::{transactions::Transaction, write_guard::WriteTransactionGuard},
    direct_access::repository_factory::write as repo,
    entities::*,
    event::EventBuffer,
    types::EntityId,
    undo_redo::{UndoLabel, UndoRedoCommand},
};
use frontend::AppContext;
use noton_format as format;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

/// The kinds of entity a command creates. Entity ids are only unique within
/// a kind: the first folder, note and block are all id 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Folder,
    Note,
    Block,
    Stroke,
}
/// Entities whose creation is currently undone and that held nothing added
/// since. They stay in the store (as trashed) so a redo can bring them back,
/// but they are not in the Trash, the lists, search or the saved file: the
/// user never kept them. An undone creation that did gain content (text typed
/// into a block, blocks added to a note: histories of their own) goes to the
/// Trash instead, where it stays restorable.
type Discarded = Arc<Mutex<HashSet<(Kind, EntityId)>>>;
/// What a creation command created, with the text a block started with.
struct Created {
    kind: Kind,
    id: EntityId,
    text: String,
}
/// Whether `c` holds nothing added since it was created. Strokes, a sketch's
/// description and an image's alt text are taken back by earlier steps of the
/// same stack; a note's blocks and a block's text are not.
fn untouched(
    db: &common::database::db_context::DbContext,
    c: &Created,
    discarded: &HashSet<(Kind, EntityId)>,
) -> Result<bool> {
    let tx = Transaction::begin_read_transaction(db)?;
    let gone = |kind, ids: &[EntityId]| ids.iter().all(|id| discarded.contains(&(kind, *id)));
    Ok(match c.kind {
        Kind::Folder => gone(
            Kind::Note,
            &repo::create_folder_repository(&tx)?
                .get(&c.id)?
                .context("Missing folder")?
                .notes,
        ),
        Kind::Note => gone(
            Kind::Block,
            &repo::create_note_repository(&tx)?
                .get(&c.id)?
                .context("Missing note")?
                .blocks,
        ),
        Kind::Block => {
            let b = repo::create_block_repository(&tx)?
                .get(&c.id)?
                .context("Missing block")?;
            match b.kind {
                BlockKind::RichText => b.content_djot == c.text,
                BlockKind::Drawing => b.description.is_empty() && gone(Kind::Stroke, &b.strokes),
                BlockKind::Image => true,
            }
        }
        Kind::Stroke => true,
    })
}

#[derive(Clone, Debug)]
pub struct LibraryService {
    pub context: AppContext,
    pub library_id: EntityId,
    discarded: Discarded,
}
fn transaction<T>(
    ctx: &AppContext,
    f: impl FnOnce(&Transaction, &mut EventBuffer) -> Result<T>,
) -> Result<T> {
    transaction_parts(&ctx.db_context, &ctx.event_hub, f)
}
fn transaction_parts<T>(
    db: &common::database::db_context::DbContext,
    hub: &std::sync::Arc<common::event::EventHub>,
    f: impl FnOnce(&Transaction, &mut EventBuffer) -> Result<T>,
) -> Result<T> {
    let guard = WriteTransactionGuard::acquire(db, "noton_use_case")?;
    let mut tx = Transaction::begin_write_transaction(db)?;
    let mut events = EventBuffer::new();
    events.begin_buffering();
    let result = f(&tx, &mut events)?;
    tx.commit()?;
    drop(guard);
    for event in events.flush() {
        hub.send_event(event);
    }
    Ok(result)
}
#[derive(Clone, Debug)]
pub enum Patch {
    FolderTrash(EntityId, bool),
    NoteTrash(EntityId, bool),
    BlockTrash(EntityId, bool),
    StrokeTrash(EntityId, bool),
    FolderTitle(EntityId, String),
    NoteTitle(EntityId, String),
    BlockLabel(EntityId, String),
    Geometry(EntityId, format::Geometry),
    Description(EntityId, String),
    AltText(EntityId, String),
    Decorative(EntityId, bool),
    FolderNotes(EntityId, Vec<EntityId>),
    NoteBlocks(EntityId, Vec<EntityId>),
    LibraryFolders(EntityId, Vec<EntityId>),
}
fn apply(ctx: &AppContext, patches: &[Patch]) -> Result<()> {
    apply_parts(&ctx.db_context, &ctx.event_hub, patches)
}
fn apply_parts(
    db: &common::database::db_context::DbContext,
    events: &std::sync::Arc<common::event::EventHub>,
    patches: &[Patch],
) -> Result<()> {
    transaction_parts(db, events, |tx, ev| {
        for patch in patches {
            match patch {
                Patch::FolderTrash(id, value) => {
                    let mut r = repo::create_folder_repository(tx)?;
                    let mut e = r.get(id)?.context("Missing folder")?;
                    e.trashed = *value;
                    e.updated_at = chrono::Utc::now();
                    r.update(ev, &e)?;
                }
                Patch::NoteTrash(id, value) => {
                    let mut r = repo::create_note_repository(tx)?;
                    let mut e = r.get(id)?.context("Missing note")?;
                    e.trashed = *value;
                    e.updated_at = chrono::Utc::now();
                    r.update(ev, &e)?;
                }
                Patch::BlockTrash(id, value) => {
                    let mut r = repo::create_block_repository(tx)?;
                    let mut e = r.get(id)?.context("Missing block")?;
                    e.trashed = *value;
                    e.updated_at = chrono::Utc::now();
                    r.update(ev, &e)?;
                }
                Patch::StrokeTrash(id, value) => {
                    let mut r = repo::create_ink_stroke_repository(tx)?;
                    let mut e = r.get(id)?.context("Missing ink_stroke")?;
                    e.trashed = *value;
                    e.updated_at = chrono::Utc::now();
                    r.update(ev, &e)?;
                }
                Patch::FolderTitle(id, value) => {
                    let mut r = repo::create_folder_repository(tx)?;
                    let mut e = r.get(id)?.context("Missing folder")?;
                    e.title = value.clone();
                    e.updated_at = chrono::Utc::now();
                    r.update(ev, &e)?;
                }
                Patch::NoteTitle(id, value) => {
                    let mut r = repo::create_note_repository(tx)?;
                    let mut e = r.get(id)?.context("Missing note")?;
                    e.title = value.clone();
                    e.updated_at = chrono::Utc::now();
                    r.update(ev, &e)?;
                }
                Patch::BlockLabel(id, value) => {
                    let mut r = repo::create_block_repository(tx)?;
                    let mut e = r.get(id)?.context("Missing block")?;
                    e.label = value.clone();
                    e.updated_at = chrono::Utc::now();
                    r.update(ev, &e)?;
                }
                Patch::Description(id, value) => {
                    let mut r = repo::create_block_repository(tx)?;
                    let mut e = r.get(id)?.context("Missing block")?;
                    e.description = value.clone();
                    e.updated_at = chrono::Utc::now();
                    r.update(ev, &e)?;
                }
                Patch::AltText(id, value) => {
                    let mut r = repo::create_block_repository(tx)?;
                    let mut e = r.get(id)?.context("Missing block")?;
                    e.alt_text = value.clone();
                    e.updated_at = chrono::Utc::now();
                    r.update(ev, &e)?;
                }
                Patch::Decorative(id, value) => {
                    let mut r = repo::create_block_repository(tx)?;
                    let mut e = r.get(id)?.context("Missing block")?;
                    e.decorative = *value;
                    e.updated_at = chrono::Utc::now();
                    r.update(ev, &e)?;
                }
                Patch::Geometry(id, value) => {
                    value.validate()?;
                    let mut r = repo::create_block_repository(tx)?;
                    let mut e = r.get(id)?.context("Missing block")?;
                    e.x = value.x;
                    e.y = value.y;
                    e.width = value.width;
                    e.height = value.height;
                    e.z = value.z;
                    e.updated_at = chrono::Utc::now();
                    r.update(ev, &e)?;
                }
                Patch::FolderNotes(id, ids) => {
                    repo::create_folder_repository(tx)?.set_relationship(
                        ev,
                        id,
                        &common::direct_access::folder::FolderRelationshipField::Notes,
                        ids,
                    )?;
                }
                Patch::NoteBlocks(id, ids) => {
                    repo::create_note_repository(tx)?.set_relationship(
                        ev,
                        id,
                        &common::direct_access::note::NoteRelationshipField::Blocks,
                        ids,
                    )?;
                }
                Patch::LibraryFolders(id, ids) => {
                    repo::create_library_repository(tx)?.set_relationship(
                        ev,
                        id,
                        &common::direct_access::library::LibraryRelationshipField::Folders,
                        ids,
                    )?;
                }
            }
        }
        Ok(())
    })
}
struct Command {
    db: common::database::db_context::DbContext,
    events: std::sync::Arc<common::event::EventHub>,
    before: Vec<Patch>,
    after: Vec<Patch>,
    /// What this command created: discarded while it is undone, if untouched.
    created: Option<Created>,
    discarded: Discarded,
    label: &'static str,
}
impl UndoRedoCommand for Command {
    fn undo(&mut self) -> Result<()> {
        apply_parts(&self.db, &self.events, &self.before)?;
        if let Some(c) = &self.created {
            let mut discarded = lock(&self.discarded);
            if untouched(&self.db, c, &discarded)? {
                discarded.insert((c.kind, c.id));
            }
        }
        Ok(())
    }
    fn redo(&mut self) -> Result<()> {
        if let Some(c) = &self.created {
            lock(&self.discarded).remove(&(c.kind, c.id));
        }
        apply_parts(&self.db, &self.events, &self.after)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn label(&self) -> Option<UndoLabel> {
        Some(UndoLabel::act(self.label))
    }
}
fn lock(discarded: &Discarded) -> std::sync::MutexGuard<'_, HashSet<(Kind, EntityId)>> {
    discarded.lock().unwrap_or_else(|e| e.into_inner())
}
impl LibraryService {
    pub fn new(title: String) -> Result<Self> {
        Self::load(&format::Library::empty(title))
    }
    fn record(
        &self,
        stack: u64,
        before: Vec<Patch>,
        after: Vec<Patch>,
        created: Option<Created>,
        label: &'static str,
    ) -> Result<()> {
        common::long_operation::lock_or_recover(&self.context.undo_redo_manager)
            .add_command_to_stack(
                Box::new(Command {
                    db: self.context.db_context.clone(),
                    events: self.context.event_hub.clone(),
                    before,
                    after,
                    created,
                    discarded: self.discarded.clone(),
                    label,
                }),
                Some(stack),
            )?;
        Ok(())
    }
    /// Record the creation of `id`: undo trashes it (and discards it if it is
    /// untouched), redo restores it.
    fn record_creation(
        &self,
        stack: u64,
        kind: Kind,
        id: EntityId,
        label: &'static str,
    ) -> Result<()> {
        let trash = match kind {
            Kind::Folder => Patch::FolderTrash,
            Kind::Note => Patch::NoteTrash,
            Kind::Block => Patch::BlockTrash,
            Kind::Stroke => Patch::StrokeTrash,
        };
        let text = if kind == Kind::Block {
            self.block(id)?.content_djot
        } else {
            String::new()
        };
        self.record(
            stack,
            vec![trash(id, true)],
            vec![trash(id, false)],
            Some(Created { kind, id, text }),
            label,
        )
    }
    /// Whether the `kind` entity `id` was created by a command that is
    /// currently undone, and so is in neither the lists nor the Trash.
    pub fn is_discarded(&self, kind: Kind, id: EntityId) -> bool {
        lock(&self.discarded).contains(&(kind, id))
    }
    fn edit(
        &self,
        stack: u64,
        before: Vec<Patch>,
        after: Vec<Patch>,
        label: &'static str,
    ) -> Result<()> {
        apply(&self.context, &after)?;
        self.record(stack, before, after, None, label)
    }
    pub fn undo(&self, stack: u64) -> Result<()> {
        common::long_operation::lock_or_recover(&self.context.undo_redo_manager).undo(Some(stack))
    }
    pub fn redo(&self, stack: u64) -> Result<()> {
        common::long_operation::lock_or_recover(&self.context.undo_redo_manager).redo(Some(stack))
    }
    /// What the next undo on `stack` would take back, as the command's key
    /// (`"move_block"`, `"rename_folder"`, …).
    pub fn undo_label(&self, stack: u64) -> Option<&'static str> {
        common::long_operation::lock_or_recover(&self.context.undo_redo_manager)
            .undo_label(Some(stack))
            .map(|l| l.subject)
    }
    pub fn redo_label(&self, stack: u64) -> Option<&'static str> {
        common::long_operation::lock_or_recover(&self.context.undo_redo_manager)
            .redo_label(Some(stack))
            .map(|l| l.subject)
    }
    /// The sequence number of the command on top of `stack`, to hand to
    /// [`undo_if_head`](Self::undo_if_head).
    pub fn head(&self, stack: u64) -> Option<u64> {
        common::long_operation::lock_or_recover(&self.context.undo_redo_manager)
            .head_seq(Some(stack))
    }
    /// Undo the top of `stack` only if it is still the command `seq`; a
    /// toast's Undo must never take back something pushed after it appeared.
    pub fn undo_if_head(&self, stack: u64, seq: u64) -> Result<bool> {
        Ok(matches!(
            common::long_operation::lock_or_recover(&self.context.undo_redo_manager)
                .undo_if_head(Some(stack), seq)?,
            common::undo_redo::UndoStatus::Undone
        ))
    }
    pub fn new_stack(&self) -> u64 {
        common::long_operation::lock_or_recover(&self.context.undo_redo_manager).create_new_stack()
    }
    pub fn library(&self, id: EntityId) -> Result<Library> {
        let tx = Transaction::begin_read_transaction(&self.context.db_context)?;
        repo::create_library_repository(&tx)?
            .get(&id)?
            .context("Missing library")
    }
    pub fn folder(&self, id: EntityId) -> Result<Folder> {
        let tx = Transaction::begin_read_transaction(&self.context.db_context)?;
        repo::create_folder_repository(&tx)?
            .get(&id)?
            .context("Missing folder")
    }
    pub fn note(&self, id: EntityId) -> Result<Note> {
        let tx = Transaction::begin_read_transaction(&self.context.db_context)?;
        repo::create_note_repository(&tx)?
            .get(&id)?
            .context("Missing note")
    }
    pub fn block(&self, id: EntityId) -> Result<Block> {
        let tx = Transaction::begin_read_transaction(&self.context.db_context)?;
        repo::create_block_repository(&tx)?
            .get(&id)?
            .context("Missing block")
    }
    pub fn ink_stroke(&self, id: EntityId) -> Result<InkStroke> {
        let tx = Transaction::begin_read_transaction(&self.context.db_context)?;
        repo::create_ink_stroke_repository(&tx)?
            .get(&id)?
            .context("Missing ink_stroke")
    }
    /// The library's folders, trashed ones included, discarded ones not.
    pub fn folders(&self) -> Result<Vec<Folder>> {
        self.library(self.library_id)?
            .folders
            .into_iter()
            .filter(|id| !self.is_discarded(Kind::Folder, *id))
            .map(|id| self.folder(id))
            .collect()
    }
    /// The folder's notes, trashed ones included, discarded ones not.
    pub fn notes(&self, folder: EntityId) -> Result<Vec<Note>> {
        self.folder(folder)?
            .notes
            .into_iter()
            .filter(|id| !self.is_discarded(Kind::Note, *id))
            .map(|id| self.note(id))
            .collect()
    }
    /// The note's blocks, trashed ones included, discarded ones not.
    pub fn blocks(&self, note: EntityId) -> Result<Vec<Block>> {
        self.note(note)?
            .blocks
            .into_iter()
            .filter(|id| !self.is_discarded(Kind::Block, *id))
            .map(|id| self.block(id))
            .collect()
    }
    /// The UUID of the asset an image block shows.
    pub fn image_asset(&self, block: EntityId) -> Result<Uuid> {
        let b = self.block(block)?;
        ensure!(b.kind == BlockKind::Image, "Not an image");
        let tx = Transaction::begin_read_transaction(&self.context.db_context)?;
        Ok(repo::create_asset_repository(&tx)?
            .get(&b.asset.context("Missing asset reference")?)?
            .context("Missing asset")?
            .uid)
    }
    pub fn create_folder(&self, title: String, stack: u64) -> Result<EntityId> {
        let e = transaction(&self.context, |tx, ev| {
            Ok(repo::create_folder_repository(tx)?.create(
                ev,
                &Folder {
                    uid: Uuid::new_v4(),
                    title,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                    ..Default::default()
                },
                self.library_id,
                -1,
            )?)
        })?;
        self.record_creation(stack, Kind::Folder, e.id, "create_folder")?;
        Ok(e.id)
    }
    pub fn create_note(&self, folder: EntityId, title: String, stack: u64) -> Result<EntityId> {
        ensure!(
            self.library(self.library_id)?.folders.contains(&folder)
                && !self.folder(folder)?.trashed,
            "Invalid destination folder"
        );
        let e = transaction(&self.context, |tx, ev| {
            Ok(repo::create_note_repository(tx)?.create(
                ev,
                &Note {
                    uid: Uuid::new_v4(),
                    title,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                    ..Default::default()
                },
                folder,
                -1,
            )?)
        })?;
        self.record_creation(stack, Kind::Note, e.id, "create_note")?;
        Ok(e.id)
    }
    /// The size a new block of `kind` starts at.
    pub fn default_size(kind: &BlockKind) -> (f64, f64) {
        if *kind == BlockKind::RichText {
            (360., 120.)
        } else {
            (420., 270.)
        }
    }
    pub fn create_block(
        &self,
        note: EntityId,
        kind: BlockKind,
        label: String,
        stack: u64,
    ) -> Result<EntityId> {
        let (width, height) = Self::default_size(&kind);
        let spot = self.free_spot(note, width, height)?;
        self.create_block_at(note, kind, label, spot, stack)
    }
    /// Create a block with its top-left corner at `(x, y)`.
    pub fn create_block_at(
        &self,
        note: EntityId,
        kind: BlockKind,
        label: String,
        (x, y): (f64, f64),
        stack: u64,
    ) -> Result<EntityId> {
        ensure!(!self.note(note)?.trashed, "Note is in trash");
        ensure!(kind != BlockKind::Image, "Images require an imported asset");
        let (width, height) = Self::default_size(&kind);
        let z = self.top_z(note)? + 1.;
        let now = chrono::Utc::now();
        let e = transaction(&self.context, |tx, ev| {
            Ok(repo::create_block_repository(tx)?.create(
                ev,
                &Block {
                    uid: Uuid::new_v4(),
                    label,
                    kind: kind.clone(),
                    x,
                    y,
                    width,
                    height,
                    z,
                    sizing: if kind == BlockKind::RichText {
                        BlockSizing::ContentHeight
                    } else {
                        BlockSizing::FixedSize
                    },
                    content_version: 1,
                    created_at: now,
                    updated_at: now,
                    ..Default::default()
                },
                note,
                -1,
            )?)
        })?;
        self.record_creation(stack, Kind::Block, e.id, "create_block")?;
        Ok(e.id)
    }
    pub fn insert_image(
        &self,
        note: EntityId,
        asset: &format::Asset,
        width: f64,
        height: f64,
        stack: u64,
    ) -> Result<EntityId> {
        ensure!(!self.note(note)?.trashed, "Note is in trash");
        let width_on_page = width.clamp(80., 480.);
        let height_on_page = (height * width_on_page / width).max(40.);
        let (x, y) = self.free_spot(note, width_on_page, height_on_page)?;
        let geometry = format::Geometry {
            x,
            y,
            width: width_on_page,
            height: height_on_page,
            z: self.top_z(note)? + 1.,
        };
        geometry.validate()?;
        let e = transaction(&self.context, |tx, ev| {
            let now = chrono::Utc::now();
            let a = repo::create_asset_repository(tx)?.create(
                ev,
                &Asset {
                    uid: asset.uid,
                    digest: asset.digest.clone(),
                    mime_type: asset.mime.clone(),
                    original_name: asset.name.clone(),
                    byte_length: asset.bytes.len() as u64,
                    created_at: now,
                    updated_at: now,
                    ..Default::default()
                },
                self.library_id,
                -1,
            )?;
            Ok(repo::create_block_repository(tx)?.create(
                ev,
                &Block {
                    uid: Uuid::new_v4(),
                    label: asset.name.clone(),
                    kind: BlockKind::Image,
                    asset: Some(a.id),
                    alt_text: asset.name.clone(),
                    sizing: BlockSizing::FixedAspect,
                    x: geometry.x,
                    y: geometry.y,
                    width: geometry.width,
                    height: geometry.height,
                    z: geometry.z,
                    content_version: 1,
                    created_at: now,
                    updated_at: now,
                    ..Default::default()
                },
                note,
                -1,
            )?)
        })?;
        self.record_creation(stack, Kind::Block, e.id, "insert_image")?;
        Ok(e.id)
    }
    pub fn flush_text(&self, id: EntityId, djot: String) -> Result<bool> {
        let mut e = self.block(id)?;
        ensure!(e.kind == BlockKind::RichText, "Not a text block");
        if e.content_djot == djot {
            return Ok(false);
        }
        ensure!(djot.len() <= 32 * 1024 * 1024, "Text exceeds 32 MiB");
        e.content_djot = djot;
        e.updated_at = chrono::Utc::now();
        transaction(&self.context, |tx, ev| {
            repo::create_block_repository(tx)?.update(ev, &e)?;
            Ok(())
        })?;
        Ok(true)
    }
    pub fn geometries(&self, values: &[(EntityId, format::Geometry)], stack: u64) -> Result<()> {
        let mut before = vec![];
        let mut after = vec![];
        for (id, value) in values {
            value.validate()?;
            let b = self.block(*id)?;
            let old = format::Geometry {
                x: b.x,
                y: b.y,
                width: b.width,
                height: b.height,
                z: b.z,
            };
            if old != *value {
                before.push(Patch::Geometry(*id, old));
                after.push(Patch::Geometry(*id, *value));
            }
        }
        if after.is_empty() {
            return Ok(());
        }
        self.edit(stack, before, after, "transform_blocks")
    }
    pub fn geometry(&self, id: EntityId, value: format::Geometry, stack: u64) -> Result<()> {
        let e = self.block(id)?;
        value.validate()?;
        let before = format::Geometry {
            x: e.x,
            y: e.y,
            width: e.width,
            height: e.height,
            z: e.z,
        };
        if before == value {
            return Ok(());
        }
        self.edit(
            stack,
            vec![Patch::Geometry(id, before)],
            vec![Patch::Geometry(id, value)],
            "move_block",
        )
    }
    pub fn rename_folder(&self, id: EntityId, value: String, stack: u64) -> Result<()> {
        let e = self.folder(id)?;
        ensure!(!value.trim().is_empty(), "A title cannot be empty");
        ensure!(value.len() <= 4096, "Title too long");
        self.edit(
            stack,
            vec![Patch::FolderTitle(id, e.title)],
            vec![Patch::FolderTitle(id, value)],
            "rename_folder",
        )
    }
    pub fn trash_folder(&self, id: EntityId, value: bool, stack: u64) -> Result<()> {
        let e = self.folder(id)?;
        self.edit(
            stack,
            vec![Patch::FolderTrash(id, e.trashed)],
            vec![Patch::FolderTrash(id, value)],
            "trash_folder",
        )
    }
    pub fn rename_note(&self, id: EntityId, value: String, stack: u64) -> Result<()> {
        let e = self.note(id)?;
        ensure!(!value.trim().is_empty(), "A title cannot be empty");
        ensure!(value.len() <= 4096, "Title too long");
        self.edit(
            stack,
            vec![Patch::NoteTitle(id, e.title)],
            vec![Patch::NoteTitle(id, value)],
            "rename_note",
        )
    }
    pub fn trash_note(&self, id: EntityId, value: bool, stack: u64) -> Result<()> {
        let e = self.note(id)?;
        self.edit(
            stack,
            vec![Patch::NoteTrash(id, e.trashed)],
            vec![Patch::NoteTrash(id, value)],
            "trash_note",
        )
    }
    pub fn rename_block(&self, id: EntityId, value: String, stack: u64) -> Result<()> {
        let e = self.block(id)?;
        ensure!(value.len() <= 4096, "Title too long");
        self.edit(
            stack,
            vec![Patch::BlockLabel(id, e.label)],
            vec![Patch::BlockLabel(id, value)],
            "rename_block",
        )
    }
    pub fn trash_block(&self, id: EntityId, value: bool, stack: u64) -> Result<()> {
        let e = self.block(id)?;
        self.edit(
            stack,
            vec![Patch::BlockTrash(id, e.trashed)],
            vec![Patch::BlockTrash(id, value)],
            "trash_block",
        )
    }
    pub fn describe_sketch(&self, id: EntityId, value: String, stack: u64) -> Result<()> {
        let e = self.block(id)?;
        ensure!(e.kind == BlockKind::Drawing, "Not a sketch");
        self.edit(
            stack,
            vec![Patch::Description(id, e.description)],
            vec![Patch::Description(id, value)],
            "describe_sketch",
        )
    }
    pub fn move_note(&self, id: EntityId, destination: EntityId, stack: u64) -> Result<()> {
        let folders = self.folders()?;
        let source = folders
            .iter()
            .find(|f| f.notes.contains(&id))
            .context("Note has no owner")?;
        let dest = folders
            .iter()
            .find(|f| f.id == destination && !f.trashed)
            .context("Invalid destination")?;
        if source.id == dest.id {
            return Ok(());
        }
        let mut a = source.notes.clone();
        a.retain(|n| *n != id);
        let mut b = dest.notes.clone();
        b.push(id);
        self.edit(
            stack,
            vec![
                Patch::FolderNotes(source.id, source.notes.clone()),
                Patch::FolderNotes(dest.id, dest.notes.clone()),
            ],
            vec![
                Patch::FolderNotes(source.id, a),
                Patch::FolderNotes(dest.id, b),
            ],
            "move_note",
        )
    }
    /// Move a folder to `index` in the library's folder order.
    pub fn reorder_folder(&self, id: EntityId, index: usize, stack: u64) -> Result<()> {
        let l = self.library(self.library_id)?;
        ensure!(l.folders.contains(&id), "Folder is not in library");
        let mut ids = l.folders.clone();
        ids.retain(|v| *v != id);
        ids.insert(index.min(ids.len()), id);
        if ids == l.folders {
            return Ok(());
        }
        self.edit(
            stack,
            vec![Patch::LibraryFolders(self.library_id, l.folders)],
            vec![Patch::LibraryFolders(self.library_id, ids)],
            "reorder_folder",
        )
    }
    /// Move a note to `index` in its folder's note order.
    pub fn reorder_note(
        &self,
        folder: EntityId,
        id: EntityId,
        index: usize,
        stack: u64,
    ) -> Result<()> {
        let f = self.folder(folder)?;
        ensure!(f.notes.contains(&id), "Note is not in folder");
        let mut ids = f.notes.clone();
        ids.retain(|v| *v != id);
        ids.insert(index.min(ids.len()), id);
        if ids == f.notes {
            return Ok(());
        }
        self.edit(
            stack,
            vec![Patch::FolderNotes(folder, f.notes)],
            vec![Patch::FolderNotes(folder, ids)],
            "reorder_note",
        )
    }
    pub fn reorder_block(
        &self,
        note: EntityId,
        id: EntityId,
        index: usize,
        stack: u64,
    ) -> Result<()> {
        let n = self.note(note)?;
        ensure!(n.blocks.contains(&id), "Block is not in note");
        let mut ids = n.blocks.clone();
        ids.retain(|v| *v != id);
        ids.insert(index.min(ids.len()), id);
        if ids == n.blocks {
            return Ok(());
        }
        self.edit(
            stack,
            vec![Patch::NoteBlocks(note, n.blocks)],
            vec![Patch::NoteBlocks(note, ids)],
            "reorder_block",
        )
    }
    pub fn add_stroke(
        &self,
        block: EntityId,
        stroke: format::Stroke,
        stack: u64,
    ) -> Result<EntityId> {
        stroke.validate()?;
        let b = self.block(block)?;
        ensure!(
            b.kind == BlockKind::Drawing && !b.trashed,
            "Not an active sketch"
        );
        let now = chrono::Utc::now();
        let e = transaction(&self.context, |tx, ev| {
            Ok(repo::create_ink_stroke_repository(tx)?.create(
                ev,
                &InkStroke {
                    uid: stroke.uid,
                    trashed: false,
                    sample_format_version: 1,
                    samples_json: serde_json::to_string(&stroke.samples)?,
                    brush_version: 1,
                    brush_width: stroke.width,
                    brush_rgba: stroke.rgba as u64,
                    use_pressure: stroke.pressure,
                    created_at: now,
                    updated_at: now,
                    ..Default::default()
                },
                block,
                -1,
            )?)
        })?;
        self.record_creation(stack, Kind::Stroke, e.id, "draw_stroke")?;
        Ok(e.id)
    }
    pub fn erase_strokes(&self, block: EntityId, ids: &[EntityId], stack: u64) -> Result<()> {
        let b = self.block(block)?;
        ensure!(
            ids.iter().all(|id| b.strokes.contains(id)),
            "Stroke outside sketch"
        );
        let before = ids
            .iter()
            .map(|id| Ok(Patch::StrokeTrash(*id, self.ink_stroke(*id)?.trashed)))
            .collect::<Result<_>>()?;
        self.edit(
            stack,
            before,
            ids.iter().map(|id| Patch::StrokeTrash(*id, true)).collect(),
            "erase_strokes",
        )
    }
    /// The top-left corner of the first free spot for a `width` × `height`
    /// block: 24 px from its neighbours, never overlapping one, filling rows
    /// left to right before starting a new row below.
    pub fn free_spot(&self, note: EntityId, width: f64, height: f64) -> Result<(f64, f64)> {
        let taken: Vec<_> = self
            .blocks(note)?
            .into_iter()
            .filter(|b| !b.trashed)
            .map(|b| (b.x, b.y, b.width, b.height))
            .collect();
        Ok(Self::free_spot_among(&taken, width, height, 1200.))
    }
    /// [`free_spot`](Self::free_spot) among explicit rectangles `(x, y, w, h)`,
    /// for callers that know the laid-out size of blocks whose height follows
    /// their content.
    /// Rows wrap before `row_limit`, the visible width of the canvas.
    pub fn free_spot_among(
        taken: &[(f64, f64, f64, f64)],
        width: f64,
        height: f64,
        row_limit: f64,
    ) -> (f64, f64) {
        const GAP: f64 = 24.;
        const ORIGIN: (f64, f64) = (32., 28.);
        let row_limit = row_limit.max(ORIGIN.0 + width);
        let free = |x: f64, y: f64| {
            taken.iter().all(|&(bx, by, bw, bh)| {
                x + width + GAP <= bx
                    || bx + bw + GAP <= x
                    || y + height + GAP <= by
                    || by + bh + GAP <= y
            })
        };
        let mut candidates = vec![ORIGIN];
        for &(bx, by, bw, bh) in taken {
            candidates.push((bx + bw + GAP, by));
            candidates.push((bx, by + bh + GAP));
            candidates.push((ORIGIN.0, by + bh + GAP));
        }
        candidates.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)));
        candidates
            .iter()
            .copied()
            .filter(|&(x, y)| x >= 0. && y >= 0.)
            .find(|&(x, y)| x + width <= row_limit && free(x, y))
            .or_else(|| candidates.iter().copied().find(|&(x, y)| free(x, y)))
            .unwrap_or(ORIGIN)
    }
    fn top_z(&self, note: EntityId) -> Result<f64> {
        Ok(self.blocks(note)?.iter().map(|b| b.z).fold(0., f64::max))
    }
    /// Raise a block above every other block of its note, or lower it below them.
    pub fn arrange_block(&self, id: EntityId, front: bool, stack: u64) -> Result<()> {
        let b = self.block(id)?;
        let note = self.note_of(id)?;
        let others: Vec<f64> = self
            .blocks(note)?
            .iter()
            .filter(|o| o.id != id && !o.trashed)
            .map(|o| o.z)
            .collect();
        // Already above (or below) every other block: nothing to arrange.
        let arranged = if front {
            others.iter().all(|z| *z < b.z)
        } else {
            others.iter().all(|z| *z > b.z)
        };
        if arranged {
            return Ok(());
        }
        let z = if front {
            others.iter().copied().fold(b.z, f64::max) + 1.
        } else {
            others.iter().copied().fold(b.z, f64::min) - 1.
        };
        let old = format::Geometry {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
            z: b.z,
        };
        if old.z == z {
            return Ok(());
        }
        self.edit(
            stack,
            vec![Patch::Geometry(id, old)],
            vec![Patch::Geometry(id, format::Geometry { z, ..old })],
            if front {
                "bring_to_front"
            } else {
                "send_to_back"
            },
        )
    }
    fn note_of(&self, block: EntityId) -> Result<EntityId> {
        for f in self.folders()? {
            for n in self.notes(f.id)? {
                if n.blocks.contains(&block) {
                    return Ok(n.id);
                }
            }
        }
        anyhow::bail!("Block has no note")
    }
    /// Copy a block, its content and its strokes into free space of the same
    /// note. One undo step removes the copy.
    pub fn duplicate_block(&self, id: EntityId, stack: u64) -> Result<EntityId> {
        let b = self.block(id)?;
        ensure!(!b.trashed, "Block is in trash");
        let note = self.note_of(id)?;
        let (x, y) = self.free_spot(note, b.width, b.height)?;
        let z = self.top_z(note)? + 1.;
        let strokes = b
            .strokes
            .iter()
            .map(|s| self.ink_stroke(*s))
            .collect::<Result<Vec<_>>>()?;
        let now = chrono::Utc::now();
        let e = transaction(&self.context, |tx, ev| {
            let copy = repo::create_block_repository(tx)?.create(
                ev,
                &Block {
                    id: 0,
                    uid: Uuid::new_v4(),
                    x,
                    y,
                    z,
                    strokes: vec![],
                    created_at: now,
                    updated_at: now,
                    ..b.clone()
                },
                note,
                -1,
            )?;
            for s in strokes.iter().filter(|s| !s.trashed) {
                repo::create_ink_stroke_repository(tx)?.create(
                    ev,
                    &InkStroke {
                        id: 0,
                        uid: Uuid::new_v4(),
                        created_at: now,
                        updated_at: now,
                        ..s.clone()
                    },
                    copy.id,
                    -1,
                )?;
            }
            Ok(copy)
        })?;
        self.record_creation(stack, Kind::Block, e.id, "duplicate_block")?;
        Ok(e.id)
    }
    pub fn set_alt_text(&self, id: EntityId, value: String, stack: u64) -> Result<()> {
        let decorative = self.block(id)?.decorative;
        self.describe_image(id, value, decorative, stack)
    }
    /// Set an image's alt text and whether it is decorative, in one undo step.
    /// A decorative image is skipped by screen readers and exported with an
    /// empty `alt`.
    pub fn describe_image(
        &self,
        id: EntityId,
        alt: String,
        decorative: bool,
        stack: u64,
    ) -> Result<()> {
        let e = self.block(id)?;
        ensure!(e.kind == BlockKind::Image, "Not an image");
        ensure!(alt.len() <= 4096, "Description too long");
        let (mut before, mut after) = (vec![], vec![]);
        if e.alt_text != alt {
            before.push(Patch::AltText(id, e.alt_text));
            after.push(Patch::AltText(id, alt));
        }
        if e.decorative != decorative {
            before.push(Patch::Decorative(id, e.decorative));
            after.push(Patch::Decorative(id, decorative));
        }
        if after.is_empty() {
            return Ok(());
        }
        self.edit(stack, before, after, "edit_alt_text")
    }
    /// The library as it is saved: discarded entities are left out, and so are
    /// assets no saved image uses.
    pub fn snapshot(&self, assets: Vec<format::Asset>) -> Result<format::Library> {
        let tx = Transaction::begin_frozen_read_transaction(&self.context.db_context)?;
        let l = repo::create_library_repository(&tx)?
            .get(&self.library_id)?
            .context("Missing library")?;
        let mut result = format::Library {
            uid: l.uid,
            title: l.title,
            folders: vec![],
            assets: vec![],
        };
        let mut used = HashSet::new();
        for fid in l.folders {
            if self.is_discarded(Kind::Folder, fid) {
                continue;
            }
            let f = repo::create_folder_repository(&tx)?
                .get(&fid)?
                .context("Missing folder")?;
            let mut folder = format::Folder {
                uid: f.uid,
                title: f.title,
                trashed: f.trashed,
                notes: vec![],
            };
            for nid in f.notes {
                if self.is_discarded(Kind::Note, nid) {
                    continue;
                }
                let n = repo::create_note_repository(&tx)?
                    .get(&nid)?
                    .context("Missing note")?;
                let mut note = format::Note {
                    uid: n.uid,
                    title: n.title,
                    trashed: n.trashed,
                    blocks: vec![],
                };
                for bid in n.blocks {
                    if self.is_discarded(Kind::Block, bid) {
                        continue;
                    }
                    let b = repo::create_block_repository(&tx)?
                        .get(&bid)?
                        .context("Missing block")?;
                    let content = match b.kind {
                        BlockKind::RichText => format::Content::RichText {
                            version: b.content_version.try_into()?,
                            djot: b.content_djot,
                        },
                        BlockKind::Image => {
                            let a = repo::create_asset_repository(&tx)?
                                .get(&b.asset.context("Missing asset reference")?)?
                                .context("Missing asset")?;
                            used.insert(a.uid);
                            format::Content::Image {
                                asset: a.uid,
                                alt: b.alt_text,
                                decorative: b.decorative,
                            }
                        }
                        BlockKind::Drawing => {
                            let mut strokes = vec![];
                            for sid in b.strokes {
                                if self.is_discarded(Kind::Stroke, sid) {
                                    continue;
                                }
                                let s = repo::create_ink_stroke_repository(&tx)?
                                    .get(&sid)?
                                    .context("Missing stroke")?;
                                strokes.push(format::Stroke {
                                    uid: s.uid,
                                    trashed: s.trashed,
                                    version: s.sample_format_version.try_into()?,
                                    width: s.brush_width,
                                    rgba: s.brush_rgba.try_into()?,
                                    pressure: s.use_pressure,
                                    samples: serde_json::from_str(&s.samples_json)?,
                                });
                            }
                            format::Content::Drawing {
                                description: b.description,
                                strokes,
                            }
                        }
                    };
                    note.blocks.push(format::Block {
                        uid: b.uid,
                        label: b.label,
                        trashed: b.trashed,
                        geometry: format::Geometry {
                            x: b.x,
                            y: b.y,
                            width: b.width,
                            height: b.height,
                            z: b.z,
                        },
                        content,
                    });
                }
                folder.notes.push(note);
            }
            result.folders.push(folder);
        }
        result.assets = assets
            .into_iter()
            .filter(|a| used.contains(&a.uid))
            .collect();
        result.validate()?;
        Ok(result)
    }
    pub fn load(data: &format::Library) -> Result<Self> {
        data.validate()?;
        let context = AppContext::new();
        let library_id = transaction(&context, |tx, ev| {
            let now = chrono::Utc::now();
            let root = repo::create_root_repository(tx)?.create_orphan(
                ev,
                &Root {
                    created_at: now,
                    updated_at: now,
                    ..Default::default()
                },
            )?;
            let l = repo::create_library_repository(tx)?.create(
                ev,
                &Library {
                    uid: data.uid,
                    title: data.title.clone(),
                    created_at: now,
                    updated_at: now,
                    ..Default::default()
                },
                root.id,
                -1,
            )?;
            let mut assets = HashMap::new();
            for a in &data.assets {
                let e = repo::create_asset_repository(tx)?.create(
                    ev,
                    &Asset {
                        uid: a.uid,
                        digest: a.digest.clone(),
                        mime_type: a.mime.clone(),
                        original_name: a.name.clone(),
                        byte_length: a.bytes.len() as u64,
                        created_at: now,
                        updated_at: now,
                        ..Default::default()
                    },
                    l.id,
                    -1,
                )?;
                assets.insert(a.uid, e.id);
            }
            for f in &data.folders {
                let folder = repo::create_folder_repository(tx)?.create(
                    ev,
                    &Folder {
                        uid: f.uid,
                        title: f.title.clone(),
                        trashed: f.trashed,
                        created_at: now,
                        updated_at: now,
                        ..Default::default()
                    },
                    l.id,
                    -1,
                )?;
                for n in &f.notes {
                    let note = repo::create_note_repository(tx)?.create(
                        ev,
                        &Note {
                            uid: n.uid,
                            title: n.title.clone(),
                            trashed: n.trashed,
                            created_at: now,
                            updated_at: now,
                            ..Default::default()
                        },
                        folder.id,
                        -1,
                    )?;
                    for b in &n.blocks {
                        let mut block = Block {
                            uid: b.uid,
                            label: b.label.clone(),
                            trashed: b.trashed,
                            x: b.geometry.x,
                            y: b.geometry.y,
                            width: b.geometry.width,
                            height: b.geometry.height,
                            z: b.geometry.z,
                            content_version: 1,
                            created_at: now,
                            updated_at: now,
                            ..Default::default()
                        };
                        match &b.content {
                            format::Content::RichText { version, djot } => {
                                block.kind = BlockKind::RichText;
                                block.sizing = BlockSizing::ContentHeight;
                                block.content_version = *version as u64;
                                block.content_djot = djot.clone();
                            }
                            format::Content::Image {
                                asset,
                                alt,
                                decorative,
                            } => {
                                block.kind = BlockKind::Image;
                                block.sizing = BlockSizing::FixedAspect;
                                block.asset = Some(*assets.get(asset).context("Missing asset")?);
                                block.alt_text = alt.clone();
                                block.decorative = *decorative;
                            }
                            format::Content::Drawing { description, .. } => {
                                block.kind = BlockKind::Drawing;
                                block.sizing = BlockSizing::FixedSize;
                                block.description = description.clone();
                            }
                        }
                        let block =
                            repo::create_block_repository(tx)?.create(ev, &block, note.id, -1)?;
                        if let format::Content::Drawing { strokes, .. } = &b.content {
                            for s in strokes {
                                repo::create_ink_stroke_repository(tx)?.create(
                                    ev,
                                    &InkStroke {
                                        uid: s.uid,
                                        trashed: s.trashed,
                                        sample_format_version: s.version as u64,
                                        samples_json: serde_json::to_string(&s.samples)?,
                                        brush_version: 1,
                                        brush_width: s.width,
                                        brush_rgba: s.rgba as u64,
                                        use_pressure: s.pressure,
                                        created_at: now,
                                        updated_at: now,
                                        ..Default::default()
                                    },
                                    block.id,
                                    -1,
                                )?;
                            }
                        }
                    }
                }
            }
            Ok(l.id)
        })?;
        Ok(Self {
            context,
            library_id,
            discarded: Discarded::default(),
        })
    }
}

pub mod save;
