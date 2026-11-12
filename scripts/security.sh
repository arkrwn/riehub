#!/usr/bin/env bash
# security.sh: the dependency and supply-chain stage of the gate
# (`make security`). A missing tool is a failure, never a skip:
# run `make tools` once to install what this script needs.
set -uo pipefail

cd "$(git rev-parse --show-toplevel)"
fail=0

step() { echo "== $*"; }

step "cargo deny (advisories, licenses, bans, sources)"
if ! cargo deny --version >/dev/null 2>&1; then
  echo "cargo-deny is not installed, run: make tools"; fail=1
elif [ ! -f deny.toml ]; then
  echo "deny.toml is missing (task T-0001 creates it)"; fail=1
else
  cargo deny check || fail=1
fi

if [ -f web/package-lock.json ]; then
  step "npm audit (web, high and critical)"
  (cd web && npm audit --audit-level=high) || fail=1
  step "web lockfile is in sync"
  (cd web && npm ci --ignore-scripts --dry-run >/dev/null) || { echo "web/package-lock.json is out of sync with package.json"; fail=1; }
fi

if [ "$fail" -eq 0 ]; then echo "security: ok"; fi
exit "$fail"
