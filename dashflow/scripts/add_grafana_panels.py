#!/usr/bin/env python3
"""
Add 5 new panels to Grafana dashboard for sequence validation and DLQ metrics.

Issue #19: Grafana Dashboard Panels
- Panel 17: Sequence Gaps Timeline
- Panel 18: Duplicate Rate
- Panel 19: Reorder Detection
- Panel 20: DLQ Write Rate by Error Type
- Panel 21: DLQ Health

Usage:
    python3 scripts/add_grafana_panels.py
"""

import json
import sys
from pathlib import Path

def create_sequence_gaps_panel():
    """Panel 17: Sequence Gaps Timeline (Graph)"""
    return {
        "id": 17,
        "title": "Sequence Gaps (Message Loss Detection)",
        "type": "timeseries",
        "gridPos": {"x": 0, "y": 32, "w": 12, "h": 6},
        "targets": [
            {
                "expr": "rate(dashstream_sequence_gaps_total[5m])",
                "legendFormat": "Gaps/sec (thread: {{thread_id}})"
            }
        ],
        "fieldConfig": {
            "defaults": {
                "unit": "short",
                "custom": {
                    "drawStyle": "line",
                    "lineInterpolation": "linear",
                    "lineWidth": 2,
                    "fillOpacity": 10
                },
                "thresholds": {
                    "mode": "absolute",
                    "steps": [
                        {"value": 0, "color": "green"},
                        {"value": 0.001, "color": "red"}
                    ]
                }
            }
        },
        "options": {
            "legend": {
                "displayMode": "table",
                "placement": "bottom",
                "calcs": ["last", "max", "sum"]
            },
            "tooltip": {
                "mode": "multi"
            }
        }
    }

def create_duplicate_rate_panel():
    """Panel 18: Duplicate Rate (Graph)"""
    return {
        "id": 18,
        "title": "Duplicate Message Rate",
        "type": "timeseries",
        "gridPos": {"x": 12, "y": 32, "w": 12, "h": 6},
        "targets": [
            {
                "expr": "rate(dashstream_sequence_duplicates_total[5m])",
                "legendFormat": "Duplicates/sec (thread: {{thread_id}})"
            }
        ],
        "fieldConfig": {
            "defaults": {
                "unit": "short",
                "custom": {
                    "drawStyle": "line",
                    "lineInterpolation": "linear",
                    "lineWidth": 2,
                    "fillOpacity": 10
                },
                "thresholds": {
                    "mode": "absolute",
                    "steps": [
                        {"value": 0, "color": "green"},
                        {"value": 0.1, "color": "yellow"},
                        {"value": 0.5, "color": "red"}
                    ]
                }
            }
        },
        "options": {
            "legend": {
                "displayMode": "table",
                "placement": "bottom",
                "calcs": ["last", "max", "sum"]
            },
            "tooltip": {
                "mode": "multi"
            }
        }
    }

def create_reorder_rate_panel():
    """Panel 19: Reorder Detection (Graph)"""
    return {
        "id": 19,
        "title": "Out-of-Order Message Rate",
        "type": "timeseries",
        "gridPos": {"x": 0, "y": 38, "w": 12, "h": 6},
        "targets": [
            {
                "expr": "rate(dashstream_sequence_reorders_total[5m])",
                "legendFormat": "Reorders/sec (thread: {{thread_id}})"
            }
        ],
        "fieldConfig": {
            "defaults": {
                "unit": "short",
                "custom": {
                    "drawStyle": "line",
                    "lineInterpolation": "linear",
                    "lineWidth": 2,
                    "fillOpacity": 10
                },
                "thresholds": {
                    "mode": "absolute",
                    "steps": [
                        {"value": 0, "color": "green"},
                        {"value": 0.1, "color": "yellow"},
                        {"value": 0.5, "color": "red"}
                    ]
                }
            }
        },
        "options": {
            "legend": {
                "displayMode": "table",
                "placement": "bottom",
                "calcs": ["last", "max", "sum"]
            },
            "tooltip": {
                "mode": "multi"
            }
        }
    }

