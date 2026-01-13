#!/bin/bash
# scripts/doctor.sh - M-84: Repo health diagnostics
#
# Usage:
#   ./scripts/doctor.sh           # Run all checks
#   ./scripts/doctor.sh --fix     # Attempt to fix issues where possible
#   ./scripts/doctor.sh --json    # Output JSON for automation
#   ./scripts/doctor.sh --quick   # Skip slow `du` scans (fast status check)
#
# Checks:
#   1. Tracked build artifacts (target_*/, fuzz/target/)
#   2. Giant directories (>1GB)
#   3. Stale worker status/heartbeat
#   4. Dashboard drift (grafana lint)
#   5. Git repository health
#   6. Cargo lock issues

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

FIX_MODE=false
JSON_MODE=false
QUICK_MODE=false
ISSUES_FOUND=0
WARNINGS_FOUND=0

# Parse an ISO-8601-ish timestamp into epoch seconds.
# Supports timestamps like:
#   2026-01-05T18:10:50Z
#   2026-01-05T18:10:50.123Z
# Returns 0 on parse failure.
parse_iso8601_epoch() {
    local ts="${1:-}"
    if [ -z "$ts" ]; then
        echo "0"
        return 0
    fi

    local candidate="$ts"
    if [[ "$candidate" == *.* ]]; then
        local prefix="${candidate%%.*}"
        if [[ "$candidate" == *Z ]]; then
            candidate="${prefix}Z"
        else
            candidate="$prefix"
        fi
    fi

    if [ "$(uname)" = "Darwin" ]; then
        if [[ "$candidate" == *Z ]]; then
            date -j -u -f "%Y-%m-%dT%H:%M:%SZ" "$candidate" "+%s" 2>/dev/null || echo "0"
        else
            date -j -u -f "%Y-%m-%dT%H:%M:%S" "$candidate" "+%s" 2>/dev/null || echo "0"
        fi
    else
        date -d "$candidate" "+%s" 2>/dev/null || echo "0"
    fi
}

# Colors (disabled for JSON mode)
RED='\033[0;31m'
YELLOW='\033[0;33m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

while [[ $# -gt 0 ]]; do
    case $1 in
        --fix|-f)
            FIX_MODE=true
            shift
            ;;
        --json|-j)
            JSON_MODE=true
            RED=""
            YELLOW=""
            GREEN=""
            BLUE=""
            NC=""
            shift
            ;;
        --quick|-q)
            QUICK_MODE=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--fix] [--json] [--quick]"
            echo ""
            echo "Options:"
            echo "  --fix, -f    Attempt to fix issues where possible"
            echo "  --json, -j   Output JSON for automation"
            echo "  --quick, -q  Skip slow 'du' scans (fast status check)"
            echo "  --help, -h   Show this help"
            echo ""
            echo "Checks:"
            echo "  1. Tracked build artifacts in git"
            echo "  2. Giant local directories (>1GB)"
            echo "  3. Stale worker status/heartbeat"
            echo "  4. Grafana dashboard drift"
            echo "  5. Git repository health"
            echo "  6. Cargo lock issues"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# JSON results array
JSON_RESULTS="[]"

add_result() {
    local check="$1"
    local status="$2"
    local message="$3"
    local details="${4:-}"

    if [ "$JSON_MODE" = true ]; then
        local json_entry
        json_entry=$(printf '{"check":"%s","status":"%s","message":"%s","details":"%s"}' \
            "$check" "$status" "$message" "$details")
        JSON_RESULTS=$(echo "$JSON_RESULTS" | sed 's/\]$//' | sed 's/^\[//')
        if [ -n "$JSON_RESULTS" ]; then
            JSON_RESULTS="[$JSON_RESULTS,$json_entry]"
        else
            JSON_RESULTS="[$json_entry]"
        fi
    fi
}

print_header() {
    if [ "$JSON_MODE" != true ]; then
        echo ""
        echo -e "${BLUE}=== $1 ===${NC}"
    fi
}

print_ok() {
    if [ "$JSON_MODE" != true ]; then
        echo -e "   ${GREEN}✓${NC} $1"
    fi
}

print_warn() {
    if [ "$JSON_MODE" != true ]; then
        echo -e "   ${YELLOW}⚠${NC} $1"
    fi
    WARNINGS_FOUND=$((WARNINGS_FOUND + 1))
}

print_error() {
    if [ "$JSON_MODE" != true ]; then
        echo -e "   ${RED}✗${NC} $1"
    fi
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
}

# Header
if [ "$JSON_MODE" != true ]; then
    echo "=== DashFlow Repo Doctor (M-84) ==="
    echo "Repository: $REPO_ROOT"
    echo "Timestamp: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
