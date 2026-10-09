pub mod pool;

pub use pool::open_connection;

use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::error::{AppErrorDto, CmdResult};

/// Lock the shared connection. Keep the guard scope short and never hold it across
/// network calls or other long-running work.
pub fn lock(db: &Mutex<Connection>) -> CmdResult<MutexGuard<'_, Connection>> {
    db.lock().map_err(|_| {
        tracing::error!("database mutex poisoned");
        AppErrorDto::internal("Database lock poisoned")
    })
}

/// Run `f` with the connection locked for the duration of the call only.
pub fn with<T>(
    db: &Mutex<Connection>,
    f: impl FnOnce(&Connection) -> CmdResult<T>,
) -> CmdResult<T> {
    f(&*lock(db)?)
}
