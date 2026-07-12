use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct CommentSnapshot {
    pub comment_id: String,
    pub platform: String,
    pub owner: String,
    pub repo: String,
    pub pr_number: u64,
    pub commit_id: Option<String>,
    pub original_commit_id: Option<String>,
    pub diff_hunk: Option<String>,
    pub original_line: Option<u32>,
    pub original_start_line: Option<u32>,
}

pub struct CommentSnapshotStore {
    conn: Mutex<Connection>,
}

impl CommentSnapshotStore {
    pub fn new(db_path: &Path) -> Self {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(db_path).expect("Failed to open SQLite database");
        conn.execute_batch("PRAGMA journal_mode=WAL;").ok();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS comment_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                comment_id TEXT NOT NULL,
                platform TEXT NOT NULL,
                owner TEXT NOT NULL,
                repo TEXT NOT NULL,
                pr_number INTEGER NOT NULL,
                commit_id TEXT,
                original_commit_id TEXT,
                diff_hunk TEXT,
                original_line INTEGER,
                original_start_line INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(comment_id, platform)
            );",
        )
        .expect("Failed to create comment_snapshots table");
        Self {
            conn: Mutex::new(conn),
        }
    }

    pub fn save_snapshot(&self, snapshot: &CommentSnapshot) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO comment_snapshots
                (comment_id, platform, owner, repo, pr_number, commit_id, original_commit_id, diff_hunk, original_line, original_start_line)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                snapshot.comment_id,
                snapshot.platform,
                snapshot.owner,
                snapshot.repo,
                snapshot.pr_number,
                snapshot.commit_id,
                snapshot.original_commit_id,
                snapshot.diff_hunk,
                snapshot.original_line,
                snapshot.original_start_line,
            ],
        )?;
        Ok(())
    }

    pub fn get_snapshot(
        &self,
        comment_id: &str,
        platform: &str,
    ) -> Result<Option<CommentSnapshot>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT comment_id, platform, owner, repo, pr_number, commit_id, original_commit_id, diff_hunk, original_line, original_start_line
             FROM comment_snapshots
             WHERE comment_id = ?1 AND platform = ?2"
        )?;
        let mut rows = stmt.query(params![comment_id, platform])?;
        if let Some(row) = rows.next()? {
            Ok(Some(CommentSnapshot {
                comment_id: row.get(0)?,
                platform: row.get(1)?,
                owner: row.get(2)?,
                repo: row.get(3)?,
                pr_number: row.get::<_, i64>(4)? as u64,
                commit_id: row.get(5)?,
                original_commit_id: row.get(6)?,
                diff_hunk: row.get(7)?,
                original_line: row.get::<_, Option<i64>>(8)?.map(|v| v as u32),
                original_start_line: row.get::<_, Option<i64>>(9)?.map(|v| v as u32),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_snapshots_for_pr(
        &self,
        platform: &str,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<CommentSnapshot>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT comment_id, platform, owner, repo, pr_number, commit_id, original_commit_id, diff_hunk, original_line, original_start_line
             FROM comment_snapshots
             WHERE platform = ?1 AND owner = ?2 AND repo = ?3 AND pr_number = ?4"
        )?;
        let rows = stmt.query_map(params![platform, owner, repo, pr_number as i64], |row| {
            Ok(CommentSnapshot {
                comment_id: row.get(0)?,
                platform: row.get(1)?,
                owner: row.get(2)?,
                repo: row.get(3)?,
                pr_number: row.get::<_, i64>(4)? as u64,
                commit_id: row.get(5)?,
                original_commit_id: row.get(6)?,
                diff_hunk: row.get(7)?,
                original_line: row.get::<_, Option<i64>>(8)?.map(|v| v as u32),
                original_start_line: row.get::<_, Option<i64>>(9)?.map(|v| v as u32),
            })
        })?;
        let mut snapshots = Vec::new();
        for row in rows {
            snapshots.push(row?);
        }
        Ok(snapshots)
    }
}
