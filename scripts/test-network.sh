#!/bin/bash
set -e

echo "=== Dolphin Remote Gaming System - Network Test ==="
echo "Date: $(date)"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Load environment variables
if [ -f ".env" ]; then
    source .env
else
    echo -e "${YELLOW}Warning: .env file not found. Using defaults.${NC}"
    SERVER_IP="100.64.0.1"
    SERVER_PORT="47989"
fi

echo "Testing network connectivity for dpstream..."
echo "Server IP: ${SERVER_IP}"
echo "Server Port: ${SERVER_PORT}"
echo ""

# Test 1: Basic connectivity
echo -e "${BLUE}[1/6] Testing basic connectivity...${NC}"
if command -v ping &> /dev/null; then
    if ping -c 3 -W 3000 "${SERVER_IP}" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Host is reachable${NC}"
    else
        echo -e "${RED}✗ Host is not reachable${NC}"
        echo "  Please check Tailscale connection or server IP"
    fi
else
    echo -e "${YELLOW}? ping command not available${NC}"
fi

# Test 2: Tailscale status
echo -e "${BLUE}[2/6] Checking Tailscale status...${NC}"
if command -v tailscale &> /dev/null; then
    TAILSCALE_STATUS=$(tailscale status --json 2>/dev/null || echo "error")
    if [ "$TAILSCALE_STATUS" != "error" ]; then
        echo -e "${GREEN}✓ Tailscale is running${NC}"

        # Extract current IP
        CURRENT_IP=$(tailscale ip -4 2>/dev/null || echo "unknown")
        echo "  Current Tailscale IP: $CURRENT_IP"

        # Check if server IP is in Tailscale network
        if echo "$SERVER_IP" | grep -q "^100\."; then
            echo -e "${GREEN}  ✓ Server IP appears to be in Tailscale network${NC}"
        else
            echo -e "${YELLOW}  ? Server IP may not be in Tailscale network${NC}"
        fi
    else
        echo -e "${RED}✗ Tailscale is not running or not authenticated${NC}"
        echo "  Run: tailscale up"
    fi
else
    echo -e "${RED}✗ Tailscale not installed${NC}"
    echo "  Install from: https://tailscale.com/download"
fi

# Test 3: Port connectivity
echo -e "${BLUE}[3/6] Testing GameStream port connectivity...${NC}"
if command -v nc &> /dev/null; then
    if timeout 5 nc -z "${SERVER_IP}" "${SERVER_PORT}" 2>/dev/null; then
        echo -e "${GREEN}✓ Port ${SERVER_PORT} is open${NC}"
    else
        echo -e "${RED}✗ Port ${SERVER_PORT} is not accessible${NC}"
        echo "  Server may not be running or firewall is blocking"
    fi
elif command -v telnet &> /dev/null; then
    if timeout 5 bash -c "echo > /dev/tcp/${SERVER_IP}/${SERVER_PORT}" 2>/dev/null; then
        echo -e "${GREEN}✓ Port ${SERVER_PORT} is open${NC}"
    else
        echo -e "${RED}✗ Port ${SERVER_PORT} is not accessible${NC}"
    fi
else
    echo -e "${YELLOW}? No port testing tool available (nc or telnet)${NC}"
fi

# Test 4: Bandwidth estimation
echo -e "${BLUE}[4/6] Estimating bandwidth...${NC}"
if command -v iperf3 &> /dev/null; then
    echo "  Running iperf3 test (requires server at ${SERVER_IP})..."
    if timeout 10 iperf3 -c "${SERVER_IP}" -t 5 -f M 2>/dev/null; then
        echo -e "${GREEN}✓ Bandwidth test completed${NC}"
    else
        echo -e "${YELLOW}? iperf3 server not available at ${SERVER_IP}${NC}"
    fi
else
    echo -e "${YELLOW}? iperf3 not installed${NC}"
    echo "  Install: sudo apt install iperf3 (Linux) or brew install iperf3 (macOS)"
fi

