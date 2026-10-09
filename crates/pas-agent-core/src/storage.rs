use crate::git::git_toplevel;
use crate::session::{Session, SCHEMA_VERSION};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use thiserror::Error;

/// Name of the per-project state directory.
pub const STORE_DIR: &str = ".pas-agent";

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("no PAS-Agent session found at {0}")]
    NotFound(PathBuf),

    #[error("I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{path} is not a valid session file: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "{path} uses session schema v{found}, but this build supports up to v{supported}; upgrade pas-agent"
    )]
    UnsupportedSchema {
        path: PathBuf,
        found: u32,
        supported: u32,
    },

    #[error("the session is locked by another pas-agent process ({0}); retry in a moment")]
    Locked(PathBuf),

    #[error(
        "{0} has a PAS-Agent start marker without a matching end marker (or vice versa); fix it by hand or re-run with --force"
    )]
    MalformedBlock(PathBuf),
}

impl SessionError {
    fn io(path: &Path, source: std::io::Error) -> Self {
        SessionError::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

/// How long to wait for another `pas-agent` process to finish before giving up.
const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(5);
const LOCK_FILE: &str = "session.lock";

/// Holds an exclusive OS lock on `session.lock` for as long as it lives, so a
/// load-modify-save cycle is not interleaved with another process's. The lock belongs to
/// the open file, so the kernel releases it if the process crashes: there is no stale lock
/// to clean up. The (empty) lock file itself is left in place on purpose.
#[derive(Debug)]
pub struct SessionLock {
    _file: fs::File,
}

fn env_millis(name: &str, default: Duration) -> Duration {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map_or(default, Duration::from_millis)
}

pub struct SessionStore {
    project_dir: PathBuf,
    base_dir: PathBuf,
}

impl SessionStore {
    #[must_use]
    pub fn new(project_dir: &Path) -> Self {
        Self {
            project_dir: project_dir.to_path_buf(),
            base_dir: project_dir.join(STORE_DIR),
        }
    }

    /// Finds the nearest ancestor of `start` (inclusive) that holds a session.
    #[must_use]
    pub fn discover(start: &Path) -> Option<Self> {
        start
            .ancestors()
            .find(|dir| dir.join(STORE_DIR).join("session.json").is_file())
            .map(Self::new)
    }

    /// Where `init` should create a session: the git work-tree root, or `start` outside git.
    #[must_use]
    pub fn init_root(start: &Path) -> PathBuf {
        git_toplevel(start).unwrap_or_else(|| start.to_path_buf())
    }

    #[must_use]
    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }

    #[must_use]
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    #[must_use]
    pub fn session_path(&self) -> PathBuf {
        self.base_dir.join("session.json")
    }

    #[must_use]
    pub fn exists(&self) -> bool {
        self.session_path().exists()
    }

    /// Takes the session lock, waiting for other processes up to
    /// `PAS_AGENT_LOCK_TIMEOUT_MS` (default 5000).
    pub fn lock(&self) -> Result<SessionLock, SessionError> {
        self.lock_with(env_millis(
            "PAS_AGENT_LOCK_TIMEOUT_MS",
            DEFAULT_LOCK_TIMEOUT,
        ))
    }

    fn lock_with(&self, timeout: Duration) -> Result<SessionLock, SessionError> {
        fs::create_dir_all(&self.base_dir).map_err(|e| SessionError::io(&self.base_dir, e))?;
        // The lock file stays on disk, so keep it out of a committed `.pas-agent/`.
        let ignore = self.base_dir.join(".gitignore");
        if !ignore.exists() {
            let _ = fs::write(&ignore, format!("{LOCK_FILE}\n"));
        }

        let path = self.base_dir.join(LOCK_FILE);
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|e| SessionError::io(&path, e))?;

        let started = Instant::now();
        loop {
            // Fully qualified: newer Rust has an inherent `File::try_lock` with a
            // different error type, which would otherwise win over this trait method.
            match fs4::FileExt::try_lock(&file) {
                Ok(()) => return Ok(SessionLock { _file: file }),
                Err(fs4::TryLockError::WouldBlock) => {
                    if started.elapsed() >= timeout {
                        return Err(SessionError::Locked(path));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(fs4::TryLockError::Error(e)) => return Err(SessionError::io(&path, e)),
            }
        }
    }

    pub fn save(&self, session: &Session) -> Result<(), SessionError> {
        fs::create_dir_all(&self.base_dir).map_err(|e| SessionError::io(&self.base_dir, e))?;
        let path = self.session_path();
        let mut json =
            serde_json::to_string_pretty(session).map_err(|source| SessionError::Parse {
                path: path.clone(),
                source,
            })?;
        json.push('\n');
        write_atomic(&path, json.as_bytes())
    }

    pub fn load(&self) -> Result<Session, SessionError> {
        let path = self.session_path();
        let data = match fs::read_to_string(&path) {
            Ok(data) => data,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(SessionError::NotFound(path));
            }
            Err(e) => return Err(SessionError::io(&path, e)),
        };

        // Check the version before full parsing so a newer file gets a clear error
        // instead of a confusing field-level parse failure.
        let raw: serde_json::Value =
            serde_json::from_str(&data).map_err(|source| SessionError::Parse {
                path: path.clone(),
                source,
            })?;
        let found = raw
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .map_or(1, |v| u32::try_from(v).unwrap_or(u32::MAX));
        if found > SCHEMA_VERSION {
            return Err(SessionError::UnsupportedSchema {
                path,
                found,
                supported: SCHEMA_VERSION,
            });
        }

        let mut session: Session =
            serde_json::from_value(raw).map_err(|source| SessionError::Parse {
                path: path.clone(),
                source,
            })?;
        session.schema_version = SCHEMA_VERSION;
        Ok(session)
    }
}

/// Writes `contents` to a sibling temp file, then renames it over `path`, so a crash
/// mid-write never leaves a truncated file behind.
///
/// A symlink at `path` is followed, so the file it points to is updated and the link is
/// kept; an existing file's permissions are preserved.
pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), SessionError> {
    let resolved = resolve_symlink(path);
    let path = resolved.as_path();
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));

    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(contents)?;
        file.sync_all()?;
        if let Ok(meta) = fs::metadata(path) {
            fs::set_permissions(&tmp, meta.permissions())?;
        }
        fs::rename(&tmp, path)
    })();

    result.map_err(|e| {
        let _ = fs::remove_file(&tmp);
        SessionError::io(path, e)
    })
}

