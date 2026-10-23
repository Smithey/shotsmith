use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("{0}")]
    Message(String),
}

impl From<rusqlite::Error> for HistoryError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Message(value.to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRecord {
    pub id: String,
    pub date: String,
    pub mode: String,
    pub source_app: Option<String>,
    pub window_title: Option<String>,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub scale: f32,
    pub original_path: String,
    pub document_path: Option<String>,
    pub pinned: bool,
    /// Flattened thumbnail written beside the original. Not stored in the database.
    #[serde(default)]
    pub preview_path: Option<String>,
    #[serde(default)]
    pub preview_rev: u64,
}

pub struct HistoryDb {
    conn: Connection,
}

impl HistoryDb {
    pub fn open(path: &Path) -> Result<Self, HistoryError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| HistoryError::Message(err.to_string()))?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS captures (
                id TEXT PRIMARY KEY,
                date TEXT NOT NULL,
                mode TEXT NOT NULL,
                source_app TEXT,
                window_title TEXT,
                pixel_width INTEGER NOT NULL,
                pixel_height INTEGER NOT NULL,
                scale REAL NOT NULL,
                original_path TEXT NOT NULL,
                document_path TEXT,
                pinned INTEGER NOT NULL DEFAULT 0
            );",
        )?;
        Ok(Self { conn })
    }

    pub fn insert(&self, record: &CaptureRecord) -> Result<(), HistoryError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO captures
            (id, date, mode, source_app, window_title, pixel_width, pixel_height, scale, original_path, document_path, pinned)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                record.id,
                record.date,
                record.mode,
                record.source_app,
                record.window_title,
                record.pixel_width,
                record.pixel_height,
                record.scale,
                record.original_path,
                record.document_path,
                record.pinned as i32,
            ],
        )?;
        Ok(())
    }

    pub fn list(&self, limit: u32) -> Result<Vec<CaptureRecord>, HistoryError> {
        self.query(
            "SELECT id, date, mode, source_app, window_title, pixel_width, pixel_height, scale, original_path, document_path, pinned
             FROM captures ORDER BY date DESC LIMIT ?1",
            [limit],
        )
    }

    pub fn search(&self, query: &str) -> Result<Vec<CaptureRecord>, HistoryError> {
        let like = format!("%{}%", query.replace('%', ""));
        let mut stmt = self.conn.prepare(
            "SELECT id, date, mode, source_app, window_title, pixel_width, pixel_height, scale, original_path, document_path, pinned
             FROM captures
             WHERE source_app LIKE ?1 OR window_title LIKE ?1 OR date LIKE ?1
             ORDER BY date DESC LIMIT 200",
        )?;
        let rows = stmt.query_map(params![like], row_to_record)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get(&self, id: &str) -> Result<Option<CaptureRecord>, HistoryError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, date, mode, source_app, window_title, pixel_width, pixel_height, scale, original_path, document_path, pinned
             FROM captures WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_record(&row)?))
        } else {
            Ok(None)
        }
    }

    pub fn delete(&self, id: &str) -> Result<Option<CaptureRecord>, HistoryError> {
        let existing = self.get(id)?;
        self.conn.execute("DELETE FROM captures WHERE id = ?1", params![id])?;
        Ok(existing)
    }

    pub fn set_pinned(&self, id: &str, pinned: bool) -> Result<(), HistoryError> {
        self.conn.execute(
            "UPDATE captures SET pinned = ?1 WHERE id = ?2",
            params![pinned as i32, id],
        )?;
        Ok(())
    }

    pub fn set_document(&self, id: &str, path: &str) -> Result<(), HistoryError> {
        self.conn.execute(
            "UPDATE captures SET document_path = ?1 WHERE id = ?2",
            params![path, id],
        )?;
        Ok(())
    }

    pub fn prune(&self, keep: u32, days: u32) -> Result<Vec<CaptureRecord>, HistoryError> {
        let mut doomed = Vec::new();
        if keep > 0 {
            let extra = self.query(
                "SELECT id, date, mode, source_app, window_title, pixel_width, pixel_height, scale, original_path, document_path, pinned
                 FROM captures WHERE pinned = 0 ORDER BY date DESC LIMIT -1 OFFSET ?1",
                [keep],
            )?;
            doomed.extend(extra);
        }
        if days > 0 {
            let old = self.conn.prepare(
                "SELECT id, date, mode, source_app, window_title, pixel_width, pixel_height, scale, original_path, document_path, pinned
                 FROM captures WHERE pinned = 0 AND julianday(date) < julianday('now') - ?1",
            );
            if let Ok(mut stmt) = old {
                let rows = stmt.query_map(params![days], row_to_record)?;
                for row in rows.flatten() {
                    if !doomed.iter().any(|item| item.id == row.id) {
                        doomed.push(row);
                    }
                }
            }
        }
        for record in &doomed {
            self.conn.execute("DELETE FROM captures WHERE id = ?1", params![record.id])?;
        }
        Ok(doomed)
    }

    fn query(&self, sql: &str, limit: [u32; 1]) -> Result<Vec<CaptureRecord>, HistoryError> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![limit[0]], row_to_record)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<CaptureRecord> {
    Ok(CaptureRecord {
        id: row.get(0)?,
        date: row.get(1)?,
        mode: row.get(2)?,
        source_app: row.get(3)?,
        window_title: row.get(4)?,
        pixel_width: row.get(5)?,
        pixel_height: row.get(6)?,
        scale: row.get(7)?,
        original_path: row.get(8)?,
        document_path: row.get(9)?,
        pinned: row.get::<_, i32>(10)? != 0,
        preview_path: None,
        preview_rev: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_search_and_pin() {
        let dir = std::env::temp_dir().join(format!("shotsmith-history-{}", uuid_ish()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = HistoryDb::open(&dir.join("history.db")).unwrap();
        db.insert(&sample("1", "Safari", "Inbox")).unwrap();
        db.insert(&sample("2", "Mail", "Draft")).unwrap();
        assert_eq!(db.search("Safari").unwrap().len(), 1);
        db.set_pinned("1", true).unwrap();
        assert!(db.get("1").unwrap().unwrap().pinned);
        let _ = std::fs::remove_dir_all(dir);
    }

    fn sample(id: &str, app: &str, window: &str) -> CaptureRecord {
        CaptureRecord {
            id: id.into(),
            date: "2026-10-19T09:41:12Z".into(),
            mode: "region".into(),
            source_app: Some(app.into()),
            window_title: Some(window.into()),
            pixel_width: 100,
            pixel_height: 80,
            scale: 2.0,
            original_path: format!("/tmp/{id}.png"),
            document_path: None,
            pinned: false,
            preview_path: None,
            preview_rev: 0,
        }
    }

    fn uuid_ish() -> u32 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    }
}