fi

# ========================================
# Check 1: Tracked Build Artifacts
# ========================================
print_header "1. Tracked Build Artifacts"

# Check if any target directories are tracked in git
TRACKED_TARGETS=$(git ls-files | grep -E '^(target/|target_|fuzz/target/)' | head -20 || true)
if [ -n "$TRACKED_TARGETS" ]; then
    print_error "Build artifacts tracked in git:"
    if [ "$JSON_MODE" != true ]; then
        echo "$TRACKED_TARGETS" | head -5 | sed 's/^/      /'
        count=$(echo "$TRACKED_TARGETS" | wc -l | tr -d ' ')
        if [ "$count" -gt 5 ]; then
            echo "      ... and $((count - 5)) more"
        fi
    fi
    add_result "tracked_artifacts" "error" "Build artifacts tracked in git" "$(echo "$TRACKED_TARGETS" | wc -l | tr -d ' ') files"

    if [ "$FIX_MODE" = true ]; then
        print_warn "Fix: Run 'git rm -r --cached <dir>' for tracked artifacts"
    fi
else
    print_ok "No build artifacts tracked in git"
    add_result "tracked_artifacts" "ok" "No build artifacts tracked"
fi

# ========================================
# Check 2: Giant Directories (>1GB)
# ========================================
print_header "2. Giant Directories (>1GB)"

if [ "$QUICK_MODE" = true ]; then
    # In quick mode, just check if directories exist (skip slow du scan)
    FOUND_DIRS=""
    for dir in target target_* fuzz/target .cargo/registry; do
        if [ -d "$dir" ]; then
            FOUND_DIRS="$FOUND_DIRS $dir"
        fi
    done
    if [ -n "$FOUND_DIRS" ]; then
        print_warn "Build directories exist (sizes not scanned):$FOUND_DIRS"
        add_result "giant_dirs" "warn" "Build dirs exist (quick mode)" "$FOUND_DIRS"
    else
        print_ok "No build directories found"
        add_result "giant_dirs" "ok" "No build directories"
    fi
    print_ok ".git size check skipped (quick mode)"
    add_result "git_size" "ok" "Skipped (quick mode)"
else
    # Full mode: Check for directories > 1GB (excluding .git which is expected to be large)
    GIANT_DIRS=""
    for dir in target target_* fuzz/target .cargo/registry; do
        if [ -d "$dir" ]; then
            size_kb=$(du -sk "$dir" 2>/dev/null | cut -f1 || echo "0")
            size_gb=$(awk "BEGIN {printf \"%.1f\", $size_kb / 1048576}")
            if [ "$size_kb" -gt 1048576 ]; then
                if [ "$JSON_MODE" != true ]; then
                    print_warn "$dir: ${size_gb}GB"
                fi
                GIANT_DIRS="$GIANT_DIRS $dir:${size_gb}GB"
            fi
        fi
    done

    if [ -z "$GIANT_DIRS" ]; then
        print_ok "No giant directories (>1GB) outside .git"
        add_result "giant_dirs" "ok" "No giant directories"
    else
        add_result "giant_dirs" "warn" "Giant directories found" "$GIANT_DIRS"
        if [ "$FIX_MODE" = true ]; then
            print_warn "Fix: Run 'scripts/cleanup.sh --force' to clean build artifacts"
        fi
    fi

    # Check .git size
    if [ -d ".git" ]; then
        git_size_kb=$(du -sk ".git" 2>/dev/null | cut -f1 || echo "0")
        git_size_gb=$(awk "BEGIN {printf \"%.1f\", $git_size_kb / 1048576}")
        if [ "$git_size_kb" -gt 10485760 ]; then  # > 10GB
            print_warn ".git directory is large: ${git_size_gb}GB (consider 'git gc --aggressive')"
            add_result "git_size" "warn" ".git is very large" "${git_size_gb}GB"
        elif [ "$git_size_kb" -gt 5242880 ]; then  # > 5GB
            print_warn ".git directory: ${git_size_gb}GB"
            add_result "git_size" "warn" ".git is large" "${git_size_gb}GB"
        else
            print_ok ".git directory: ${git_size_gb}GB"
            add_result "git_size" "ok" ".git size acceptable" "${git_size_gb}GB"
        fi
    fi
fi

# ========================================
# Check 3: Worker Status/Heartbeat
# ========================================
print_header "3. Worker Status/Heartbeat"