# Test 5: Latency measurement
echo -e "${BLUE}[5/6] Measuring latency...${NC}"
if command -v ping &> /dev/null; then
    LATENCY=$(ping -c 10 -q "${SERVER_IP}" 2>/dev/null | tail -1 | awk -F '/' '{print $5}' 2>/dev/null || echo "unknown")
    if [ "$LATENCY" != "unknown" ]; then
        echo "  Average latency: ${LATENCY}ms"

        # Evaluate latency for gaming
        LATENCY_NUM=$(echo "$LATENCY" | cut -d'.' -f1)
        if [ "$LATENCY_NUM" -lt 20 ]; then
            echo -e "${GREEN}  ✓ Excellent latency for gaming${NC}"
        elif [ "$LATENCY_NUM" -lt 50 ]; then
            echo -e "${GREEN}  ✓ Good latency for gaming${NC}"
        elif [ "$LATENCY_NUM" -lt 100 ]; then
            echo -e "${YELLOW}  ⚠ Acceptable latency (may notice input lag)${NC}"
        else
            echo -e "${RED}  ✗ High latency (not recommended for gaming)${NC}"
        fi
    else
        echo -e "${YELLOW}? Could not measure latency${NC}"
    fi
else
    echo -e "${YELLOW}? ping command not available${NC}"
fi

# Test 6: DNS resolution
echo -e "${BLUE}[6/6] Testing DNS resolution...${NC}"
if command -v nslookup &> /dev/null; then
    # Test common domains to verify DNS is working
    if nslookup google.com > /dev/null 2>&1; then
        echo -e "${GREEN}✓ DNS resolution working${NC}"
    else
        echo -e "${RED}✗ DNS resolution issues${NC}"
        echo "  This may affect Tailscale functionality"
    fi
else
    echo -e "${YELLOW}? nslookup not available${NC}"
fi

echo ""
echo "=== Network Test Summary ==="

# Generate recommendations
echo "Recommendations:"
echo ""

if ! command -v tailscale &> /dev/null; then
    echo -e "${RED}❌ Install Tailscale: https://tailscale.com/download${NC}"
fi

if ! timeout 5 nc -z "${SERVER_IP}" "${SERVER_PORT}" 2>/dev/null && ! timeout 5 bash -c "echo > /dev/tcp/${SERVER_IP}/${SERVER_PORT}" 2>/dev/null; then
    echo -e "${RED}❌ Server is not running or not accessible${NC}"
    echo "   - Check if dpstream-server is running"
    echo "   - Verify firewall settings"
    echo "   - Confirm Tailscale connectivity"
fi

if [ "${LATENCY_NUM:-0}" -gt 50 ] 2>/dev/null; then
    echo -e "${YELLOW}⚠️  High latency detected${NC}"
    echo "   - Use 5GHz WiFi if possible"
    echo "   - Check for network congestion"
    echo "   - Consider reducing video quality"
fi

echo ""
echo "For optimal gaming performance:"
echo "• Latency: <20ms (excellent), <50ms (good)"
echo "• Bandwidth: 10+ Mbps for 720p, 20+ Mbps for 1080p"
echo "• Use 5GHz WiFi when possible"
echo "• Ensure Tailscale direct connections (not DERP relay)"
echo ""

# Log results
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOG_FILE=".history/network_test_${TIMESTAMP}.log"
mkdir -p .history

cat > "$LOG_FILE" << EOF
Network Test Results - $TIMESTAMP

Configuration:
- Server IP: $SERVER_IP
- Server Port: $SERVER_PORT
- Client IP: $CURRENT_IP

Test Results:
- Connectivity: $(ping -c 1 -W 1000 "${SERVER_IP}" > /dev/null 2>&1 && echo "OK" || echo "FAILED")
- Port Access: $(timeout 5 nc -z "${SERVER_IP}" "${SERVER_PORT}" 2>/dev/null && echo "OK" || echo "FAILED")
- Average Latency: ${LATENCY:-unknown}ms
- Tailscale Status: $(command -v tailscale &> /dev/null && tailscale status --json > /dev/null 2>&1 && echo "Connected" || echo "Not Connected")

Generated: $(date)
EOF

echo -e "${GREEN}Test results logged to: $LOG_FILE${NC}"