#!/bin/bash

echo "Setting up complete Mini-Redis monitoring environment..."
echo "=================================================="

# Function to check if a port is in use
check_port() {
    local port=$1
    if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; then
        return 0
    else
        return 1
    fi
}

# Function to wait for a service to be ready
wait_for_service() {
    local url=$1
    local service_name=$2
    local max_attempts=30
    local attempt=1
    
    echo "Waiting for $service_name to be ready..."
    while [ $attempt -le $max_attempts ]; do
        if curl -s "$url" > /dev/null 2>&1; then
            echo "$service_name is ready!"
            return 0
        fi
        echo "Attempt $attempt/$max_attempts: $service_name not ready yet..."
        sleep 2
        attempt=$((attempt + 1))
    done
    
    echo "Error: $service_name failed to start within expected time"
    return 1
}

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "Error: Docker is not running. Please start Docker first."
    exit 1
fi

# Start the monitoring stack
echo "Starting monitoring stack (Prometheus + Grafana)..."
./start-monitoring.sh

# Wait for services to be ready
if ! wait_for_service "http://localhost:9090/api/v1/status/config" "Prometheus"; then
    echo "Failed to start Prometheus"
    exit 1
fi

if ! wait_for_service "http://localhost:3000/api/health" "Grafana"; then
    echo "Failed to start Grafana"
    exit 1
fi

echo ""
echo "Monitoring stack is ready!"
echo ""

# Build and start Mini-Redis
echo "Building and starting Mini-Redis with metrics..."
./run-mini-redis.sh &

# Wait a moment for the server to start
sleep 3

# Check if mini-redis is running
if check_port 6379; then
    echo ""
    echo "✅ Setup complete! All services are running:"
    echo ""
    echo "🌐 Services:"
    echo "  • Mini-Redis: localhost:6379"
    echo "  • Metrics:    http://localhost:9123/metrics"
    echo "  • Prometheus: http://localhost:9090"
    echo "  • Grafana:    http://localhost:3000 (admin/admin)"
    echo ""
    echo "📊 Next steps:"
    echo "  1. Open Grafana at http://localhost:3000"
    echo "  2. Login with admin/admin"
    echo "  3. The Prometheus datasource should be auto-configured"
    echo "  4. Mini-Redis dashboard should be available"
    echo ""
    echo "🛑 To stop everything:"
    echo "  • Stop Mini-Redis: Ctrl+C in the terminal"
    echo "  • Stop monitoring: docker-compose down"
    echo ""
else
    echo "❌ Failed to start Mini-Redis"
    exit 1
fi 