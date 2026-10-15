use anyhow::{Context, Result};
use common::{
    entities::{Block, BlockKind},
    types::EntityId,
};
use noton_application::{Kind, LibraryService, save::SaveWorker};
use noton_format::{Asset, LibraryFile};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};
use teksilo::{canvas::RasterIcon, data::ListModel, prelude::*, widgets::rich_text::EditorHandle};
use text_document::TextDocument;
use uuid::Uuid;
struct DocumentChanges {
    revision: Arc<AtomicU64>,
    _subscription: text_document::Subscription,
}
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
/// The first line of note text worth showing in a list, without markup.
fn snippet(text: &str) -> Option<String> {
    let line = text
        .lines()
        .map(|l| {
            l.trim_start_matches(|c: char| "#>-*+|:0123456789.)".contains(c) || c.is_whitespace())
                .replace(['*', '_', '|', '`'], " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .find(|l| !l.is_empty())?;
    Some(if line.chars().count() > 80 {
        line.chars().take(79).chain(['…']).collect()
    } else {
        line
    })
}
fn search_key(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}
struct SearchEntry {
    stamp: (chrono::DateTime<chrono::Utc>, Option<u64>),
    text: String,
}
type SearchOrigin = (Option<EntityId>, Option<EntityId>, bool, usize, bool);
type BlockFocus = (EntityId, Option<(usize, usize)>);
/// Whether `name` is a backup of the library `stem` as [`SessionVm::backup`]
/// names them: `{stem}.{YYYYMMDD}T{HHMMSS}.{mmm}.noton`, and nothing else.
fn is_backup_of(name: &str, stem: &str) -> bool {
    let Some(stamp) = name
        .strip_prefix(stem)
        .and_then(|rest| rest.strip_prefix('.'))
        .and_then(|rest| rest.strip_suffix(".noton"))
    else {
        return false;
    };
    stamp.len() == 19
        && stamp.bytes().enumerate().all(|(i, c)| match i {
            8 => c == b'T',
            15 => c == b'.',
            _ => c.is_ascii_digit(),
        })
}
/// Decoded images are kept at most this many pixels on a side.
const MAX_IMAGE_EDGE: u32 = 2048;
fn display_image(image: RasterIcon) -> RasterIcon {
    image.downsample_to_max(MAX_IMAGE_EDGE).unwrap_or(image)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: EntityId,
    pub title: String,
    pub trashed: bool,
    /// Second line: a note's first words, its folder while searching, or "In trash".
    pub subtitle: String,
    /// Trailing meta: a folder's note count, a note's block count.
    pub meta: String,
    /// The note holds a sketch.
    pub sketch: bool,
    pub hit: Option<crate::search::SearchHit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveState {
    Saved,
    Unsaved,
    Saving,
    Failed,
}

/// What each undo scope would take back next, as translated phrases
/// ("move block"). `None` when that scope has nothing to undo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UndoInfo {
    /// The focused scope is a text block; carries its label.
    pub typing_in: Option<String>,
    pub page_undo: Option<String>,
    pub page_redo: Option<String>,
    pub library_undo: Option<String>,
    pub library_redo: Option<String>,
    /// The focused scope is the folder and note lists.
    pub library_focused: bool,
}

/// Formatting at the caret of the text block being edited, for the pressed
/// state of the formatting toolbar.
#[derive(Clone, Debug)]
pub struct FormatState {
    pub bold: Signal<bool>,
    pub italic: Signal<bool>,
    pub underline: Signal<bool>,
    pub strike: Signal<bool>,
    pub link: Signal<bool>,
    /// The caret is in a table: row and column commands apply.
    pub in_table: Signal<bool>,
    /// 0 = paragraph, 1–3 = heading level.
    pub heading: Signal<Option<u8>>,
}
impl Default for FormatState {
    fn default() -> Self {
        Self {
            bold: Signal::new(false),
            italic: Signal::new(false),
            underline: Signal::new(false),
            strike: Signal::new(false),
            link: Signal::new(false),
            in_table: Signal::new(false),
            heading: Signal::new(Some(0)),
        }
    }
}

/// The live canvas camera. Rebuilt with each canvas; the values survive.
#[derive(Clone, Debug)]
pub struct Camera {
    pub zoom: Signal<f32>,
    pub pan_x: Signal<f32>,
    pub pan_y: Signal<f32>,
    pub viewport: Signal<Size>,
}
pub const MIN_ZOOM: f32 = 0.25;
pub const MAX_ZOOM: f32 = 4.;
#[derive(Clone)]
pub struct SessionVm(pub Rc<Inner>);
pub struct Inner {
    pub(crate) wake: crate::coordinator::CoordinatorWake,
    deadline: Cell<Option<Instant>>,
    tree_wake: RefCell<Option<Rc<Cell<Option<Instant>>>>>,
    fit_at: Cell<Option<Instant>>,
    camera_observer: RefCell<Option<teksilo::core::signal::ObserverHandle>>,
    history_observers: RefCell<Vec<frontend::event_hub_client::SubscriptionToken>>,
    pub service: LibraryService,
    pub edit: crate::edit_vm::EditVm,
    pub folder: Signal<Option<EntityId>>,
    pub folder_trashed: Signal<bool>,
    pub note_trashed: Signal<bool>,
    pub navigation: Signal<usize>,
    pub navigation_visible: Signal<bool>,
    search_origin: Cell<Option<SearchOrigin>>,
    pub focus_block: Cell<Option<BlockFocus>>,
    pub focus_frame: Cell<Option<EntityId>>,
    pub note: Signal<Option<EntityId>>,
    pub folders: ListModel<Row>,
    pub notes: ListModel<Row>,
    pub title: Signal<String>,
    pub query: Signal<String>,
    pub reading: Signal<bool>,
    pub trash: Signal<bool>,
    pub revision: Signal<u64>,
    /// Bumped by every refresh of the lists.
    pub folders_version: Signal<u64>,
    pub status: Signal<String>,
    pub error: Signal<String>,
    pub documents: RefCell<HashMap<EntityId, TextDocument>>,
    flushed_revisions: RefCell<HashMap<EntityId, u64>>,
    document_changes: RefCell<HashMap<EntityId, DocumentChanges>>,
    pub handles: RefCell<HashMap<EntityId, EditorHandle>>,
    pub pending: RefCell<HashSet<EntityId>>,
    pub focus_observers: RefCell<HashMap<EntityId, teksilo::core::signal::ObserverHandle>>,
    pub active_text: Cell<Option<EntityId>>,
    pub active_block: Cell<Option<EntityId>>,
    pub organization: u64,
    pub page_stacks: RefCell<HashMap<EntityId, u64>>,
    pub assets: RefCell<Vec<Asset>>,
    /// Decoded images of the page on screen, by asset.
    images: RefCell<HashMap<Uuid, Rc<RasterIcon>>>,
    search_index: RefCell<HashMap<EntityId, SearchEntry>>,
    /// The user chose to close the window without saving.
    close_anyway: Cell<bool>,
    pub worker: Option<SaveWorker>,
    pub path: PathBuf,
    pub prefs: RefCell<crate::settings::Preferences>,
    /// The text block whose formatting toolbar is showing.
    pub editing: Signal<Option<EntityId>>,
    /// The sketch in drawing mode.
    pub drawing: Signal<Option<EntityId>>,
    /// The block selected on the canvas.
    pub selected: Signal<Option<EntityId>>,
    pub save_state: Signal<SaveState>,
    pub saved_at: Signal<String>,
    pub undo_info: Signal<UndoInfo>,
    pub camera: RefCell<Option<Camera>>,
    /// The canvas zoom, mirrored for the status bar.
    pub zoom: Signal<f32>,
    /// Camera values carried across canvas rebuilds of the same note.
    pub kept_camera: Cell<Option<(EntityId, f32, f32, f32)>>,
    /// The note title is being edited in the note header.
    pub renaming_note: Signal<bool>,
    pub renaming_folder: Signal<Option<EntityId>>,
    /// The inline rename's text, kept across the row's rebuilds.
    pub folder_rename: RefCell<Option<(EntityId, Signal<String>)>>,
    /// The inline rename field has had its one focus request.
    pub folder_rename_focused: Rc<Cell<bool>>,
    /// The window is narrower than the three-pane layout.
    pub narrow: Signal<bool>,
    /// The narrow-window offer to read the note is showing (not dismissed).
    pub narrow_hint: Signal<bool>,
    /// A trash that can still be undone from the snackbar: (stack, command, message).
    pub undo_toast: Signal<Option<(u64, u64, String)>>,
    pub search_field: Cell<Option<teksilo::core::WidgetId>>,
    /// The block the next "block.position" / "block.describe" dialog is for.
    pub dialog_block: Cell<Option<EntityId>>,
    pub format: FormatState,
    pub ink: crate::page::ink::InkTools,
    /// The caret moved or the format changed; the coordinator syncs after the mutation.
    pub format_dirty: Cell<bool>,
    /// The canvas on screen, when the note is shown as a canvas.
    pub scene: RefCell<Option<crate::page::LiveScene>>,
    /// Zoom this note's canvas to fit once it is on screen (narrow windows).
    pub fit_pending: Cell<Option<EntityId>>,
    /// Wait for a usable viewport, then fit after its layout changes settle.
    fit_waiting: Cell<bool>,
    /// Caret observers of each live editor, replaced when it is rebuilt.
    pub format_observers: RefCell<HashMap<EntityId, Vec<teksilo::core::signal::ObserverHandle>>>,
    /// Observers that live as long as the window: dropping one unsubscribes it.
    pub observers: RefCell<Vec<teksilo::core::signal::ObserverHandle>>,
    save_failed: Cell<bool>,
    seq: Cell<u64>,
    saved: Cell<u64>,
    inflight: Cell<Option<u64>>,
    last_edit: Cell<Instant>,
    first_edit: Cell<Option<Instant>>,
}
impl std::fmt::Debug for SessionVm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionVm").finish_non_exhaustive()
    }
}
impl SessionVm {
    pub fn open(path: PathBuf) -> Result<Self> {
        let exists = path.exists();
        let mut file = LibraryFile::open(&path, true)?;
        let data = if exists {
            file.load()?
        } else {
            noton_format::Library::empty("Noton".into())
        };
        if !exists {
            file.save(&data)?;
        }
        let service = LibraryService::load(&data)?;
        Self::from_service(service, data.assets, Some(SaveWorker::start(file)), path)
    }
    pub fn memory() -> Result<Self> {
        Self::from_service(
            LibraryService::new("Noton".into())?,
            vec![],
            None,
            PathBuf::new(),
        )
    }
    fn from_service(
        service: LibraryService,
        assets: Vec<Asset>,
        worker: Option<SaveWorker>,
        path: PathBuf,
    ) -> Result<Self> {
        let organization = service.new_stack();
        let prefs = crate::settings::load_or_default();
        let now = Instant::now();
        let wake = crate::coordinator::CoordinatorWake::default();
        if let Some(worker) = &worker {
            let notify = wake.clone();
            worker.set_completion_wake(Arc::new(move || notify.notify()));
        }
        let vm = Self(Rc::new(Inner {
            wake,
            deadline: Cell::new(None),
            tree_wake: RefCell::new(None),
            fit_at: Cell::new(None),
            camera_observer: RefCell::new(None),
            history_observers: RefCell::new(Vec::new()),
            service,
            edit: crate::edit_vm::EditVm::default(),
            folder: Signal::new(None),
            folder_trashed: Signal::new(false),
            note_trashed: Signal::new(false),
            navigation: Signal::new(0),
            navigation_visible: Signal::new(true),
            search_origin: Cell::new(None),
            focus_block: Cell::new(None),
            focus_frame: Cell::new(None),
            note: Signal::new(None),
            folders: ListModel::from_vec(vec![]),
            notes: ListModel::from_vec(vec![]),
            title: Signal::new(String::new()),
            query: Signal::new(String::new()),
            reading: Signal::new(false),
            trash: Signal::new(false),
            revision: Signal::new(0),
            folders_version: Signal::new(0),
            status: Signal::new(tr!(saved()).resolve_now()),
            error: Signal::new(String::new()),
            documents: RefCell::new(HashMap::new()),
            flushed_revisions: RefCell::new(HashMap::new()),
            document_changes: RefCell::new(HashMap::new()),
            handles: RefCell::new(HashMap::new()),
            pending: RefCell::new(HashSet::new()),
            focus_observers: RefCell::new(HashMap::new()),
            active_text: Cell::new(None),
            active_block: Cell::new(None),
            organization,
            page_stacks: RefCell::new(HashMap::new()),
            assets: RefCell::new(assets),
            images: RefCell::new(HashMap::new()),
            search_index: RefCell::new(HashMap::new()),
            close_anyway: Cell::new(false),
            worker,
            path,
            narrow_hint: Signal::new(!prefs.narrow_hint_dismissed),
            prefs: RefCell::new(prefs),
            editing: Signal::new(None),
            drawing: Signal::new(None),
            selected: Signal::new(None),
            save_state: Signal::new(SaveState::Saved),
            saved_at: Signal::new(String::new()),
            undo_info: Signal::new(UndoInfo::default()),
            camera: RefCell::new(None),
            zoom: Signal::new(1.),
            kept_camera: Cell::new(None),
            renaming_note: Signal::new(false),
            renaming_folder: Signal::new(None),
            folder_rename: RefCell::new(None),
            folder_rename_focused: Rc::new(Cell::new(false)),
            narrow: Signal::new(false),
            undo_toast: Signal::new(None),
            search_field: Cell::new(None),
            dialog_block: Cell::new(None),
            observers: RefCell::new(vec![]),
            format: FormatState::default(),
            ink: Default::default(),
            format_dirty: Cell::new(false),
            scene: RefCell::new(None),
            fit_pending: Cell::new(None),
            fit_waiting: Cell::new(false),
            format_observers: RefCell::new(HashMap::new()),
            save_failed: Cell::new(false),
            seq: Cell::new(0),
            saved: Cell::new(0),
            inflight: Cell::new(None),
            last_edit: Cell::new(now),
            first_edit: Cell::new(None),
        }));
        vm.refresh()?;
        Ok(vm)
    }
    pub fn report(&self, result: Result<()>) {
        if let Err(e) = result {
            self.0
                .error
                .set(format!("{}: {e:#}", tr!(error()).resolve_now()));
        }
    }
    pub fn changed(&self) {
        self.0.wake.notify();
        self.0.seq.set(self.0.seq.get() + 1);
        self.0.last_edit.set(Instant::now());
        if self.0.first_edit.get().is_none() {
            self.0.first_edit.set(Some(Instant::now()));
        }
        self.0.status.set_if_changed(tr!(unsaved()).resolve_now());
        if !self.0.save_failed.get() {
            self.0.save_state.set_if_changed(SaveState::Unsaved);
        }
    }
    /// Show that the library could not be saved; autosave waits for a retry.
    fn mark_save_failed(&self) {
        self.0.save_failed.set(true);
        self.0.save_state.set_if_changed(SaveState::Failed);
        self.0
            .status
            .set_if_changed(tr!(save_failed()).resolve_now());
    }
    /// Make the page's structure (not a text block) what Ctrl+Z undoes: the
    /// user has just acted on blocks.
    pub fn claim_page(&self) {
        self.0.edit.claim_structure(false);
    }
    pub fn rebuild_page(&self) {
        self.keep_camera();
        self.0.revision.set(self.0.revision.get() + 1);
    }
    pub fn page_stack(&self) -> Result<u64> {
        let note = self.0.note.get().context("No selected note")?;
        Ok(*self
            .0
            .page_stacks
            .borrow_mut()
            .entry(note)
            .or_insert_with(|| self.0.service.new_stack()))
    }
    pub fn refresh(&self) -> Result<()> {
        let show_trash = self.0.trash.get();
        let all = self.0.service.folders()?;
        let mut folders = vec![];
        for f in all.iter().filter(|f| show_trash || !f.trashed) {
            let live = self
                .0
                .service
                .notes(f.id)?
                .iter()
                .filter(|n| show_trash || !n.trashed)
                .count();
            folders.push(Row {
                id: f.id,
                title: f.title.clone(),
                trashed: f.trashed,
                subtitle: if f.trashed {
                    tr!(in_trash()).resolve_now()
                } else {
                    String::new()
                },
                meta: live.to_string(),
                sketch: false,
                hit: None,
            });
        }
        if !folders.iter().any(|f| Some(f.id) == self.0.folder.get()) {
            self.0.folder.set(folders.first().map(|r| r.id));
        }
        self.0.folders.replace_all(folders);
        let query = search_key(&self.0.query.get());
        let mut notes = vec![];
        for f in all.iter().filter(|f| show_trash || !f.trashed) {
            if query.is_empty() && Some(f.id) != self.0.folder.get() {
                continue;
            }
            for n in self.0.service.notes(f.id)? {
                if !(show_trash || !n.trashed) {
                    continue;
                }
                let blocks: Vec<_> = self
                    .0
                    .service
                    .blocks(n.id)?
                    .into_iter()
                    .filter(|b| show_trash || !b.trashed)
                    .collect();
                let mut hit = if query.is_empty() {
                    None
                } else {
                    crate::search::hit(&n.title, &query, None, false)
                };
                let mut count = hit.as_ref().map_or(0, |h| h.count);
                if !query.is_empty() {
                    for block in &blocks {
                        let text = if block.kind == BlockKind::RichText {
                            self.search_text(block)?
                        } else {
                            format!("{} {} {}", block.label, block.description, block.alt_text)
                        };
                        if let Some(found) = crate::search::hit(
                            &text,
                            &query,
                            Some(block.id),
                            block.kind == BlockKind::RichText,
                        ) {
                            count += found.count;
                            if hit.as_ref().is_none_or(|h| h.block.is_none()) {
                                hit = Some(found);
                            }
                        }
                        // Rich-text labels and descriptions remain searchable,
                        // but their offsets must never select document text.
                        if block.kind == BlockKind::RichText {
                            let metadata =
                                format!("{} {} {}", block.label, block.description, block.alt_text);
                            if let Some(found) =
                                crate::search::hit(&metadata, &query, Some(block.id), false)
                            {
                                count += found.count;
                                if hit.as_ref().is_none_or(|h| h.block.is_none()) {
                                    hit = Some(found);
                                }
                            }
                        }
                    }
                    if hit.is_none() {
                        continue;
                    }
                    if let Some(h) = &mut hit {
                        h.count = count;
                    }
                }
                let trashed = n.trashed || f.trashed;
                let subtitle = if trashed {
                    tr!(in_trash()).resolve_now()
                } else if !query.is_empty() {
                    f.title.clone()
                } else {
                    blocks
                        .iter()
                        .find_map(|b| match b.kind {
                            BlockKind::RichText => snippet(&self.block_text(b)),
                            BlockKind::Drawing if !b.description.is_empty() => {
                                snippet(&b.description)
                            }
                            _ => None,
                        })
                        .unwrap_or_default()
                };
                notes.push(Row {
                    id: n.id,
                    title: n.title,
                    trashed,
                    subtitle,
                    meta: if let Some(h) = &hit {
                        tr!(search_matches(count = h.count as i64)).resolve_now()
                    } else {
                        tr!(block_count(count = blocks.len() as i64)).resolve_now()
                    },
                    hit,
                    sketch: blocks.iter().any(|b| b.kind == BlockKind::Drawing),
                });
            }
        }
        if query.is_empty() && !notes.iter().any(|n| Some(n.id) == self.0.note.get()) {
            self.select_note(notes.first().map(|r| r.id))?;
        }
        self.0.notes.replace_all(notes);
        self.0.folder_trashed.set(
            self.0
                .folder
                .get()
                .and_then(|id| self.0.service.folder(id).ok())
                .is_some_and(|f| f.trashed),
        );
        self.0.note_trashed.set(
            self.0
                .note
                .get()
                .and_then(|id| self.0.service.note(id).ok())
                .is_some_and(|n| n.trashed),
        );
        self.0.folders_version.set(self.0.folders_version.get() + 1);
        Ok(())
    }
    /// The text of a block as a reader sees it: its live document when one
    /// is open, else what is stored.
    fn block_text(&self, b: &Block) -> String {
        self.0
            .documents
            .borrow()
            .get(&b.id)
            .and_then(|d| d.to_plain_text().ok())
            .unwrap_or_else(|| b.content_djot.clone())
    }
    fn search_text(&self, block: &Block) -> Result<String> {
        let revision = self
            .0
            .document_changes
            .borrow()
            .get(&block.id)
            .map(|c| c.revision.load(Ordering::Relaxed));
        let stamp = (block.updated_at, revision);
        if let Some(entry) = self.0.search_index.borrow().get(&block.id) {
            if entry.stamp == stamp {
                return Ok(entry.text.clone());
            }
        }
        let doc = self.0.documents.borrow().get(&block.id).cloned();
        let text = if let Some(doc) = doc {
            doc.to_plain_text()?
        } else {
            let doc = TextDocument::new();
            doc.set_djot_sync(&block.content_djot)?;
            doc.to_plain_text()?
        };
        self.0.search_index.borrow_mut().insert(
            block.id,
            SearchEntry {
                stamp,
                text: text.clone(),
            },
        );
        Ok(text)
    }
    pub fn search_changed(&self) -> Result<()> {
        if !self.0.query.get().trim().is_empty() {
            if self.0.search_origin.get().is_none() {
                self.0.search_origin.set(Some((
                    self.0.folder.get(),
                    self.0.note.get(),
                    self.0.reading.get(),
                    self.0.navigation.get(),
                    self.0.navigation_visible.get(),
                )));
            }
            self.0.navigation_visible.set(true);
            self.0.navigation.set(1);
        } else if let Some((folder, note, reading, stage, visible)) = self.0.search_origin.take() {
            self.0.folder.set(folder);
            let note = note.filter(|id| self.0.service.note(*id).is_ok_and(|n| !n.trashed));
            self.select_note(note)?;
            self.0.reading.set(reading);
            self.0.navigation.set(stage);
            self.0.navigation_visible.set(visible);
        }
        self.refresh()
    }
    pub fn activate_note(&self, id: EntityId) -> Result<()> {
        let hit = (0..self.0.notes.len()).find_map(|i| {
            self.0
                .notes
                .with_item(i, |r| (r.id == id).then(|| r.hit.clone()))
                .flatten()
                .flatten()
        });
        self.select_note(Some(id))?;
        if let Some(hit) = hit {
            if let Some(block) = hit.block {
                self.0.selected.set(Some(block));
                self.0.focus_block.set(Some((block, hit.range)));
                self.rebuild_page();
            }
        }
        Ok(())
    }
    /// One action from an empty library to a focused editor. Existing folders
    /// and notes keep their explicit creation actions for organization.
    pub fn start_note(&self) -> Result<()> {
        self.0.query.set(String::new());
        self.search_changed()?;
        self.0.trash.set(false);
        self.refresh()?;
        if self.0.folder.get().is_none() {
            self.new_folder()?;
        }
        self.new_note()?;
        self.start_writing()
    }
    pub fn write_at(&self, x: f64, y: f64) -> Result<()> {
        let note = self.0.note.get().context("Select a note")?;
        let id = self.0.service.create_block_at(
            note,
            BlockKind::RichText,
            tr!(new_text()).resolve_now(),
            (x, y),
            self.page_stack()?,
        )?;
        self.0.selected.set(Some(id));
        self.0.active_block.set(Some(id));
        self.0.focus_block.set(Some((id, None)));
        self.changed();
        self.refresh()?;
        self.rebuild_page();
        Ok(())
    }
    pub fn start_writing(&self) -> Result<()> {
        self.insert(BlockKind::RichText)?;
        self.0.focus_frame.set(None);
        if let Some(id) = self.0.active_block.get() {
            self.0.focus_block.set(Some((id, None)));
        }
        self.rebuild_page();
        Ok(())
    }
    pub fn select_folder(&self, id: EntityId) -> Result<()> {
        if self.0.renaming_folder.get().is_some_and(|r| r != id) {
            self.commit_folder_rename()?;
        }
        self.0.navigation.set(1);
        self.flush()?;
        self.0.edit.claim_structure(true);
        self.0.folder.set(Some(id));
        self.refresh()
    }
    pub fn select_note(&self, id: Option<EntityId>) -> Result<()> {
        self.0.focus_block.set(None);
        self.0.focus_frame.set(None);
        if id.is_some() {
            self.0.navigation.set(2);
        }
        self.flush()?;
        // Keep an unsubmitted title edit of the note being left.
        self.rename_note()?;
        self.0.edit.claim_structure(false);
        if self.0.note.get() != id {
            self.0.editing.set(None);
            self.0.drawing.set(None);
            self.0.selected.set(None);
            self.0.renaming_note.set(false);
            self.0.undo_toast.set(None);
            self.0
                .reading
                .set(self.0.prefs.borrow().default_view == crate::settings::DefaultView::Reading);
            self.0.fit_pending.set(id.filter(|_| self.0.narrow.get()));
        }
        self.0.note.set(id);
        self.0.active_text.set(None);
        self.0.active_block.set(None);
        self.0.title.set(match id {
            Some(id) => self.0.service.note(id)?.title,
            None => String::new(),
        });
        self.rebuild_page();
        Ok(())
    }
    pub fn new_folder(&self) -> Result<()> {
        self.commit_folder_rename()?;
        let id = self
            .0
            .service
            .create_folder(tr!(new_folder()).resolve_now(), self.0.organization)?;
        self.0.edit.claim_structure(true);
        self.0.folder.set(Some(id));
        self.changed();
        self.refresh()
    }
    pub fn new_note(&self) -> Result<()> {
        self.commit_folder_rename()?;
        let folder = self.0.folder.get().context("Create a folder first")?;
        let id = self.0.service.create_note(
            folder,
            tr!(new_note()).resolve_now(),
            self.0.organization,
        )?;
        self.changed();
        self.refresh()?;
        self.select_note(Some(id))
    }
    pub fn insert(&self, kind: BlockKind) -> Result<()> {
        let note = self.0.note.get().context("Select a note")?;
        let label = if kind == BlockKind::Drawing {
            tr!(new_sketch()).resolve_now()
        } else {
            tr!(new_text()).resolve_now()
        };
        let (w, h) = LibraryService::default_size(&kind);
        // Wrap rows at the canvas's visible width, in scene units.
        let row_limit = self
            .0
            .camera
            .borrow()
            .as_ref()
            .map(|c| {
                let v = c.viewport.get();
                ((v.width - c.pan_x.get()) / c.zoom.get().max(0.01)) as f64 - 24.
            })
            .filter(|w| *w > 200.)
            .unwrap_or(1200.);
        let spot = LibraryService::free_spot_among(&self.block_rects(note)?, w, h, row_limit);
        let id = self
            .0
            .service
            .create_block_at(note, kind, label, spot, self.page_stack()?)?;
        self.0.edit.claim_structure(false);
        self.0.active_block.set(Some(id));
        self.0.selected.set(Some(id));
        self.0.focus_frame.set(Some(id));
        self.changed();
        self.refresh()?;
        self.rebuild_page();
        Ok(())
    }
    /// Where each live block of `note` is, as laid out on the canvas when it
    /// is on screen: a text block's height follows its words, not the store.
    pub fn block_rects(&self, note: EntityId) -> Result<Vec<(f64, f64, f64, f64)>> {
        let scene = self.0.scene.borrow();
        Ok(self
            .0
            .service
            .blocks(note)?
            .into_iter()
            .filter(|b| !b.trashed)
            .map(|b| {
                scene
                    .as_ref()
                    .and_then(|s| s.items.get(&b.id).and_then(|i| s.model.scene_rect(*i)))
                    .map(|r| (r.x as f64, r.y as f64, r.width as f64, r.height as f64))
                    .unwrap_or((b.x, b.y, b.width, b.height))
            })
            .collect())
    }
    /// Keep the title typed in the note header. Surrounding spaces are not
    /// kept, and an empty title is not a rename: once editing ends, the field
    /// shows the stored title again.
    pub fn rename_note(&self) -> Result<()> {
        if let Some(id) = self.0.note.get() {
            let stored = self.0.service.note(id)?.title;
            let typed = self.0.title.get();
            let title = typed.trim();
            if title.is_empty() {
                if !self.0.renaming_note.get() {
                    self.0.title.set_if_changed(stored);
                }
            } else if title != stored {
                self.0
                    .service
                    .rename_note(id, title.to_owned(), self.0.organization)?;
                self.changed();
                self.refresh()?;
            }
        }
        Ok(())
    }
    pub fn document(&self, id: EntityId) -> Result<TextDocument> {
        if let Some(doc) = self.0.documents.borrow().get(&id) {
            return Ok(doc.clone());
        }
        let block = self.0.service.block(id)?;
        let doc = TextDocument::new();
        doc.set_djot_sync(&block.content_djot)?;
        let revision = Arc::new(AtomicU64::new(0));
        let counter = revision.clone();
        let wake = self.0.wake.clone();
        let subscription = doc.on_change(move |event| {
            use text_document::DocumentEvent::*;
            if matches!(
                event,
                ContentsChanged { .. }
                    | FormatChanged { .. }
                    | DocumentReset
                    | UndoRedoChanged { .. }
                    | FlowElementsInserted { .. }
                    | FlowElementsRemoved { .. }
            ) {
                counter.fetch_add(1, Ordering::Relaxed);
                wake.notify();
            }
        });
        self.0.document_changes.borrow_mut().insert(
            id,
            DocumentChanges {
                revision,
                _subscription: subscription,
            },
        );
        self.0.flushed_revisions.borrow_mut().insert(id, 0);
        self.0.documents.borrow_mut().insert(id, doc.clone());
        Ok(doc)
    }
    pub fn flush(&self) -> Result<()> {
        // Editor on_change is a user-input notification, not a complete
        // durability boundary: toolbar formatting and history also mutate
        // document content. Compare revisions before exporting any snapshot.
        for (id, changes) in self.0.document_changes.borrow().iter() {
            if self.0.flushed_revisions.borrow().get(id).copied()
                != Some(changes.revision.load(Ordering::Relaxed))
            {
                self.0.pending.borrow_mut().insert(*id);
                if self.0.editing.get() == Some(*id) {
                    self.0.format_dirty.set(true);
                }
            }
        }
        let pending: Vec<_> = self.0.pending.borrow().iter().copied().collect();
        for id in pending {
            if self
                .0
                .handles
                .borrow()
                .get(&id)
                .is_some_and(|h| h.is_composing())
            {
                continue;
            }
            let revision = self
                .0
                .document_changes
                .borrow()
                .get(&id)
                .context("Missing document subscription")?
                .revision
                .load(Ordering::Relaxed);
            let text = self
                .0
                .documents
                .borrow()
                .get(&id)
                .context("Missing live document")?
                .to_djot()?;
            if self.0.service.flush_text(id, text)? {
                self.changed();
            }
            self.0.flushed_revisions.borrow_mut().insert(id, revision);
            self.0.pending.borrow_mut().remove(&id);
        }
        Ok(())
    }
    fn coordinate(&self, now: Instant) -> Result<()> {
        self.flush()?;
        if let Some(worker) = &self.0.worker {
            while let Some(reply) = worker.poll() {
                if reply.sequence.is_some() {
                    self.0.inflight.set(None);
                }
                match reply.result {
                    Ok(()) => {
                        if let Some(sequence) = reply.sequence {
                            self.0.saved.set(self.0.saved.get().max(sequence));
                        }
                    }
                    Err(e) => {
                        self.mark_save_failed();
                        return Err(e);
                    }
                }
            }
        }
        self.refresh_undo_info();
        if self.0.format_dirty.replace(false) {
            self.sync_format();
        }
        // Fit a narrow window's canvas once it has been laid out, outside layout.
        if self.0.fit_at.get().is_some_and(|at| at <= now) {
            self.0.fit_at.set(None);
            let width = self
                .0
                .camera
                .borrow()
                .as_ref()
                .map(|c| c.viewport.get().width)
                .unwrap_or(0.);
            if width > 0. {
                self.0.fit_waiting.set(false);
                self.zoom_to_fit()?;
            }
        }
        if self.0.saved.get() == self.0.seq.get() {
            self.0.first_edit.set(None);
            if self.0.save_state.get() != SaveState::Saved {
                self.0
                    .saved_at
                    .set(chrono::Local::now().format("%H:%M").to_string());
                self.0.save_state.set(SaveState::Saved);
            }
            self.0.status.set_if_changed(tr!(saved()).resolve_now());
        } else if !self.0.save_failed.get()
            && self.0.inflight.get().is_none()
            && (now.saturating_duration_since(self.0.last_edit.get()) >= Duration::from_millis(750)
                || self
                    .0
                    .first_edit
                    .get()
                    .is_some_and(|t| now.saturating_duration_since(t) >= Duration::from_secs(5)))
        {
            self.save()?;
        }
        Ok(())
    }
    pub fn save(&self) -> Result<()> {
        self.0.save_failed.set(false);
        if self.0.save_state.get() == SaveState::Failed {
            self.0.save_state.set(SaveState::Unsaved);
        }
        self.flush()?;
        self.rename_note()?;
        if self.0.inflight.get().is_some() {
            return Ok(());
        }
        if let Some(worker) = &self.0.worker {
            let seq = self.0.seq.get();
            let sent = self
                .0
                .service
                .snapshot(self.0.assets.borrow().clone())
                .and_then(|snapshot| worker.save(seq, snapshot));
            if let Err(e) = sent {
                self.mark_save_failed();
                return Err(e);
            }
            self.0.inflight.set(Some(seq));
            self.0.save_state.set(SaveState::Saving);
            self.0.status.set(tr!(saving()).resolve_now());
        }
        Ok(())
    }
    /// Save everything before the window closes. A failure keeps the window
    /// open with the save-failed banner, which offers to close without saving.
    pub fn close(&self) -> Result<()> {
        if self.0.close_anyway.get() {
            return Ok(());
        }
        self.flush()?;
        anyhow::ensure!(
            self.0.pending.borrow().is_empty(),
            "Finish text composition before closing"
        );
        self.rename_note()?;
        let result = self.finish_saving();
        if result.is_err() {
            self.mark_save_failed();
        }
        result
    }
    /// Let the next close go ahead without saving: the save keeps failing and
    /// the user chose to lose what is not saved.
    pub fn close_without_saving(&self) {
        self.0.close_anyway.set(true);
    }
    /// Wait for the save in flight, then save what came after it.
    fn finish_saving(&self) -> Result<()> {
        if let Some(worker) = &self.0.worker {
            if self.0.inflight.get().is_some() {
                let reply = worker.wait()?;
                self.0.inflight.set(None);
                reply.result?;
                if let Some(sequence) = reply.sequence {
                    self.0.saved.set(sequence);
                }
            }
            if self.0.saved.get() < self.0.seq.get() {
                self.save()?;
                let reply = worker.wait()?;
                self.0.inflight.set(None);
                reply.result?;
                if let Some(sequence) = reply.sequence {
                    self.0.saved.set(sequence);
                }
            }
        }
        Ok(())
    }
    pub fn undo(&self, redo: bool, organization: bool) -> Result<()> {
        self.flush()?;
        let stack = if organization {
            self.0.organization
        } else {
            self.page_stack()?
        };
        if redo {
            self.0.service.redo(stack)?
        } else {
            self.0.service.undo(stack)?
        };
        self.changed();
        self.refresh()?;
        self.rebuild_page();
        Ok(())
    }
    /// Back up to the chosen folder (next to the library by default), then
    /// delete this library's oldest backups beyond the number kept.
    pub fn backup(&self) -> Result<()> {
        self.close()?;
        if let Some(w) = &self.0.worker {
            let (dir, keep) = {
                let prefs = self.0.prefs.borrow();
                (prefs.backup_dir.clone(), prefs.backup_keep)
            };
            let stem = self
                .0
                .path
                .file_stem()
                .context("Library has no name")?
                .to_string_lossy()
                .into_owned();
            let dir = dir
                .or_else(|| self.0.path.parent().map(|p| p.to_path_buf()))
                .context("No backup folder")?;
            let path = dir.join(format!(
                "{stem}.{}.noton",
                chrono::Utc::now().format("%Y%m%dT%H%M%S%.3f")
            ));
            w.backup(path)?;
            w.wait()?.result?;
            if keep > 0 {
                // Only files named exactly as backups of this library, and
                // never one open as a library, here or in another window.
                let mut backups: Vec<_> = std::fs::read_dir(&dir)?
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| {
                        p != &self.0.path
                            && p.file_name()
                                .is_some_and(|n| is_backup_of(&n.to_string_lossy(), &stem))
                            && !LibraryFile::in_use(p)
                    })
                    .collect();
                backups.sort();
                let excess = backups.len().saturating_sub(keep as usize);
                for old in &backups[..excess] {
                    std::fs::remove_file(old)?;
                }
            }
        }
        Ok(())
    }
    /// Check that `path` opens as a library — or, when `new`, create it — so
    /// that a locked, corrupt or unwritable file is reported in this window
    /// rather than lost in the process that would have opened it.
    pub fn prepare_library(path: &Path, new: bool) -> Result<()> {
        if new {
            anyhow::ensure!(!path.exists(), "Choose a new library filename");
            let mut file = LibraryFile::open(path, true)?;
            if let Err(e) = file.save(&noton_format::Library::empty("Noton".into())) {
                // Half a library would only be refused on the next attempt.
                drop(file);
                let _ = std::fs::remove_file(path);
                return Err(e);
            }
        } else {
            anyhow::ensure!(path.exists(), "Library does not exist");
            LibraryFile::open(path, false)?.load()?;
        }
        Ok(())
    }
    /// The pixels of image asset `uid`, decoded once while a page shows it.
    pub fn image(&self, uid: Uuid) -> Result<Rc<RasterIcon>> {
        if let Some(image) = self.0.images.borrow().get(&uid) {
            return Ok(image.clone());
        }
        let bytes = self
            .0
            .assets
            .borrow()
            .iter()
            .find(|a| a.uid == uid)
            .map(|a| a.bytes.clone())
            .context("Missing image asset")?;
        let image = Rc::new(display_image(RasterIcon::decode(&bytes)?));
        self.0.images.borrow_mut().insert(uid, image.clone());
        Ok(image)
    }
    /// Forget the decoded images no page on screen shows.
    pub fn keep_images(&self, shown: &HashSet<Uuid>) {
        self.0
            .images
            .borrow_mut()
            .retain(|uid, _| shown.contains(uid));
    }
    /// Write the library to `path` while the save to the library file keeps failing.
    pub fn save_copy(&self, path: &std::path::Path) -> Result<()> {
        self.flush()?;
        let mut file = LibraryFile::open(path, true)?;
        file.save(&self.0.service.snapshot(self.0.assets.borrow().clone())?)?;
        Ok(())
    }
    pub fn move_note(&self) -> Result<()> {
        let note = self.0.note.get().context("Select a note")?;
        let fs: Vec<_> = self
            .0
            .service
            .folders()?
            .into_iter()
            .filter(|f| !f.trashed)
            .collect();
        let pos = fs
            .iter()
            .position(|f| f.notes.contains(&note))
            .context("Missing note owner")?;
        anyhow::ensure!(fs.len() > 1, "Create another folder first");
        self.0
            .service
            .move_note(note, fs[(pos + 1) % fs.len()].id, self.0.organization)?;
        self.changed();
        self.refresh()
    }
}
impl SessionVm {
    pub fn import_image(&self, path: &std::path::Path) -> Result<()> {
        anyhow::ensure!(
            std::fs::metadata(path)?.len() <= 100 * 1024 * 1024,
            "Image exceeds 100 MiB"
        );
        let bytes = std::fs::read(path)?;
        let name = path
            .file_name()
            .context("Missing image name")?
            .to_string_lossy()
            .into_owned();
        self.import_image_bytes(name, bytes, None).map(|_| ())
    }
    pub fn import_image_bytes(
        &self,
        name: String,
        bytes: Vec<u8>,
        position: Option<(f64, f64)>,
    ) -> Result<EntityId> {
        anyhow::ensure!(bytes.len() <= 100 * 1024 * 1024, "Image exceeds 100 MiB");
        let image = teksilo::canvas::RasterIcon::decode(&bytes)?;
        anyhow::ensure!(
            image.width() <= 16000 && image.height() <= 16000,
            "Image dimensions exceed 16000 pixels"
        );
        let mime = teksilo::canvas::ImageFormat::sniff(&bytes)
            .context("Unsupported image format")?
            .mime_type()
            .to_owned();
        anyhow::ensure!(
            !self.0.trash.get() && !self.0.note_trashed.get(),
            "Cannot add images in trash"
        );
        // Decode before creating anything: a rejected drop leaves no empty note.
        if self.0.note.get().is_none() {
            if self.0.folder.get().is_none() {
                self.new_folder()?;
            }
            self.new_note()?;
        }
        let asset = Asset::new(name, mime, bytes);
        let note = self.0.note.get().context("Select a note")?;
        let id = self.0.service.insert_image_at(
            note,
            &asset,
            (image.width() as f64, image.height() as f64),
            position,
            self.page_stack()?,
        )?;
        self.0.editing.set(None);
        self.0.drawing.set(None);
        self.0.active_text.set(None);
        self.0.selected.set(Some(id));
        self.0.active_block.set(Some(id));
        self.0.focus_frame.set(Some(id));
        self.claim_page();
        self.0
            .images
            .borrow_mut()
            .insert(asset.uid, Rc::new(display_image(image)));
        self.0.assets.borrow_mut().push(asset);
        self.changed();
        self.refresh()?;
        self.rebuild_page();
        Ok(id)
    }
    pub fn export_note(&self, path: &std::path::Path, html: bool) -> Result<()> {
        self.flush()?;
        let note = self
            .0
            .service
            .note(self.0.note.get().context("Select a note")?)?;
        fn escape(s: &str) -> String {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
        }
        let mut out = if html {
            format!(
                "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title></head><body><h1>{}</h1>\n",
                escape(&note.title),
                escape(&note.title)
            )
        } else {
            format!("{}\n\n", note.title)
        };
        for b in self
            .0
            .service
            .blocks(note.id)?
            .into_iter()
            .filter(|b| !b.trashed)
        {
            match b.kind {
                BlockKind::RichText => {
                    let doc = self.document(b.id)?;
                    out.push_str(&if html {
                        doc.to_html()?
                    } else {
                        doc.to_plain_text()?
                    });
                }
                BlockKind::Image => {
                    // A decorative image says nothing to a reader: empty alt.
                    let alt = if b.decorative {
                        ""
                    } else {
                        b.alt_text.as_str()
                    };
                    if html {
                        let uid = self.0.service.image_asset(b.id)?;
                        let assets = self.0.assets.borrow();
                        let a = assets
                            .iter()
                            .find(|a| a.uid == uid)
                            .context("Missing image")?;
                        use base64::Engine;
                        out.push_str(&format!(
                            "<img alt=\"{}\" src=\"data:{};base64,{}\">",
                            escape(alt),
                            a.mime,
                            base64::engine::general_purpose::STANDARD.encode(&a.bytes)
                        ));
                    } else {
                        out.push_str(alt);
                    }
                }
                BlockKind::Drawing => {
                    if html {
                        // Samples live in a 560 × 320 space stretched over the
                        // sketch, on paper, as on the canvas.
                        out.push_str(&format!(
                            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 560 320\" preserveAspectRatio=\"none\" role=\"img\"><title>{}</title><rect width=\"560\" height=\"320\" fill=\"#FFFEFB\"/>",
                            b.width.round(),
                            b.height.round(),
                            escape(&b.description)
                        ));
                        for id in b.strokes {
                            let s = self.0.service.ink_stroke(id)?;
                            if !s.trashed && !self.0.service.is_discarded(Kind::Stroke, id) {
                                let samples: Vec<noton_format::Sample> =
                                    serde_json::from_str(&s.samples_json)?;
                                let points = samples
                                    .iter()
                                    .map(|p| format!("{},{}", p.x, p.y))
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                let rgba = s.brush_rgba as u32;
                                out.push_str(&format!(
                                    "<polyline points=\"{points}\" fill=\"none\" stroke=\"#{:06X}\" stroke-opacity=\"{:.3}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" vector-effect=\"non-scaling-stroke\"/>",
                                    rgba >> 8,
                                    (rgba & 0xff) as f64 / 255.,
                                    s.brush_width
                                ));
                            }
                        }
                        out.push_str("</svg>");
                    } else {
                        out.push_str(&b.description);
                    }
                }
            }
            out.push('\n');
        }
        if html {
            out.push_str("</body></html>\n");
        }
        // Native save dialogs authorize replacement. A same-directory temporary file
        // is synced before the destination is replaced.
        let mut file =
            tempfile::NamedTempFile::new_in(path.parent().unwrap_or(std::path::Path::new(".")))?;
        use std::io::Write;
        file.write_all(out.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(path)?;
        Ok(())
    }
}

/// The translated phrase for an undo command key: "move block", "rename folder".
pub fn undo_phrase(key: &str) -> String {
    match key {
        "create_block" | "insert_image" => tr!(undo_insert_block()),
        "move_block" | "transform_blocks" => tr!(undo_move_block()),
        "bring_to_front" | "send_to_back" => tr!(undo_arrange_block()),
        "duplicate_block" => tr!(undo_duplicate_block()),
        "rename_block" => tr!(undo_rename_block()),
        "trash_block" => tr!(undo_trash_block()),
        "describe_sketch" => tr!(undo_describe_sketch()),
        "edit_alt_text" => tr!(undo_edit_alt_text()),
        "reorder_block" => tr!(undo_reorder_block()),
        "draw_stroke" => tr!(undo_draw_stroke()),
        "erase_strokes" => tr!(undo_erase_strokes()),
        "create_folder" => tr!(undo_create_folder()),
        "rename_folder" => tr!(undo_rename_folder()),
        "trash_folder" => tr!(undo_trash_folder()),
        "create_note" => tr!(undo_create_note()),
        "rename_note" => tr!(undo_rename_note()),
        "trash_note" => tr!(undo_trash_note()),
        "move_note" => tr!(undo_move_note()),
        "reorder_folder" => tr!(undo_reorder_folders()),
        "reorder_note" => tr!(undo_reorder_notes()),
        _ => tr!(undo_change()),
    }
    .resolve_now()
}
impl SessionVm {
    /// The editor of the text block being edited.
    pub fn editing_handle(&self) -> Option<EditorHandle> {
        let id = self.0.editing.get()?;
        self.0.handles.borrow().get(&id).cloned()
    }
    /// Mirror the caret's formatting into the toolbar's pressed states.
    pub fn sync_format(&self) {
        let Some(h) = self.editing_handle() else {
            return;
        };
        let f = &self.0.format;
        f.bold.set_if_changed(h.is_bold());
        f.italic.set_if_changed(h.is_italic());
        f.underline.set_if_changed(h.is_underline());
        f.strike.set_if_changed(h.is_strikethrough());
        f.link.set_if_changed(h.is_link());
        f.in_table.set_if_changed(h.is_in_table());
        f.heading.set_if_changed(Some(h.get_heading_level().min(3)));
    }
    /// Put a sketch in drawing mode: its tools replace the formatting toolbar.
    pub fn enter_drawing(&self, id: EntityId) {
        self.0.editing.set(None);
        self.0.selected.set(Some(id));
        self.0.drawing.set(Some(id));
        self.0.edit.claim_structure(false);
        self.rebuild_page();
    }
    pub fn leave_drawing(&self) {
        if self.0.drawing.get().is_some() {
            self.0.drawing.set(None);
            self.rebuild_page();
        }
    }
    /// Keep an observer for the life of the window.
    pub fn keep(&self, handle: teksilo::core::signal::ObserverHandle) {
        self.0.observers.borrow_mut().push(handle);
    }
    pub fn refresh_undo_info(&self) {
        let page = self
            .0
            .note
            .get()
            .and_then(|n| self.0.page_stacks.borrow().get(&n).copied());
        let (typing, library) = self.0.edit.target();
        // An empty label means a plain field (rename, search) has the keyboard.
        // The block last typed in: stable while a menu has the focus, so the
        // Edit menu is not rebuilt (and closed) as it opens.
        let typing_in = typing.then(|| {
            self.0
                .active_text
                .get()
                .and_then(|id| self.0.service.block(id).ok())
                .map(|b| b.label)
                .unwrap_or_default()
        });
        let info = UndoInfo {
            typing_in,
            page_undo: page
                .and_then(|s| self.0.service.undo_label(s))
                .map(undo_phrase),
            page_redo: page
                .and_then(|s| self.0.service.redo_label(s))
                .map(undo_phrase),
            library_undo: self
                .0
                .service
                .undo_label(self.0.organization)
                .map(undo_phrase),
            library_redo: self
                .0
                .service
                .redo_label(self.0.organization)
                .map(undo_phrase),
            library_focused: library,
        };
        self.0.undo_info.set_if_changed(info);
    }
    /// Start the camera of a new canvas from where the last one of this note was.
    pub fn attach_camera(&self, camera: Camera) {
        let note = self.0.note.get();
        if let (Some(note), Some((kept, zoom, x, y))) = (note, self.0.kept_camera.get()) {
            if kept == note {
                camera.zoom.set(zoom);
                camera.pan_x.set(x);
                camera.pan_y.set(y);
            }
        }
        self.0.zoom.set(camera.zoom.get());
        let weak = Rc::downgrade(&self.0);
        *self.0.camera_observer.borrow_mut() = Some(camera.viewport.observe(move |_| {
            if let Some(inner) = weak.upgrade() {
                if inner.fit_waiting.get() {
                    inner
                        .fit_at
                        .set(Some(Instant::now() + Duration::from_millis(16)));
                    inner.wake.notify();
                }
            }
        }));
        *self.0.camera.borrow_mut() = Some(camera);
        if note.is_some() && self.0.fit_pending.get() == note {
            self.0.fit_pending.set(None);
            self.0.fit_waiting.set(true);
            self.0
                .fit_at
                .set(Some(Instant::now() + Duration::from_millis(16)));
            self.0.wake.notify();
        }
    }
    /// Remember the camera before the canvas is rebuilt.
    pub fn keep_camera(&self) {
        if let (Some(note), Some(c)) = (self.0.note.get(), self.0.camera.borrow().as_ref()) {
            self.0
                .kept_camera
                .set(Some((note, c.zoom.get(), c.pan_x.get(), c.pan_y.get())));
        }
    }
    /// Zoom about the centre of the viewport.
    pub fn zoom_to(&self, zoom: f32) {
        if let Some(c) = self.0.camera.borrow().as_ref() {
            let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
            let old = c.zoom.get();
            let v = c.viewport.get();
            let (cx, cy) = (v.width / 2., v.height / 2.);
            // Keep the scene point under the centre fixed: view = scale * scene + pan.
            c.pan_x.set(cx - (cx - c.pan_x.get()) * zoom / old);
            c.pan_y.set(cy - (cy - c.pan_y.get()) * zoom / old);
            c.zoom.set(zoom);
            self.0.zoom.set(zoom);
        }
    }
    pub fn zoom_step(&self, zoom_in: bool) {
        const STEPS: [f32; 11] = [0.25, 0.33, 0.5, 0.67, 0.75, 0.9, 1., 1.25, 1.5, 2., 3.];
        let z = self.0.zoom.get();
        let next = if zoom_in {
            STEPS
                .iter()
                .copied()
                .find(|s| *s > z + 0.01)
                .unwrap_or(MAX_ZOOM)
        } else {
            STEPS
                .iter()
                .rev()
                .copied()
                .find(|s| *s < z - 0.01)
                .unwrap_or(MIN_ZOOM)
        };
        self.zoom_to(next);
    }
    /// Frame every block of the note, never above 100 %.
    pub fn zoom_to_fit(&self) -> Result<()> {
        let Some(note) = self.0.note.get() else {
            return Ok(());
        };
        let blocks: Vec<_> = self
            .0
            .service
            .blocks(note)?
            .into_iter()
            .filter(|b| !b.trashed)
            .collect();
        let Some(c) = self.0.camera.borrow().clone() else {
            return Ok(());
        };
        if blocks.is_empty() {
            c.zoom.set(1.);
            c.pan_x.set(0.);
            c.pan_y.set(0.);
            self.0.zoom.set(1.);
            return Ok(());
        }
        let x0 = blocks.iter().map(|b| b.x).fold(f64::MAX, f64::min) as f32;
        let y0 = blocks.iter().map(|b| b.y).fold(f64::MAX, f64::min) as f32;
        let x1 = blocks
            .iter()
            .map(|b| b.x + b.width)
            .fold(f64::MIN, f64::max) as f32;
        let y1 = blocks
            .iter()
            .map(|b| b.y + b.height)
            .fold(f64::MIN, f64::max) as f32;
        let v = c.viewport.get();
        let margin = 24.;
        let scale = ((v.width - 2. * margin) / (x1 - x0).max(1.))
            .min((v.height - 2. * margin) / (y1 - y0).max(1.))
            .clamp(MIN_ZOOM, 1.);
        c.zoom.set(scale);
        c.pan_x.set(v.width / 2. - scale * (x0 + x1) / 2.);
        c.pan_y.set(v.height / 2. - scale * (y0 + y1) / 2.);
        self.0.zoom.set(scale);
        Ok(())
    }
    fn set_geometry(
        &self,
        id: EntityId,
        f: impl FnOnce(&mut noton_format::Geometry, &common::entities::Block),
    ) -> Result<()> {
        let b = self.0.service.block(id)?;
        let mut g = noton_format::Geometry {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
            z: b.z,
        };
        f(&mut g, &b);
        g.width = g.width.max(80.);
        g.height = g.height.max(40.);
        self.0.service.geometry(id, g, self.page_stack()?)?;
        self.claim_page();
        self.changed();
        self.sync_block(id)
    }
    /// Show a block's stored geometry on the canvas without rebuilding it.
    fn sync_block(&self, id: EntityId) -> Result<()> {
        let b = self.0.service.block(id)?;
        if let Some(scene) = self.0.scene.borrow().as_ref() {
            scene.sync(&b);
        }
        Ok(())
    }
    /// Move a block by `dx`, `dy` scene pixels.
    pub fn nudge(&self, id: EntityId, dx: f64, dy: f64) -> Result<()> {
        self.set_geometry(id, |g, _| {
            g.x += dx;
            g.y += dy;
        })
    }
    /// Resize a block; images and sketches keep their aspect ratio when asked.
    pub fn resize(&self, id: EntityId, dw: f64, dh: f64) -> Result<()> {
        self.set_geometry(id, |g, b| {
            g.width += dw;
            if b.kind != BlockKind::RichText {
                g.height += dh;
            }
        })
    }
    /// Apply the Position and size fields.
    pub fn place(
        &self,
        id: EntityId,
        x: f64,
        y: f64,
        width: f64,
        height: Option<f64>,
    ) -> Result<()> {
        self.set_geometry(id, |g, _| {
            g.x = x;
            g.y = y;
            g.width = width;
            if let Some(h) = height {
                g.height = h;
            }
        })
    }
    pub fn arrange(&self, id: EntityId, front: bool) -> Result<()> {
        self.0
            .service
            .arrange_block(id, front, self.page_stack()?)?;
        self.claim_page();
        self.changed();
        self.sync_block(id)
    }
    pub fn duplicate(&self, id: EntityId) -> Result<()> {
        self.flush()?;
        let copy = self.0.service.duplicate_block(id, self.page_stack()?)?;
        // The copy starts from the stored text; a composing editor may be ahead.
        if self.0.service.block(copy)?.kind == BlockKind::RichText {
            self.0
                .service
                .flush_text(copy, self.document(id)?.to_djot()?)?;
        }
        self.claim_page();
        self.0.selected.set(Some(copy));
        self.changed();
        self.refresh()?;
        self.rebuild_page();
        Ok(())
    }
    /// Move a block `offset` places in reading order, among the blocks a
    /// reader sees: a block in the trash holds no place.
    pub fn reorder(&self, id: EntityId, offset: i32) -> Result<()> {
        let note = self.0.note.get().context("Select a note")?;
        let ids = self.0.service.note(note)?.blocks;
        let live: Vec<usize> = ids
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                !self.0.service.is_discarded(Kind::Block, **b)
                    && self.0.service.block(**b).is_ok_and(|b| !b.trashed)
            })
            .map(|(i, _)| i)
            .collect();
        let Some(from) = live.iter().position(|i| ids[*i] == id) else {
            return Ok(());
        };
        let to = (from as i64 + offset as i64).clamp(0, live.len() as i64 - 1) as usize;
        if to != from {
            // Taking the neighbour's index puts the block before it when
            // moving earlier and, once the block is out of the list, after it
            // when moving later.
            self.0
                .service
                .reorder_block(note, id, live[to], self.page_stack()?)?;
            self.claim_page();
            self.changed();
            self.rebuild_page();
        }
        Ok(())
    }
    /// A block's place in reading order among the live blocks: (position, count), 1-based.
    pub fn reading_position(&self, id: EntityId) -> Option<(usize, usize)> {
        let note = self.0.note.get()?;
        let blocks: Vec<_> = self
            .0
            .service
            .blocks(note)
            .ok()?
            .into_iter()
            .filter(|b| !b.trashed)
            .collect();
        let i = blocks.iter().position(|b| b.id == id)?;
        Some((i + 1, blocks.len()))
    }
    pub fn trash_block(&self, id: EntityId) -> Result<()> {
        let b = self.0.service.block(id)?;
        let stack = self.page_stack()?;
        self.0.undo_toast.set_if_changed(None);
        self.0.service.trash_block(id, !b.trashed, stack)?;
        self.claim_page();
        if self.0.editing.get() == Some(id) {
            self.0.editing.set(None);
        }
        if self.0.selected.get() == Some(id) {
            self.0.selected.set(None);
        }
        self.changed();
        self.refresh()?;
        self.rebuild_page();
        // After the refresh, which may select another note and so drop offers.
        if !b.trashed {
            self.offer_undo(stack, tr!(block_trashed(name = b.label)).resolve_now());
        }
        Ok(())
    }
    pub fn rename_block(&self, id: EntityId, label: String) -> Result<()> {
        if self.0.service.block(id)?.label != label {
            self.0.service.rename_block(id, label, self.page_stack()?)?;
            self.claim_page();
            self.changed();
            self.rebuild_page();
        }
        Ok(())
    }
    pub fn describe(&self, id: EntityId, text: String) -> Result<()> {
        let b = self.0.service.block(id)?;
        let stack = self.page_stack()?;
        match b.kind {
            BlockKind::Drawing if b.description != text => {
                self.0.service.describe_sketch(id, text, stack)?
            }
            BlockKind::Image if b.alt_text != text => {
                self.0.service.set_alt_text(id, text, stack)?
            }
            _ => return Ok(()),
        }
        self.claim_page();
        self.changed();
        self.rebuild_page();
        Ok(())
    }
    /// Set an image's alt text and whether it is decorative, as one step.
    pub fn describe_image(&self, id: EntityId, alt: String, decorative: bool) -> Result<()> {
        let b = self.0.service.block(id)?;
        if b.alt_text == alt && b.decorative == decorative {
            return Ok(());
        }
        self.0
            .service
            .describe_image(id, alt, decorative, self.page_stack()?)?;
        self.claim_page();
        self.changed();
        self.rebuild_page();
        Ok(())
    }
    /// Show the snackbar that can take back the command just pushed on `stack`.
    fn offer_undo(&self, stack: u64, message: String) {
        if let Some(seq) = self.0.service.head(stack) {
            self.0.undo_toast.set(Some((stack, seq, message)));
        }
    }
    /// The snackbar's Undo of command `seq` on `stack`: only if nothing was
    /// pushed on that stack since.
    pub fn undo_offered(&self, stack: u64, seq: u64) -> Result<()> {
        self.flush()?;
        if self.0.service.undo_if_head(stack, seq)? {
            self.changed();
            self.refresh()?;
            self.rebuild_page();
        }
        Ok(())
    }
    pub fn trash_note(&self, id: EntityId) -> Result<()> {
        let n = self.0.service.note(id)?;
        self.flush()?;
        self.0.undo_toast.set_if_changed(None);
        self.0
            .service
            .trash_note(id, !n.trashed, self.0.organization)?;
        self.changed();
        // Trashing the open note selects another, which drops pending offers:
        // make this one after it.
        self.refresh()?;
        if !n.trashed {
            self.offer_undo(
                self.0.organization,
                tr!(note_trashed(name = n.title)).resolve_now(),
            );
        }
        Ok(())
    }
    pub fn trash_folder(&self, id: EntityId) -> Result<()> {
        let f = self.0.service.folder(id)?;
        self.flush()?;
        self.0.undo_toast.set_if_changed(None);
        self.0
            .service
            .trash_folder(id, !f.trashed, self.0.organization)?;
        self.changed();
        // After the refresh, which may select another note and so drop offers.
        self.refresh()?;
        if !f.trashed {
            self.offer_undo(
                self.0.organization,
                tr!(folder_trashed(name = f.title)).resolve_now(),
            );
        }
        Ok(())
    }
    /// Start renaming a folder in place.
    pub fn start_folder_rename(&self, id: EntityId) -> Result<()> {
        self.commit_folder_rename()?;
        let title = self.0.service.folder(id)?.title;
        *self.0.folder_rename.borrow_mut() = Some((id, Signal::new(title)));
        self.0.folder_rename_focused.set(false);
        self.0.renaming_folder.set(Some(id));
        self.refresh()
    }
    /// The inline rename's text for `id`, created on first use.
    pub fn folder_rename_text(&self, id: EntityId, title: &str) -> Signal<String> {
        let mut slot = self.0.folder_rename.borrow_mut();
        match slot.as_ref() {
            Some((current, text)) if *current == id => text.clone(),
            _ => {
                let text = Signal::new(title.to_owned());
                *slot = Some((id, text.clone()));
                text
            }
        }
    }
    /// Keep what was typed in an inline folder rename before doing something else.
    pub fn commit_folder_rename(&self) -> Result<()> {
        if let Some(id) = self.0.renaming_folder.get() {
            let text = self
                .0
                .folder_rename
                .borrow()
                .as_ref()
                .filter(|(current, _)| *current == id)
                .map(|(_, t)| t.get());
            match text {
                Some(text) => self.rename_folder(id, text)?,
                None => self.0.renaming_folder.set(None),
            }
        }
        Ok(())
    }
    pub fn rename_folder(&self, id: EntityId, title: String) -> Result<()> {
        self.0.renaming_folder.set(None);
        *self.0.folder_rename.borrow_mut() = None;
        let title = title.trim().to_owned();
        if !title.is_empty() && self.0.service.folder(id)?.title != title {
            self.0
                .service
                .rename_folder(id, title, self.0.organization)?;
            self.changed();
        }
        self.refresh()
    }
    /// Drop the folder `key` before or after the folder `target`.
    pub fn drop_folder(&self, key: EntityId, target: EntityId, after: bool) -> Result<bool> {
        let mut ids: Vec<_> = self.0.service.folders()?.iter().map(|f| f.id).collect();
        ids.retain(|id| *id != key);
        let Some(at) = ids.iter().position(|id| *id == target) else {
            return Ok(false);
        };
        self.0
            .service
            .reorder_folder(key, at + usize::from(after), self.0.organization)?;
        self.changed();
        self.refresh()?;
        Ok(true)
    }
    /// Drop the note `key` before or after the note `target`, in the open folder.
    pub fn drop_note(&self, key: EntityId, target: EntityId, after: bool) -> Result<bool> {
        let Some(folder) = self.0.folder.get() else {
            return Ok(false);
        };
        let mut ids = self.0.service.folder(folder)?.notes;
        if !ids.contains(&key) || !self.0.query.get().trim().is_empty() {
            return Ok(false);
        }
        ids.retain(|id| *id != key);
        let Some(at) = ids.iter().position(|id| *id == target) else {
            return Ok(false);
        };
        self.0
            .service
            .reorder_note(folder, key, at + usize::from(after), self.0.organization)?;
        self.changed();
        self.refresh()?;
        Ok(true)
    }
    pub fn move_note_to(&self, note: EntityId, folder: EntityId) -> Result<()> {
        self.flush()?;
        self.0
            .service
            .move_note(note, folder, self.0.organization)?;
        self.changed();
        self.refresh()
    }
    /// Live folders other than the one holding `note`, for "Move to folder".
    pub fn other_folders(&self, note: EntityId) -> Vec<(EntityId, String)> {
        self.0
            .service
            .folders()
            .unwrap_or_default()
            .into_iter()
            .filter(|f| !f.trashed && !f.notes.contains(&note))
            .map(|f| (f.id, f.title))
            .collect()
    }
    /// Whether the library has nothing in its trash.
    pub fn trash_count(&self) -> usize {
        let Ok(folders) = self.0.service.folders() else {
            return 0;
        };
        folders
            .iter()
            .map(|f| {
                usize::from(f.trashed)
                    + self
                        .0
                        .service
                        .notes(f.id)
                        .map(|ns| ns.iter().filter(|n| n.trashed).count())
                        .unwrap_or(0)
            })
            .sum()
    }
    pub fn library_name(&self) -> String {
        self.0
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| tr!(untitled_library()).resolve_now())
    }
}

