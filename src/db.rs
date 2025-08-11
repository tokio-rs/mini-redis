use tokio::sync::{broadcast, Notify};
use tokio::time::{self, Duration, Instant};

use bytes::Bytes;
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use crate::Metrics;

/// A wrapper around a `Db` instance. This exists to allow orderly cleanup
/// of the `Db` by signalling the background purge task to shut down when
/// this struct is dropped.
#[derive(Debug)]
pub(crate) struct DbDropGuard {
    /// The `Db` instance that will be shut down when this `DbDropGuard` struct
    /// is dropped.
    db: Db,
}

/// Server state shared across all connections.
///
/// `Db` contains a `HashMap` storing the key/value data and all
/// `broadcast::Sender` values for active pub/sub channels.
///
/// A `Db` instance is a handle to shared state. Cloning `Db` is shallow and
/// only incurs an atomic ref count increment.
///
/// When a `Db` value is created, a background task is spawned. This task is
/// used to expire values after the requested duration has elapsed. The task
/// runs until all instances of `Db` are dropped, at which point the task
/// terminates.
#[derive(Debug, Clone)]
pub(crate) struct Db {
    /// Handle to shared state. The background task will also have an
    /// `Arc<Shared>`.
    shared: Arc<Shared>,
}

#[derive(Debug)]
struct Shared {
    /// The shared state is guarded by a mutex. This is a `std::sync::Mutex` and
    /// not a Tokio mutex. This is because there are no asynchronous operations
    /// being performed while holding the mutex. Additionally, the critical
    /// sections are very small.
    ///
    /// A Tokio mutex is mostly intended to be used when locks need to be held
    /// across `.await` yield points. All other cases are **usually** best
    /// served by a std mutex. If the critical section does not include any
    /// async operations but is long (CPU intensive or performing blocking
    /// operations), then the entire operation, including waiting for the mutex,
    /// is considered a "blocking" operation and `tokio::task::spawn_blocking`
    /// should be used.
    state: Mutex<State>,

    /// Notifies the background task handling entry expiration. The background
    /// task waits on this to be notified, then checks for expired values or the
    /// shutdown signal.
    background_task: Notify,

    /// Metrics for tracking server operations
    metrics: Arc<Metrics>,
}

#[derive(Debug)]
struct State {
    /// The key-value data. We are not trying to do anything fancy so a
    /// `std::collections::HashMap` works fine.
    entries: HashMap<String, Entry>,

    /// The pub/sub key-space. Redis uses a **separate** key space for key-value
    /// and pub/sub. `mini-redis` handles this by using a separate `HashMap`.
    pub_sub: HashMap<String, broadcast::Sender<Bytes>>,

    /// Tracks key TTLs.
    ///
    /// A `BTreeSet` is used to maintain expirations sorted by when they expire.
    /// This allows the background task to iterate this map to find the value
    /// expiring next.
    ///
    /// While highly unlikely, it is possible for more than one expiration to be
    /// created for the same instant. Because of this, the `Instant` is
    /// insufficient for the key. A unique key (`String`) is used to
    /// break these ties.
    expirations: BTreeSet<(Instant, String)>,

    /// True when the Db instance is shutting down. This happens when all `Db`
    /// values drop. Setting this to `true` signals to the background task to
    /// exit.
    shutdown: bool,
}

/// Entry in the key-value store
#[derive(Debug)]
struct Entry {
    /// Stored data
    data: Bytes,

    /// Instant at which the entry expires and should be removed from the
    /// database.
    expires_at: Option<Instant>,
}

impl DbDropGuard {
    /// Create a new `DbDropGuard`, wrapping a `Db` instance. When this is dropped
    /// the background task will be shut down.
    pub(crate) fn new() -> DbDropGuard {
        let metrics = Arc::new(Metrics::new());
        let db = Db::new(metrics.clone());
        DbDropGuard { db }
    }

