//! System-wide theme assets (cursors and lock-screen fonts) the compositor
//! needs, installed through a polkit-authorised helper. Runs off the event
//! loop: the user may take a while to authenticate.

use std::path::Path;
use std::process::Command;

use tracing::{info, warn};

pub const HELPER: &str = "/usr/lib/crownos/crownplugin-sysassets";

pub fn spawn(install: bool, id: &str, dir: &Path) {
    let id = id.to_owned();
    let dir = dir.to_owned();
    let spawned = std::thread::Builder::new()
        .name("sysassets".into())
        .spawn(move || {
            let mut command = Command::new("pkexec");
            command.arg(HELPER);
            if install {
                command.arg("install").arg(&id).arg(&dir);
            } else {
                command.arg("remove").arg(&id);
            }
            match command.status() {
                Ok(status) if status.success() => info!(
                    "{} system assets for {id}",
                    if install { "installed" } else { "removed" }
                ),
                Ok(status) => warn!("system assets helper for {id} exited with {status}"),
                Err(error) => warn!("cannot run pkexec for {id}: {error}"),
            }
        });
    if let Err(error) = spawned {
        warn!("cannot start the system assets helper: {error}");
    }
}
