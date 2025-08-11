#!/bin/bash

echo "Starting Mini-Redis monitoring stack..."

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "Error: Docker is not running. Please start Docker first."
    exit 1
fi

# Start the monitoring stack
echo "Starting Prometheus and Grafana..."
docker-compose up -d

echo ""
echo "Monitoring stack started successfully!"
echo ""
echo "Services:"
echo "  Prometheus: http://localhost:9090"
echo "  Grafana:    http://localhost:3000 (admin/admin)"
echo ""
echo "To stop the stack, run: docker-compose down"
echo "To view logs, run: docker-compose logs -f" 