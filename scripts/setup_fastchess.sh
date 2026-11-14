#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLS_DIR="$ROOT_DIR/tools"
mkdir -p "$TOOLS_DIR"

echo "==> 1/3 Checking compiler & optional Stockfish..."
if command -v brew >/dev/null 2>&1; then
    if ! command -v stockfish >/dev/null 2>&1; then
        echo "Installing Stockfish via Homebrew (for calibrated Elo reference games)..."
        brew install stockfish
    else
        echo "Stockfish already installed: $(command -v stockfish)"
    fi
elif command -v stockfish >/dev/null 2>&1; then
    echo "Stockfish already installed: $(command -v stockfish)"
else
    echo "Note: stockfish binary not found on PATH."
    echo "      On Arch/Omarchy: sudo pacman -S --needed stockfish base-devel"
    echo "      On Ubuntu/Debian: sudo apt install -y stockfish build-essential"
fi

echo "==> 2/3 Building fastchess into $TOOLS_DIR/fastchess..."
if [ -x "$TOOLS_DIR/fastchess" ]; then
    echo "fastchess already exists at $TOOLS_DIR/fastchess"
else
    TMP_BUILD="$(mktemp -d)"
    trap 'rm -rf "$TMP_BUILD"' EXIT
    git clone --depth 1 https://github.com/Disservin/fastchess.git "$TMP_BUILD/fastchess"
    JOBS=4
    if command -v nproc >/dev/null 2>&1; then
        JOBS="$(nproc)"
    elif command -v sysctl >/dev/null 2>&1; then
        JOBS="$(sysctl -n hw.ncpu)"
    fi
    make -C "$TMP_BUILD/fastchess" -j"$JOBS"
    cp "$TMP_BUILD/fastchess/fastchess" "$TOOLS_DIR/fastchess"
    chmod +x "$TOOLS_DIR/fastchess"
    echo "Built $TOOLS_DIR/fastchess"
fi

echo "==> 3/3 Creating balanced opening suite at $TOOLS_DIR/openings.pgn..."
cat <<'EOF' > "$TOOLS_DIR/openings.pgn"
[Event "Opening 1"]
1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 *

[Event "Opening 2"]
1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 *

[Event "Opening 3"]
1. e4 e5 2. Nf3 Nc6 3. d4 exd4 *

[Event "Opening 4"]
1. e4 c5 2. Nf3 d6 3. d4 cxd4 *

[Event "Opening 5"]
1. e4 c5 2. Nf3 Nc6 3. d4 cxd4 *

[Event "Opening 6"]
1. e4 c5 2. Nf3 e6 3. d4 cxd4 *

[Event "Opening 7"]
1. e4 e6 2. d4 d5 3. Nc3 Bb4 *

[Event "Opening 8"]
1. e4 e6 2. d4 d5 3. Nd2 c5 *

[Event "Opening 9"]
1. e4 c6 2. d4 d5 3. Nc3 dxe4 *

[Event "Opening 10"]
1. e4 c6 2. d4 d5 3. e5 Bf5 *

[Event "Opening 11"]
1. d4 d5 2. c4 e6 3. Nc3 Nf6 *

[Event "Opening 12"]
1. d4 d5 2. c4 c6 3. Nf3 Nf6 *

[Event "Opening 13"]
1. d4 d5 2. c4 dxc4 3. Nf3 Nf6 *

[Event "Opening 14"]
1. d4 Nf6 2. c4 g6 3. Nc3 Bg7 *

[Event "Opening 15"]
1. d4 Nf6 2. c4 e6 3. Nc3 Bb4 *

[Event "Opening 16"]
1. d4 Nf6 2. c4 e6 3. Nf3 d5 *

[Event "Opening 17"]
1. c4 e5 2. Nc3 Nf6 3. Nf3 Nc6 *

[Event "Opening 18"]
1. c4 c5 2. Nc3 Nc6 3. Nf3 Nf6 *

[Event "Opening 19"]
1. Nf3 d5 2. d4 Nf6 3. c4 e6 *

[Event "Opening 20"]
1. Nf3 Nf6 2. c4 g6 3. Nc3 Bg7 *
EOF

echo "Setup complete! Run ./scripts/elo_test.sh to measure Monolith's Elo."
