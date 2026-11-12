#!/usr/bin/env bash
# install-tools.sh: install the developer tools the gate needs
# (`make tools`). Safe to run again, it skips what is present.
# Linux and macOS only.
set -euo pipefail

case "$(uname -s)" in
  Linux|Darwin) ;;
  *) echo "riehub supports Linux and macOS only" >&2; exit 1 ;;
esac

need() { command -v "$1" >/dev/null 2>&1 || { echo "missing: $1 ($2)" >&2; exit 1; }; }
need cargo "install Rust with rustup, the version is pinned in rust-toolchain.toml"
need git "install git"
need make "install make"
need python3 "needed for the docs site"

if ! cargo deny --version >/dev/null 2>&1; then
  echo "installing cargo-deny"
  cargo install --locked cargo-deny
fi

echo "tools: ok"
echo "Later tasks add their own tools here (node for web/, cargo-fuzz, helm, kind)."
