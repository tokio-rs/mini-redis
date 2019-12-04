use bytes::Bytes;
use tokio::sync::broadcast;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone)]
pub(crate) struct Kv {
    shared: Arc<Mutex<Shared>>,
}

#[derive(Debug)]
struct Shared {
    // The key-value data
    data: HashMap<String, Bytes>,

    // The pub/sub key-space
    pub_sub: HashMap<String, broadcast::Sender<Bytes>>,
}

impl Kv {
    pub(crate) fn new() -> Kv {
        Kv {
            shared: Arc::new(Mutex::new(Shared {
                data: HashMap::new(),
                pub_sub: HashMap::new(),
            })),
        }
    }

    pub(crate) fn get(&self, key: &str) -> Option<Bytes> {
        let shared = self.shared.lock().unwrap();
        shared.data.get(key).map(|data| data.clone())
    }

    pub(crate) fn set(&self, key: String, value: Bytes, _expire: Option<Duration>) {
        let mut shared = self.shared.lock().unwrap();
        shared.data.insert(key, value);
    }

    pub(crate) fn subscribe(&self, key: String) -> broadcast::Receiver<Bytes> {
        use std::collections::hash_map::Entry;

        let mut shared = self.shared.lock().unwrap();

        match shared.pub_sub.entry(key) {
            Entry::Occupied(e) => {
                e.get().subscribe()
            }
            Entry::Vacant(e) => {
                let (tx, rx) = broadcast::channel(1028);
                e.insert(tx);
                rx
            }
        }
    }

    pub(crate) fn publish(&self, key: &str, value: Bytes) -> usize {
        let shared = self.shared.lock().unwrap();

        shared.pub_sub.get(key)
            .map(|tx| tx.send(value).unwrap_or(0))
            .unwrap_or(0)
    }
}
