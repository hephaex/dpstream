# Tailscale VPN Setup Guide

This guide covers setting up Tailscale VPN for the Dolphin Remote Gaming System.

## Prerequisites

- Ubuntu 24.04 server (for game streaming)
- Nintendo Switch with custom firmware (Atmosphere 1.7.0+)
- Tailscale account (free tier sufficient for testing)

## Installation

### Server Setup (Ubuntu 24.04)

1. **Install Tailscale**
   ```bash
   curl -fsSL https://tailscale.com/install.sh | sh
   ```

2. **Authenticate with Tailscale**
   ```bash
   sudo tailscale up --advertise-routes=192.168.1.0/24 --accept-dns=true
   ```
   - Visit the URL provided to authenticate
   - Enable subnet routing if needed

3. **Get Auth Key for dpstream**
   - Visit https://login.tailscale.com/admin/settings/keys
   - Generate new auth key with appropriate permissions:
     - Reusable: Yes (for development)
     - Ephemeral: No
     - Tags: Add 'dpstream-server' tag

### Client Setup (Development Machine)

1. **Install Tailscale** (macOS/Windows/Linux)
   - Download from: https://tailscale.com/download
   - Follow OS-specific installation

2. **Connect to Network**
   ```bash
   tailscale up
   ```

## Configuration

### Environment Variables

Update your `.env` file with Tailscale settings:

```bash
# Tailscale Configuration
TAILSCALE_AUTH_KEY=tskey-auth-xxxxxxxxxxxxx  # From admin console
TAILSCALE_HOSTNAME=dpstream-server           # Server hostname
TAILSCALE_ROUTES=192.168.1.0/24             # Local network routes
TAILSCALE_ACCEPT_DNS=true                    # Use Tailscale DNS

# Server will bind to Tailscale IP
SERVER_IP=100.64.0.1                        # Will be auto-detected
SERVER_PORT=47989                           # GameStream compatible port
```

### Network Architecture

```
Internet
   │
   ├─ Tailscale DERP Servers (relay if needed)
   │
   ├─ Ubuntu Server (100.x.x.1)
   │  ├─ Dolphin Emulator
   │  ├─ Sunshine Host
   │  └─ dpstream-server
   │
   └─ Client Devices
      ├─ Nintendo Switch (100.x.x.2) - via atmosphere-nx
      ├─ Development Machine (100.x.x.3)
      └─ Other authorized devices...
```

## Testing Connectivity

### From Server
```bash
# Check Tailscale status
tailscale status

# Test ping to client
tailscale ping [client-hostname]

# Check IP address
tailscale ip -4
```

### From Client
```bash
# Ping server
ping 100.x.x.1

# Test GameStream port
telnet 100.x.x.1 47989
```

## Switch Integration

The Nintendo Switch client will discover the server automatically via:

1. **Tailscale Node Discovery**
   - Query Tailscale network for dpstream-server nodes
   - Filter by hostname and available services

2. **mDNS Broadcasting**
   - Server advertises `_nvstream._tcp` service
   - Compatible with existing Moonlight clients

3. **Direct Connection**
   - Use server's Tailscale IP directly
   - Bypass local network discovery

## Security Considerations

### Network Isolation
- Tailscale provides encrypted overlay network
- Each device gets unique cryptographic identity
- No open ports on public internet required

### Access Control
```bash
# Tag-based ACLs in Tailscale admin console
{
  "tagOwners": {
    "tag:dpstream-server": ["autogroup:admin"],
    "tag:dpstream-client": ["autogroup:admin"]
  },

  "acls": [
    // Allow dpstream clients to access server
    {
      "action": "accept",
      "src": ["tag:dpstream-client"],
      "dst": ["tag:dpstream-server:47989"]
    },

    // Allow discovery protocols
    {
      "action": "accept",
      "src": ["tag:dpstream-client"],
      "dst": ["tag:dpstream-server:5353"] // mDNS
    }
  ]
}
```

## Troubleshooting

### Connection Issues
1. **Can't reach server**
   - Check `tailscale status` on both ends
   - Verify firewall rules: `sudo ufw status`
   - Test with `tailscale ping`

2. **High latency**
   - Check if using DERP relay: `tailscale netcheck`
   - Enable direct connections in admin console
   - Prefer 5GHz WiFi on Switch

3. **Authentication problems**
   - Regenerate auth key if expired
   - Check device authorization in admin console
   - Verify tags and ACLs

### Performance Optimization
```bash
# Server optimization
echo 'net.core.rmem_max = 134217728' >> /etc/sysctl.conf
echo 'net.core.wmem_max = 134217728' >> /etc/sysctl.conf

# Enable IP forwarding for subnet routing
echo 'net.ipv4.ip_forward = 1' >> /etc/sysctl.conf
sysctl -p
```

## Integration with dpstream

The dpstream server automatically:
1. Detects Tailscale network on startup
2. Binds to Tailscale IP address
3. Advertises service to network
4. Handles client authentication via Tailscale identity

Example server logs:
```
[INFO] Tailscale VPN manager initialized
[INFO] Connected to Tailscale network with IP: 100.64.0.123
[INFO] Starting mDNS advertising for service discovery
[INFO] Server ready for connections on 100.64.0.123:47989
```

## Switch Client Configuration

The Switch homebrew will:
1. Initialize Tailscale connection (via atmosphere-nx integration)
2. Discover available dpstream servers
3. Present server selection UI
4. Connect using Moonlight protocol over Tailscale

```rust
// Pseudo-code for Switch client
let tailscale = TailscaleClient::new()?;
let nodes = tailscale.list_nodes().await?;
let servers = nodes.iter()
    .filter(|n| n.has_service("dpstream"))
    .collect();

for server in servers {
    println!("Found server: {} ({})", server.hostname, server.ip);
}
```

## Production Deployment

For production use:
1. Use separate Tailscale tailnet for gaming
2. Set up monitoring with Tailscale API
3. Configure automatic key rotation
4. Enable exit nodes for global access
5. Set up failover with multiple servers

## References

- [Tailscale Documentation](https://tailscale.com/kb/)
- [GameStream Protocol Specs](https://github.com/moonlight-stream/moonlight-docs/wiki/Setup-Guide)
- [Atmosphere Switch CFW](https://github.com/Atmosphere-NX/Atmosphere)