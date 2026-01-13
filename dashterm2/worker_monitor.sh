#!/bin/bash
# worker_monitor.sh - Monitor worker health and collect stats
#
# Features:
#   - Stall detection: alerts if heartbeat file is stale
#   - Iteration stats: tracks timing per commit
#   - Optional webhook/sound alerts
#
# Usage: ./worker_monitor.sh [stall_threshold_seconds]

set -euo pipefail

STALL_THRESHOLD="${1:-120}"  # Default: 2 minutes without heartbeat = stall
HEARTBEAT_FILE="worker_heartbeat"
STATUS_FILE="worker_status.json"
STATS_FILE="worker_stats.json"
CHECK_INTERVAL=10  # Check every 10 seconds

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Initialize stats file if not exists
if [ ! -f "$STATS_FILE" ]; then
    echo '{"iterations": [], "total_commits": 0, "total_time_seconds": 0}' > "$STATS_FILE"
fi

last_iteration=0
last_commit_time=$(date +%s)
stall_alerted=false

echo "========================================"
echo "Worker Monitor Started"
echo "Stall threshold: ${STALL_THRESHOLD}s"
echo "Checking every: ${CHECK_INTERVAL}s"
echo "========================================"
echo ""

alert_stall() {
    local stale_seconds="$1"
    echo -e "${RED}⚠️  WORKER STALL DETECTED${NC}"
    echo "   Heartbeat stale for ${stale_seconds}s (threshold: ${STALL_THRESHOLD}s)"

    # macOS: play alert sound
    if command -v afplay &> /dev/null; then
        afplay /System/Library/Sounds/Basso.aiff 2>/dev/null &
    fi

    # Optional: webhook alert (uncomment and configure)
    # curl -X POST -H 'Content-type: application/json' \
    #     --data '{"text":"Worker stalled!"}' \
    #     "$SLACK_WEBHOOK_URL" 2>/dev/null &
}

alert_recovered() {
    echo -e "${GREEN}✓ Worker recovered${NC}"
    if command -v afplay &> /dev/null; then
        afplay /System/Library/Sounds/Pop.aiff 2>/dev/null &
    fi
}

get_commit_count() {
    git rev-list --count HEAD 2>/dev/null || echo "0"
}

while true; do
    # Check heartbeat staleness
    if [ -f "$HEARTBEAT_FILE" ]; then
        heartbeat_age=$(($(date +%s) - $(stat -f %m "$HEARTBEAT_FILE" 2>/dev/null || echo "0")))

        if [ "$heartbeat_age" -gt "$STALL_THRESHOLD" ]; then
            if [ "$stall_alerted" = false ]; then
                alert_stall "$heartbeat_age"
                stall_alerted=true
            fi
        else
            if [ "$stall_alerted" = true ]; then
                alert_recovered
                stall_alerted=false
            fi
        fi
    fi

    # Read current status
    if [ -f "$STATUS_FILE" ]; then
        current_iteration=$(jq -r '.iteration // 0' "$STATUS_FILE" 2>/dev/null || echo "0")
        ai_tool=$(jq -r '.ai_tool // "unknown"' "$STATUS_FILE" 2>/dev/null || echo "unknown")
        status=$(jq -r '.status // "unknown"' "$STATUS_FILE" 2>/dev/null || echo "unknown")

        # Detect new iteration
        if [ "$current_iteration" -gt "$last_iteration" ] && [ "$last_iteration" -gt 0 ]; then
            now=$(date +%s)
            iteration_time=$((now - last_commit_time))

            # Log iteration timing
            echo -e "${GREEN}✓ Iteration $last_iteration completed in ${iteration_time}s${NC}"

            # Update stats
            if [ -f "$STATS_FILE" ]; then
                jq --argjson iter "$last_iteration" \
                   --argjson time "$iteration_time" \
                   --arg tool "$ai_tool" \
                   '.iterations += [{"iteration": $iter, "time_seconds": $time, "ai_tool": $tool, "timestamp": now}] | .total_time_seconds += $time' \
                   "$STATS_FILE" > "${STATS_FILE}.tmp" && mv "${STATS_FILE}.tmp" "$STATS_FILE"
            fi

            last_commit_time=$now
        fi

        last_iteration=$current_iteration

        # Display current status
        if [ -f "$HEARTBEAT_FILE" ]; then
            heartbeat_age=$(($(date +%s) - $(stat -f %m "$HEARTBEAT_FILE" 2>/dev/null || echo "0")))
        else
            heartbeat_age="N/A"
        fi

        # Calculate average iteration time
        if [ -f "$STATS_FILE" ]; then
            avg_time=$(jq 'if (.iterations | length) > 0 then (.total_time_seconds / (.iterations | length) | floor) else 0 end' "$STATS_FILE" 2>/dev/null || echo "0")
            iter_count=$(jq '.iterations | length' "$STATS_FILE" 2>/dev/null || echo "0")
        else
            avg_time=0
            iter_count=0
        fi

        # Status line (overwrite previous)
        printf "\r%-80s" ""  # Clear line
        printf "\r[%s] Iter: %s | Tool: %s | Status: %s | Heartbeat: %ss ago | Avg: %ss (%s iters)" \
            "$(date +%H:%M:%S)" \
            "$current_iteration" \
            "$ai_tool" \
            "$status" \
            "$heartbeat_age" \
            "$avg_time" \
            "$iter_count"
    fi

    sleep "$CHECK_INTERVAL"
done
