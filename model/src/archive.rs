//! `.crownpkg`: a gzipped tar of one package directory, entries relative to it.

use std::fs::File;
use std::path::Path;

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

use crate::package::relative_path;
use crate::{ModelError, Package};

pub const EXTENSION: &str = "crownpkg";

pub fn pack(package: &Package, out: &Path) -> Result<(), ModelError> {
    let file = File::create(out).map_err(ModelError::io(out))?;
    let mut tar = tar::Builder::new(GzEncoder::new(file, Compression::best()));
    tar.follow_symlinks(false);
    tar.append_dir_all(".", &package.dir)
        .map_err(ModelError::io(&package.dir))?;
    tar.into_inner()
        .and_then(|gz| gz.finish())
        .map(drop)
        .map_err(ModelError::io(out))
}

/// Unpack into `dest`, refusing absolute paths, `..` and links before
/// anything is written.
pub fn unpack(archive: &Path, dest: &Path) -> Result<(), ModelError> {
    let open = || {
        File::open(archive)
            .map(|file| tar::Archive::new(GzDecoder::new(file)))
            .map_err(ModelError::io(archive))
    };
    for entry in open()?.entries().map_err(ModelError::io(archive))? {
        let entry = entry.map_err(ModelError::io(archive))?;
        let path = entry.path().map_err(ModelError::io(archive))?;
        let shown = path.display().to_string();
        let kind = entry.header().entry_type();
        if relative_path(&shown).is_none() || kind.is_symlink() || kind.is_hard_link() {
            return Err(ModelError::UnsafeArchivePath(shown));
        }
    }
    std::fs::create_dir_all(dest).map_err(ModelError::io(dest))?;
    open()?.unpack(dest).map_err(ModelError::io(dest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packed_package_unpacks_to_the_same_manifest() {
        let src = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            src.path().join("plugin.ron"),
            r#"Plugin(id: "dev.test.hello", name: "Hello", version: "1.0.0")"#,
        )
        .expect("write");
        std::fs::write(src.path().join("main.luau"), "return {}").expect("write");
        let package = Package::load(src.path()).expect("loads");

        let out = tempfile::tempdir().expect("tempdir");
        let archive = out.path().join("hello.crownpkg");
        pack(&package, &archive).expect("packs");
        let dest = out.path().join("unpacked");
        unpack(&archive, &dest).expect("unpacks");

        assert_eq!(Package::load(&dest).expect("loads").manifest, package.manifest);
        assert!(dest.join("main.luau").is_file());
    }
}
