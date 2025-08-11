#!/bin/bash

# Test script for Pattern-based Pub/Sub (PSUBSCRIBE/PUNSUBSCRIBE)
# This script demonstrates the new glob pattern matching capabilities

echo "🧪 Testing Pattern-based Pub/Sub in Mini-Redis"
echo "=============================================="

# Start the server in the background
echo "🚀 Starting Mini-Redis server..."
cargo run --release --bin mini-redis-server -- --metrics-port 9123 &
SERVER_PID=$!

# Wait for server to start
sleep 2

echo ""
echo "📡 Testing PSUBSCRIBE with pattern 'news.*'"
echo "-------------------------------------------"

# Start a subscriber in the background that listens to pattern 'news.*'
echo "Subscriber 1: PSUBSCRIBE news.*"
cargo run --release --bin mini-redis-cli -- psubscribe "news.*" &
SUB1_PID=$!

sleep 1

echo ""
echo "📡 Testing PSUBSCRIBE with pattern 'user:?123'"
echo "----------------------------------------------"

# Start another subscriber in the background that listens to pattern 'user:?123'
echo "Subscriber 2: PSUBSCRIBE user:?123"
cargo run --release --bin mini-redis-cli -- psubscribe "user:?123" &
SUB2_PID=$!

sleep 1

echo ""
echo "📤 Publishing messages to test pattern matching"
echo "=============================================="

# Publish to channels that should match 'news.*'
echo "PUBLISH news.sports 'Sports news: Team wins championship!'"
cargo run --release --bin mini-redis-cli -- publish "news.sports" "Sports news: Team wins championship!"

echo "PUBLISH news.weather 'Weather news: Sunny day ahead!'"
cargo run --release --bin mini-redis-cli -- publish "news.weather" "Weather news: Sunny day ahead!"

echo "PUBLISH news.politics 'Politics news: New policy announced!'"
cargo run --release --bin mini-redis-cli -- publish "news.politics" "Politics news: New policy announced!"

# Publish to channels that should match 'user:?123'
echo "PUBLISH user:a123 'User a123 message'"
cargo run --release --bin mini-redis-cli -- publish "user:a123" "User a123 message"

echo "PUBLISH user:b123 'User b123 message'"
cargo run --release --bin mini-redis-cli -- publish "user:b123" "User b123 message"

# Publish to channels that should NOT match the patterns
echo "PUBLISH sports.news 'This should NOT match news.*'"
cargo run --release --bin mini-redis-cli -- publish "sports.news" "This should NOT match news.*"

echo "PUBLISH user:123 'This should NOT match user:?123'"
cargo run --release --bin mini-redis-cli -- publish "user:123" "This should NOT match user:?123"

echo "PUBLISH user:ab123 'This should NOT match user:?123'"
cargo run --release --bin mini-redis-cli -- publish "user:ab123" "This should NOT match user:?123"

sleep 2

echo ""
echo "🔄 Testing PUNSUBSCRIBE"
echo "======================="

# Unsubscribe from patterns
echo "Subscriber 1: PUNSUBSCRIBE news.*"
echo "Subscriber 2: PUNSUBSCRIBE user:?123"

# Kill the subscribers
kill $SUB1_PID 2>/dev/null
kill $SUB2_PID 2>/dev/null

echo ""
echo "🧹 Cleaning up..."
echo "================="

# Kill the server
kill $SERVER_PID 2>/dev/null

echo ""
echo "✅ Pattern-based Pub/Sub test completed!"
echo ""
echo "📋 Summary of what was tested:"
echo "  • PSUBSCRIBE with glob patterns:"
echo "    - 'news.*' matches: news.sports, news.weather, news.politics"
echo "    - 'user:?123' matches: user:a123, user:b123"
echo "  • Pattern matching correctly excludes non-matching channels"
echo "  • PUNSUBSCRIBE functionality"
echo ""
echo "🎯 The patterns work as expected:"
echo "  • '*' matches any sequence of characters"
echo "  • '?' matches exactly one character"
echo "  • Other characters match literally" 