#[cfg(test)]
mod tests {
    use super::is_backup_of;

    #[test]
    fn only_timestamped_backups_of_this_library_are_pruned() {
        assert!(is_backup_of("Notes.20260927T231245.123.noton", "Notes"));
        // Another library whose name starts like a backup.
        assert!(!is_backup_of("Notes.2.noton", "Notes"));
        assert!(!is_backup_of("Notes.2026 archive.noton", "Notes"));
        // A backup of another library that shares the prefix.
        assert!(!is_backup_of("Notes.2.20260927T231245.123.noton", "Notes"));
        assert!(!is_backup_of(
            "Notes.20260927T231245.123.noton.lock",
            "Notes"
        ));
        assert!(!is_backup_of("Notes.noton", "Notes"));
        assert!(is_backup_of("Notes.2.20260927T231245.123.noton", "Notes.2"));
    }
}

impl SessionVm {
    pub(crate) fn wire_coordinator(&self, ctx: &mut BuildContext) {
        self.0.wake.attach(ctx.poster().cloned());
        self.0.edit.set_wake(self.0.wake.clone());
        *self.0.tree_wake.borrow_mut() = Some(ctx.wake_at_handle());
        let vm = self.clone();
        ctx.effect(&ctx.frame_tick(), move |_| vm.report(vm.run_scheduled()));
        self.0.wake.notify();
        ctx.request_frame(); // One bootstrap pass; no re-arming subscription.
    }

