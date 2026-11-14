#!/usr/bin/env python3
"""Adapts upstream jw1912/bullet examples/progression/1_simple.rs for Monolith (HL=512)."""

import re
import sys
from pathlib import Path


def patch_example(src: str) -> str:
    helpers = (
        "fn env_str(key: &str, default: &str) -> String {\n"
        "    std::env::var(key).unwrap_or_else(|_| default.to_string())\n"
        "}\n\n"
        "fn env_usize(key: &str, default: usize) -> usize {\n"
        "    std::env::var(key)\n"
        "        .ok()\n"
        "        .and_then(|v| v.parse().ok())\n"
        "        .unwrap_or(default)\n"
        "}\n\n"
    )

    out, n_hl = re.subn(
        r"(const\s+HIDDEN_SIZE\s*:\s*usize\s*=\s*)\d+(\s*;)",
        r"\g<1>512\2",
        src,
        count=1,
    )
    if n_hl == 0:
        out, n_hl = re.subn(
            r"(let\s+hl_size\s*=\s*)\d+(\s*;)",
            r"\g<1>512\2",
            out,
            count=1,
        )
    if n_hl == 0:
        out, n_hl = re.subn(
            r'(\.new_affine\("l0",\s*768,\s*)\d+(\s*\))',
            r"\g<1>512\2",
            out,
            count=1,
        )
        out = re.sub(
            r'(\.new_affine\("l1",\s*)2\s*\*\s*\d+(\s*,\s*1\s*\))',
            r"\g<1>2 * 512\2",
            out,
            count=1,
        )
    if n_hl == 0:
        raise RuntimeError("Could not find hidden layer size definition in 1_simple.rs")

    # Enforce out_bias quantization = QA * QB = 255 * 64 matching monolith-engine nnue.rs.
    out = re.sub(
        r'(SavedFormat::id\("l1b"\)[^\n]*?\.quantise::<i16>\()\s*64\s*(\))',
        r"\g<1>255 * 64\2",
        out,
    )
    out = re.sub(
        r'(SavedFormat::id\("l1b"\)[^\n]*?\.quantise::<i32>\([^\n]*?\))',
        'SavedFormat::id("l1b").round().quantise::<i16>(255 * 64)',
        out,
    )

    # Wire environment overrides into TrainingSchedule, LocalSettings, and DataLoader.
    out = re.sub(
        r'(net_id\s*:\s*)"[^"]*"(\.to_string\(\)|\.into\(\))?',
        r'\1Box::leak(env_str("BULLET_NET_ID", "monolith").into_boxed_str()).into()',
        out,
        count=1,
    )
    out = re.sub(
        r'(output_directory\s*:\s*)"[^"]*"(\.into\(\))?',
        r'\1Box::leak(env_str("BULLET_OUT_DIR", "checkpoints").into_boxed_str()).into()',
        out,
        count=1,
    )
    out = re.sub(
        r"(superbatches\s*:\s*)\d+",
        r'\1env_usize("BULLET_SUPERBATCHES", 80)',
        out,
        count=1,
    )
    out = re.sub(
        r"(threads\s*:\s*)\d+",
        r'\1env_usize("BULLET_THREADS", 4)',
        out,
        count=1,
    )

    # Replace dataset path either via `let dataset_path = "...";` or inline literal in DataLoader.
    if re.search(r'let\s+dataset_path\s*=\s*"[^"]*"', out):
        out = re.sub(
            r'(let\s+dataset_path\s*=\s*)"[^"]*"',
            r'\1Box::leak(env_str("BULLET_DATA", "data/baseline.data").into_boxed_str())',
            out,
            count=1,
        )
    else:
        out = re.sub(
            r'(DirectSequentialDataLoader::new\(\s*&?\[?\s*)"[^"]*"',
            r'\1Box::leak(env_str("BULLET_DATA", "data/baseline.data").into_boxed_str())',
            out,
            count=1,
        )

    fn_idx = out.find("fn main()")
    if fn_idx == -1:
        raise RuntimeError("Could not locate fn main() in 1_simple.rs")
    out = out[:fn_idx] + helpers + out[fn_idx:]
    return out


def main() -> int:
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <src_1_simple.rs> <dst_monolith_cuda.rs>", file=sys.stderr)
        return 1
    src_path = Path(sys.argv[1])
    dst_path = Path(sys.argv[2])
    patched = patch_example(src_path.read_text(encoding="utf-8"))
    dst_path.write_text(patched, encoding="utf-8")
    print(f"Patched {src_path} -> {dst_path} (HL=512, QA*QB=255*64)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
