//! Core session model, storage and context export for PAS-Agent.

mod context;
mod git;
mod session;
mod stale;
mod storage;

pub use context::{
    generate_context, generate_context_with, write_context_file, ContextFormat, WriteOutcome,
    MAX_CHECKPOINTS_IN_CONTEXT, MAX_FILES_IN_CONTEXT,
};
pub use git::{capture_git_state, get_changed_files, git_toplevel, head_commit};
pub use session::{
    short_hash, Agent, Checkpoint, FileState, FileStatus, GitRef, Session, TaskState,
    SCHEMA_VERSION,
};
pub use stale::{check_staleness, compare_with_checkpoint, Staleness};
pub use storage::{SessionError, SessionStore, STORE_DIR};
