#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FASTCHESS="$ROOT_DIR/tools/fastchess"
OPENINGS="$ROOT_DIR/tools/openings.pgn"
MONOLITH_BIN="$ROOT_DIR/target/release/monolith-engine"

if [ ! -x "$FASTCHESS" ]; then
    echo "fastchess not found at $FASTCHESS. Run ./scripts/setup_fastchess.sh first." >&2
    exit 1
fi

if [ ! -x "$MONOLITH_BIN" ]; then
    echo "Building release monolith-engine..."
    RUSTFLAGS="-C target-cpu=native" cargo build --release -p monolith-engine --bin monolith-engine
fi

# Configurable via environment variables:
#   OPPONENT_BIN=/opt/homebrew/bin/stockfish
#   OPPONENT_ELO=2000      (only used when opponent is Stockfish)
#   THREADS=1              (UCI Threads per engine)
#   CONCURRENCY=4          (parallel fastchess games)
#   ROUNDS=100             (each round = 2 games with colors swapped)
#   TC="10+0.1"
#   EVAL_FILE=""           (optional .mnue path for Monolith)
OPPONENT_BIN="${OPPONENT_BIN:-$(command -v stockfish || true)}"
OPPONENT_ELO="${OPPONENT_ELO:-2000}"
THREADS="${THREADS:-1}"
CONCURRENCY="${CONCURRENCY:-4}"
ROUNDS="${ROUNDS:-100}"
TC="${TC:-10+0.1}"
EVAL_FILE="${EVAL_FILE:-}"

if [ -z "$OPPONENT_BIN" ]; then
    echo "No opponent binary found (set OPPONENT_BIN=/path/to/engine or install stockfish)." >&2
    exit 1
fi

MONO_ARGS=("option.Threads=$THREADS" "option.Hash=64")
if [ -n "$EVAL_FILE" ]; then
    MONO_ARGS+=("option.EvalFile=$EVAL_FILE")
fi

OPP_NAME="$(basename "$OPPONENT_BIN")"
OPP_ARGS=("option.Threads=$THREADS" "option.Hash=64")
if [[ "$OPP_NAME" == *"stockfish"* ]]; then
    OPP_ARGS+=("option.UCI_LimitStrength=true" "option.UCI_Elo=$OPPONENT_ELO")
    OPP_LABEL="Stockfish-$OPPONENT_ELO"
else
    OPP_LABEL="$OPP_NAME"
fi

echo "==> Running fastchess: Monolith vs $OPP_LABEL | rounds=$ROUNDS (2 games/round) | tc=$TC | concurrency=$CONCURRENCY"
"$FASTCHESS" \
    -engine cmd="$MONOLITH_BIN" name="Monolith" "${MONO_ARGS[@]}" \
    -engine cmd="$OPPONENT_BIN" name="$OPP_LABEL" "${OPP_ARGS[@]}" \
    -each tc="$TC" \
    -openings file="$OPENINGS" format=pgn order=random \
    -rounds "$ROUNDS" -games 2 -repeat \
    -concurrency "$CONCURRENCY" \
    -recover
