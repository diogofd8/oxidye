//! Shared finite configuration I/O; toolkit policy stays in each installer.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    env,
    fs::{self, File},
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct Paths {
    pub config: PathBuf,
    pub state: PathBuf,
}
impl Paths {
    pub fn environment() -> Result<Self, String> {
        let xdg = |key: &str, fallback: &str| -> Result<PathBuf, String> {
            let path = env::var_os(key).filter(|s| !s.is_empty()).map(PathBuf::from).or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(fallback))).ok_or_else(|| format!("set {key} or HOME"))?;
            if !path.is_absolute() {
                return Err(format!("{key} must be an absolute directory"));
            }
            Ok(path)
        };
        Ok(Self { config: xdg("XDG_CONFIG_HOME", ".config")?, state: xdg("XDG_STATE_HOME", ".local/state")?.join("oxidye") })
    }
    pub(crate) fn lock(&self) -> Result<File, String> {
        fs::create_dir_all(&self.state).map_err(|e| e.to_string())?;
        let file = File::options().create(true).truncate(false).read(true).write(true).open(self.state.join("theme.lock")).map_err(|e| e.to_string())?;
        file.try_lock().map_err(|e| format!("another theme operation is running, or the lock is unavailable: {e}"))?;
        Ok(file)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Snapshot {
    pub relative: String,
    pub contents: Option<String>,
    pub permissions: u32,
}
pub(crate) fn read(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(value) => Ok(Some(value)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}
pub(crate) fn capture(config: &Path, files: &[&str]) -> Result<Vec<Snapshot>, String> {
    files
        .iter()
        .map(|relative| {
            let target = config.join(relative);
            if fs::symlink_metadata(&target).is_ok_and(|m| m.is_dir() || m.file_type().is_symlink()) {
                return Err(format!("{}: expected a regular file, not a directory or symlink", target.display()));
            }
            Ok(Snapshot { relative: (*relative).into(), contents: read(&target)?, permissions: fs::metadata(&target).map(|m| m.permissions().mode() & 0o777).unwrap_or(0o644) })
        })
        .collect()
}
pub(crate) fn atomic(path: &Path, data: &[u8], permissions: u32) -> Result<(), String> {
    let parent = path.parent().ok_or("output has no parent directory")?;
    fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    let mut pending = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    pending.write_all(data).map_err(|e| e.to_string())?;
    pending.as_file().set_permissions(fs::Permissions::from_mode(permissions)).map_err(|e| e.to_string())?;
    pending.as_file().sync_all().map_err(|e| e.to_string())?;
    pending.persist(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}
fn install(config: &Path, file: &Snapshot) -> Result<(), String> {
    let target = config.join(&file.relative);
    if let Some(contents) = &file.contents {
        atomic(&target, contents.as_bytes(), file.permissions)
    } else {
        match fs::remove_file(&target) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("{}: {e}", target.display())),
        }
    }
}
pub(crate) fn transaction(config: &Path, before: &[Snapshot], after: &[Snapshot], toolkit: &str) -> Result<(), String> {
    let mut installed = BTreeSet::new();
    for file in after {
        if let Err(error) = install(config, file) {
            let failures: Vec<_> = before.iter().filter(|old| installed.contains(&old.relative)).filter_map(|old| install(config, old).err()).collect();
            if failures.is_empty() {
                return Err(format!("installation failed; previous {toolkit} files restored: {error}"));
            }
            return Err(format!("installation failed: {error}; restoration errors: {}", failures.join("; ")));
        }
        installed.insert(file.relative.clone());
    }
    Ok(())
}
