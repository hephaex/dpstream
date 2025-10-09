# dpstream Deployment Guide

## Quick Start

### Prerequisites

**Ubuntu 24.04 Server:**
- CPU: 8+ cores (AMD Ryzen 5 3600+ or Intel i7-8700K+)
- RAM: 16GB minimum, 32GB recommended
- GPU: NVIDIA GTX 1060+ with 6GB+ VRAM (for NVENC hardware encoding)
- Network: Gigabit ethernet, 5GHz WiFi capability
- Storage: 100GB+ SSD for system, 1TB+ for game ROMs

**Software Dependencies:**
- Docker and Docker Compose
- OR manual installation with Rust 1.70+ and system dependencies

### Option 1: Docker Deployment (Recommended)

```bash
# Clone repository
git clone https://github.com/hephaex/dpstream.git
cd dpstream

# Configure environment
cp .env.example .env
# Edit .env with your Tailscale auth key and other settings

# Start all services
docker-compose --profile production up -d

# Check status
docker-compose ps
docker-compose logs dpstream-server
```

### Option 2: Native Installation

```bash
# Install system dependencies
sudo apt update && sudo apt install -y \
    build-essential \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    nvidia-cuda-toolkit \
    dolphin-emu \
    redis-server

# Create dpstream user
sudo useradd -r -m -d /opt/dpstream -s /bin/bash dpstream
sudo usermod -a -G video,audio dpstream

# Build and install
cd server
cargo build --release --features full
sudo cp target/release/dpstream-server /opt/dpstream/
sudo chown -R dpstream:dpstream /opt/dpstream

# Install systemd service
sudo cp ../dpstream-server.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable dpstream-server
sudo systemctl start dpstream-server
```

## Configuration

### Environment Variables

```bash
# Tailscale VPN
TAILSCALE_AUTH_KEY=tskey-auth-xxxxxxxxxxxxx
TAILSCALE_HOSTNAME=dpstream-server

# Server Settings
SERVER_PORT=47989
MAX_CLIENTS=8
RUST_LOG=info

# Dolphin Configuration
DOLPHIN_PATH=/usr/bin/dolphin-emu
ROM_PATH=/opt/dpstream/roms
SAVE_PATH=/opt/dpstream/saves

# Performance Tuning
ENCODER_TYPE=nvenc
BITRATE=15000
RESOLUTION=1080p
FPS=60
```

### Directory Structure

```
/opt/dpstream/
├── dpstream-server          # Main executable
├── roms/                    # Game ROM files
│   ├── gc/                  # GameCube ROMs
│   └── wii/                 # Wii ROMs
├── saves/                   # Save files
├── logs/                    # Application logs
└── config/                  # Configuration files
```

## Kubernetes Deployment

### Install dpstream on Kubernetes

```bash
# Create namespace
kubectl apply -f k8s/namespace.yaml

# Apply all configurations
kubectl apply -f k8s/

# Check deployment status
kubectl get pods -n dpstream
kubectl logs -n dpstream deployment/dpstream-server
```

### Horizontal Pod Autoscaling

The deployment includes HPA configuration that automatically scales based on:
- CPU utilization (target: 70%)
- Memory utilization (target: 80%)
- Scale range: 2-8 pods

### Resource Requirements

**Per Pod:**
- CPU: 1-2 cores
- Memory: 2-4 GB
- GPU: 1 NVIDIA GPU (required)

## Monitoring and Health Checks

### Health Endpoints

- **Health Check**: `http://server:8080/health`
- **Readiness Check**: `http://server:8080/ready`
- **Metrics**: `http://server:8080/metrics`

### Monitoring Stack

**Prometheus Metrics:**
- Active sessions and connected clients
- Streaming latency and bandwidth
- System resource usage
- Packet loss and frame drops

**Grafana Dashboard:**
- Real-time performance monitoring
- Historical trends and alerts
- Custom dpstream dashboard at `http://server:3000`

**Alerting Rules:**
- High latency (>50ms)
- Server downtime
- Resource exhaustion
- Network issues

### Log Management

```bash
# View live logs
journalctl -u dpstream-server -f

# Docker logs
docker-compose logs -f dpstream-server

# Kubernetes logs
kubectl logs -f -n dpstream deployment/dpstream-server
```

