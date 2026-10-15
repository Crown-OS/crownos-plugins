//! `crownplugin-sysassets install <id> <dir>` / `remove <id>`, run as root
//! through pkexec. Copies the files a theme needs the compositor to see
//! (cursors, lock-screen fonts) out of the caller's own theme package.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use crownplugin_model::package::SYSTEM_THEME_ASSETS;
use crownplugin_model::{EntityId, Manifest, Package};

const FONTS: &str = "/usr/share/fonts";
const ICONS: &str = "/usr/share/icons";

fn caller_uid() -> Result<u32> {
    std::env::var("PKEXEC_UID")
        .context("run me through pkexec")?
        .parse()
        .context("PKEXEC_UID is not a number")
}

fn copy_regular(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_regular(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn marker(id: &EntityId) -> String {
    format!(".crownos-theme-{id}")
}

fn install(id: &EntityId, dir: &Path) -> Result<()> {
    let uid = caller_uid()?;
    let dir = dir.canonicalize()?;
    if std::fs::metadata(&dir)?.uid() != uid {
        bail!("{} does not belong to the calling user", dir.display());
    }
    let package = Package::load(&dir)?;
    let Manifest::Theme(theme) = &package.manifest else {
        bail!("{} is not a theme", dir.display());
    };
    if package.id() != id {
        bail!("{} holds {}, not {id}", dir.display(), package.id());
    }
    remove(id)?;
    let dest = Path::new(SYSTEM_THEME_ASSETS).join(id.as_str());
    if let Some(system) = theme.assets.system.as_deref().and_then(|rel| package.resolve(rel)) {
        copy_regular(&system, &dest)?;
        let fonts = dest.join("fonts");
        if fonts.is_dir() {
            std::os::unix::fs::symlink(&fonts, Path::new(FONTS).join(format!("crownos-{id}")))?;
        }
    }
    if let Some(cursors) = theme.assets.cursors.as_deref().and_then(|rel| package.resolve(rel)) {
        let name = cursors.file_name().context("cursor theme without a name")?;
        let target = Path::new(ICONS).join(name);
        if target.exists() && !target.join(marker(id)).exists() {
            bail!("{} already exists and is not ours", target.display());
        }
        copy_regular(&cursors, &target)?;
        std::fs::write(target.join(marker(id)), "")?;
    }
    let _ = std::process::Command::new("fc-cache").status();
    Ok(())
}

fn remove(id: &EntityId) -> Result<()> {
    let dest: PathBuf = Path::new(SYSTEM_THEME_ASSETS).join(id.as_str());
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    let fonts = Path::new(FONTS).join(format!("crownos-{id}"));
    if std::fs::symlink_metadata(&fonts).is_ok() {
        std::fs::remove_file(&fonts)?;
    }
    if let Ok(entries) = std::fs::read_dir(ICONS) {
        for entry in entries.flatten() {
            if entry.path().join(marker(id)).exists() {
                std::fs::remove_dir_all(entry.path())?;
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [verb, id, dir] if verb == "install" => install(&EntityId::new(id.as_str())?, Path::new(dir)),
        [verb, id] if verb == "remove" => {
            caller_uid()?;
            remove(&EntityId::new(id.as_str())?)
        }
        _ => bail!("usage: crownplugin-sysassets install <id> <dir> | remove <id>"),
    }
}
