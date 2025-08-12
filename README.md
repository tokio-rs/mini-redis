# Mini-Redis - Enhanced Features

This document describes the additional features and enhancements I have added to the Mini-Redis project.

## 🆕 New Features Added

### Core Redis Commands

- **TTL & PTTL**: `TTL` and `PTTL` commands for checking key expiration times
- **INFO**: `INFO` command for server information and statistics
- **DEL**: `DEL` command for deleting keys with keyspace notifications
- **QUIT**: `QUIT` command for graceful connection termination

### Advanced Pub/Sub

- **Pattern-Based Pub/Sub**: `PSUBSCRIBE` and `PUNSUBSCRIBE` with glob pattern support
  - `*` matches any sequence of characters
  - `?` matches exactly one character
  - Example: `PSUBSCRIBE news.*` subscribes to all news channels

### Keyspace Notifications

- **Database Event Publishing**: Automatic notifications when database operations occur
- **Event Types**: SET, DEL, and EXPIRED operations
- **Configurable**: Enable/disable via `CONFIG SET notify-keyspace-events 1/0`
- **Redis Compatible**: Standard `__keyevent@0__:event` channel format

### Configuration Management

- **CONFIG Command**: `CONFIG GET/SET/LIST` for managing server settings
- **Runtime Configuration**: Change settings without restarting the server
- **Environment Variables**: Support for `MINI_REDIS_NOTIFY_KEYSPACE_EVENTS`

### Monitoring & Observability

- **Prometheus Integration**: Built-in metrics server at `/metrics` endpoint
- **Grafana Dashboards**: Pre-configured dashboards for Mini-Redis metrics
- **Key Metrics**: Operations count, memory usage, key counts, pub/sub operations
- **Docker Compose**: Complete monitoring stack setup

## 🚀 Quick Start

### Start the Enhanced Server

```bash
# Build and run with metrics enabled
cargo run --release --bin mini-redis-server -- --metrics-port 9123
```

### Test New Commands

```bash
# TTL operations
SET mykey "value" EX 60
TTL mykey
PTTL mykey

# Pattern Pub/Sub
PSUBSCRIBE news.*
PUBLISH news.sports "Update"

# Keyspace notifications
CONFIG SET notify-keyspace-events 1
SUBSCRIBE __keyevent@0__:set
SET testkey "value"  # Triggers notification

# Server info
INFO
INFO server
```

### Start Monitoring Stack

```bash
# Start everything at once
./setup-complete.sh

# Or individually
./start-monitoring.sh
./run-mini-redis.sh

# Access services
# Mini-Redis: localhost:6379
# Metrics: http://localhost:9123/metrics
# Prometheus: http://localhost:9090
# Grafana: http://localhost:3000 (admin/admin)
```

## 📁 Project Structure

```
mini-redis/
├── src/
│   ├── cmd/
│   │   ├── ttl.rs          # TTL and PTTL commands
│   │   ├── info.rs         # INFO command
│   │   ├── del.rs          # DEL command
│   │   ├── quit.rs         # QUIT command
│   │   ├── psubscribe.rs   # Pattern Pub/Sub
│   │   └── config.rs       # CONFIG command
│   ├── db.rs               # Enhanced with keyspace notifications
│   ├── pattern.rs          # Glob pattern matching
│   ├── config.rs           # Configuration management
│   ├── keyspace_notifications.rs  # Event publishing
│   └── metrics_server.rs   # Prometheus metrics endpoint
├── dashboards/              # Grafana dashboards
├── prometheus/              # Prometheus configuration
├── docs/                    # Feature documentation
└── scripts/                 # Utility scripts
```

## 🧪 Testing

### Run All Tests

```bash
cargo test
```

### Test Specific Features

```bash
# Pattern Pub/Sub
./test-pattern-pubsub.sh

# Keyspace Notifications
./test-keyspace-notifications.sh

# Monitoring
./start-monitoring.sh
```

## 📚 Documentation

- **[Pattern Pub/Sub](docs/PATTERN_PUBSUB_README.md)** - Detailed pattern matching guide
- **[Keyspace Notifications](docs/KEYSPACE_NOTIFICATIONS_README.md)** - Event system documentation
- **[Monitoring Setup](docs/MONITORING_README.md)** - Prometheus & Grafana configuration

## 🔧 Scripts Added

- `setup-complete.sh` - Start all services
- `start-monitoring.sh` - Start monitoring stack
- `run-mini-redis.sh` - Run server with metrics
- `stop-all.sh` - Stop all services
- `test-pattern-pubsub.sh` - Test pattern Pub/Sub
- `test-keyspace-notifications.sh` - Test keyspace events

## 🎯 Key Enhancements

1. **Extended Command Set**: Added missing Redis commands for better compatibility
2. **Pattern Pub/Sub**: Advanced subscription patterns with efficient regex matching
3. **Keyspace Notifications**: Real-time database event publishing
4. **Configuration Management**: Runtime server configuration
5. **Monitoring Stack**: Complete observability with Prometheus and Grafana
6. **Production Ready**: Proper error handling, testing, and documentation

## 🚀 Usage Examples

### Pattern Pub/Sub
```bash
# Subscribe to multiple patterns
PSUBSCRIBE user:* news.* updates:?

# Publish to matching channels
PUBLISH user:123 "User message"
PUBLISH news.sports "Sports update"
PUBLISH updates:a "Update A"
```

### Keyspace Notifications
```bash
# Enable notifications
CONFIG SET notify-keyspace-events 1

# Subscribe to events
SUBSCRIBE __keyevent@0__:set
SUBSCRIBE __keyevent@0__:del
SUBSCRIBE __keyevent@0__:expired

# Operations trigger notifications
SET mykey "value"    # → SET event
DEL mykey            # → DEL event
SET expiring "val" EX 5  # → EXPIRED event after 5s
```

### Monitoring
```bash
# View metrics
curl http://localhost:9123/metrics

# Check key statistics
INFO keyspace
INFO memory
```

---

*This README documents the specific enhancements I have contributed to the Mini-Redis project.*
