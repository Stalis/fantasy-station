use std::error::Error;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct DiskManifest {
    pub(crate) name: String,
    entrypoint: PathBuf,
}

pub(crate) fn load_disk(path: &Path) -> Result<(DiskManifest, PathBuf), Box<dyn Error>> {
    let manifest_path = path.join("manifest.json");
    let source = fs::read_to_string(&manifest_path).map_err(|err| {
        format!(
            "Failed to read disk manifest '{}': {err}",
            manifest_path.display()
        )
    })?;
    let manifest: DiskManifest = serde_json::from_str(&source)
        .map_err(|err| format!("Invalid disk manifest '{}': {err}", manifest_path.display()))?;
    if manifest.name.trim().is_empty() {
        return Err("Disk name must not be empty".into());
    }
    if manifest.entrypoint.as_os_str().is_empty()
        || !manifest
            .entrypoint
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err("Disk entrypoint must be a relative path inside the disk".into());
    }
    let root = fs::canonicalize(path)?;
    let script_path = fs::canonicalize(root.join(&manifest.entrypoint)).map_err(|err| {
        format!(
            "Failed to resolve disk entrypoint '{}': {err}",
            manifest.entrypoint.display()
        )
    })?;
    if !script_path.starts_with(&root) {
        return Err("Disk entrypoint must be inside the disk".into());
    }
    Ok((manifest, script_path))
}
