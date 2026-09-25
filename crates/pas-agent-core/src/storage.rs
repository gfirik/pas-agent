use crate::git::git_toplevel;
use crate::session::{Session, SCHEMA_VERSION};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
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
pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), SessionError> {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));

    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(contents)?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();

    result.map_err(|e| {
        let _ = fs::remove_file(&tmp);
        SessionError::io(path, e)
    })
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
}
