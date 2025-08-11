# Pattern-Based Pub/Sub (PSUBSCRIBE/PUNSUBSCRIBE)

This document describes the implementation of pattern-based Pub/Sub functionality in Mini-Redis, which allows clients to subscribe to channels using glob-style patterns.

## 🎯 Overview

Pattern-based Pub/Sub extends the existing Redis Pub/Sub system by allowing clients to subscribe to multiple channels that match a specific pattern, rather than subscribing to individual channels one by one.

## ✨ Features

- **PSUBSCRIBE**: Subscribe to channels matching a glob pattern
- **PUNSUBSCRIBE**: Unsubscribe from pattern-based subscriptions
- **Glob Pattern Support**: 
  - `*` matches any sequence of characters
  - `?` matches exactly one character
  - Other characters match literally
- **Efficient Pattern Matching**: Uses compiled regex for fast pattern matching
- **Redis Protocol Compatible**: Follows Redis Pub/Sub protocol standards

## 🚀 Usage Examples

### Basic Pattern Subscriptions

```bash
# Subscribe to all news channels
PSUBSCRIBE news.*

# Subscribe to user channels with single character and "123"
PSUBSCRIBE user:?123

# Subscribe to multiple patterns
PSUBSCRIBE news.* user:?123 events.*
```

### Publishing Messages

```bash
# These will match 'news.*'
PUBLISH news.sports "Sports update"
PUBLISH news.weather "Weather forecast"
PUBLISH news.politics "Political news"

# These will match 'user:?123'
PUBLISH user:a123 "Message for user a123"
PUBLISH user:b123 "Message for user b123"

# These will NOT match the patterns
PUBLISH sports.news "This won't match news.*"
PUBLISH user:123 "This won't match user:?123"
```

### Unsubscribing

```bash
# Unsubscribe from specific patterns
PUNSUBSCRIBE news.* user:?123

# Unsubscribe from all patterns (no arguments)
PUNSUBSCRIBE
```

## 🏗️ Architecture

### Core Components

1. **Pattern Module** (`src/pattern.rs`)
   - `Pattern` struct with compiled regex for efficient matching
   - Glob-to-regex conversion with proper escaping
   - Pattern validation and compilation

2. **Command Implementations** (`src/cmd/psubscribe.rs`)
   - `PSubscribe` command for pattern subscriptions
   - `PUnsubscribe` command for pattern unsubscriptions
   - Stream-based message handling with `tokio::select!`

3. **Database Integration** (`src/db.rs`)
   - Pattern subscription storage in `State.pattern_subscriptions`
   - Enhanced `publish()` method that delivers to both exact and pattern subscribers
   - Metrics tracking for pattern subscriptions

### Data Flow

```
Client PSUBSCRIBE → Pattern Compilation → Store in pattern_subscriptions
                                    ↓
Client PUBLISH → Check exact channels → Check pattern matches → Deliver to all subscribers
                                    ↓
Client PUNSUBSCRIBE → Remove from pattern_subscriptions
```

## 🔧 Implementation Details

### Pattern Compilation

Patterns are compiled to regex at subscription time for efficient matching:

```rust
// Convert glob pattern to regex
let regex_str = pattern
    .chars()
    .map(|c| match c {
        '*' => ".*".to_string(),
        '?' => ".".to_string(),
        '.' => "\\.".to_string(),
        // ... other special character escaping
        _ => c.escape_default().to_string(),
    })
    .collect::<String>();

let regex = regex::Regex::new(&format!("^{}$", regex_str))?;
```

### Message Delivery

When publishing a message, the system:

1. Delivers to exact channel subscribers (existing behavior)
2. Checks all pattern subscriptions for matches
3. Delivers to matching pattern subscribers
4. Returns total recipient count

```rust
// Send to pattern subscribers
for (pattern, sender) in &state.pattern_subscriptions {
    if pattern.matches(key) {
        let _ = sender.send(value.clone());
        total_recipients += 1;
    }
}
```

### Stream Management

Pattern subscriptions use `StreamMap` for efficient message handling:

```rust
let mut subscriptions = StreamMap::new();

tokio::select! {
    // Receive messages from pattern subscriptions
    result = subscriptions.next() => {
        let (pattern, msg) = result.unwrap();
        dst.write_frame(&make_message_frame(pattern, msg)).await?;
    }
    // Handle additional commands
    cmd = dst.read_frame() => { /* ... */ }
}
```

## 🧪 Testing

### Running Tests

```bash
# Test pattern matching functionality
cargo test pattern

# Test PSUBSCRIBE/PUNSUBSCRIBE commands
cargo test psubscribe

# Test the complete system
cargo test
```

### Manual Testing

Use the provided test script:

```bash
./test-pattern-pubsub.sh
```

This script demonstrates:
- Pattern subscription with `news.*` and `user:?123`
- Message publishing to matching and non-matching channels
- Pattern unsubscription
- Clean shutdown

## 📊 Metrics

Pattern subscriptions are tracked in the existing metrics system:

- `inc_sub_count()`: Incremented for each pattern subscription
- `inc_pub_count()`: Incremented for each published message
- `inc_ops_ok()`: Incremented for successful operations

## 🔒 Security Considerations

- Pattern compilation errors are handled gracefully
- Invalid patterns result in subscriptions that never receive messages
- No pattern injection vulnerabilities due to proper escaping

## 🚧 Limitations

- Patterns are stored in memory (not persisted)
- No pattern validation beyond regex compilation
- Limited to single-node operation (no cluster support)

## 🔮 Future Enhancements

Potential improvements could include:

- Pattern persistence across server restarts
- More sophisticated pattern validation
- Pattern statistics and monitoring
- Cluster-wide pattern distribution
- Pattern-based message routing rules

## 📚 References

- [Redis Pub/Sub Documentation](https://redis.io/topics/pubsub)
- [Glob Pattern Syntax](https://en.wikipedia.org/wiki/Glob_(programming))
- [Rust Regex Documentation](https://docs.rs/regex/)
- [Tokio Stream Documentation](https://docs.rs/tokio-stream/)

## 🤝 Contributing

When contributing to pattern-based Pub/Sub:

1. Ensure all tests pass: `cargo test`
2. Add tests for new functionality
3. Update this documentation
4. Follow the existing code style and patterns
5. Consider performance implications of pattern matching 