//! Installing, removing and reloading packages.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use crownplugin_model::archive::{EXTENSION, unpack};
use crownplugin_model::package::{data_root, install_dir};
use crownplugin_model::{Package, validate};
use crownplugin_proto::{LogLevel, Source};

use super::Daemon;
use crate::journal::Journal;
use crate::util::fsx;

fn staging_dir() -> PathBuf {
    data_root().join(".staging").join(format!(
        "{}-{}",
        std::process::id(),
        crate::registry::now_millis()
    ))
}

fn checked(dir: &Path) -> Result<Package> {
    let package = Package::load(dir)?;
    let report = validate(&package);
    if !report.is_ok() {
        let errors: Vec<String> = report.errors.iter().map(ToString::to_string).collect();
        bail!("the package is not valid:\n  {}", errors.join("\n  "));
    }
    Ok(package)
}

fn fetch(source: &Source, staging: &Path) -> Result<PathBuf> {
    match source {
        Source::Path(path) => {
            let path = Path::new(path);
            if path.extension().is_some_and(|extension| extension == EXTENSION) {
                unpack(path, staging)?;
            } else {
                fsx::copy_tree(path, staging).with_context(|| format!("copying {}", path.display()))?;
            }
        }
        Source::Git { url, rev } => {
            if !(url.starts_with("https://") || url.starts_with("git@")) {
                bail!("only https:// and git@ URLs are cloned");
            }
            let mut git = Command::new("git");
            git.args(["clone", "--depth", "1"]);
            if let Some(rev) = rev {
                git.args(["--branch", rev]);
            }
            let status = git
                .arg("--")
                .arg(url)
                .arg(staging)
                .env("GIT_TERMINAL_PROMPT", "0")
                .status()
                .context("running git")?;
            if !status.success() {
                bail!("git clone {url} failed");
            }
            let _ = std::fs::remove_dir_all(staging.join(".git"));
        }
    }
    Ok(staging.to_owned())
}

impl Daemon {
    pub fn install_from(&mut self, source: &Source) -> Result<String> {
        let staging = staging_dir();
        let result = (|| {
            let dir = fetch(source, &staging)?;
            let package = checked(&dir)?;
            let id = package.id().clone();
            let kind = package.manifest.kind();
            let was_active = self.running.contains_key(id.as_str()) || Journal::load(id.as_str()).is_some();
            if was_active {
                self.deactivate(id.as_str());
            }
            fsx::swap_in(&dir, &install_dir(kind, &id)).context("installing")?;
            self.registry.rescan();
            self.log(id.as_str(), LogLevel::Info, format!("installed {}", package.manifest.version()));
            self.reconcile();
            Ok(id.to_string())
        })();
        let _ = std::fs::remove_dir_all(&staging);
        result
    }

    pub fn uninstall_entity(&mut self, id: &str) -> Result<()> {
        let entry = self.registry.get(id).with_context(|| format!("`{id}` is not installed"))?;
        if entry.package.system {
            bail!("`{id}` ships with the system and cannot be removed");
        }
        let dir = entry.package.dir.clone();
        let dev = entry.dev;
        let package_id = entry.package.id().clone();
        self.deactivate(id);
        self.store.forget(&package_id);
        if !dev {
            std::fs::remove_dir_all(&dir).with_context(|| format!("removing {}", dir.display()))?;
        }
        self.watches.forget_dev(id);
        self.registry.remove(id);
        self.reconcile();
        Ok(())
    }

    /// Load an unpacked package in place and follow its edits.
    pub fn dev_load_dir(&mut self, dir: &Path) -> Result<String> {
        let dir = dir.canonicalize().with_context(|| format!("{} does not exist", dir.display()))?;
        let package = checked(&dir)?;
        let id = self.registry.insert_dev(package).to_string();
        self.watches.dev(&id, &dir);
        self.reload(&id);
        Ok(id)
    }

    /// Re-read a package from disk and re-apply it if it is active.
    pub fn reload(&mut self, id: &str) {
        let Some(entry) = self.registry.get(id) else {
            return;
        };
        let dir = entry.package.dir.clone();
        let dev = entry.dev;
        let package = match checked(&dir) {
            Ok(package) => package,
            Err(error) => {
                self.log(id, LogLevel::Error, format!("{error:#}"));
                return;
            }
        };
        let was_active = self.running.contains_key(id) || Journal::load(id).is_some();
        if was_active {
            self.deactivate(id);
        }
        if dev {
            self.registry.insert_dev(package);
        } else {
            self.registry.rescan();
        }
        self.log(id, LogLevel::Info, "reloaded".into());
        self.reconcile();
    }
}
