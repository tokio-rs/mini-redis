#!/bin/bash

echo "Starting continuous data generation for real-time dashboard monitoring..."
echo "This will continuously generate Redis operations to show live metrics"
echo "Press Ctrl+C to stop"
echo ""

# Function to run a Redis command
redis_cmd() {
    cargo run --bin mini-redis-cli "$@"
}

# Function to add some delay
delay() {
    sleep 2
}

# Counter for operations
counter=1

while true; do
    echo "=== Round $counter ==="
    
    # Set a new key with timestamp
    timestamp=$(date +%s)
    redis_cmd set "timestamp:$timestamp" "Data generated at $(date)"
    
    # Get some existing keys
    redis_cmd get "user:1"
    redis_cmd get "product:1"
    
    # Update a counter
    redis_cmd set "round_counter" "$counter"
    redis_cmd get "round_counter"
    
    # Set a random key
    random_key="random_$(shuf -i 1000-9999 -n 1)"
    redis_cmd set "$random_key" "Random value $counter"
    
    echo "Generated operations for round $counter"
    echo "Current metrics:"
    curl -s http://localhost:9123/metrics | grep -E "(ops_ok|get_hits|keys)" | head -3
    echo ""
    
    counter=$((counter + 1))
    delay
done 