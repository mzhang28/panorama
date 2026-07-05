#!/usr/bin/env bash
# Legacy wrapper — forwards all arguments to scripts/e2e.ts
exec bun "$(dirname "$0")/e2e.ts" "$@"
