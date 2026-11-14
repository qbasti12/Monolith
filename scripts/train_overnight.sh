#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [ -d "/opt/cuda/bin" ]; then
    export CUDA_PATH="${CUDA_PATH:-/opt/cuda}"
    export PATH="/opt/cuda/bin:$PATH"
    export LD_LIBRARY_PATH="/opt/cuda/lib64:${LD_LIBRARY_PATH:-}"
fi

if command -v nproc >/dev/null 2>&1; then
    DEFAULT_THREADS="$(nproc)"
else
    DEFAULT_THREADS="$(sysctl -n hw.ncpu 2>/dev/null || echo 4)"
fi

GENS="${GENS:-3}"
THREADS="${THREADS:-$DEFAULT_THREADS}"
DEFAULT_CONCURRENCY=$(( DEFAULT_THREADS > 2 ? DEFAULT_THREADS / 2 : 1 ))
CONCURRENCY="${CONCURRENCY:-$DEFAULT_CONCURRENCY}"
NODES_PER_MOVE="${NODES_PER_MOVE:-6000}"
GEN1_POS="${GEN1_POS:-15000000}"
GEN2_POS="${GEN2_POS:-30000000}"
GEN3_POS="${GEN3_POS:-50000000}"
CUDA_SUPERBATCHES="${CUDA_SUPERBATCHES:-120}"
CPU_EPOCHS="${CPU_EPOCHS:-25}"
CPU_BATCH="${CPU_BATCH:-2048}"
VERIFY_ROUNDS="${VERIFY_ROUNDS:-80}"
VERIFY_TC="${VERIFY_TC:-5+0.05}"
SF_BENCH_ROUNDS="${SF_BENCH_ROUNDS:-60}"

mkdir -p data nnue
LOG_FILE="nnue/overnight_$(date +%Y%m%d_%H%M%S).log"
exec > >(tee -a "$LOG_FILE") 2>&1

echo "============================================================"
echo "Monolith Overnight NNUE Bootstrapping Pipeline"
echo "Host: $(uname -s)/$(uname -m) | Threads: $THREADS | Log: $LOG_FILE"
echo "Generations: $GENS | Nodes/move: $NODES_PER_MOVE"
echo "Positions per gen: G1=$GEN1_POS G2=$GEN2_POS G3=$GEN3_POS"
echo "============================================================"

echo "==> [0/4] Building native release binaries (target-cpu=native)..."
RUSTFLAGS="-C target-cpu=native" cargo build --release -p monolith-engine

BULLET_DIR="$ROOT_DIR/tools/bullet"
USE_CUDA=0
if command -v nvidia-smi >/dev/null 2>&1 && [ -x "$BULLET_DIR/target/release/examples/monolith_cuda" ]; then
    USE_CUDA=1
    BULLET_UTILS=""
    for candidate in "$BULLET_DIR/target/release/bullet"*; do
        if [ -f "$candidate" ] && [ -x "$candidate" ]; then
            BULLET_UTILS="$candidate"
            break
        fi
    done
    if [ -z "$BULLET_UTILS" ]; then
        echo "ERROR: bullet-utils binary not found in $BULLET_DIR/target/release" >&2
        exit 1
    fi
    echo "==> Mode: CUDA GPU (bullet monolith_cuda + $BULLET_UTILS)"
else
    echo "==> Mode: CPU Trainer (monolith-train; run ./scripts/setup_bullet_cuda.sh on CUDA PC for GPU speed)"
fi

BEST_NET=""
if [ -f "nnue/best.mnue" ]; then
    BEST_NET="nnue/best.mnue"
    echo "==> Starting from existing nnue/best.mnue"
fi

run_cuda_convert_and_shuffle() {
    local txt_in="$1"
    local raw_bin="${txt_in%.txt}.raw.data"
    local out_bin="${txt_in%.txt}.data"
    local help_txt
    help_txt="$("$BULLET_UTILS" convert --help 2>&1 || true)"

    if echo "$help_txt" | grep -q -- "--input"; then
        "$BULLET_UTILS" convert --from text --input "$txt_in" --output "$raw_bin"
    else
        "$BULLET_UTILS" convert --from text "$txt_in" "$raw_bin"
    fi

    if "$BULLET_UTILS" shuffle --help >/dev/null 2>&1; then
        local shuf_help
        shuf_help="$("$BULLET_UTILS" shuffle --help 2>&1 || true)"
        if echo "$shuf_help" | grep -q -- "--input"; then
            "$BULLET_UTILS" shuffle --input "$raw_bin" --output "$out_bin"
        else
            "$BULLET_UTILS" shuffle "$raw_bin" "$out_bin"
        fi
        rm -f "$raw_bin"
    else
        mv "$raw_bin" "$out_bin"
    fi
}