## Networking Configuration

### Firewall Rules

```bash
# Allow dpstream traffic
sudo ufw allow 47989/tcp   # Control port
sudo ufw allow 47998/udp   # Video stream
sudo ufw allow 47999/udp   # Audio stream
sudo ufw allow 8080/tcp    # Health/monitoring

# For Kubernetes
sudo ufw allow 30000:32767/tcp  # NodePort range
```

### Tailscale Setup

```bash
# Install Tailscale
curl -fsSL https://tailscale.com/install.sh | sh

# Connect to network
sudo tailscale up --auth-key=$TAILSCALE_AUTH_KEY --hostname=dpstream-server

# Verify connection
tailscale status
tailscale ip -4
```

## Performance Optimization

### GPU Configuration

```bash
# Verify NVIDIA drivers
nvidia-smi

# Enable persistence mode
sudo nvidia-smi -pm 1

# Set power management mode
sudo nvidia-smi -pl 300  # 300W power limit
```

### System Tuning

```bash
# Network optimizations
echo 'net.core.rmem_max = 134217728' >> /etc/sysctl.conf
echo 'net.core.wmem_max = 134217728' >> /etc/sysctl.conf
echo 'net.ipv4.tcp_rmem = 4096 65536 134217728' >> /etc/sysctl.conf
echo 'net.ipv4.tcp_wmem = 4096 65536 134217728' >> /etc/sysctl.conf
sudo sysctl -p

# CPU governor
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
```

## Security Considerations

### Network Security

- Use Tailscale VPN for secure remote access
- Implement firewall rules to restrict access
- Enable TLS for all external connections

### Application Security

- Run server as non-root user
- Use systemd security features
- Regular security updates
- Monitor access logs

### Data Protection

- Encrypt save files at rest
- Secure ROM storage
- Regular backups
- Access control for game files

## Troubleshooting

### Common Issues

**Server won't start:**
```bash
# Check logs
journalctl -u dpstream-server -n 50

# Verify dependencies
dpstream-server --version
dolphin-emu --version

# Check ports
sudo netstat -tlnp | grep 47989
```

**High latency:**
```bash
# Check network
ping -c 5 client-ip
mtr client-ip

# Monitor resources
htop
nvidia-smi

# Check Grafana dashboard
curl http://localhost:8080/metrics
```

**Connection issues:**
```bash
# Verify Tailscale
tailscale status
tailscale ping client-ip

# Check firewall
sudo ufw status
sudo iptables -L
```

### Performance Debugging

```bash
# Enable debug logging
export RUST_LOG=debug

# Monitor with htop
htop -d 1

# Network monitoring
iftop -i tailscale0

# GPU monitoring
watch -n 1 nvidia-smi
```

## Backup and Recovery

### Data Backup

```bash
# Backup saves
tar -czf saves-backup-$(date +%Y%m%d).tar.gz /opt/dpstream/saves/

# Backup configuration
tar -czf config-backup-$(date +%Y%m%d).tar.gz /opt/dpstream/config/ .env
```

### Disaster Recovery

```bash
# Restore from backup
sudo systemctl stop dpstream-server
tar -xzf saves-backup-YYYYMMDD.tar.gz -C /
tar -xzf config-backup-YYYYMMDD.tar.gz -C /opt/dpstream/
sudo chown -R dpstream:dpstream /opt/dpstream
sudo systemctl start dpstream-server
```

## Maintenance

### Regular Maintenance Tasks

**Daily:**
- Monitor dashboard for alerts
- Check disk space usage
- Verify system health

**Weekly:**
- Update system packages
- Clean old log files
- Backup save files

**Monthly:**
- Update dpstream server
- Review performance metrics
- Security audit

### Updates

```bash
# Update server
git pull
cargo build --release --features full
sudo systemctl stop dpstream-server
sudo cp target/release/dpstream-server /opt/dpstream/
sudo systemctl start dpstream-server

# Update monitoring
docker-compose pull
docker-compose up -d
```

This deployment guide provides comprehensive instructions for production deployment of the dpstream remote gaming system with proper monitoring, security, and maintenance procedures.