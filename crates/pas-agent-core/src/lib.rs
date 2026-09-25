//! Core session model, storage and context export for PAS-Agent.

mod context;
mod git;
mod session;
mod storage;

pub use context::{
    generate_context, write_context_file, ContextFormat, WriteOutcome, MAX_CHECKPOINTS_IN_CONTEXT,
    MAX_FILES_IN_CONTEXT,
};
pub use git::{capture_git_state, get_changed_files, git_toplevel};
pub use session::{
    Agent, Checkpoint, FileState, FileStatus, GitRef, Session, TaskState, SCHEMA_VERSION,
};
pub use storage::{SessionError, SessionStore, STORE_DIR};
