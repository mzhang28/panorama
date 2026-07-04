#!/usr/bin/env bash
# Legacy wrapper — forwards all arguments to scripts/e2e.py
exec python3 "$(dirname "$0")/e2e.py" "$@"