    /// Get a reference to the `Db` instance.
    pub(crate) fn db(&self) -> Db {
        self.db.clone()
    }
}

impl Drop for DbDropGuard {
    fn drop(&mut self) {
        // Signal the background task to shut down
        self.db.shutdown_purge_task();
    }
}

impl Db {
    /// Create a new `Db` instance with metrics.
    pub(crate) fn new(metrics: Arc<Metrics>) -> Db {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                entries: HashMap::new(),
                pub_sub: HashMap::new(),
                expirations: BTreeSet::new(),
                shutdown: false,
            }),
            background_task: Notify::new(),
            metrics,
        });

        // Spawn the background task
        tokio::spawn(purge_expired_tasks(Arc::clone(&shared)));

        Db { shared }
    }

    /// Get the metrics instance
    pub(crate) fn get_metrics(&self) -> Arc<Metrics> {
        Arc::clone(&self.shared.metrics)
    }

    /// Get the value associated with a key.
    ///
    /// Returns `None` if the key does not exist.
    pub(crate) fn get(&self, key: &str) -> Option<Bytes> {
        let state = self.shared.state.lock().unwrap();
        let entry = state.entries.get(key)?;

        // Check if the entry has expired
        if let Some(expires_at) = entry.expires_at {
            if expires_at <= Instant::now() {
                return None;
            }
        }

        // Update metrics
        self.shared.metrics.inc_get_hits();
        self.shared.metrics.inc_ops_ok();

        Some(entry.data.clone())
    }

    /// Set a key-value pair in the database.
    ///
    /// If a value already exists for the key, it is overwritten.
    pub(crate) fn set(&self, key: String, value: Bytes, expire: Option<Duration>) {
        let mut state = self.shared.state.lock().unwrap();

        // Remove old entry if it exists to update memory usage
        if let Some(_old_entry) = state.entries.get(&key) {
            // Remove old expiration if it exists
            if let Some(old_expires_at) = _old_entry.expires_at {
                state.expirations.remove(&(old_expires_at, key.clone()));
            }
        }

        // Create new entry
        let expires_at = expire.map(|duration| Instant::now() + duration);
        let entry = Entry {
            data: value,
            expires_at,
        };

        // Add to entries
        state.entries.insert(key.clone(), entry);

        // Add to expirations if TTL is set
        if let Some(expires_at) = expires_at {
            state.expirations.insert((expires_at, key));
        }

        // Update metrics
        self.shared.metrics.inc_ops_ok();
        self.update_metrics(&state);
    }

    /// Get the TTL for a key.
    ///
    /// Returns `None` if the key does not exist or has no TTL.
    pub(crate) fn ttl(&self, key: &str) -> Option<Duration> {
        let state = self.shared.state.lock().unwrap();
        let entry = state.entries.get(key)?;

        // Check if the entry has expired
        if let Some(expires_at) = entry.expires_at {
            if expires_at <= Instant::now() {
                return None;
            }
            return Some(expires_at - Instant::now());
        }

        // Update metrics
        self.shared.metrics.inc_ops_ok();

        None
    }

    /// Get the PTTL for a key.
    ///
    /// Returns `None` if the key does not exist or has no TTL.
    pub(crate) fn pttl(&self, key: &str) -> Option<Duration> {
        self.ttl(key)
    }

    /// Check if a key exists in the database.
    ///
    /// Returns `false` if the key does not exist or has expired.
    pub(crate) fn contains_key(&self, key: &str) -> bool {
        let state = self.shared.state.lock().unwrap();
        if let Some(entry) = state.entries.get(key) {
            // Check if the entry has expired
            if let Some(expires_at) = entry.expires_at {
                if expires_at <= Instant::now() {
                    return false;
                }
            }
            // Update metrics
            self.shared.metrics.inc_ops_ok();
            true
        } else {
            false
        }
    }

    /// Subscribe to a channel.
    ///
    /// Returns a receiver that will receive messages published to the channel.
    pub(crate) fn subscribe(&self, key: String) -> broadcast::Receiver<Bytes> {
        let mut state = self.shared.state.lock().unwrap();

        // Get or create the channel
        let sender = state.pub_sub.entry(key.clone()).or_insert_with(|| {
            let (tx, _) = broadcast::channel(1024);
            tx
        });

        // Update metrics
        self.shared.metrics.inc_sub_count();
        self.shared.metrics.inc_ops_ok();

        sender.subscribe()
    }

    /// Publish a message to a channel.
    ///
    /// Returns the number of subscribers that received the message.
    pub(crate) fn publish(&self, key: &str, value: Bytes) -> usize {
        let state = self.shared.state.lock().unwrap();

        // Get the channel
        let sender = state.pub_sub.get(key);

        // Update metrics
        self.shared.metrics.inc_pub_count();
        self.shared.metrics.inc_ops_ok();

        // Send the message
        if let Some(sender) = sender {
            sender.send(value).unwrap_or(0)
        } else {
            0
        }
    }

    /// Shutdown the background purge task.
    fn shutdown_purge_task(&self) {
        let mut state = self.shared.state.lock().unwrap();
        state.shutdown = true;
        self.shared.background_task.notify_one();
    }

    /// Update metrics based on current state
    fn update_metrics(&self, state: &State) {
        let key_count = state.entries.len() as u64;
        let mut total_memory = 0u64;

        // Calculate approximate memory usage
        for (key, entry) in &state.entries {
            total_memory += key.len() as u64 + entry.data.len() as u64;
        }

        self.shared.metrics.set_keys(key_count);
        self.shared.metrics.set_mem_bytes(total_memory);
    }
}

