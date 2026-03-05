use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use crate::application::ports::{
    BookmarkRepository, HealthRepository, ProgressRepository, SyncLogRepository,
};
use crate::domain::{AyahRef, Bookmark, BookmarkId, StudyProgress};

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Gagal membuka database {}", path.display()))?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS bookmarks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                surah_no INTEGER NOT NULL,
                ayah_no INTEGER NOT NULL,
                note TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS study_progress (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                last_surah_no INTEGER NOT NULL,
                last_ayah_no INTEGER NOT NULL,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS sync_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                status TEXT NOT NULL,
                message TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )?;
        Ok(())
    }

    pub fn add_bookmark(&self, target: AyahRef, note: Option<&str>) -> Result<BookmarkId> {
        self.conn.execute(
            "INSERT INTO bookmarks (surah_no, ayah_no, note) VALUES (?1, ?2, ?3)",
            params![target.surah().value(), target.ayah().value(), note],
        )?;
        Ok(BookmarkId::new(self.conn.last_insert_rowid())?)
    }

    pub fn list_bookmarks(&self) -> Result<Vec<Bookmark>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, surah_no, ayah_no, note, created_at FROM bookmarks ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Bookmark {
                id: row.get(0)?,
                surah_no: row.get(1)?,
                ayah_no: row.get(2)?,
                note: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn remove_bookmark(&self, bookmark_id: BookmarkId) -> Result<usize> {
        let rows = self.conn.execute(
            "DELETE FROM bookmarks WHERE id = ?1",
            params![bookmark_id.value()],
        )?;
        Ok(rows)
    }

    pub fn set_progress(&self, target: AyahRef) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO study_progress (id, last_surah_no, last_ayah_no, updated_at)
            VALUES (1, ?1, ?2, CURRENT_TIMESTAMP)
            ON CONFLICT(id)
            DO UPDATE SET
                last_surah_no = excluded.last_surah_no,
                last_ayah_no = excluded.last_ayah_no,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![target.surah().value(), target.ayah().value()],
        )?;
        Ok(())
    }

    pub fn get_progress(&self) -> Result<Option<StudyProgress>> {
        self.conn
            .query_row(
                "SELECT last_surah_no, last_ayah_no FROM study_progress WHERE id = 1",
                [],
                |row| {
                    Ok(StudyProgress {
                        last_surah_no: row.get(0)?,
                        last_ayah_no: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn bookmark_count(&self) -> Result<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn record_sync(&self, status: &str, message: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sync_log (status, message) VALUES (?1, ?2)",
            params![status, message],
        )?;
        Ok(())
    }
}

impl BookmarkRepository for Database {
    fn add_bookmark(&self, target: AyahRef, note: Option<&str>) -> Result<BookmarkId> {
        Database::add_bookmark(self, target, note)
    }

    fn list_bookmarks(&self) -> Result<Vec<Bookmark>> {
        Database::list_bookmarks(self)
    }

    fn remove_bookmark(&self, bookmark_id: BookmarkId) -> Result<usize> {
        Database::remove_bookmark(self, bookmark_id)
    }
}

impl ProgressRepository for Database {
    fn set_progress(&self, target: AyahRef) -> Result<()> {
        Database::set_progress(self, target)
    }

    fn get_progress(&self) -> Result<Option<StudyProgress>> {
        Database::get_progress(self)
    }
}

impl HealthRepository for Database {
    fn bookmark_count(&self) -> Result<i64> {
        Database::bookmark_count(self)
    }
}

impl SyncLogRepository for Database {
    fn record_sync(&self, status: &str, message: &str) -> Result<()> {
        Database::record_sync(self, status, message)
    }
}
