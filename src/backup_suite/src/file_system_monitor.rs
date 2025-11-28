use chrono::Utc;
use crossbeam::channel::{bounded, unbounded, Receiver, Sender};
use log::{debug, error, warn};
use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher,
};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

type MonitorResult<T> = Result<T, FileSystemMonitorError>;

#[derive(Debug, Error)]
pub enum FileSystemMonitorError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("notify error: {0}")]
    Notify(#[from] notify::Error),
    #[error("failed to spawn monitor thread: {0}")]
    ThreadSpawn(io::Error),
}

#[derive(Debug)]
struct WatchEntry {
    path: PathBuf,
    is_dir: bool,
    last_update_time: AtomicU64,
}

impl WatchEntry {
    fn new(path: PathBuf, is_dir: bool) -> Self {
        Self {
            path,
            is_dir,
            last_update_time: AtomicU64::new(0),
        }
    }

    fn set_last_update_time(&self, timestamp: u64) {
        self.last_update_time.store(timestamp, Ordering::SeqCst);
    }

    fn try_set_initial_update_time(&self, timestamp: u64) {
        let _ = self.last_update_time.compare_exchange(
            0,
            timestamp,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }

    fn last_update_time(&self) -> u64 {
        self.last_update_time.load(Ordering::SeqCst)
    }
}

#[derive(Debug)]
pub struct FileSystemMonitor {
    watcher: Arc<Mutex<RecommendedWatcher>>,
    watched: Arc<Mutex<HashMap<PathBuf, Arc<WatchEntry>>>>,
    shutdown_tx: Option<Sender<()>>,
    event_thread: Option<JoinHandle<()>>,
}

impl FileSystemMonitor {
    pub fn new() -> MonitorResult<Self> {
        let watched: Arc<Mutex<HashMap<PathBuf, Arc<WatchEntry>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (event_tx, event_rx) = unbounded::<NotifyResult<Event>>();
        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = event_tx.send(res);
            },
            Config::default(),
        )?;

        let (shutdown_tx, shutdown_rx) = bounded::<()>(1);
        let event_thread = Self::spawn_event_thread(Arc::clone(&watched), event_rx, shutdown_rx)?;

