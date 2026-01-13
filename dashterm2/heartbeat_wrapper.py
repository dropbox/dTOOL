#!/usr/bin/env python3
"""
heartbeat_wrapper.py - Pass through stdin, touch heartbeat file periodically.
Rate-limited to once per second. Extremely lightweight.
Usage: ... | ./heartbeat_wrapper.py [heartbeat_file]
"""
import sys
import os
import time

heartbeat_file = sys.argv[1] if len(sys.argv) > 1 else "worker_heartbeat"
last_touch = 0

for line in sys.stdin:
    sys.stdout.write(line)
    sys.stdout.flush()
    now = int(time.time())
    if now > last_touch:
        os.utime(heartbeat_file, None)  # touch
        last_touch = now
