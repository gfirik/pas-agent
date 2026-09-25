use crate::session::{FileState, FileStatus, GitRef};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pathspec that keeps PAS-Agent's own state out of dirty checks and change lists.
const EXCLUDE_STORE: &str = ":(exclude).pas-agent";

/// Captures branch, commit and dirty state, or `None` outside a git work tree.
#[must_use]
pub fn capture_git_state(project_dir: &Path) -> Option<GitRef> {
    if !is_git_repo(project_dir) {
        return None;
    }

    // `symbolic-ref` works on an unborn branch and fails (→ None) on a detached HEAD.
    let branch = run_git(project_dir, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // `--verify` fails (→ None) when the repository has no commits yet.
    let commit = run_git(project_dir, &["rev-parse", "--verify", "--quiet", "HEAD"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let dirty = run_git(
        project_dir,
        &["status", "--porcelain", "--", ".", EXCLUDE_STORE],
    )
    .is_some_and(|s| !s.trim().is_empty());

    Some(GitRef {
        branch,
        commit,
        dirty,
    })
}

/// Lists files with uncommitted changes (staged, unstaged and untracked).
#[must_use]
pub fn get_changed_files(project_dir: &Path) -> Vec<FileState> {
    run_git(
        project_dir,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            ".",
            EXCLUDE_STORE,
        ],
    )
    .map(|output| parse_porcelain_z(&output))
    .unwrap_or_default()
}

/// Returns the top-level directory of the git work tree containing `dir`.
#[must_use]
pub fn git_toplevel(dir: &Path) -> Option<PathBuf> {
    run_git(dir, &["rev-parse", "--show-toplevel"])
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| !p.as_os_str().is_empty())
}

/// Parses `git status --porcelain=v1 -z` output.
///
/// Entries are `XY <path>\0`; renames and copies are followed by an extra `<orig-path>\0`.
fn parse_porcelain_z(output: &str) -> Vec<FileState> {
    let mut files = Vec::new();
    let mut entries = output.split('\0').filter(|e| !e.is_empty());

    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        let (code, path) = entry.split_at(3);
        let mut code = code.chars();
        let x = code.next().unwrap_or(' ');
        let y = code.next().unwrap_or(' ');

        let status = if x == '?' {
            FileStatus::Untracked
        } else if x == 'R' || y == 'R' {
            let from = entries.next().unwrap_or_default().to_string();
            FileStatus::Renamed { from }
        } else if x == 'C' || y == 'C' {
            entries.next();
            FileStatus::Added
        } else if x == 'D' || y == 'D' {
            FileStatus::Deleted
        } else if x == 'A' {
            FileStatus::Added
        } else {
            FileStatus::Modified
        };

        files.push(FileState {
            path: path.to_string(),
            status,
        });
    }

    files
}

fn is_git_repo(dir: &Path) -> bool {
    run_git(dir, &["rev-parse", "--is-inside-work-tree"]).is_some_and(|s| s.trim() == "true")
}

/// Runs git and returns stdout, or `None` if git is missing or exits non-zero.
fn run_git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap()
            .status;
        assert!(status.success(), "git {args:?} failed");
    }

    #[test]
    fn test_non_git_repo() {
        let dir = TempDir::new().unwrap();
        assert!(capture_git_state(dir.path()).is_none());
    }

    #[test]
    fn test_unborn_branch_has_branch_but_no_commit() {
        let dir = TempDir::new().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        let state = capture_git_state(dir.path()).unwrap();
        assert_eq!(state.branch.as_deref(), Some("main"));
        assert_eq!(state.commit, None);
        assert!(!state.dirty);
    }

    #[test]
    fn test_detached_head_has_no_branch() {
        let dir = TempDir::new().unwrap();
        git(dir.path(), &["init", "-q"]);
        git(dir.path(), &["commit", "-q", "--allow-empty", "-m", "c"]);
        git(dir.path(), &["checkout", "-q", "--detach"]);
        let state = capture_git_state(dir.path()).unwrap();
        assert_eq!(state.branch, None);
        assert_eq!(state.commit.as_ref().map(String::len), Some(40));
    }

    #[test]
    fn test_changed_files_statuses_and_store_excluded() {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]);
        fs::write(p.join("keep.txt"), "a").unwrap();
        fs::write(p.join("old.txt"), "rename me").unwrap();
        fs::write(p.join("gone.txt"), "b").unwrap();
        git(p, &["add", "-A"]);
        git(p, &["commit", "-q", "-m", "init"]);

        fs::write(p.join("keep.txt"), "changed").unwrap();
        git(p, &["mv", "old.txt", "new name.txt"]);
        fs::remove_file(p.join("gone.txt")).unwrap();
        fs::write(p.join("fresh.txt"), "c").unwrap();
        fs::create_dir(p.join(".pas-agent")).unwrap();
        fs::write(p.join(".pas-agent/session.json"), "{}").unwrap();

        let mut files = get_changed_files(p);
        files.sort_by(|a, b| a.path.cmp(&b.path));
        assert_eq!(
            files,
            vec![
                FileState {
                    path: "fresh.txt".into(),
                    status: FileStatus::Untracked
                },
                FileState {
                    path: "gone.txt".into(),
                    status: FileStatus::Deleted
                },
                FileState {
                    path: "keep.txt".into(),
                    status: FileStatus::Modified
                },
                FileState {
                    path: "new name.txt".into(),
                    status: FileStatus::Renamed {
                        from: "old.txt".into()
                    },
                },
            ]
        );
    }

    #[test]
    fn test_store_alone_does_not_make_tree_dirty() {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]);
        git(p, &["commit", "-q", "--allow-empty", "-m", "c"]);
        fs::create_dir(p.join(".pas-agent")).unwrap();
        fs::write(p.join(".pas-agent/session.json"), "{}").unwrap();
        assert!(!capture_git_state(p).unwrap().dirty);
    }
}
