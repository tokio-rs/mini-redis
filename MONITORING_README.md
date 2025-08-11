# Mini-Redis Monitoring Setup

This directory contains a complete monitoring stack for Mini-Redis using Prometheus and Grafana.

## Quick Start

1. **Start the monitoring stack:**
   ```bash
   ./start-monitoring.sh
   ```

2. **Access the services:**
   - **Prometheus**: http://localhost:9090
   - **Grafana**: http://localhost:3000 (admin/admin)

3. **Stop the stack:**
   ```bash
   docker-compose down
   ```

## What's Included

- **Prometheus**: Metrics collection and storage
- **Grafana**: Metrics visualization and dashboards
- **Pre-configured datasource**: Prometheus is automatically configured in Grafana
- **Pre-built dashboard**: Mini-Redis metrics dashboard is automatically loaded

## Configuration

- **Prometheus config**: `prometheus/prometheus.yml`
- **Grafana dashboards**: `dashboards/`
- **Docker compose**: `docker-compose.yml`

## Requirements

- Docker and Docker Compose
- Mini-Redis running on port 9123 (metrics endpoint)

## Troubleshooting

- Check if Docker is running: `docker info`
- View logs: `docker-compose logs -f`
- Restart services: `docker-compose restart` 