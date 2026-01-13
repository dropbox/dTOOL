//! Test-only shared helpers.
//!
//! These are only compiled for `cfg(test)` and provide cross-module coordination for tests that
//! manipulate global process state (e.g., current working directory).

use std::sync::{Mutex, OnceLock};

static CWD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub(crate) fn cwd_lock() -> &'static Mutex<()> {
    CWD_LOCK.get_or_init(|| Mutex::new(()))
}

