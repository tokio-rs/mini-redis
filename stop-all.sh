#!/bin/bash

echo "Stopping all Mini-Redis monitoring services..."

# Stop the monitoring stack
echo "Stopping Prometheus and Grafana..."
docker-compose down

# Stop mini-redis if it's running
if pgrep -f "mini-redis-server" > /dev/null; then
    echo "Stopping Mini-Redis server..."
    pkill -f "mini-redis-server"
    echo "Mini-Redis server stopped"
else
    echo "Mini-Redis server is not running"
fi

echo ""
echo "✅ All services stopped!"
echo ""
echo "To start everything again, run: ./setup-complete.sh" 