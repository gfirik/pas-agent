use crate::context::{contains_block, MAX_FILES_IN_CONTEXT};
use crate::git::{get_changed_files, git_toplevel, head_commit};
use crate::session::{short_hash, Checkpoint, FileState, GitRef, Session};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// Largest file inspected when checking whether it is a generated context file.
const MAX_GENERATED_FILE_BYTES: u64 = 1024 * 1024;

/// How the working tree differs from the latest checkpoint.
#[derive(Debug, Clone, PartialEq)]
pub struct Staleness {
    pub checkpoint_time: DateTime<Utc>,
    /// `(checkpoint commit, current commit)` when HEAD moved since the checkpoint.
    pub commit_change: Option<(Option<String>, Option<String>)>,
    /// Files that are new, or whose status or content differs from the checkpoint.
    pub changed: Vec<FileState>,
    /// Paths the checkpoint listed that no longer have uncommitted changes (committed,
    /// reverted or stashed).
    pub resolved: Vec<String>,
}

/// Replaces control characters so a hostile filename cannot forge or overwrite
/// terminal output.
fn terminal_safe(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

impl Staleness {
    /// Number of files that are new or edited since the checkpoint.
    #[must_use]
    pub fn changed_count(&self) -> usize {
        self.changed.len()
    }

    /// One-line warning for the terminal.
    #[must_use]
    pub fn headline(&self) -> String {
        let when = self.checkpoint_time.format("%Y-%m-%d %H:%M UTC");
        let what = match (self.changed.len(), self.resolved.len()) {
            (1, _) => "1 file changed".to_string(),
            (n, _) if n > 1 => format!("{n} files changed"),
            _ if self.commit_change.is_some() => "HEAD moved".to_string(),
            (_, n) => format!(
                "{n} checkpointed file{} no longer changed",
                if n == 1 { "" } else { "s" }
            ),
        };
        format!(
            "⚠ {what} since the last checkpoint ({when}). Run `pas-agent checkpoint` to update."
        )
    }

    /// Short commit hashes of the `commit_change`, for display.
    #[must_use]
    pub fn commit_summary(&self) -> Option<String> {
        let (old, new) = self.commit_change.as_ref()?;
        let short = |c: &Option<String>| c.as_deref().map_or("no commits".to_string(), short_hash);
        Some(format!("HEAD {} → {}", short(old), short(new)))
    }

    /// `(path, note)` for the files that differ, capped at `limit`, plus how many were
    /// left out. The single source of truth for the terminal and the exported context.
    #[must_use]
    pub fn entries(&self, limit: usize) -> (Vec<(String, String)>, usize) {
        let all = self
            .changed
            .iter()
            .map(|f| (f.path.clone(), f.status.to_string()))
            .chain(
                self.resolved
                    .iter()
                    .map(|p| (p.clone(), "no longer changed".to_string())),
            );
        let total = all.clone().count();
        (all.take(limit).collect(), total.saturating_sub(limit))
    }

    /// Detail lines for the terminal (capped like the context file).
    #[must_use]
    pub fn detail_lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self.commit_summary().into_iter().collect();
        let (entries, hidden) = self.entries(MAX_FILES_IN_CONTEXT);
        lines.extend(
            entries
                .into_iter()
                .map(|(path, note)| terminal_safe(&format!("{path} ({note})"))),
        );
        if hidden > 0 {
            lines.push(format!("…and {hidden} more"));
        }
        lines
    }
}

/// Compares `checkpoint` with the current commit and changed files.
///
/// Returns `None` when they match. The commit is only compared when the checkpoint
/// recorded git state; fingerprints only when both sides have one.
#[must_use]
pub fn compare_with_checkpoint(
    checkpoint: &Checkpoint,
    current_commit: Option<&str>,
    current_files: &[FileState],
) -> Option<Staleness> {
    compare(
        checkpoint.timestamp,
        checkpoint.git.as_ref(),
        &checkpoint.files_changed,
        current_commit,
        current_files,
    )
}

fn compare(
    checkpoint_time: DateTime<Utc>,
    checkpoint_git: Option<&GitRef>,
    checkpoint_files: &[FileState],
    current_commit: Option<&str>,
    current_files: &[FileState],
) -> Option<Staleness> {
    let commit_change = checkpoint_git.and_then(|g| {
        (g.commit.as_deref() != current_commit)
            .then(|| (g.commit.clone(), current_commit.map(str::to_string)))
    });

    let before: HashMap<&str, &FileState> = checkpoint_files
        .iter()
        .map(|f| (f.path.as_str(), f))
        .collect();
    let changed: Vec<FileState> = current_files
        .iter()
        .filter(|now| {
            before.get(now.path.as_str()).is_none_or(|old| {
                old.status != now.status
                    || matches!((&old.fingerprint, &now.fingerprint), (Some(a), Some(b)) if a != b)
            })
        })
        .cloned()
        .collect();
    let now_paths: HashSet<&str> = current_files.iter().map(|f| f.path.as_str()).collect();
    let resolved: Vec<String> = checkpoint_files
        .iter()
        .filter(|f| !now_paths.contains(f.path.as_str()))
        .map(|f| f.path.clone())
        .collect();

    if commit_change.is_none() && changed.is_empty() && resolved.is_empty() {
        return None;
    }
    Some(Staleness {
        checkpoint_time,
        commit_change,
        changed,
        resolved,
    })
}

