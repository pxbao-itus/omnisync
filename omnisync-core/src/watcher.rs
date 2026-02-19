use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use anyhow::Result;

pub struct FilesystemWatcher {
    watcher: RecommendedWatcher,
    rx: Receiver<std::result::Result<Event, notify::Error>>,
}

impl FilesystemWatcher {
    pub fn new() -> Result<Self> {
        let (tx, rx) = channel();
        
        // We use a channel to receive events on the main thread (or wherever we poll)
        let watcher = RecommendedWatcher::new(tx, Config::default())?;

        Ok(Self { watcher, rx })
    }

    pub fn watch(&mut self, path: &Path) -> Result<()> {
        self.watcher.watch(path, RecursiveMode::Recursive)?;
        Ok(())
    }

    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        self.watcher.unwatch(path)?;
        Ok(())
    }

    pub fn try_recv(&self) -> Option<std::result::Result<Event, notify::Error>> {
        self.rx.try_recv().ok()
    }
}
