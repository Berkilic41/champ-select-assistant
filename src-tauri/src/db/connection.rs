use rusqlite::Connection;
use std::path::Path;

pub fn open_db(path: &Path) -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

pub fn run_migrations(conn: &mut Connection) -> anyhow::Result<()> {
    super::migrations::runner()
        .run(conn)
        .map_err(|e| anyhow::anyhow!("Migration failed: {e}"))?;
    Ok(())
}
