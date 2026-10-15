use std::io;
use std::path::Path;

/// Copy a package tree, skipping symlinks and VCS metadata.
pub fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        let kind = entry.file_type()?;
        let target = to.join(&name);
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Replace `dest` with `staged` as atomically as the filesystem allows.
pub fn swap_in(staged: &Path, dest: &Path) -> io::Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let old = dest.with_extension("old");
    if dest.exists() {
        let _ = std::fs::remove_dir_all(&old);
        std::fs::rename(dest, &old)?;
    }
    std::fs::rename(staged, dest)?;
    let _ = std::fs::remove_dir_all(&old);
    Ok(())
}

/// Write through a temp file and rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("crownplugind.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)
}
