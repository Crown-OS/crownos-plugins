//! Talking to a running crownplugind.

use std::time::Duration;

use anyhow::{Context, Result};
use crownos_ipc::Pending;
use crownplugin_proto::{EntityInfo, LogLine, plugind};

const TIMEOUT: Duration = Duration::from_secs(120);

pub struct Daemon {
    client: plugind::Client,
}

impl Daemon {
    pub fn connect() -> Result<Self> {
        let client = plugind::Client::connect()
            .context("crownplugind is not running (systemctl --user start crownplugind.socket)")?;
        Ok(Self { client })
    }

    pub fn wait<R: serde::de::DeserializeOwned>(&mut self, pending: Result<Pending<R>, crownos_ipc::Error>) -> Result<R> {
        let pending = pending?;
        Ok(self.client.wait(pending, Some(TIMEOUT))?)
    }

    pub fn client(&mut self) -> &mut plugind::Client {
        &mut self.client
    }

    pub fn list(&mut self) -> Result<Vec<EntityInfo>> {
        let pending = self.client.list();
        self.wait(pending)
    }

    /// Print log lines for `id` as they arrive, until interrupted.
    pub fn follow_logs(&mut self, id: &str) -> Result<()> {
        self.client.subscribe_blocking::<plugind::Logged>(Some(Duration::from_secs(5)))?;
        loop {
            while let Some(event) = self.client.next_event() {
                if let plugind::Event::Logged(plugind::Logged { line }) = event
                    && line.id == id
                {
                    print_line(&line);
                }
            }
            let mut fds = [rustix::event::PollFd::new(&self.client, rustix::event::PollFlags::IN)];
            rustix::event::poll(&mut fds, None)?;
            self.client.handle_readable()?;
        }
    }
}

pub fn print_line(line: &LogLine) {
    println!("[{:?}] {}", line.level, line.message);
}