def create_dlq_write_rate_panel():
    """Panel 20: DLQ Write Rate by Error Type (Stacked Graph)"""
    return {
        "id": 20,
        "title": "DLQ Write Rate by Error Type",
        "type": "timeseries",
        "gridPos": {"x": 12, "y": 38, "w": 12, "h": 6},
        "targets": [
            {
                "expr": "rate(dashstream_dlq_messages_total[5m])",
                "legendFormat": "{{error_type}}"
            }
        ],
        "fieldConfig": {
            "defaults": {
                "unit": "short",
                "custom": {
                    "drawStyle": "line",
                    "lineInterpolation": "linear",
                    "lineWidth": 2,
                    "fillOpacity": 50,
                    "stacking": {
                        "mode": "normal"
                    }
                },
                "thresholds": {
                    "mode": "absolute",
                    "steps": [
                        {"value": 0, "color": "green"},
                        {"value": 0.1, "color": "yellow"},
                        {"value": 1, "color": "red"}
                    ]
                }
            }
        },
        "options": {
            "legend": {
                "displayMode": "table",
                "placement": "bottom",
                "calcs": ["last", "max", "sum"]
            },
            "tooltip": {
                "mode": "multi"
            }
        }
    }

def create_dlq_health_panel():
    """Panel 21: DLQ Health (Stat Panel)"""
    return {
        "id": 21,
        "title": "DLQ Health (Send Failures)",
        "type": "stat",
        "gridPos": {"x": 0, "y": 44, "w": 8, "h": 4},
        "targets": [
            {
                "expr": "rate(dashstream_dlq_failures_total[5m])",
                "legendFormat": "DLQ Failures/sec"
            }
        ],
        "fieldConfig": {
            "defaults": {
                "unit": "short",
                "decimals": 3,
                "thresholds": {
                    "mode": "absolute",
                    "steps": [
                        {"value": 0, "color": "green"},
                        {"value": 0.001, "color": "red"}
                    ]
                }
            }
        },
        "options": {
            "colorMode": "background",
            "graphMode": "area",
            "reduceOptions": {
                "values": False,
                "calcs": ["lastNotNull"]
            },
            "text": {
                "titleSize": 18,
                "valueSize": 24
            }
        }
    }

def add_panels_to_dashboard(dashboard_path: Path):
    """Add 5 new panels to the Grafana dashboard JSON."""

    # Read existing dashboard
    with open(dashboard_path, 'r') as f:
        data = json.load(f)

    dashboard = data['dashboard']

    # Check if panels already exist
    existing_ids = {panel['id'] for panel in dashboard['panels']}
    if any(id in existing_ids for id in [17, 18, 19, 20, 21]):
        print("❌ Some panels already exist. Aborting to avoid duplicates.")
        print(f"   Existing panel IDs: {sorted(existing_ids)}")
        return False

    # Create new panels
    new_panels = [
        create_sequence_gaps_panel(),
        create_duplicate_rate_panel(),
        create_reorder_rate_panel(),
        create_dlq_write_rate_panel(),
        create_dlq_health_panel()
    ]

    # Add panels to dashboard
    dashboard['panels'].extend(new_panels)

    # Increment dashboard version
    dashboard['version'] += 1

    # Write updated dashboard
    with open(dashboard_path, 'w') as f:
        json.dump(data, f, indent=2)

    print(f"✅ Added 5 new panels to {dashboard_path}")
    print(f"   Panel IDs: 17-21")
    print(f"   Dashboard version: {dashboard['version']}")
    print(f"   Total panels: {len(dashboard['panels'])}")

    return True

def main():
    """Main entry point."""
    dashboard_path = Path(__file__).parent.parent / "monitoring" / "grafana_quality_dashboard.json"

    if not dashboard_path.exists():
        print(f"❌ Dashboard file not found: {dashboard_path}")
        sys.exit(1)

    print("=" * 70)
    print("Adding 5 Grafana Panels (Issue #19)")
    print("=" * 70)
    print()

    success = add_panels_to_dashboard(dashboard_path)

    if success:
        print()
        print("Next steps:")
        print("1. Restart Grafana: docker restart dashstream-grafana")
        print("2. Open Grafana: http://localhost:3000")
        print("3. Verify 5 new panels visible")
        print("4. Run LLM validation: python3 scripts/llm_validate_grafana.py")
        sys.exit(0)
    else:
        sys.exit(1)

if __name__ == "__main__":
    main()
