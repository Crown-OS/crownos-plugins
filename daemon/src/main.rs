//! `crownplugind`: runs the active CrownOS theme and the enabled plugins.

mod changes;
mod compositor;
mod daemon;
mod feeds;
mod journal;
mod lock_assets;
mod prompts;
mod registry;
mod store;
mod surfaces;
mod theme;
mod util;
mod vm;
mod watch;
mod workers;

use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::net::UnixListener;
use std::rc::Rc;

use anyhow::{Context, Result};
use calloop::EventLoop;
use calloop::channel::{Event, channel};
use crownos_ipc::ServiceBuilder;
use crownos_ipc::adapter::calloop::CalloopService;
use crownplugin_proto::plugind;

use daemon::Daemon;

/// First fd systemd passes to a socket-activated service.
const LISTEN_FDS_START: i32 = 3;

fn activated_listener() -> Option<UnixListener> {
    let pid: u32 = std::env::var("LISTEN_PID").ok()?.parse().ok()?;
    let count: i32 = std::env::var("LISTEN_FDS").ok()?.parse().ok()?;
    if pid != std::process::id() || count < 1 {
        return None;
    }
    // SAFETY: systemd hands this process ownership of fd 3 when LISTEN_PID
    // names it, and nothing else in the process has touched it yet.
    let fd = unsafe { OwnedFd::from_raw_fd(LISTEN_FDS_START) };
    Some(UnixListener::from(fd))
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "crownplugind=info".into()),
        )
        .init();

    let mut event_loop: EventLoop<'static, Daemon> = EventLoop::try_new()?;
    let handle = event_loop.handle();
    let (sender, receiver) = channel();
    handle
        .insert_source(receiver, |event, _, daemon: &mut Daemon| {
            if let Event::Msg(message) = event {
                daemon.handle(message);
            }
        })
        .map_err(|error| anyhow::anyhow!("{}", error.error))?;

    let mut daemon = Daemon::new(handle.clone(), sender);
    let builder = ServiceBuilder::new(plugind::SERVICE);
    let server = match activated_listener() {
        Some(listener) => builder.build_with_listener(listener),
        None => builder.build(),
    }
    .context("binding the crownplugind socket")?;
    let service = CalloopService::insert(&handle, server, |daemon: &mut Daemon, server, peer, message| {
        plugind::dispatch(daemon, server, peer, message)
    })?;
    daemon.ipc = Some(Rc::new(service));
    daemon.connect_compositor();
    daemon.recover();

    tracing::info!("crownplugind is running");
    event_loop.run(None, &mut daemon, |_| {})?;
    Ok(())
}
