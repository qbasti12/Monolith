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
#   OPPONENT_BIN=/opt/homebrew/bin/stockfish  (or path to baseline monolith-engine)
#   OPPONENT_ELO=2000        (only used when opponent is Stockfish)
#   OPPONENT_EVAL_FILE=""    (optional .mnue/.bin when opponent is another Monolith)
#   THREADS=1                (UCI Threads per engine)
#   CONCURRENCY=4            (parallel fastchess games)
#   ROUNDS=100               (each round = 2 games with colors swapped)
#   TC="10+0.1"
#   EVAL_FILE=""             (optional .mnue / quantised.bin path for Monolith)
#   SPRT=0                   (set to 1 to enable fastchess SPRT [0, 15])
OPPONENT_BIN="${OPPONENT_BIN:-$(command -v stockfish || true)}"
OPPONENT_ELO="${OPPONENT_ELO:-2000}"
OPPONENT_EVAL_FILE="${OPPONENT_EVAL_FILE:-}"
THREADS="${THREADS:-1}"
CONCURRENCY="${CONCURRENCY:-4}"
ROUNDS="${ROUNDS:-100}"
TC="${TC:-10+0.1}"
EVAL_FILE="${EVAL_FILE:-}"
SPRT="${SPRT:-0}"

if [ -z "$OPPONENT_BIN" ]; then
    echo "No opponent binary found (set OPPONENT_BIN=/path/to/engine or install stockfish)." >&2
    exit 1
fi

MONO_ARGS=("option.Threads=$THREADS" "option.Hash=64")
MONO_NAME="Monolith"
if [ -n "$EVAL_FILE" ]; then
    MONO_ARGS+=("option.UseNNUE=true" "option.EvalFile=$EVAL_FILE")
    MONO_NAME="Monolith-$(basename "$EVAL_FILE")"
fi

OPP_NAME="$(basename "$OPPONENT_BIN")"
OPP_ARGS=("option.Threads=$THREADS" "option.Hash=64")
if [[ "$OPP_NAME" == *"stockfish"* ]]; then
    OPP_ARGS+=("option.UCI_LimitStrength=true" "option.UCI_Elo=$OPPONENT_ELO")
    OPP_LABEL="Stockfish-$OPPONENT_ELO"
else
    if [ -n "$OPPONENT_EVAL_FILE" ]; then
        OPP_ARGS+=("option.UseNNUE=true" "option.EvalFile=$OPPONENT_EVAL_FILE")
        OPP_LABEL="Base-$(basename "$OPPONENT_EVAL_FILE")"
    else
        OPP_ARGS+=("option.UseNNUE=false")
        OPP_LABEL="Monolith-PeSTO"
    fi
fi

EXTRA_FLAGS=()
if [ "$SPRT" = "1" ]; then
    EXTRA_FLAGS+=(-sprt elo0=0 elo1=15 alpha=0.05 beta=0.05)
fi

echo "==> Running fastchess: $MONO_NAME vs $OPP_LABEL | rounds=$ROUNDS (2 games/round) | tc=$TC | concurrency=$CONCURRENCY"
"$FASTCHESS" \
    -engine cmd="$MONOLITH_BIN" name="$MONO_NAME" "${MONO_ARGS[@]}" \
    -engine cmd="$OPPONENT_BIN" name="$OPP_LABEL" "${OPP_ARGS[@]}" \
    -each tc="$TC" \
    -openings file="$OPENINGS" format=pgn order=random \
    -rounds "$ROUNDS" -games 2 -repeat \
    -concurrency "$CONCURRENCY" \
    ${EXTRA_FLAGS[@]+"${EXTRA_FLAGS[@]}"} \
    -recover