# Check worker_status.json
if [ -f "worker_status.json" ]; then
    # Check if status indicates running but timestamp is stale
    if command -v jq &>/dev/null; then
        status=$(jq -r '.status // "unknown"' worker_status.json 2>/dev/null || echo "unknown")
        updated_at=$(jq -r '.updated_at // ""' worker_status.json 2>/dev/null || echo "")

        if [ "$status" = "running" ] && [ -n "$updated_at" ]; then
            updated_epoch=$(parse_iso8601_epoch "$updated_at")
            if [ "$updated_epoch" -eq 0 ]; then
                print_warn "Unable to parse worker_status.json updated_at: $updated_at"
                add_result "worker_status" "warn" "Cannot parse updated_at" "$updated_at"
            else
                now_epoch=$(date "+%s")
                age_min=$(( (now_epoch - updated_epoch) / 60 ))

                if [ "$age_min" -lt 0 ]; then
                    print_warn "worker_status.json updated_at appears to be in the future (${age_min}min). Check system clock/timezone."
                    add_result "worker_status" "warn" "updated_at in future" "${age_min}min"
                elif [ "$age_min" -gt 30 ]; then
                    print_error "Worker status says 'running' but hasn't updated in ${age_min} minutes"
                    add_result "worker_status" "error" "Stale worker status" "${age_min}min old"
                else
                    print_ok "Worker status: $status (updated ${age_min}min ago)"
                    add_result "worker_status" "ok" "Worker status current"
                fi
            fi

        else
            print_ok "Worker status: $status"
            add_result "worker_status" "ok" "Worker status: $status"
        fi
    else
        print_warn "jq not installed - cannot parse worker_status.json"
        add_result "worker_status" "warn" "Cannot parse status (jq not installed)"
    fi
else
    print_ok "No worker_status.json (no active worker)"
    add_result "worker_status" "ok" "No active worker"
fi

# Check worker_heartbeat file
if [ -f "worker_heartbeat" ]; then
    if [ "$(uname)" = "Darwin" ]; then
        heartbeat_epoch=$(stat -f %m "worker_heartbeat" 2>/dev/null || echo "0")
    else
        heartbeat_epoch=$(stat -c %Y "worker_heartbeat" 2>/dev/null || echo "0")
    fi
    now_epoch=$(date "+%s")
    age_min=$(( (now_epoch - heartbeat_epoch) / 60 ))

    if [ "$age_min" -gt 15 ]; then
        print_warn "worker_heartbeat is stale (${age_min} minutes old)"
        add_result "worker_heartbeat" "warn" "Stale heartbeat" "${age_min}min old"
    else
        print_ok "worker_heartbeat is fresh (${age_min}min old)"
        add_result "worker_heartbeat" "ok" "Heartbeat fresh"
    fi
fi

# ========================================
# Check 4: Dashboard Drift
# ========================================
print_header "4. Grafana Dashboard Drift"

DASHBOARD_FILE="grafana/dashboards/grafana_quality_dashboard.json"
LINT_SCRIPT="scripts/lint_grafana_dashboard.py"

if [ -f "$DASHBOARD_FILE" ] && [ -f "$LINT_SCRIPT" ]; then
    if command -v python3 &>/dev/null; then
        lint_output=$(python3 "$LINT_SCRIPT" "$DASHBOARD_FILE" 2>&1 || true)
        lint_exit=$?

        if [ $lint_exit -eq 0 ]; then
            print_ok "Dashboard lint passed"
            add_result "dashboard_lint" "ok" "No lint issues"
        elif [ $lint_exit -eq 1 ]; then
            print_warn "Dashboard lint found issues"
            if [ "$JSON_MODE" != true ]; then
                echo "$lint_output" | head -10 | sed 's/^/      /'
            fi
            add_result "dashboard_lint" "warn" "Dashboard lint issues found"
        else
            print_warn "Dashboard lint error: $lint_output"
            add_result "dashboard_lint" "warn" "Dashboard lint error"
        fi
    else
        print_warn "python3 not installed - cannot lint dashboard"
        add_result "dashboard_lint" "warn" "Cannot lint (python3 not installed)"
    fi
else
    if [ ! -f "$DASHBOARD_FILE" ]; then
        print_warn "Dashboard file not found: $DASHBOARD_FILE"
        add_result "dashboard_lint" "warn" "Dashboard file not found"
    else
        print_warn "Dashboard lint script not found: $LINT_SCRIPT"
        add_result "dashboard_lint" "warn" "Lint script not found"
    fi
fi

# ========================================
# Check 5: Git Repository Health
# ========================================
print_header "5. Git Repository Health"

