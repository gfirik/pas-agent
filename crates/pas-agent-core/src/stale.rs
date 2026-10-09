use crate::context::{BLOCK_START_PREFIX, MAX_FILES_IN_CONTEXT};
use crate::git::{capture_git_state, get_changed_files};
use crate::session::{Checkpoint, FileState, Session};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Largest file inspected when checking whether it is a generated context file.
const MAX_GENERATED_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// How the working tree differs from the latest checkpoint.
#[derive(Debug, Clone, PartialEq)]
pub struct Staleness {
    pub checkpoint_time: DateTime<Utc>,
    /// `(checkpoint commit, current commit)` when HEAD moved since the checkpoint.
    pub commit_change: Option<(Option<String>, Option<String>)>,
    /// Files that are new, or whose status differs from the checkpoint.
    pub changed: Vec<FileState>,
    /// Paths the checkpoint listed that no longer have uncommitted changes.
    pub resolved: Vec<String>,
}

impl Staleness {
    /// Number of files that differ from the checkpoint.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.changed.len() + self.resolved.len()
    }

    /// One-line warning for the terminal.
    #[must_use]
    pub fn headline(&self) -> String {
        let when = self.checkpoint_time.format("%Y-%m-%d %H:%M UTC");
        let n = self.file_count();
        let what = match (n, &self.commit_change) {
            (0, _) => "HEAD moved".to_string(),
            (1, _) => "1 file changed".to_string(),
            (n, _) => format!("{n} files changed"),
        };
        format!(
            "⚠ {what} since the last checkpoint ({when}). Run `pas-agent checkpoint` to update."
        )
    }

    /// Short commit hashes of the `commit_change`, for display.
    #[must_use]
    pub fn commit_summary(&self) -> Option<String> {
        let (old, new) = self.commit_change.as_ref()?;
        let short = |c: &Option<String>| {
            c.as_deref().map_or("no commits".to_string(), |c| {
                c[..c.len().min(8)].to_string()
            })
        };
        Some(format!("HEAD {} → {}", short(old), short(new)))
    }

    /// Detail lines for the terminal (capped like the context file).
    #[must_use]
    pub fn detail_lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self.commit_summary().into_iter().collect();
        let entries = self
            .changed
            .iter()
            .map(|f| format!("{} ({})", f.path, f.status))
            .chain(
                self.resolved
                    .iter()
                    .map(|p| format!("{p} (no longer changed)")),
            );
        lines.extend(entries.clone().take(MAX_FILES_IN_CONTEXT));
        let hidden = entries.count().saturating_sub(MAX_FILES_IN_CONTEXT);
        if hidden > 0 {
            lines.push(format!("…and {hidden} more"));
        }
        lines
    }
}

/// Compares `checkpoint` with the current commit and changed files.
///
/// Returns `None` when they match. The commit is only compared when the checkpoint
/// recorded git state.
#[must_use]
pub fn compare_with_checkpoint(
    checkpoint: &Checkpoint,
    current_commit: Option<&str>,
    current_files: &[FileState],
) -> Option<Staleness> {
    let commit_change = checkpoint.git.as_ref().and_then(|g| {
        (g.commit.as_deref() != current_commit)
            .then(|| (g.commit.clone(), current_commit.map(str::to_string)))
    });

    let before: HashMap<&str, &FileState> = checkpoint
        .files_changed
        .iter()
        .map(|f| (f.path.as_str(), f))
        .collect();
    let changed: Vec<FileState> = current_files
        .iter()
        .filter(|f| {
            before
                .get(f.path.as_str())
                .is_none_or(|old| old.status != f.status)
        })
        .cloned()
        .collect();
    let resolved: Vec<String> = checkpoint
        .files_changed
        .iter()
        .filter(|f| !current_files.iter().any(|c| c.path == f.path))
        .map(|f| f.path.clone())
        .collect();

    if commit_change.is_none() && changed.is_empty() && resolved.is_empty() {
        return None;
    }
    Some(Staleness {
        checkpoint_time: checkpoint.timestamp,
        commit_change,
        changed,
        resolved,
    })
}

/// True for files that `pas-agent export` wrote, so exporting does not make the
/// checkpoint look stale.
fn is_generated_context(project_dir: &Path, path: &str) -> bool {
    let full = project_dir.join(path);
    let Ok(meta) = fs::metadata(&full) else {
        return false;
    };
    meta.is_file()
        && meta.len() <= MAX_GENERATED_FILE_BYTES
        && fs::read_to_string(full).is_ok_and(|t| t.contains(BLOCK_START_PREFIX))
}

/// Checks the working tree in `project_dir` against the session's latest checkpoint.
///
/// `None` means there is no checkpoint, the project is not a git work tree, or nothing
/// changed since the checkpoint.
#[must_use]
pub fn check_staleness(project_dir: &Path, session: &Session) -> Option<Staleness> {
    let checkpoint = session.latest_checkpoint()?;
    let git = capture_git_state(project_dir)?;
    let mut current = get_changed_files(project_dir);
    current.retain(|f| !is_generated_context(project_dir, &f.path));

    let mut baseline = checkpoint.clone();
    baseline
        .files_changed
        .retain(|f| !is_generated_context(project_dir, &f.path));
    compare_with_checkpoint(&baseline, git.commit.as_deref(), &current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{FileStatus, GitRef};
    use uuid::Uuid;

    fn file(path: &str, status: FileStatus) -> FileState {
        FileState {
            path: path.into(),
            status,
        }
    }

    fn checkpoint(commit: &str, files: Vec<FileState>) -> Checkpoint {
        Checkpoint {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            message: "m".into(),
            task_snapshot: Default::default(),
            git: Some(GitRef {
                branch: Some("main".into()),
                commit: Some(commit.into()),
                dirty: !files.is_empty(),
            }),
            files_changed: files,
        }
    }

    #[test]
    fn identical_state_is_fresh() {
        let files = vec![file("a.rs", FileStatus::Modified)];
        let cp = checkpoint("abc", files.clone());
        assert!(compare_with_checkpoint(&cp, Some("abc"), &files).is_none());
    }

    #[test]
    fn new_removed_and_restatused_files_are_stale() {
        let cp = checkpoint(
            "abc",
            vec![
                file("a.rs", FileStatus::Modified),
                file("gone.rs", FileStatus::Modified),
                file("b.rs", FileStatus::Added),
            ],
        );
        let now = vec![
            file("a.rs", FileStatus::Modified),
            file("b.rs", FileStatus::Modified),
            file("new.rs", FileStatus::Untracked),
        ];
        let s = compare_with_checkpoint(&cp, Some("abc"), &now).unwrap();
        assert_eq!(s.changed.len(), 2);
        assert_eq!(s.resolved, vec!["gone.rs".to_string()]);
        assert_eq!(s.file_count(), 3);
        assert!(s.commit_change.is_none());
    }

    #[test]
    fn commit_change_alone_is_stale() {
        let cp = checkpoint("abc", vec![]);
        let s = compare_with_checkpoint(&cp, Some("def"), &[]).unwrap();
        assert_eq!(s.file_count(), 0);
        assert!(s.headline().contains("HEAD moved"));
        assert_eq!(s.commit_summary().unwrap(), "HEAD abc → def");
    }

    #[test]
    fn checkpoint_without_git_ignores_commit() {
        let mut cp = checkpoint("abc", vec![]);
        cp.git = None;
        assert!(compare_with_checkpoint(&cp, Some("def"), &[]).is_none());
    }
}
