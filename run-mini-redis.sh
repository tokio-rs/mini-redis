#!/bin/bash

echo "Building and running Mini-Redis with metrics..."

# Build the project
echo "Building Mini-Redis..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

echo "Build successful!"

# Check if mini-redis server is already running
if pgrep -f "mini-redis-server" > /dev/null; then
    echo "Mini-Redis server is already running."
    echo "You can access it at: http://localhost:6379"
    echo "Metrics endpoint: http://localhost:9123/metrics"
else
    echo "Starting Mini-Redis server with metrics..."
    echo "Redis server will be available at: localhost:6379"
    echo "Metrics server will be available at: http://localhost:9123"
    echo "Metrics endpoint: http://localhost:9123/metrics"
    echo ""
    echo "Press Ctrl+C to stop the server"
    
    # Run the server with metrics enabled
    cargo run --release --bin mini-redis-server -- --metrics-port 9123
fi 