# Check for uncommitted changes
if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
    changed_count=$(git status --porcelain | wc -l | tr -d ' ')
    print_warn "Uncommitted changes: $changed_count files"
    add_result "git_clean" "warn" "Uncommitted changes" "$changed_count files"
else
    print_ok "Working tree is clean"
    add_result "git_clean" "ok" "Working tree clean"
fi

# Check for unpushed commits
if git rev-parse --abbrev-ref --symbolic-full-name @{u} &>/dev/null; then
    ahead=$(git rev-list --count @{u}..HEAD 2>/dev/null || echo "0")
    if [ "$ahead" -gt 0 ]; then
        print_warn "$ahead unpushed commits"
        add_result "git_pushed" "warn" "Unpushed commits" "$ahead commits"
    else
        print_ok "Up to date with remote"
        add_result "git_pushed" "ok" "Up to date"
    fi
else
    print_warn "No upstream branch configured"
    add_result "git_pushed" "warn" "No upstream branch"
fi

# Check gc.log for issues
if [ -f ".git/gc.log" ]; then
    print_warn ".git/gc.log exists (run 'git gc' or delete this file)"
    add_result "git_gc" "warn" "gc.log exists"

    if [ "$FIX_MODE" = true ]; then
        rm -f ".git/gc.log"
        print_ok "Removed .git/gc.log"
    fi
else
    print_ok "No gc.log issues"
    add_result "git_gc" "ok" "No gc issues"
fi

# ========================================
# Check 6: Cargo Lock Issues
# ========================================
print_header "6. Cargo Lock Issues"

LOCK_FILE="$HOME/.cargo/.package-cache"
if [ -f "$LOCK_FILE" ]; then
    if [ "$(uname)" = "Darwin" ]; then
        lock_epoch=$(stat -f %m "$LOCK_FILE" 2>/dev/null || echo "0")
    else
        lock_epoch=$(stat -c %Y "$LOCK_FILE" 2>/dev/null || echo "0")
    fi
    now_epoch=$(date "+%s")
    lock_age=$((now_epoch - lock_epoch))

    if [ "$lock_age" -gt 600 ]; then
        print_warn "Stale cargo lock file (${lock_age}s old)"
        add_result "cargo_lock" "warn" "Stale cargo lock" "${lock_age}s old"

        if [ "$FIX_MODE" = true ]; then
            rm -f "$LOCK_FILE"
            print_ok "Removed stale cargo lock"
        fi
    else
        print_ok "Cargo lock is recent (${lock_age}s old)"
        add_result "cargo_lock" "ok" "Cargo lock fresh"
    fi
else
    print_ok "No cargo lock file"
    add_result "cargo_lock" "ok" "No lock file"
fi

# Check for stale cargo processes
STALE_CARGO=$(ps aux 2>/dev/null | grep -E 'cargo|rustc' | grep -v grep | wc -l | tr -d ' ')
if [ "$STALE_CARGO" -gt 0 ]; then
    print_warn "$STALE_CARGO cargo/rustc processes running"
    add_result "cargo_processes" "warn" "Cargo processes running" "$STALE_CARGO"
else
    print_ok "No cargo processes running"
    add_result "cargo_processes" "ok" "No cargo processes"
fi

# ========================================
# Summary
# ========================================
if [ "$JSON_MODE" = true ]; then
    echo "{\"issues\":$ISSUES_FOUND,\"warnings\":$WARNINGS_FOUND,\"results\":$JSON_RESULTS}"
else
    echo ""
    echo "=== Summary ==="
    if [ "$ISSUES_FOUND" -gt 0 ]; then
        echo -e "${RED}Issues found: $ISSUES_FOUND${NC}"
    fi
    if [ "$WARNINGS_FOUND" -gt 0 ]; then
        echo -e "${YELLOW}Warnings: $WARNINGS_FOUND${NC}"
    fi
    if [ "$ISSUES_FOUND" -eq 0 ] && [ "$WARNINGS_FOUND" -eq 0 ]; then
        echo -e "${GREEN}All checks passed!${NC}"
    fi
    echo ""

    if [ "$ISSUES_FOUND" -gt 0 ] || [ "$WARNINGS_FOUND" -gt 0 ]; then
        echo "Recommended actions:"
        if [ "$FIX_MODE" != true ]; then
            echo "  - Run '$0 --fix' to auto-fix some issues"
        fi
        echo "  - Run 'scripts/cleanup.sh --force' to clean build artifacts"
        echo "  - Run 'scripts/preflight.sh' before starting work"
    fi
fi

# Exit with appropriate code
if [ "$ISSUES_FOUND" -gt 0 ]; then
    exit 1
else
    exit 0
fi
