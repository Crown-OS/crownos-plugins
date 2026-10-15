//! Batching what hosts hear: at most one push per surface kind per frame.

use std::time::Duration;

use calloop::timer::{TimeoutAction, Timer};
use crownplugin_proto::plugind::{BarChanged, EntitiesChanged, LockChanged, Logged};

use super::Daemon;

/// ~30 Hz, the fastest a plugin surface is ever re-sent.
const FLUSH_DELAY: Duration = Duration::from_millis(33);

impl Daemon {
    pub fn schedule_flush(&mut self) {
        if self.outbox.scheduled {
            return;
        }
        self.outbox.scheduled = true;
        let inserted = self
            .handle
            .insert_source(Timer::from_duration(FLUSH_DELAY), |_, _, daemon: &mut Daemon| {
                daemon.flush();
                TimeoutAction::Drop
            });
        if inserted.is_err() {
            self.outbox.scheduled = false;
        }
    }

    fn push_lock(&mut self) {
        let theme = self.store.active_theme().map(str::to_owned);
        let theme_dir = theme
            .as_deref()
            .and_then(|id| self.registry.get(id))
            .map(|entry| entry.package.dir.clone());
        let Some(link) = self.compositor.as_mut() else {
            return;
        };
        if let Err(error) = link.push_lock(&self.surfaces, &self.registry, theme_dir.as_deref()) {
            match theme {
                Some(theme) => self.log(&theme, crownplugin_proto::LogLevel::Error, format!("{error:#}")),
                None => tracing::warn!("{error:#}"),
            }
        }
    }

    fn flush(&mut self) {
        self.outbox.scheduled = false;
        let outbox = std::mem::take(&mut self.outbox);
        if outbox.lock && self.compositor.is_some() {
            self.push_lock();
        }
        let Some(ipc) = self.ipc.clone() else {
            return;
        };
        let emit = |result: Result<(), crownos_ipc::Error>| {
            if let Err(error) = result {
                tracing::warn!("emitting to hosts: {error}");
            }
        };
        if outbox.bar {
            emit(ipc.emit(&BarChanged { snapshot: self.surfaces.bar() }));
        }
        if outbox.lock {
            emit(ipc.emit(&LockChanged { snapshot: self.surfaces.lock() }));
        }
        if outbox.entities {
            self.entities_revision += 1;
            emit(ipc.emit(&EntitiesChanged { revision: self.entities_revision }));
        }
        for line in outbox.logs {
            emit(ipc.emit(&Logged { line }));
        }
    }
}
