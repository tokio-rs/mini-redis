use std::sync::atomic::{AtomicU64, Ordering};

/// Metrics for tracking Redis server operations
#[derive(Debug)]
pub struct Metrics {
    /// Count of successful operations
    pub ops_ok: AtomicU64,
    /// Count of failed operations
    pub ops_err: AtomicU64,
    /// Count of successful GET operations (cache hits)
    pub get_hits: AtomicU64,
    /// Count of failed GET operations (cache misses)
    pub get_misses: AtomicU64,
    /// Count of PUBLISH operations
    pub pub_count: AtomicU64,
    /// Count of SUBSCRIBE operations
    pub sub_count: AtomicU64,
    /// Current number of keys in the database
    pub keys: AtomicU64,
    /// Current memory usage in bytes (approximate)
    pub mem_bytes: AtomicU64,
}

impl Metrics {
    /// Create a new metrics instance
    pub fn new() -> Self {
        Self {
            ops_ok: AtomicU64::new(0),
            ops_err: AtomicU64::new(0),
            get_hits: AtomicU64::new(0),
            get_misses: AtomicU64::new(0),
            pub_count: AtomicU64::new(0),
            sub_count: AtomicU64::new(0),
            keys: AtomicU64::new(0),
            mem_bytes: AtomicU64::new(0),
        }
    }

    /// Increment successful operations counter
    pub fn inc_ops_ok(&self) {
        self.ops_ok.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment failed operations counter
    pub fn inc_ops_err(&self) {
        self.ops_err.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment GET hits counter
    pub fn inc_get_hits(&self) {
        self.get_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment GET misses counter
    pub fn inc_get_misses(&self) {
        self.get_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment PUBLISH counter
    pub fn inc_pub_count(&self) {
        self.pub_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment SUBSCRIBE counter
    pub fn inc_sub_count(&self) {
        self.sub_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Update the current number of keys
    pub fn set_keys(&self, count: u64) {
        self.keys.store(count, Ordering::Relaxed);
    }

    /// Update the current memory usage
    pub fn set_mem_bytes(&self, bytes: u64) {
        self.mem_bytes.store(bytes, Ordering::Relaxed);
    }

    /// Get all metrics as a formatted string for INFO command
    pub fn info_string(&self) -> String {
        format!(
            "ops_ok:{}\r\nops_err:{}\r\nget_hits:{}\r\nget_misses:{}\r\npub_count:{}\r\nsub_count:{}\r\nkeys:{}\r\nmem_bytes:{}\r\n",
            self.ops_ok.load(Ordering::Relaxed),
            self.ops_err.load(Ordering::Relaxed),
            self.get_hits.load(Ordering::Relaxed),
            self.get_misses.load(Ordering::Relaxed),
            self.pub_count.load(Ordering::Relaxed),
            self.sub_count.load(Ordering::Relaxed),
            self.keys.load(Ordering::Relaxed),
            self.mem_bytes.load(Ordering::Relaxed),
        )
    }

    /// Get metrics in Prometheus format
    pub fn prometheus_string(&self) -> String {
        format!(
            "# HELP mini_redis_ops_ok Total number of successful operations\n\
# TYPE mini_redis_ops_ok counter\n\
mini_redis_ops_ok {}\n\
# HELP mini_redis_ops_err Total number of failed operations\n\
# TYPE mini_redis_ops_err counter\n\
mini_redis_ops_err {}\n\
# HELP mini_redis_get_hits Total number of successful GET operations\n\
# TYPE mini_redis_get_hits counter\n\
mini_redis_get_hits {}\n\
# HELP mini_redis_get_misses Total number of failed GET operations\n\
# TYPE mini_redis_get_misses counter\n\
mini_redis_get_misses {}\n\
# HELP mini_redis_pub_count Total number of PUBLISH operations\n\
# TYPE mini_redis_pub_count counter\n\
mini_redis_pub_count {}\n\
# HELP mini_redis_sub_count Total number of SUBSCRIBE operations\n\
# TYPE mini_redis_sub_count counter\n\
mini_redis_sub_count {}\n\
# HELP mini_redis_keys Current number of keys in the database\n\
# TYPE mini_redis_keys gauge\n\
mini_redis_keys {}\n\
# HELP mini_redis_mem_bytes Current memory usage in bytes\n\
# TYPE mini_redis_mem_bytes gauge\n\
mini_redis_mem_bytes {}\n",
            self.ops_ok.load(Ordering::Relaxed),
            self.ops_err.load(Ordering::Relaxed),
            self.get_hits.load(Ordering::Relaxed),
            self.get_misses.load(Ordering::Relaxed),
            self.pub_count.load(Ordering::Relaxed),
            self.sub_count.load(Ordering::Relaxed),
            self.keys.load(Ordering::Relaxed),
            self.mem_bytes.load(Ordering::Relaxed),
        )
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new();
        assert_eq!(metrics.ops_ok.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.ops_err.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.get_hits.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.get_misses.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.pub_count.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.sub_count.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.keys.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.mem_bytes.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_metrics_increment() {
        let metrics = Metrics::new();
        
        metrics.inc_ops_ok();
        metrics.inc_ops_ok();
        assert_eq!(metrics.ops_ok.load(Ordering::Relaxed), 2);
        
        metrics.inc_ops_err();
        assert_eq!(metrics.ops_err.load(Ordering::Relaxed), 1);
        
        metrics.inc_get_hits();
        metrics.inc_get_hits();
        metrics.inc_get_hits();
        assert_eq!(metrics.get_hits.load(Ordering::Relaxed), 3);
        
        metrics.inc_get_misses();
        assert_eq!(metrics.get_misses.load(Ordering::Relaxed), 1);
        
        metrics.inc_pub_count();
        metrics.inc_pub_count();
        assert_eq!(metrics.pub_count.load(Ordering::Relaxed), 2);
        
        metrics.inc_sub_count();
        assert_eq!(metrics.sub_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_metrics_set() {
        let metrics = Metrics::new();
        
        metrics.set_keys(42);
        assert_eq!(metrics.keys.load(Ordering::Relaxed), 42);
        
        metrics.set_mem_bytes(1024);
        assert_eq!(metrics.mem_bytes.load(Ordering::Relaxed), 1024);
    }

    #[test]
    fn test_info_string() {
        let metrics = Metrics::new();
        metrics.inc_ops_ok();
        metrics.inc_get_hits();
        metrics.set_keys(5);
        metrics.set_mem_bytes(100);
        
        let info = metrics.info_string();
        assert!(info.contains("ops_ok:1"));
        assert!(info.contains("get_hits:1"));
        assert!(info.contains("keys:5"));
        assert!(info.contains("mem_bytes:100"));
        assert!(info.ends_with("\r\n"));
    }

    #[test]
    fn test_prometheus_string() {
        let metrics = Metrics::new();
        metrics.inc_ops_ok();
        metrics.inc_get_hits();
        metrics.set_keys(5);
        metrics.set_mem_bytes(100);
        
        let prometheus = metrics.prometheus_string();
        assert!(prometheus.contains("# HELP mini_redis_ops_ok"));
        assert!(prometheus.contains("# TYPE mini_redis_ops_ok counter"));
        assert!(prometheus.contains("mini_redis_ops_ok 1"));
        assert!(prometheus.contains("mini_redis_keys 5"));
        assert!(prometheus.contains("mini_redis_mem_bytes 100"));
    }
} 