        Ok(Self {
            watcher: Arc::new(Mutex::new(watcher)),
            watched,
            shutdown_tx: Some(shutdown_tx),
            event_thread: Some(event_thread),
        })
    }

    pub fn add_path<P: AsRef<Path>>(&self, path: P) -> MonitorResult<()> {
        let normalized = normalize_path(path.as_ref())?;
        {
            let watched = self.watched.lock().unwrap();
            if watched.contains_key(&normalized) {
                debug!(
                    "path {} already monitored, skip Installing duplicate watcher",
                    normalized.display()
                );
                return Ok(());
            }
        }

        let metadata = std::fs::metadata(&normalized)?;
        let is_dir = metadata.is_dir();
        let mode = if is_dir {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        {
            let mut watcher = self.watcher.lock().unwrap();
            watcher.watch(&normalized, mode)?;
        }

        let entry = Arc::new(WatchEntry::new(normalized.clone(), is_dir));
        let entry_cloned = entry.clone();
        tokio::spawn(async move {
            if let Err(err) = Self::initialize_entry_timestamp(&entry_cloned) {
                warn!(
                    "failed to initialize last update time for {}: {}",
                    entry_cloned.path.display(),
                    err
                );
            }
        });

        let mut watched = self.watched.lock().unwrap();
        watched.insert(normalized, entry);
        Ok(())
    }

    pub fn remove_path<P: AsRef<Path>>(&self, path: P) -> MonitorResult<()> {
        let normalized = normalize_path(path.as_ref())?;
        let removed = {
            let mut watched = self.watched.lock().unwrap();
            watched.remove(&normalized)
        };

        if removed.is_some() {
            let mut watcher = self.watcher.lock().unwrap();
            watcher.unwatch(&normalized)?;
        }

        Ok(())
    }

    pub fn get_last_update_time<P: AsRef<Path>>(&self, path: P) -> Option<u64> {
        let normalized = normalize_path(path.as_ref()).ok()?;
        let watched = self.watched.lock().ok()?;
        watched
            .get(&normalized)
            .map(|entry| entry.last_update_time())
    }

    fn spawn_event_thread(
        watched: Arc<Mutex<HashMap<PathBuf, Arc<WatchEntry>>>>,
        event_rx: Receiver<NotifyResult<Event>>,
        shutdown_rx: Receiver<()>,
    ) -> MonitorResult<JoinHandle<()>> {
        thread::Builder::new()
            .name("fs-monitor".to_owned())
            .spawn(move || loop {
                crossbeam::select! {
                    recv(shutdown_rx) -> _ => {
                        debug!("file system monitor shutting down");
                        break;
                    }
                    recv(event_rx) -> msg => {
                        match msg {
                            Ok(event_res) => Self::handle_event(&watched, event_res),
                            Err(_) => break,
                        }
                    }
                }
            })
            .map_err(FileSystemMonitorError::ThreadSpawn)
    }

    fn handle_event(
        watched: &Arc<Mutex<HashMap<PathBuf, Arc<WatchEntry>>>>,
        event_res: NotifyResult<Event>,
    ) {
        match event_res {
            Ok(event) => {
                if !is_relevant_event(&event.kind) {
                    return;
                }

                let timestamp = current_timestamp_millis();
                for raw_path in event.paths {
                    let normalized = normalize_path_lossy(&raw_path);
                    let targets = {
                        let guard = watched.lock().unwrap();
                        guard
                            .iter()
                            .filter_map(|(watch_path, entry)| {
                                if matches_watch(watch_path, entry.is_dir, &normalized) {
                                    Some(Arc::clone(entry))
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                    };

                    if targets.is_empty() {
                        continue;
                    }

                    for entry in targets {
                        entry.set_last_update_time(timestamp);
                        debug!(
                            "updated {} last time to {}",
                            entry.path.display(),
                            timestamp
                        );
                    }
                }
            }
            Err(err) => {
                warn!("file system monitor event error: {}", err);
            }
        }
    }

    fn initialize_entry_timestamp(entry: &Arc<WatchEntry>) -> io::Result<()> {
        if let Some(ts) = latest_modification_millis(&entry.path)? {
            entry.try_set_initial_update_time(ts);
        }
        Ok(())
    }
}

impl Drop for FileSystemMonitor {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.event_thread.take() {
            if let Err(err) = handle.join() {
                error!("failed to join file system monitor thread: {:?}", err);
            }
        }
    }
}

fn current_timestamp_millis() -> u64 {
    Utc::now().timestamp_millis() as u64
}

fn normalize_path(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn normalize_path_lossy(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(path)
    } else {
        path.to_path_buf()
    }
}

fn matches_watch(watch_path: &Path, is_dir: bool, candidate: &Path) -> bool {
    if is_dir {
        candidate.starts_with(watch_path)
    } else {
        watch_path == candidate
    }
}

fn is_relevant_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Modify(_)
            | EventKind::Remove(_)
    )
}

fn latest_modification_millis(path: &Path) -> io::Result<Option<u64>> {
    let mut latest: Option<SystemTime> = None;
    let mut stack = vec![path.to_path_buf()];

    while let Some(current) = stack.pop() {
        match std::fs::metadata(&current) {
            Ok(metadata) => {
                if let Ok(modified) = metadata.modified() {
                    latest = match latest {
                        Some(existing) if existing >= modified => Some(existing),
                        _ => Some(modified),
                    };
                }
                if metadata.is_dir() {
                    match std::fs::read_dir(&current) {
                        Ok(entries) => {
                            for entry in entries {
                                match entry {
                                    Ok(entry) => stack.push(entry.path()),
                                    Err(err) => warn!(
                                        "failed to read directory entry in {}: {}",
                                        current.display(),
                                        err
                                    ),
                                }
                            }
                        }
                        Err(err) => {
                            warn!("failed to read directory {}: {}", current.display(), err);
                        }
                    }
                }
            }
            Err(err) => {
                warn!("failed to read metadata for {}: {}", current.display(), err);
            }
        }
    }

    latest
        .map(system_time_to_millis)
        .transpose()
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
}

fn system_time_to_millis(time: SystemTime) -> Result<u64, std::time::SystemTimeError> {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
}