impl Shared {
    /// Purge expired keys from the database.
    ///
    /// Returns the next expiration time, if any.
    fn purge_expired_keys(&self) -> Option<Instant> {
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();

        // Find expired keys
        let mut expired_keys = Vec::new();
        for &(expires_at, ref key) in &state.expirations {
            if expires_at <= now {
                expired_keys.push(key.clone());
            } else {
                // Keys are sorted by expiration time, so we can stop here
                break;
            }
        }

        // Remove expired keys
        for key in expired_keys {
            state.entries.remove(&key);
            // Note: We don't remove from expirations here as we'll do it in the loop above
        }

        // Remove expired entries from expirations
        state.expirations.retain(|&(expires_at, _)| expires_at > now);

        // Update metrics
        self.metrics.set_keys(state.entries.len() as u64);
        
        // Recalculate memory usage
        let mut total_memory = 0u64;
        for (key, entry) in &state.entries {
            total_memory += key.len() as u64 + entry.data.len() as u64;
        }
        self.metrics.set_mem_bytes(total_memory);

        // Return the next expiration time
        state.expirations.iter().next().map(|&(expires_at, _)| expires_at)
    }

    /// Check if the database is shutting down.
    fn is_shutdown(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.shutdown
    }
}

impl State {
    /// Get the next expiration time.
    fn next_expiration(&self) -> Option<Instant> {
        self.expirations.iter().next().map(|&(expires_at, _)| expires_at)
    }
}

/// Background task that purges expired keys.
async fn purge_expired_tasks(shared: Arc<Shared>) {
    while !shared.is_shutdown() {
        // Wait for the next expiration or shutdown notification
        if let Some(next_expiration) = shared.purge_expired_keys() {
            let now = Instant::now();
            if next_expiration > now {
                let duration = next_expiration - now;
                tokio::select! {
                    _ = time::sleep(duration) => {
                        // Continue to next iteration
                    }
                    _ = shared.background_task.notified() => {
                        // Shutdown requested
                        break;
                    }
                }
            }
        } else {
            // No expirations, wait for shutdown notification
            shared.background_task.notified().await;
        }
    }
}
