//! Portable UUID-based library format. No generated entity IDs enter this file.
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;

pub const FORMAT_VERSION: u32 = 1;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Library {
    pub uid: Uuid,
    pub title: String,
    pub folders: Vec<Folder>,
    pub assets: Vec<Asset>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Folder {
    pub uid: Uuid,
    pub title: String,
    pub trashed: bool,
    pub notes: Vec<Note>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub uid: Uuid,
    pub title: String,
    pub trashed: bool,
    pub blocks: Vec<Block>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Block {
    pub uid: Uuid,
    pub label: String,
    pub trashed: bool,
    pub geometry: Geometry,
    pub content: Content,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub z: f64,
}
impl Default for Geometry {
    fn default() -> Self {
        Self {
            x: 40.,
            y: 40.,
            width: 560.,
            height: 320.,
            z: 0.,
        }
    }
}
impl Geometry {
    pub fn validate(self) -> Result<()> {
        ensure!(
            [self.x, self.y, self.width, self.height, self.z]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1_000_000.),
            "Invalid block geometry"
        );
        ensure!(
            self.width >= 80. && self.height >= 40.,
            "Block is too small"
        );
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Content {
    RichText {
        version: u32,
        djot: String,
    },
    Image {
        asset: Uuid,
        alt: String,
        decorative: bool,
    },
    Drawing {
        description: String,
        strokes: Vec<Stroke>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Stroke {
    pub uid: Uuid,
    pub trashed: bool,
    pub version: u32,
    pub width: f64,
    pub rgba: u32,
    pub pressure: bool,
    pub samples: Vec<Sample>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    pub x: f64,
    pub y: f64,
    pub pressure: f64,
}
impl Stroke {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1 && self.width.is_finite() && (0.25..=100.).contains(&self.width),
            "Unsupported ink brush"
        );
        ensure!(
            !self.samples.is_empty() && self.samples.len() <= 1_000_000,
            "Invalid ink sample count"
        );
        ensure!(
            self.samples.iter().all(|p| p.x.is_finite()
                && p.y.is_finite()
                && p.x.abs() <= 1_000_000.
                && p.y.abs() <= 1_000_000.
                && (0.0..=1.).contains(&p.pressure)),
            "Invalid ink sample"
        );
        Ok(())
    }
}
/// An imported image. Assets never change once imported, so the bytes are
/// shared: cloning an asset (or a whole library snapshot) copies no pixels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub uid: Uuid,
    pub name: String,
    pub mime: String,
    pub digest: String,
    pub bytes: Arc<[u8]>,
}
impl Asset {
    /// A new asset whose digest is computed from `bytes`.
    pub fn new(name: String, mime: String, bytes: Vec<u8>) -> Self {
        Self {
            uid: Uuid::new_v4(),
            name,
            mime,
            digest: blake3::hash(&bytes).to_hex().to_string(),
            bytes: bytes.into(),
        }
    }
}
impl Library {
    pub fn empty(title: String) -> Self {
        Self {
            uid: Uuid::new_v4(),
            title,
            folders: vec![],
            assets: vec![],
        }
    }
    /// Check structure and limits. Asset digests are not recomputed here (see
    /// [`verify_asset_digests`](Self::verify_asset_digests)): this runs on
    /// every save, and assets are immutable once imported or loaded.
    pub fn validate(&self) -> Result<()> {
        let mut ids = HashSet::new();
        let mut insert = |id: Uuid| -> Result<()> {
            ensure!(!id.is_nil() && ids.insert(id), "Duplicate or nil UUID");
            Ok(())
        };
        insert(self.uid)?;
        let assets: HashSet<_> = self.assets.iter().map(|a| a.uid).collect();
        for a in &self.assets {
            insert(a.uid)?;
            ensure!(a.bytes.len() <= 100 * 1024 * 1024, "Image exceeds 100 MiB");
            ensure!(
                ["image/png", "image/jpeg", "image/webp"].contains(&a.mime.as_str()),
                "Unsupported image type"
            );
        }
        for f in &self.folders {
            insert(f.uid)?;
            for n in &f.notes {
                insert(n.uid)?;
                for b in &n.blocks {
                    insert(b.uid)?;
                    b.geometry.validate()?;
                    match &b.content {
                        Content::RichText { version, djot } => {
                            ensure!(*version == 1, "Unsupported text codec");
                            ensure!(djot.len() <= 32 * 1024 * 1024, "Text block exceeds 32 MiB");
                        }
                        Content::Image { asset, .. } => {
                            ensure!(assets.contains(asset), "Missing image asset")
                        }
                        Content::Drawing { strokes, .. } => {
                            for s in strokes {
                                insert(s.uid)?;
                                s.validate()?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    /// Recompute every asset's digest: run on data read from disk.
    pub fn verify_asset_digests(&self) -> Result<()> {
        for a in &self.assets {
            ensure!(
                blake3::hash(&a.bytes).to_hex().as_str() == a.digest,
                "Asset checksum mismatch"
            );
        }
        Ok(())
    }
}

/// Owns a cross-process advisory lock for the entire open lifetime, including autosave.
/// A separate lock inode avoids interference with SQLite's own byte-range locks.
pub struct LibraryFile {
    connection: Connection,
    _lock: File,
    path: PathBuf,
}
impl LibraryFile {
    pub fn open(path: &Path, create: bool) -> Result<Self> {
        let path = if path.exists() {
            path.canonicalize()?
        } else {
            ensure!(create, "Library does not exist");
            path.parent()
                .unwrap_or(Path::new("."))
                .canonicalize()?
                .join(path.file_name().context("Invalid file name")?)
        };
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("noton.lock"))?;
        fs2::FileExt::try_lock_exclusive(&lock)
            .context("This library is already open in another process")?;
        let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
            | if create {
                rusqlite::OpenFlags::SQLITE_OPEN_CREATE
            } else {
                rusqlite::OpenFlags::empty()
            };
        let connection = Connection::open_with_flags(&path, flags)?;
        connection.busy_timeout(std::time::Duration::from_secs(2))?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version == 0 {
            let count: u32 = connection.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get(0),
            )?;
            ensure!(create && count == 0, "Not a Noton library");
            connection.execute_batch("BEGIN IMMEDIATE; CREATE TABLE records(uid TEXT PRIMARY KEY, kind TEXT NOT NULL, parent TEXT, ordinal INTEGER NOT NULL, payload TEXT NOT NULL); CREATE TABLE assets(uid TEXT PRIMARY KEY, payload TEXT NOT NULL, bytes BLOB NOT NULL); PRAGMA application_id=1313821774; PRAGMA user_version=1; COMMIT;")?;
        }
        ensure!(
            version == 0 || version == FORMAT_VERSION,
            "Unsupported Noton file version {version}"
        );
        let app: u32 = connection.pragma_query_value(None, "application_id", |r| r.get(0))?;
        ensure!(app == 1313821774, "Not a Noton library");
        connection.pragma_update(None, "journal_mode", "DELETE")?;
        connection.pragma_update(None, "synchronous", "EXTRA")?;
        Ok(Self {
            connection,
            _lock: lock,
            path,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Whether some process (this one included) has `path` open as a library.
    pub fn in_use(path: &Path) -> bool {
        let Ok(lock) = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.with_extension("noton.lock"))
        else {
            return false;
        };
        match fs2::FileExt::try_lock_exclusive(&lock) {
            Ok(()) => {
                let _ = fs2::FileExt::unlock(&lock);
                false
            }
            Err(_) => true,
        }
    }
    pub fn save(&mut self, library: &Library) -> Result<()> {
        library.validate()?;
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        tx.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS live(uid TEXT PRIMARY KEY); DELETE FROM live;",
        )?;
        fn put(
            tx: &rusqlite::Transaction<'_>,
            uid: Uuid,
            kind: &str,
            parent: Option<Uuid>,
            ordinal: usize,
            value: serde_json::Value,
        ) -> Result<()> {
            tx.execute("INSERT INTO records VALUES(?1,?2,?3,?4,?5) ON CONFLICT(uid) DO UPDATE SET parent=excluded.parent,ordinal=excluded.ordinal,payload=excluded.payload WHERE parent IS NOT excluded.parent OR ordinal!=excluded.ordinal OR payload!=excluded.payload",params![uid.to_string(),kind,parent.map(|p|p.to_string()),ordinal as i64,serde_json::to_string(&value)?])?;
            tx.execute("INSERT INTO live VALUES(?1)", [uid.to_string()])?;
            Ok(())
        }
        put(
            &tx,
            library.uid,
            "library",
            None,
            0,
            serde_json::json!({"title":library.title}),
        )?;
        for (fi, f) in library.folders.iter().enumerate() {
            put(
                &tx,
                f.uid,
                "folder",
                Some(library.uid),
                fi,
                serde_json::json!({"title":f.title,"trashed":f.trashed}),
            )?;
            for (ni, n) in f.notes.iter().enumerate() {
                put(
                    &tx,
                    n.uid,
                    "note",
                    Some(f.uid),
                    ni,
                    serde_json::json!({"title":n.title,"trashed":n.trashed}),
                )?;
                for (bi, b) in n.blocks.iter().enumerate() {
                    put(
                        &tx,
                        b.uid,
                        "block",
                        Some(n.uid),
                        bi,
                        serde_json::to_value(b)?,
                    )?;
                }
            }
        }
        tx.execute(
            "DELETE FROM records WHERE uid NOT IN (SELECT uid FROM live)",
            [],
        )?;
        tx.execute("DELETE FROM live", [])?;
        // The payload names the digest, so an unchanged payload means unchanged
        // bytes: only new or changed assets bind (and write) their blobs.
        let stored: HashMap<String, String> = tx
            .prepare("SELECT uid,payload FROM assets")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for a in &library.assets {
            let uid = a.uid.to_string();
            let meta =
                serde_json::json!({"name":a.name,"mime":a.mime,"digest":a.digest}).to_string();
            if stored.get(&uid) != Some(&meta) {
                tx.execute("INSERT INTO assets VALUES(?1,?2,?3) ON CONFLICT(uid) DO UPDATE SET payload=excluded.payload,bytes=excluded.bytes",params![uid,meta,&a.bytes[..]])?;
            }
            tx.execute("INSERT INTO live VALUES(?1)", [uid])?;
        }
        tx.execute(
            "DELETE FROM assets WHERE uid NOT IN (SELECT uid FROM live)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn load(&self) -> Result<Library> {
        let check: String = self
            .connection
            .query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        ensure!(check == "ok", "SQLite integrity check failed: {check}");
        let mut statement = self
            .connection
            .prepare("SELECT uid,kind,parent,payload FROM records ORDER BY ordinal")?;
        let rows = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let roots: Vec<_> = rows
            .iter()
            .filter(|r| r.1 == "library" && r.2.is_none())
            .collect();
        ensure!(roots.len() == 1, "Expected one library root");
        let root = roots[0];
        let v: serde_json::Value = serde_json::from_str(&root.3)?;
        fn title(v: &serde_json::Value) -> Result<String> {
            Ok(v["title"].as_str().context("Missing title")?.to_owned())
        }
        fn trash(v: &serde_json::Value) -> Result<bool> {
            v["trashed"].as_bool().context("Missing trash state")
        }
        let mut library = Library {
            uid: root.0.parse()?,
            title: title(&v)?,
            folders: vec![],
            assets: vec![],
        };
        let mut consumed = 1;
        for f in rows
            .iter()
            .filter(|r| r.1 == "folder" && r.2.as_ref() == Some(&root.0))
        {
            consumed += 1;
            let v = serde_json::from_str(&f.3)?;
            let mut folder = Folder {
                uid: f.0.parse()?,
                title: title(&v)?,
                trashed: trash(&v)?,
                notes: vec![],
            };
            for n in rows
                .iter()
                .filter(|r| r.1 == "note" && r.2.as_ref() == Some(&f.0))
            {
                consumed += 1;
                let v = serde_json::from_str(&n.3)?;
                let mut note = Note {
                    uid: n.0.parse()?,
                    title: title(&v)?,
                    trashed: trash(&v)?,
                    blocks: vec![],
                };
                for b in rows
                    .iter()
                    .filter(|r| r.1 == "block" && r.2.as_ref() == Some(&n.0))
                {
                    consumed += 1;
                    let block: Block = serde_json::from_str(&b.3)?;
                    ensure!(block.uid.to_string() == b.0, "Block identity mismatch");
                    note.blocks.push(block);
                }
                folder.notes.push(note);
            }
            library.folders.push(folder);
        }
        ensure!(
            consumed == rows.len(),
            "Orphaned or unknown library records"
        );
        let mut q = self
            .connection
            .prepare("SELECT uid,payload,bytes FROM assets")?;
        for r in q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Vec<u8>>(2)?,
            ))
        })? {
            let (id, json, bytes) = r?;
            let v: serde_json::Value = serde_json::from_str(&json)?;
            library.assets.push(Asset {
                uid: id.parse()?,
                name: v["name"].as_str().context("Missing asset name")?.into(),
                mime: v["mime"].as_str().context("Missing MIME")?.into(),
                digest: v["digest"].as_str().context("Missing digest")?.into(),
                bytes: bytes.into(),
            });
        }
        library.validate()?;
        library.verify_asset_digests()?;
        Ok(library)
    }
    pub fn backup(&self, destination: &Path) -> Result<()> {
        ensure!(!destination.exists(), "Backup destination already exists");
        self.connection
            .backup(rusqlite::MAIN_DB, destination, None)?;
        let backup =
            Connection::open_with_flags(destination, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let check: String = backup.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        ensure!(check == "ok", "Backup verification failed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_roundtrip_and_a_changed_blob_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("t.noton");
        let mut library = Library::empty("T".into());
        library.assets.push(Asset::new(
            "a.png".into(),
            "image/png".into(),
            vec![1, 2, 3],
        ));
        let mut file = LibraryFile::open(&path, true).unwrap();
        file.save(&library).unwrap();
        // The second save finds the image unchanged and leaves it alone.
        file.save(&library).unwrap();
        assert_eq!(file.load().unwrap(), library);
        assert!(LibraryFile::in_use(&path));
        file.connection
            .execute("UPDATE assets SET bytes = x'010204'", [])
            .unwrap();
        assert!(file.load().is_err(), "the digest no longer matches");
        drop(file);
        assert!(!LibraryFile::in_use(&path));
    }
}
