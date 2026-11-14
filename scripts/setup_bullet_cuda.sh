#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BULLET_DIR="$ROOT_DIR/tools/bullet"

# Ensure Arch/Omarchy CUDA toolchain paths are exported if installed.
if [ -d "/opt/cuda/bin" ]; then
    export CUDA_PATH="${CUDA_PATH:-/opt/cuda}"
    export PATH="/opt/cuda/bin:$PATH"
    export LD_LIBRARY_PATH="/opt/cuda/lib64:${LD_LIBRARY_PATH:-}"
fi

echo "==> 1/4 Checking NVIDIA GPU & CUDA toolchain..."
if ! command -v nvidia-smi >/dev/null 2>&1; then
    echo "ERROR: nvidia-smi not found. Install NVIDIA proprietary drivers first." >&2
    exit 1
fi
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader

if ! command -v nvcc >/dev/null 2>&1; then
    echo "ERROR: nvcc not found (checked PATH and /opt/cuda/bin)." >&2
    echo "On Arch/Omarchy install CUDA: sudo pacman -S --needed cuda" >&2
    exit 1
fi
nvcc --version | tail -n 2

echo "==> 2/4 Cloning or updating jw1912/bullet in $BULLET_DIR..."
mkdir -p "$ROOT_DIR/tools"
if [ -d "$BULLET_DIR/.git" ]; then
    git -C "$BULLET_DIR" pull --ff-only || true
else
    git clone --depth 1 https://github.com/jw1912/bullet.git "$BULLET_DIR"
fi

echo "==> 3/4 Adapting 1_simple.rs -> monolith_cuda.rs (HL=512)..."
SRC_EXAMPLE="$(find "$BULLET_DIR" -path '*/examples/progression/1_simple.rs' | head -n 1)"
if [ -z "$SRC_EXAMPLE" ] || [ ! -f "$SRC_EXAMPLE" ]; then
    echo "ERROR: Could not find examples/progression/1_simple.rs inside $BULLET_DIR" >&2
    exit 1
fi
EXAMPLE_DIR="$(dirname "$(dirname "$SRC_EXAMPLE")")"
CRATE_DIR="$(dirname "$EXAMPLE_DIR")"
DST_EXAMPLE="$EXAMPLE_DIR/monolith_cuda.rs"
python3 "$ROOT_DIR/training/patch_bullet.py" "$SRC_EXAMPLE" "$DST_EXAMPLE"

echo "==> 4/4 Compiling bullet-utils and monolith_cuda (--features cuda)..."
UTILS_PKG="$(python3 -c '
import re, pathlib
t = pathlib.Path("'"$BULLET_DIR"'/crates/utils/Cargo.toml").read_text()
m = re.search(r"^\s*name\s*=\s*\"([^\"]+)\"", t, re.M)
print(m.group(1) if m else "bullet-utils")
')"
(
    cd "$BULLET_DIR"
    cargo build --release -p "$UTILS_PKG"
)
(
    cd "$CRATE_DIR"
    cargo build --release --example monolith_cuda --features cuda
)

echo "CUDA NNUE trainer ready!"
echo "Utils binary: $BULLET_DIR/target/release/$UTILS_PKG"
echo "Run ./scripts/train_overnight.sh to launch automated multi-generation training."