verify_and_maybe_promote() {
    local cand="$1"
    local gen="$2"
    if [ ! -x "$ROOT_DIR/tools/fastchess" ]; then
        echo "==> fastchess not built; promoting $cand unconditionally."
        cp "$cand" "nnue/best.mnue"
        BEST_NET="nnue/best.mnue"
        return 0
    fi

    local match_out
    if [ -n "$BEST_NET" ]; then
        echo "==> Verifying Gen $gen ($cand) vs current best ($BEST_NET)..."
        match_out="$(OPPONENT_BIN="$ROOT_DIR/target/release/monolith-engine" \
            OPPONENT_EVAL_FILE="$BEST_NET" \
            EVAL_FILE="$cand" \
            THREADS=1 \
            ROUNDS="$VERIFY_ROUNDS" \
            CONCURRENCY="$CONCURRENCY" \
            TC="$VERIFY_TC" \
            SPRT=1 \
            "$ROOT_DIR/scripts/elo_test.sh")" || true
    else
        echo "==> Verifying Gen $gen ($cand) vs PeSTO baseline..."
        match_out="$(OPPONENT_BIN="$ROOT_DIR/target/release/monolith-engine" \
            EVAL_FILE="$cand" \
            THREADS=1 \
            ROUNDS="$VERIFY_ROUNDS" \
            CONCURRENCY="$CONCURRENCY" \
            TC="$VERIFY_TC" \
            SPRT=1 \
            "$ROOT_DIR/scripts/elo_test.sh")" || true
    fi
    echo "$match_out"

    local elo_gain
    elo_gain="$(echo "$match_out" | awk -F'Elo: ' '/Elo: /{split($2,a," "); print a[1]}' | tail -n 1)"
    local promote=0
    if echo "$match_out" | grep -q "SPRT: H1 accepted"; then
        promote=1
    elif [ -n "$elo_gain" ] && python3 -c "import sys; sys.exit(0 if float('$elo_gain') > 0.0 else 1)"; then
        promote=1
    fi

    if [ "$promote" = "1" ]; then
        cp "$cand" "nnue/best.mnue"
        BEST_NET="nnue/best.mnue"
        echo ">>> PROMOTED Gen $gen (Elo: ${elo_gain:-+}) -> nnue/best.mnue"
    else
        echo ">>> Kept previous network (Gen $gen did not beat baseline: Elo=${elo_gain:-?})"
    fi
}

for gen in $(seq 1 "$GENS"); do
    case "$gen" in
        1) POS="$GEN1_POS" ;;
        2) POS="$GEN2_POS" ;;
        *) POS="$GEN3_POS" ;;
    esac

    echo ""
    echo "==================== GENERATION $gen / $GENS ===================="
    DATAGEN_ARGS=(--positions "$POS" --nodes "$NODES_PER_MOVE" --threads "$THREADS" --seed "$((424242 + gen * 1009))")
    if [ -n "$BEST_NET" ]; then
        DATAGEN_ARGS+=(--eval-file "$BEST_NET")
    fi

    if [ "$USE_CUDA" = "1" ]; then
        TXT_DATA="$ROOT_DIR/data/gen${gen}.txt"
        BIN_DATA="$ROOT_DIR/data/gen${gen}.data"
        if [ ! -f "$BIN_DATA" ]; then
            echo "==> [$gen/A] Self-play datagen ($POS positions -> $TXT_DATA)..."
            ./target/release/monolith-datagen "${DATAGEN_ARGS[@]}" --format text --out "$TXT_DATA"
            echo "==> [$gen/B] Converting & shuffling for CUDA bullet..."
            run_cuda_convert_and_shuffle "$TXT_DATA"
            rm -f "$TXT_DATA"
        else
            echo "==> [$gen/A] Reusing existing $BIN_DATA"
        fi

        NET_ID="monolith_gen${gen}"
        CKPT_DIR="$ROOT_DIR/nnue/checkpoints_gen${gen}"
        mkdir -p "$CKPT_DIR"
        echo "==> [$gen/C] Training on GPU (HL=512, superbatches=$CUDA_SUPERBATCHES)..."
        (
            cd "$BULLET_DIR"
            BULLET_DATA="$BIN_DATA" \
            BULLET_NET_ID="$NET_ID" \
            BULLET_OUT_DIR="$CKPT_DIR" \
            BULLET_SUPERBATCHES="$CUDA_SUPERBATCHES" \
            BULLET_THREADS="$THREADS" \
            ./target/release/examples/monolith_cuda
        )
        CAND_RAW="$(find "$CKPT_DIR" -name 'quantised.bin' | sort | tail -n 1)"
        if [ -z "$CAND_RAW" ] || [ ! -f "$CAND_RAW" ]; then
            echo "ERROR: quantised.bin not produced in $CKPT_DIR" >&2
            exit 1
        fi
        CAND_MNUE="$ROOT_DIR/nnue/gen${gen}.bin"
        cp "$CAND_RAW" "$CAND_MNUE"
    else
        BIN_DATA="$ROOT_DIR/data/gen${gen}.bin"
        if [ ! -f "$BIN_DATA" ]; then
            echo "==> [$gen/A] Self-play datagen ($POS positions -> $BIN_DATA)..."
            ./target/release/monolith-datagen "${DATAGEN_ARGS[@]}" --format bullet --out "$BIN_DATA"
        else
            echo "==> [$gen/A] Reusing existing $BIN_DATA"
        fi
        CAND_MNUE="$ROOT_DIR/nnue/gen${gen}.mnue"
        echo "==> [$gen/B] Training on CPU (HL=512, epochs=$CPU_EPOCHS)..."
        ./target/release/monolith-train \
            --data "$BIN_DATA" \
            --out "$CAND_MNUE" \
            --epochs "$CPU_EPOCHS" \
            --batch "$CPU_BATCH" \
            --threads "$THREADS" \
            --save-every 5
    fi

    echo "==> [$gen/D] Evaluating candidate network..."
    verify_and_maybe_promote "$CAND_MNUE" "$gen"
done

echo ""
echo "============================================================"
if [ -n "$BEST_NET" ] && command -v stockfish >/dev/null 2>&1 && [ -x "$ROOT_DIR/tools/fastchess" ]; then
    echo "Final calibrated benchmark of $BEST_NET vs Stockfish 2800 Elo (1 thread)..."
    EVAL_FILE="$BEST_NET" OPPONENT_ELO=2800 THREADS=1 ROUNDS="$SF_BENCH_ROUNDS" CONCURRENCY="$CONCURRENCY" TC="10+0.1" ./scripts/elo_test.sh || true
fi
echo "Overnight pipeline complete! Best weights: ${BEST_NET:-none}"