    pub(crate) fn watch_history(&self, client: &frontend::EventHubClient) {
        let mut subscriptions = self.0.history_observers.borrow_mut();
        subscriptions.clear();
        for event in [
            common::event::UndoRedoEvent::StackChanged,
            common::event::UndoRedoEvent::Undone,
            common::event::UndoRedoEvent::Redone,
        ] {
            let wake = self.0.wake.clone();
            subscriptions.push(
                client.subscribe(common::event::Origin::UndoRedo(event), move |_| {
                    wake.notify()
                }),
            );
        }
    }

    pub fn handle_coordinator_event(&self, payload: &(dyn std::any::Any + Send)) -> bool {
        let Some(event) = payload.downcast_ref::<crate::coordinator::CoordinatorEvent>() else {
            return false;
        };
        if !self.0.wake.owns(event) {
            return false;
        }
        self.report(self.run_scheduled());
        true
    }

    /// Shared by native event delivery and the headless host. An observation
    /// does not flush documents or poll saves unless work was actually requested.
    pub fn run_scheduled(&self) -> Result<()> {
        self.run_scheduled_at(Instant::now())
    }
    fn run_scheduled_at(&self, now: Instant) -> Result<()> {
        let notified = self.0.wake.take();
        if notified || self.0.deadline.get().is_some_and(|at| at <= now) {
            self.0.deadline.set(None);
            let result = self.coordinate(now);
            if result.is_err() {
                self.mark_save_failed();
            }
            self.update_deadline();
            result?;
        }
        self.arm_deadline();
        Ok(())
    }
    fn update_deadline(&self) {
        let autosave = if self.0.worker.is_some()
            && self.0.saved.get() < self.0.seq.get()
            && !self.0.save_failed.get()
            && self.0.inflight.get().is_none()
        {
            let quiet = self.0.last_edit.get() + Duration::from_millis(750);
            Some(
                self.0
                    .first_edit
                    .get()
                    .map_or(quiet, |first| quiet.min(first + Duration::from_secs(5))),
            )
        } else {
            None
        };
        self.0
            .deadline
            .set(autosave.into_iter().chain(self.0.fit_at.get()).min());
    }
    fn arm_deadline(&self) {
        if let (Some(at), Some(wake)) = (self.0.deadline.get(), self.0.tree_wake.borrow().as_ref())
        {
            // The cell belongs to the whole tree. Never replace an earlier
            // deadline (caret, tooltip, another widget), or clear its work.
            if wake.get().is_none_or(|existing| at < existing) {
                wake.set(Some(at));
            }
        }
    }
}

#[cfg(test)]
mod scheduling_tests;
