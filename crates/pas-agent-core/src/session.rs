use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Version of the on-disk `session.json` format written by this build.
///
/// Files without a `schema_version` field predate versioning and are read as version 1.
pub const SCHEMA_VERSION: u32 = 1;

fn default_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub id: Uuid,
    pub project_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub task: TaskState,
    #[serde(default)]
    pub git: Option<GitRef>,
    #[serde(default)]
    pub checkpoints: Vec<Checkpoint>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TaskState {
    pub description: String,
    pub objective: String,
    pub completed: Vec<String>,
    pub remaining: Vec<String>,
    pub next_action: Option<String>,
    pub blockers: Vec<String>,
    pub decisions: Vec<String>,
    pub constraints: Vec<String>,
    pub discoveries: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GitRef {
    /// Current branch, or `None` when HEAD is detached.
    pub branch: Option<String>,
    /// Full commit hash, or `None` when the repository has no commits yet.
    pub commit: Option<String>,
    /// Whether the working tree has uncommitted changes (ignoring `.pas-agent/`).
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Checkpoint {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub message: String,
    pub task_snapshot: TaskState,
    #[serde(default)]
    pub git: Option<GitRef>,
    /// Files with uncommitted changes when the checkpoint was taken.
    #[serde(default)]
    pub files_changed: Vec<FileState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileState {
    pub path: String,
    pub status: FileStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed { from: String },
    Untracked,
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileStatus::Added => write!(f, "added"),
            FileStatus::Modified => write!(f, "modified"),
            FileStatus::Deleted => write!(f, "deleted"),
            FileStatus::Renamed { from } => write!(f, "renamed from {from}"),
            FileStatus::Untracked => write!(f, "untracked"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Agent {
    ClaudeCode,
    Codex,
    OpenCode,
    KiroCli,
    Antigravity,
    GeminiCli,
    Cursor,
}

impl std::fmt::Display for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Agent::ClaudeCode => write!(f, "claude-code"),
            Agent::Codex => write!(f, "codex"),
            Agent::OpenCode => write!(f, "opencode"),
            Agent::KiroCli => write!(f, "kiro-cli"),
            Agent::Antigravity => write!(f, "antigravity"),
            Agent::GeminiCli => write!(f, "gemini-cli"),
            Agent::Cursor => write!(f, "cursor"),
        }
    }
}

impl Session {
    #[must_use]
    pub fn new(project_name: String, description: String) -> Self {
        let now = Utc::now();
        Self {
            schema_version: SCHEMA_VERSION,
            id: Uuid::new_v4(),
            project_name,
            created_at: now,
            updated_at: now,
            task: TaskState {
                description,
                ..TaskState::default()
            },
            git: None,
            checkpoints: Vec::new(),
        }
    }

    pub fn add_checkpoint(&mut self, message: String, files_changed: Vec<FileState>) {
        let now = Utc::now();
        self.checkpoints.push(Checkpoint {
            id: Uuid::new_v4(),
            timestamp: now,
            message,
            task_snapshot: self.task.clone(),
            git: self.git.clone(),
            files_changed,
        });
        self.updated_at = now;
    }

    #[must_use]
    pub fn latest_checkpoint(&self) -> Option<&Checkpoint> {
        self.checkpoints.last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = Session::new("test-project".into(), "Fix the bug".into());
        assert_eq!(session.schema_version, SCHEMA_VERSION);
        assert_eq!(session.project_name, "test-project");
        assert_eq!(session.task.description, "Fix the bug");
        assert!(session.checkpoints.is_empty());
    }

    #[test]
    fn test_checkpoint_addition() {
        let mut session = Session::new("test".into(), "task".into());
        session.add_checkpoint("step 1 done".into(), vec![]);
        assert_eq!(session.checkpoints.len(), 1);
        assert_eq!(session.checkpoints[0].message, "step 1 done");
        assert_eq!(session.latest_checkpoint().unwrap().message, "step 1 done");
    }

    #[test]
    fn test_agent_display() {
        assert_eq!(Agent::ClaudeCode.to_string(), "claude-code");
        assert_eq!(Agent::Codex.to_string(), "codex");
        assert_eq!(Agent::OpenCode.to_string(), "opencode");
        assert_eq!(Agent::GeminiCli.to_string(), "gemini-cli");
        assert_eq!(Agent::Cursor.to_string(), "cursor");
    }

    #[test]
    fn test_task_state_clone() {
        let mut task = TaskState {
            description: "test".into(),
            completed: vec!["done".into()],
            ..TaskState::default()
        };
        let cloned = task.clone();
        assert_eq!(task, cloned);
        task.completed.push("another".into());
        assert_ne!(task, cloned);
    }

    #[test]
    fn test_legacy_session_without_new_fields_loads() {
        // Shape written by v0.1.0 before schema versioning existed.
        let legacy = r#"{
            "id": "2c36c558-d3ae-486d-904e-853b4df9124c",
            "project_name": "p",
            "created_at": "2026-09-17T19:03:00Z",
            "updated_at": "2026-09-17T19:03:00Z",
            "task": { "description": "d" },
            "git": null,
            "checkpoints": []
        }"#;
        let session: Session = serde_json::from_str(legacy).unwrap();
        assert_eq!(session.schema_version, 1);
        assert_eq!(session.task.description, "d");
        assert!(session.task.completed.is_empty());
    }
}
