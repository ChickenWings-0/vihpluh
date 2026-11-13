//! The one SQLite connection (ARCHITECTURE §4.1 "db" thread, §9.6). All SQL runs on its own
//! thread; callers either wait for a result (`call`) or fire and forget (`spawn`).

use std::path::Path;
use std::sync::mpsc::{self, Sender};

use rusqlite::Connection;

/// Numbered migrations, applied in order; `PRAGMA user_version` records the last one applied.
const MIGRATIONS: &[(&str, &str)] = &[("0001_init", include_str!("migrations/0001_init.sql"))];

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("the database thread has stopped")]
    Gone,
    #[error("could not start the database thread: {0}")]
    Thread(std::io::Error),
}

impl From<DbError> for crate::error::AppError {
    fn from(err: DbError) -> Self {
        crate::error::AppError::new(crate::error::ErrorCode::Db, err.to_string())
    }
}

type Job = Box<dyn FnOnce(&mut Connection) + Send>;

pub struct Db {
    tx: Sender<Job>,
}

impl Db {
    /// Opens (creating if needed) the database at `path` and brings its schema up to date.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(path)?;
        Self::start(conn)
    }

    /// An in-memory database (tests).
    pub fn in_memory() -> Result<Self, DbError> {
        Self::start(Connection::open_in_memory()?)
    }

    fn start(mut conn: Connection) -> Result<Self, DbError> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        migrate(&mut conn)?;
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("db".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    job(&mut conn);
                }
                // All senders gone: fold the WAL back into the main file before closing.
                let _ = conn.pragma_update(None, "wal_checkpoint", "TRUNCATE");
            })
            .map_err(DbError::Thread)?;
        Ok(Self { tx })
    }

    /// Runs `f` on the database thread and waits for its result.
    pub fn call<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Connection) -> rusqlite::Result<T> + Send + 'static,
    ) -> Result<T, DbError> {
        let (reply, result) = mpsc::channel();
        self.tx
            .send(Box::new(move |conn| {
                let _ = reply.send(f(conn));
            }))
            .map_err(|_| DbError::Gone)?;
        Ok(result.recv().map_err(|_| DbError::Gone)??)
    }

    /// Runs `f` on the database thread without waiting. Errors are logged.
    pub fn spawn(
        &self,
        what: &'static str,
        f: impl FnOnce(&mut Connection) -> rusqlite::Result<()> + Send + 'static,
    ) {
        let job: Job = Box::new(move |conn| {
            if let Err(err) = f(conn) {
                tracing::warn!("database: {what} failed: {err}");
            }
        });
        if self.tx.send(job).is_err() {
            tracing::debug!("database: {what} dropped (thread stopped)");
        }
    }
}

fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let applied: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    for (index, (name, sql)) in MIGRATIONS.iter().enumerate() {
        let number = index as i64 + 1;
        if number <= applied {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", number)?;
        tx.commit()?;
        tracing::info!("database: applied migration {name}");
    }
    Ok(())
}

pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_create_the_documented_tables_once() {
        let dir = std::env::temp_dir().join(format!("vihpluh-db-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("vihpluh.db");
        {
            let db = Db::open(&path).unwrap();
            let tables: Vec<String> = db
                .call(|c| {
                    let mut s = c.prepare(
                        "SELECT name FROM sqlite_master WHERE type IN ('table') ORDER BY name",
                    )?;
                    let names =
                        s.query_map([], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
                    Ok(names)
                })
                .unwrap();
            for t in [
                "bookmarks",
                "folders",
                "history",
                "media",
                "media_fts",
                "stream_sources",
                "streams",
                "sub_links",
            ] {
                assert!(tables.iter().any(|n| n == t), "{t} missing from {tables:?}");
            }
            let version: i64 =
                db.call(|c| c.pragma_query_value(None, "user_version", |r| r.get(0))).unwrap();
            assert_eq!(version, MIGRATIONS.len() as i64);
        }
        // Re-opening applies nothing new and keeps the data.
        let db = Db::open(&path).unwrap();
        let version: i64 =
            db.call(|c| c.pragma_query_value(None, "user_version", |r| r.get(0))).unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn spawned_jobs_run_in_order_before_a_later_call() {
        let db = Db::in_memory().unwrap();
        db.spawn("create", |c| c.execute_batch("CREATE TABLE t (n INTEGER)"));
        for n in 0..5 {
            db.spawn("insert", move |c| c.execute("INSERT INTO t VALUES (?1)", [n]).map(|_| ()));
        }
        let sum: i64 = db.call(|c| c.query_row("SELECT SUM(n) FROM t", [], |r| r.get(0))).unwrap();
        assert_eq!(sum, 10);
    }
}