/// Resolves `path` through symlinks; a dangling link resolves to its (missing) target.
fn resolve_symlink(path: &Path) -> PathBuf {
    let is_link = fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink());
    if !is_link {
        return path.to_path_buf();
    }
    if let Ok(real) = fs::canonicalize(path) {
        return real;
    }
    match fs::read_link(path) {
        Ok(target) => path.parent().map_or(target.clone(), |dir| dir.join(target)),
        Err(_) => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_session_store_lifecycle() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path());
        assert!(!store.exists());
        assert!(matches!(store.load(), Err(SessionError::NotFound(_))));

        let mut session = Session::new("test-project".into(), "Fix bug".into());
        store.save(&session).unwrap();
        assert!(store.exists());
        assert_eq!(store.load().unwrap().id, session.id);

        // Checkpoints taken within the same second must all survive.
        session.add_checkpoint("step 1".into(), vec![]);
        session.add_checkpoint("step 2".into(), vec![]);
        store.save(&session).unwrap();
        assert_eq!(store.load().unwrap().checkpoints.len(), 2);
    }

    #[test]
    fn test_corrupt_file_is_parse_error_not_missing() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path());
        fs::create_dir_all(store.base_dir()).unwrap();
        fs::write(store.session_path(), "{").unwrap();
        assert!(matches!(store.load(), Err(SessionError::Parse { .. })));
    }

    #[test]
    fn test_newer_schema_is_rejected() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path());
        let session = Session::new("p".into(), "t".into());
        let mut value = serde_json::to_value(&session).unwrap();
        value["schema_version"] = serde_json::json!(SCHEMA_VERSION + 1);
        fs::create_dir_all(store.base_dir()).unwrap();
        fs::write(store.session_path(), value.to_string()).unwrap();
        assert!(matches!(
            store.load(),
            Err(SessionError::UnsupportedSchema { .. })
        ));
    }

    #[test]
    fn test_discover_walks_up_from_subdirectory() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path());
        store.save(&Session::new("p".into(), "t".into())).unwrap();
        let nested = dir.path().join("a/b/c");
        fs::create_dir_all(&nested).unwrap();

        let found = SessionStore::discover(&nested).unwrap();
        assert_eq!(found.project_dir(), dir.path());
    }

    #[test]
    fn test_save_leaves_no_temp_files() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path());
        store.save(&Session::new("p".into(), "t".into())).unwrap();
        let entries: Vec<_> = fs::read_dir(store.base_dir()).unwrap().collect();
        assert_eq!(entries.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn test_write_atomic_follows_symlink_and_keeps_permissions() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("AGENTS.md");
        let link = dir.path().join("CLAUDE.md");
        fs::write(&target, "old").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
        symlink("AGENTS.md", &link).unwrap();

        write_atomic(&link, b"new").unwrap();

        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_write_atomic_through_dangling_symlink_creates_target() {
        use std::os::unix::fs::symlink;
        let dir = TempDir::new().unwrap();
        let link = dir.path().join("CLAUDE.md");
        symlink("AGENTS.md", &link).unwrap();
        write_atomic(&link, b"new").unwrap();
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_to_string(dir.path().join("AGENTS.md")).unwrap(),
            "new"
        );
    }

    #[test]
    fn test_lock_excludes_others_until_released() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path());
        let short = Duration::from_millis(60);
        let first = store.lock_with(short).unwrap();
        assert!(matches!(
            store.lock_with(short),
            Err(SessionError::Locked(_))
        ));
        drop(first);
        assert!(store.lock_with(short).is_ok());
    }

    #[test]
    fn test_leftover_lock_file_does_not_block() {
        // The lock belongs to the open file, not to the file's existence, so the file a
        // crashed process left behind is just reused.
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path());
        drop(store.lock_with(Duration::from_millis(60)).unwrap());
        assert!(store.base_dir().join(LOCK_FILE).exists());
        assert!(store.lock_with(Duration::from_millis(60)).is_ok());
    }

    #[test]
    fn test_lock_file_is_gitignored_without_touching_an_existing_ignore() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path());
        drop(store.lock_with(Duration::from_millis(60)).unwrap());
        let ignore = store.base_dir().join(".gitignore");
        assert_eq!(fs::read_to_string(&ignore).unwrap(), "session.lock\n");

        fs::write(&ignore, "custom\n").unwrap();
        drop(store.lock_with(Duration::from_millis(60)).unwrap());
        assert_eq!(fs::read_to_string(&ignore).unwrap(), "custom\n");
    }
}