/// True for files that `pas-agent export` wrote (they hold a complete PAS-Agent block),
/// so exporting does not make the checkpoint look stale.
fn is_generated_context(root: &Path, path: &str) -> bool {
    let full = root.join(path);
    let Ok(meta) = fs::metadata(&full) else {
        return false;
    };
    meta.is_file()
        && meta.len() <= MAX_GENERATED_FILE_BYTES
        && fs::read_to_string(full).is_ok_and(|text| contains_block(&text))
}

/// Checks the working tree in `project_dir` against the session's latest checkpoint.
///
/// `None` means there is no checkpoint, the project is not a git work tree, or nothing
/// changed since the checkpoint.
#[must_use]
pub fn check_staleness(project_dir: &Path, session: &Session) -> Option<Staleness> {
    let checkpoint = session.latest_checkpoint()?;
    let commit = head_commit(project_dir)?;
    let current = get_changed_files(project_dir);

    // Each path is inspected once, across both sides of the comparison.
    let root = git_toplevel(project_dir).unwrap_or_else(|| project_dir.to_path_buf());
    let generated: HashSet<String> = current
        .iter()
        .chain(&checkpoint.files_changed)
        .map(|f| f.path.as_str())
        .collect::<HashSet<_>>()
        .into_iter()
        .filter(|p| is_generated_context(&root, p))
        .map(str::to_string)
        .collect();

    let current: Vec<FileState> = current
        .into_iter()
        .filter(|f| !generated.contains(f.path.as_str()))
        .collect();
    let before: Vec<FileState> = checkpoint
        .files_changed
        .iter()
        .filter(|f| !generated.contains(f.path.as_str()))
        .cloned()
        .collect();
    compare(
        checkpoint.timestamp,
        checkpoint.git.as_ref(),
        &before,
        commit.as_deref(),
        &current,
    )
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
            fingerprint: None,
        }
    }

    fn with_fp(mut f: FileState, fp: &str) -> FileState {
        f.fingerprint = Some(fp.into());
        f
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
        let files = vec![with_fp(file("a.rs", FileStatus::Modified), "1")];
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
        assert!(s.commit_change.is_none());
        assert!(s.headline().contains("2 files changed"));
    }

    #[test]
    fn further_edits_to_a_listed_file_are_stale() {
        let cp = checkpoint(
            "abc",
            vec![with_fp(file("a.rs", FileStatus::Modified), "1")],
        );
        let now = vec![with_fp(file("a.rs", FileStatus::Modified), "2")];
        let s = compare_with_checkpoint(&cp, Some("abc"), &now).unwrap();
        assert_eq!(s.changed.len(), 1);
    }

    #[test]
    fn missing_fingerprint_on_either_side_is_not_a_difference() {
        // Checkpoints from before fingerprints existed must not all turn stale.
        let cp = checkpoint("abc", vec![file("a.rs", FileStatus::Modified)]);
        let now = vec![with_fp(file("a.rs", FileStatus::Modified), "2")];
        assert!(compare_with_checkpoint(&cp, Some("abc"), &now).is_none());
    }

    #[test]
    fn commit_change_alone_is_stale() {
        let cp = checkpoint("abc", vec![]);
        let s = compare_with_checkpoint(&cp, Some("def"), &[]).unwrap();
        assert!(s.headline().contains("HEAD moved"));
        assert_eq!(s.commit_summary().unwrap(), "HEAD abc → def");
    }

    #[test]
    fn committing_checkpointed_files_reports_head_moved_not_a_file_count() {
        let cp = checkpoint(
            "abc",
            vec![
                file("a.rs", FileStatus::Modified),
                file("b.rs", FileStatus::Added),
            ],
        );
        let s = compare_with_checkpoint(&cp, Some("def"), &[]).unwrap();
        assert!(s.headline().contains("HEAD moved"), "{}", s.headline());
        assert_eq!(s.resolved.len(), 2);
    }

    #[test]
    fn checkpoint_without_git_ignores_commit() {
        let mut cp = checkpoint("abc", vec![]);
        cp.git = None;
        assert!(compare_with_checkpoint(&cp, Some("def"), &[]).is_none());
    }

    #[test]
    fn non_ascii_commit_in_hand_edited_session_does_not_panic() {
        let cp = checkpoint("ééééééééééé", vec![]);
        let s = compare_with_checkpoint(&cp, Some("def"), &[]).unwrap();
        assert_eq!(s.commit_summary().unwrap(), "HEAD éééééééé → def");
    }

    #[test]
    fn entries_are_capped_with_a_remainder() {
        let files: Vec<_> = (0..5)
            .map(|i| file(&format!("f{i}"), FileStatus::Untracked))
            .collect();
        let cp = checkpoint("abc", vec![]);
        let s = compare_with_checkpoint(&cp, Some("abc"), &files).unwrap();
        let (shown, hidden) = s.entries(3);
        assert_eq!((shown.len(), hidden), (3, 2));
    }

    #[test]
    fn terminal_output_has_control_characters_neutralised() {
        let files = vec![file("evil\x1b[2Kname\n.txt", FileStatus::Untracked)];
        let cp = checkpoint("abc", vec![]);
        let s = compare_with_checkpoint(&cp, Some("abc"), &files).unwrap();
        for line in s.detail_lines() {
            assert!(!line.chars().any(char::is_control), "{line:?}");
        }
    }
}
