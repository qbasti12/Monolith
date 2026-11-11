#!/usr/bin/env bash
set -euo pipefail

cargo build --release --bins

./target/release/monolith-tui --help | grep -q "Monolith TUI"
./target/release/monolith-tui --version | grep -q "monolith-tui"

if ./target/release/monolith-tui --unknown-flag >/dev/null 2>&1; then
    echo "Expected --unknown-flag to fail cleanly" >&2
    exit 1
fi

printf "uci\nisready\nquit\n" | ./target/release/monolith-engine | grep -q "uciok"

echo "smoke.sh passed cleanly."
